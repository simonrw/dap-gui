use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::Write as FmtWrite;
use std::path::Path;
use std::{env, fs};

fn main() {
    let spec_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../spec/microsoft.github.io/debug-adapter-protocol/debugAdapterProtocol.json");
    println!("cargo::rerun-if-changed={}", spec_path.display());
    println!("cargo::rerun-if-changed=build.rs");

    let spec_text = fs::read_to_string(&spec_path).expect("failed to read DAP spec JSON");
    let spec: Value = serde_json::from_str(&spec_text).expect("failed to parse DAP spec JSON");
    let definitions = spec["definitions"].as_object().expect("no definitions");

    let overrides: HashMap<(&str, &str), &str> = HashMap::from([
        (("Source", "path"), "Option<std::path::PathBuf>"),
        (("StackFrame", "line"), "usize"),
        (("StackFrame", "column"), "isize"),
        (("StackFrame", "endLine"), "Option<usize>"),
        (("StackFrame", "endColumn"), "Option<usize>"),
        (("SourceBreakpoint", "line"), "usize"),
        (("SourceBreakpoint", "column"), "Option<usize>"),
        (("Variable", "value"), "Option<String>"),
        (("Variable", "variablesReference"), "Option<i64>"),
    ]);

    // Base protocol types to skip (we don't generate these)
    let skip_types: HashSet<&str> = HashSet::from([
        "ProtocolMessage",
        "Request",
        "Response",
        "Event",
        "ErrorResponse",
    ]);

    // Classify definitions
    let mut structs: BTreeMap<&str, &Value> = BTreeMap::new();
    let mut string_enums: BTreeMap<&str, &Value> = BTreeMap::new();
    // command name -> (arguments type name, description)
    let mut command_map: BTreeMap<String, (Option<String>, Option<String>)> = BTreeMap::new();
    // command name -> (response body info, description)
    let mut response_map: BTreeMap<String, (Option<ResponseBodyInfo>, Option<String>)> =
        BTreeMap::new();
    // event name -> (event body info, description)
    let mut event_map: BTreeMap<String, (Option<EventBodyInfo>, Option<String>)> = BTreeMap::new();
    // allOf type composition (not request/response/event)
    let mut composed_types: BTreeMap<&str, &Value> = BTreeMap::new();
    // response body structs to generate inline (with optional description)
    let mut inline_response_bodies: BTreeMap<String, (Value, Option<String>)> = BTreeMap::new();
    // event body structs to generate inline (with optional description)
    let mut inline_event_bodies: BTreeMap<String, (Value, Option<String>)> = BTreeMap::new();

    for (name, def) in definitions {
        if skip_types.contains(name.as_str()) {
            continue;
        }

        if is_concrete_request(name, def) {
            let (cmd, args_type, desc) = extract_request_info(def);
            command_map.insert(cmd, (args_type, desc));
        } else if is_concrete_response(name, def) {
            let (cmd, body_info, desc) = extract_response_info(name, def);
            if let Some(ResponseBodyInfo::Inline(ref body_def)) = body_info {
                let struct_name = format!("{}Body", name.strip_suffix("Response").unwrap_or(name));
                let body_desc = desc
                    .as_deref()
                    .map(|d| format!("Body for the `{cmd}` response.\n\n{d}"));
                inline_response_bodies.insert(struct_name, (body_def.clone(), body_desc));
            }
            response_map.insert(cmd, (body_info, desc));
        } else if is_concrete_event(name, def) {
            let (evt, body_info, desc) = extract_event_info(name, def);
            if let Some(EventBodyInfo::Inline(ref body_def)) = body_info {
                let struct_name = format!("{}Body", name);
                let body_desc = desc
                    .as_deref()
                    .map(|d| format!("Body for the `{evt}` event.\n\n{d}"));
                inline_event_bodies.insert(struct_name, (body_def.clone(), body_desc));
            }
            event_map.insert(evt, (body_info, desc));
        } else if is_type_composition(def) {
            composed_types.insert(name, def);
        } else if is_string_enum(def) {
            string_enums.insert(name, def);
        } else if def.get("type").and_then(|t| t.as_str()) == Some("object")
            || def.get("properties").is_some()
        {
            structs.insert(name, def);
        } else if def.get("type").and_then(|t| t.as_str()) == Some("string") {
            // String type with no enum values - just a type alias, skip
        } else {
            // Unknown pattern, skip
        }
    }

    let mut output = String::with_capacity(64 * 1024);
    writeln!(
        output,
        "// Auto-generated from DAP specification. Do not edit."
    )
    .unwrap();
    writeln!(output).unwrap();
    writeln!(
        output,
        "use serde::{{Deserialize, Deserializer, Serialize}};"
    )
    .unwrap();
    writeln!(output).unwrap();

    // Generate string enums
    for (name, def) in &string_enums {
        generate_string_enum(&mut output, name, def);
    }

    // Generate plain structs
    for (name, def) in &structs {
        generate_struct(&mut output, name, def, &overrides, definitions);
    }

    // Generate composed types (allOf that isn't request/response/event)
    for (name, def) in &composed_types {
        generate_composed_struct(&mut output, name, def, &overrides, definitions);
    }

    // Generate inline response body structs
    for (name, (body_def, desc)) in &inline_response_bodies {
        if let Some(desc) = desc {
            write_doc_comment(&mut output, desc, "");
        }
        generate_struct_from_object(&mut output, name, body_def, &overrides, definitions);
    }

    // Generate inline event body structs
    for (name, (body_def, desc)) in &inline_event_bodies {
        if let Some(desc) = desc {
            write_doc_comment(&mut output, desc, "");
        }
        generate_struct_from_object(&mut output, name, body_def, &overrides, definitions);
    }

    // Generate RequestArguments enum
    generate_request_arguments_enum(&mut output, &command_map);

    // Generate ResponseBody enum
    generate_response_body_enum(&mut output, &response_map);

    // Generate Event enum
    generate_event_enum(&mut output, &event_map);

    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("generated.rs");
    fs::write(&out_path, &output).expect("failed to write generated.rs");
}

// --- Classification helpers ---

fn is_concrete_request(name: &str, def: &Value) -> bool {
    name.ends_with("Request") && name != "Request" && def.get("allOf").is_some() && {
        let all_of = def["allOf"].as_array().unwrap();
        all_of
            .iter()
            .any(|item| item.get("$ref").and_then(|r| r.as_str()) == Some("#/definitions/Request"))
    }
}

fn is_concrete_response(name: &str, def: &Value) -> bool {
    name.ends_with("Response")
        && name != "Response"
        && name != "ErrorResponse"
        && def.get("allOf").is_some()
        && {
            let all_of = def["allOf"].as_array().unwrap();
            all_of.iter().any(|item| {
                item.get("$ref").and_then(|r| r.as_str()) == Some("#/definitions/Response")
            })
        }
}

fn is_concrete_event(name: &str, def: &Value) -> bool {
    name.ends_with("Event") && name != "Event" && def.get("allOf").is_some() && {
        let all_of = def["allOf"].as_array().unwrap();
        all_of
            .iter()
            .any(|item| item.get("$ref").and_then(|r| r.as_str()) == Some("#/definitions/Event"))
    }
}

fn is_type_composition(def: &Value) -> bool {
    if let Some(all_of) = def.get("allOf").and_then(|v| v.as_array()) {
        // It's a type composition if it uses allOf but doesn't reference Request/Response/Event
        !all_of.iter().any(|item| {
            if let Some(r) = item.get("$ref").and_then(|r| r.as_str()) {
                matches!(
                    r,
                    "#/definitions/Request" | "#/definitions/Response" | "#/definitions/Event"
                )
            } else {
                false
            }
        })
    } else {
        false
    }
}

fn is_string_enum(def: &Value) -> bool {
    def.get("type").and_then(|t| t.as_str()) == Some("string")
        && (def.get("enum").is_some() || def.get("_enum").is_some())
}

// --- Extraction helpers ---

fn extract_request_info(def: &Value) -> (String, Option<String>, Option<String>) {
    let all_of = def["allOf"].as_array().unwrap();
    let extra = all_of
        .iter()
        .find(|item| item.get("properties").is_some())
        .unwrap();

    let command = extra["properties"]["command"]["enum"][0]
        .as_str()
        .unwrap()
        .to_string();

    let args_type = extra
        .get("properties")
        .and_then(|p| p.get("arguments"))
        .and_then(|a| a.get("$ref"))
        .and_then(|r| r.as_str())
        .map(|r| ref_to_type_name(r));

    let description = def
        .get("description")
        .or_else(|| extra.get("description"))
        .and_then(|d| d.as_str())
        .map(|s| s.to_string());

    (command, args_type, description)
}

#[derive(Clone)]
enum ResponseBodyInfo {
    Ref(String),
    Inline(Value),
}

fn extract_response_info(
    name: &str,
    def: &Value,
) -> (String, Option<ResponseBodyInfo>, Option<String>) {
    let all_of = def["allOf"].as_array().unwrap();
    let extra = all_of.iter().find(|item| item.get("properties").is_some());

    let description = def
        .get("description")
        .or_else(|| extra.and_then(|e| e.get("description")))
        .and_then(|d| d.as_str())
        .map(|s| s.to_string());

    let Some(extra) = extra else {
        // Some responses have no extra properties section (just ref to Response)
        let cmd = name.strip_suffix("Response").unwrap().to_string();
        let cmd = pascal_to_camel(&cmd);
        return (cmd, None, description);
    };

    // Derive command name from the response name
    let cmd = name.strip_suffix("Response").unwrap().to_string();
    let cmd = pascal_to_camel(&cmd);

    let body_info = extra
        .get("properties")
        .and_then(|p| p.get("body"))
        .map(|body| {
            if let Some(r) = body.get("$ref").and_then(|r| r.as_str()) {
                ResponseBodyInfo::Ref(ref_to_type_name(r))
            } else {
                ResponseBodyInfo::Inline(body.clone())
            }
        });

    (cmd, body_info, description)
}

#[derive(Clone)]
enum EventBodyInfo {
    Ref(String),
    Inline(Value),
}

fn extract_event_info(name: &str, def: &Value) -> (String, Option<EventBodyInfo>, Option<String>) {
    let all_of = def["allOf"].as_array().unwrap();
    let extra = all_of.iter().find(|item| item.get("properties").is_some());

    let description = def
        .get("description")
        .or_else(|| extra.and_then(|e| e.get("description")))
        .and_then(|d| d.as_str())
        .map(|s| s.to_string());

    let Some(extra) = extra else {
        let evt = name.strip_suffix("Event").unwrap().to_string();
        let evt = pascal_to_camel(&evt);
        return (evt, None, description);
    };

    let event_name = extra["properties"]["event"]["enum"][0]
        .as_str()
        .unwrap()
        .to_string();

    let body_info = extra
        .get("properties")
        .and_then(|p| p.get("body"))
        .map(|body| {
            if let Some(r) = body.get("$ref").and_then(|r| r.as_str()) {
                EventBodyInfo::Ref(ref_to_type_name(r))
            } else {
                EventBodyInfo::Inline(body.clone())
            }
        });

    (event_name, body_info, description)
}

// --- Code generation ---

fn generate_string_enum(output: &mut String, name: &str, def: &Value) {
    let description = def.get("description").and_then(|d| d.as_str());
    if let Some(desc) = description {
        write_doc_comment(output, desc, "");
    }

    let is_open = def.get("_enum").is_some();
    let values = if is_open {
        def["_enum"].as_array().unwrap()
    } else {
        def["enum"].as_array().unwrap()
    };

    // If there's only one value in a closed enum, it's a constant marker - skip
    if !is_open && values.len() == 1 {
        return;
    }

    let descriptions = def.get("enumDescriptions").and_then(|d| d.as_array());

    if is_open {
        // Open enum with Other(String) variant
        writeln!(output, "#[derive(Debug, Clone, PartialEq, Eq)]").unwrap();
        writeln!(output, "pub enum {name} {{").unwrap();
        for (i, val) in values.iter().enumerate() {
            let s = val.as_str().unwrap();
            if let Some(descs) = descriptions {
                if let Some(desc) = descs.get(i).and_then(|d| d.as_str()) {
                    write_doc_comment(output, desc, "    ");
                }
            }
            writeln!(output, "    {},", enum_variant_name(s)).unwrap();
        }
        writeln!(output, "    Other(String),").unwrap();
        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();

        // Custom Serialize
        writeln!(output, "impl Serialize for {name} {{").unwrap();
        writeln!(
            output,
            "    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {{"
        )
        .unwrap();
        writeln!(output, "        match self {{").unwrap();
        for val in values {
            let s = val.as_str().unwrap();
            let variant = enum_variant_name(s);
            writeln!(
                output,
                "            {name}::{variant} => serializer.serialize_str({s:?}),"
            )
            .unwrap();
        }
        writeln!(
            output,
            "            {name}::Other(s) => serializer.serialize_str(s),"
        )
        .unwrap();
        writeln!(output, "        }}").unwrap();
        writeln!(output, "    }}").unwrap();
        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();

        // Custom Deserialize
        writeln!(output, "impl<'de> Deserialize<'de> for {name} {{").unwrap();
        writeln!(
            output,
            "    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {{"
        )
        .unwrap();
        writeln!(
            output,
            "        let s = String::deserialize(deserializer)?;"
        )
        .unwrap();
        writeln!(output, "        Ok(match s.as_str() {{").unwrap();
        for val in values {
            let s = val.as_str().unwrap();
            let variant = enum_variant_name(s);
            writeln!(output, "            {s:?} => {name}::{variant},").unwrap();
        }
        writeln!(output, "            _ => {name}::Other(s),").unwrap();
        writeln!(output, "        }})").unwrap();
        writeln!(output, "    }}").unwrap();
        writeln!(output, "}}").unwrap();
    } else {
        // Closed enum
        writeln!(
            output,
            "#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]"
        )
        .unwrap();
        writeln!(output, "pub enum {name} {{").unwrap();
        for (i, val) in values.iter().enumerate() {
            let s = val.as_str().unwrap();
            if let Some(descs) = descriptions {
                if let Some(desc) = descs.get(i).and_then(|d| d.as_str()) {
                    write_doc_comment(output, desc, "    ");
                }
            }
            let variant = enum_variant_name(s);
            if variant != s {
                writeln!(output, "    #[serde(rename = {s:?})]").unwrap();
            }
            writeln!(output, "    {variant},").unwrap();
        }
        writeln!(output, "}}").unwrap();
    }
    writeln!(output).unwrap();
}

fn generate_struct(
    output: &mut String,
    name: &str,
    def: &Value,
    overrides: &HashMap<(&str, &str), &str>,
    definitions: &serde_json::Map<String, Value>,
) {
    generate_struct_from_object(output, name, def, overrides, definitions);
}

fn generate_struct_from_object(
    output: &mut String,
    name: &str,
    def: &Value,
    overrides: &HashMap<(&str, &str), &str>,
    _definitions: &serde_json::Map<String, Value>,
) {
    let description = def.get("description").and_then(|d| d.as_str());
    if let Some(desc) = description {
        write_doc_comment(output, desc, "");
    }

    let properties = def.get("properties").and_then(|p| p.as_object());
    let required: HashSet<&str> = def
        .get("required")
        .and_then(|r| r.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let has_additional_properties = def.get("additionalProperties").is_some()
        && def["additionalProperties"] != Value::Bool(false);

    let all_optional = properties
        .map(|props| props.keys().all(|k| !required.contains(k.as_str())))
        .unwrap_or(true)
        && !has_additional_properties;

    if all_optional {
        writeln!(
            output,
            "#[derive(Serialize, Deserialize, Debug, Clone, Default)]"
        )
        .unwrap();
    } else {
        writeln!(output, "#[derive(Serialize, Deserialize, Debug, Clone)]").unwrap();
    }
    writeln!(output, "#[serde(rename_all = \"camelCase\")]").unwrap();
    writeln!(output, "pub struct {name} {{").unwrap();

    if let Some(props) = properties {
        let sorted_props: BTreeMap<&String, &Value> = props.iter().collect();
        for (field_name, field_def) in &sorted_props {
            let field_desc = field_def.get("description").and_then(|d| d.as_str());
            if let Some(desc) = field_desc {
                write_doc_comment(output, desc, "    ");
            }

            let is_required = required.contains(field_name.as_str());

            // Check for override
            if let Some(&override_type) = overrides.get(&(name, field_name.as_str())) {
                let rust_field = to_rust_field_name(field_name);
                let serde_attr = serde_rename_attr(field_name, &rust_field);
                if !serde_attr.is_empty() {
                    writeln!(output, "    {serde_attr}").unwrap();
                }
                if override_type.starts_with("Option<") {
                    writeln!(
                        output,
                        "    #[serde(skip_serializing_if = \"Option::is_none\")]"
                    )
                    .unwrap();
                }
                writeln!(output, "    pub {rust_field}: {override_type},").unwrap();
                continue;
            }

            let rust_type = json_type_to_rust(field_def, is_required);
            let rust_field = to_rust_field_name(field_name);
            let serde_attr = serde_rename_attr(field_name, &rust_field);
            if !serde_attr.is_empty() {
                writeln!(output, "    {serde_attr}").unwrap();
            }
            if !is_required {
                writeln!(
                    output,
                    "    #[serde(skip_serializing_if = \"Option::is_none\")]"
                )
                .unwrap();
            }
            writeln!(output, "    pub {rust_field}: {rust_type},").unwrap();
        }
    }

    if has_additional_properties {
        writeln!(output, "    #[serde(flatten)]").unwrap();
        writeln!(
            output,
            "    pub additional_properties: Option<serde_json::Map<String, serde_json::Value>>,"
        )
        .unwrap();
    }

    writeln!(output, "}}").unwrap();
    writeln!(output).unwrap();
}

fn generate_composed_struct(
    output: &mut String,
    name: &str,
    def: &Value,
    overrides: &HashMap<(&str, &str), &str>,
    definitions: &serde_json::Map<String, Value>,
) {
    let all_of = def["allOf"].as_array().unwrap();

    // Collect all properties and required fields from all parts
    let mut all_properties: BTreeMap<String, Value> = BTreeMap::new();
    let mut all_required: HashSet<String> = HashSet::new();
    let mut description: Option<&str> = def.get("description").and_then(|d| d.as_str());

    for part in all_of {
        if let Some(r) = part.get("$ref").and_then(|r| r.as_str()) {
            let ref_name = ref_to_type_name(r);
            if let Some(ref_def) = definitions.get(&ref_name) {
                if let Some(props) = ref_def.get("properties").and_then(|p| p.as_object()) {
                    for (k, v) in props {
                        all_properties.insert(k.clone(), v.clone());
                    }
                }
                if let Some(req) = ref_def.get("required").and_then(|r| r.as_array()) {
                    for r in req {
                        if let Some(s) = r.as_str() {
                            all_required.insert(s.to_string());
                        }
                    }
                }
            }
        } else {
            if description.is_none() {
                description = part.get("description").and_then(|d| d.as_str());
            }
            if let Some(props) = part.get("properties").and_then(|p| p.as_object()) {
                for (k, v) in props {
                    all_properties.insert(k.clone(), v.clone());
                }
            }
            if let Some(req) = part.get("required").and_then(|r| r.as_array()) {
                for r in req {
                    if let Some(s) = r.as_str() {
                        all_required.insert(s.to_string());
                    }
                }
            }
        }
    }

    if let Some(desc) = description {
        write_doc_comment(output, desc, "");
    }

    let all_optional = all_properties.keys().all(|k| !all_required.contains(k));

    if all_optional {
        writeln!(
            output,
            "#[derive(Serialize, Deserialize, Debug, Clone, Default)]"
        )
        .unwrap();
    } else {
        writeln!(output, "#[derive(Serialize, Deserialize, Debug, Clone)]").unwrap();
    }
    writeln!(output, "#[serde(rename_all = \"camelCase\")]").unwrap();
    writeln!(output, "pub struct {name} {{").unwrap();

    for (field_name, field_def) in &all_properties {
        let field_desc = field_def.get("description").and_then(|d| d.as_str());
        if let Some(desc) = field_desc {
            write_doc_comment(output, desc, "    ");
        }

        let is_required = all_required.contains(field_name);

        if let Some(&override_type) = overrides.get(&(name, field_name.as_str())) {
            let rust_field = to_rust_field_name(field_name);
            let serde_attr = serde_rename_attr(field_name, &rust_field);
            if !serde_attr.is_empty() {
                writeln!(output, "    {serde_attr}").unwrap();
            }
            writeln!(output, "    pub {rust_field}: {override_type},").unwrap();
            continue;
        }

        let rust_type = json_type_to_rust(&field_def, is_required);
        let rust_field = to_rust_field_name(field_name);
        let serde_attr = serde_rename_attr(field_name, &rust_field);
        if !serde_attr.is_empty() {
            writeln!(output, "    {serde_attr}").unwrap();
        }
        if !is_required {
            writeln!(
                output,
                "    #[serde(skip_serializing_if = \"Option::is_none\")]"
            )
            .unwrap();
        }
        writeln!(output, "    pub {rust_field}: {rust_type},").unwrap();
    }

    writeln!(output, "}}").unwrap();
    writeln!(output).unwrap();
}

fn generate_request_arguments_enum(
    output: &mut String,
    command_map: &BTreeMap<String, (Option<String>, Option<String>)>,
) {
    writeln!(output, "/// Dispatch enum for all DAP request types.").unwrap();
    writeln!(output, "#[derive(Serialize, Deserialize, Debug, Clone)]").unwrap();
    writeln!(
        output,
        "#[serde(tag = \"command\", content = \"arguments\", rename_all = \"camelCase\")]"
    )
    .unwrap();
    writeln!(output, "pub enum RequestArguments {{").unwrap();

    for (cmd, (args_type, desc)) in command_map {
        if let Some(desc) = desc {
            write_doc_comment(output, desc, "    ");
        }
        let variant = to_pascal_case(cmd);
        let serde_attr = request_variant_serde_attr(cmd, &variant);
        if !serde_attr.is_empty() {
            writeln!(output, "    {serde_attr}").unwrap();
        }
        if let Some(args) = args_type {
            writeln!(output, "    {variant}({args}),").unwrap();
        } else {
            writeln!(output, "    {variant},").unwrap();
        }
    }

    writeln!(output, "}}").unwrap();
    writeln!(output).unwrap();
}

fn generate_response_body_enum(
    output: &mut String,
    response_map: &BTreeMap<String, (Option<ResponseBodyInfo>, Option<String>)>,
) {
    writeln!(output, "/// Dispatch enum for all DAP response body types.").unwrap();
    writeln!(output, "#[derive(Serialize, Deserialize, Debug, Clone)]").unwrap();
    writeln!(
        output,
        "#[serde(tag = \"command\", content = \"body\", rename_all = \"camelCase\")]"
    )
    .unwrap();
    writeln!(output, "pub enum ResponseBody {{").unwrap();

    for (cmd, (body_info, desc)) in response_map {
        if let Some(desc) = desc {
            write_doc_comment(output, desc, "    ");
        }
        let variant = to_pascal_case(cmd);
        let serde_attr = request_variant_serde_attr(cmd, &variant);
        if !serde_attr.is_empty() {
            writeln!(output, "    {serde_attr}").unwrap();
        }
        match body_info {
            Some(ResponseBodyInfo::Ref(type_name)) => {
                writeln!(output, "    {variant}({type_name}),").unwrap();
            }
            Some(ResponseBodyInfo::Inline(_)) => {
                let body_struct = format!("{variant}Body");
                writeln!(output, "    {variant}({body_struct}),").unwrap();
            }
            None => {
                writeln!(output, "    {variant},").unwrap();
            }
        }
    }

    writeln!(output, "}}").unwrap();
    writeln!(output).unwrap();
}

fn generate_event_enum(
    output: &mut String,
    event_map: &BTreeMap<String, (Option<EventBodyInfo>, Option<String>)>,
) {
    // Generate EventHelper (for serde tag-based deserialization)
    writeln!(output, "#[derive(Debug, Clone, Deserialize)]").unwrap();
    writeln!(
        output,
        "#[serde(tag = \"event\", content = \"body\", rename_all = \"camelCase\")]"
    )
    .unwrap();
    writeln!(output, "enum EventHelper {{").unwrap();
    for (evt, (body_info, _desc)) in event_map {
        let variant = to_pascal_case(evt);
        let serde_attr = request_variant_serde_attr(evt, &variant);
        if !serde_attr.is_empty() {
            writeln!(output, "    {serde_attr}").unwrap();
        }
        match body_info {
            Some(EventBodyInfo::Ref(type_name)) => {
                writeln!(output, "    {variant}({type_name}),").unwrap();
            }
            Some(EventBodyInfo::Inline(_)) => {
                let body_struct = format!("{variant}EventBody");
                writeln!(output, "    {variant}({body_struct}),").unwrap();
            }
            None => {
                writeln!(output, "    {variant},").unwrap();
            }
        }
    }
    writeln!(output, "}}").unwrap();
    writeln!(output).unwrap();

    // Generate Event enum itself
    writeln!(output, "/// Dispatch enum for all DAP event types.").unwrap();
    writeln!(output, "#[derive(Debug, Clone, Serialize)]").unwrap();
    writeln!(
        output,
        "#[serde(tag = \"event\", content = \"body\", rename_all = \"camelCase\")]"
    )
    .unwrap();
    writeln!(output, "pub enum Event {{").unwrap();
    for (evt, (body_info, desc)) in event_map {
        if let Some(desc) = desc {
            write_doc_comment(output, desc, "    ");
        }
        let variant = to_pascal_case(evt);
        let serde_attr = request_variant_serde_attr(evt, &variant);
        if !serde_attr.is_empty() {
            writeln!(output, "    {serde_attr}").unwrap();
        }
        match body_info {
            Some(EventBodyInfo::Ref(type_name)) => {
                writeln!(output, "    {variant}({type_name}),").unwrap();
            }
            Some(EventBodyInfo::Inline(_)) => {
                let body_struct = format!("{variant}EventBody");
                writeln!(output, "    {variant}({body_struct}),").unwrap();
            }
            None => {
                writeln!(output, "    {variant},").unwrap();
            }
        }
    }
    writeln!(output, "    #[serde(skip)]").unwrap();
    writeln!(output, "    Unknown,").unwrap();
    writeln!(output, "}}").unwrap();
    writeln!(output).unwrap();

    // Generate From<EventHelper> for Event
    writeln!(output, "impl From<EventHelper> for Event {{").unwrap();
    writeln!(output, "    fn from(helper: EventHelper) -> Self {{").unwrap();
    writeln!(output, "        match helper {{").unwrap();
    for (evt, (body_info, _desc)) in event_map {
        let variant = to_pascal_case(evt);
        match body_info {
            Some(_) => {
                writeln!(
                    output,
                    "            EventHelper::{variant}(body) => Event::{variant}(body),"
                )
                .unwrap();
            }
            None => {
                writeln!(
                    output,
                    "            EventHelper::{variant} => Event::{variant},"
                )
                .unwrap();
            }
        }
    }
    writeln!(output, "        }}").unwrap();
    writeln!(output, "    }}").unwrap();
    writeln!(output, "}}").unwrap();
    writeln!(output).unwrap();

    // Generate custom Deserialize for Event
    writeln!(output, "impl<'de> Deserialize<'de> for Event {{").unwrap();
    writeln!(
        output,
        "    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>"
    )
    .unwrap();
    writeln!(output, "    where").unwrap();
    writeln!(output, "        D: Deserializer<'de>,").unwrap();
    writeln!(output, "    {{").unwrap();
    writeln!(
        output,
        "        let value = serde_json::Value::deserialize(deserializer)?;"
    )
    .unwrap();
    writeln!(
        output,
        "        match serde_json::from_value::<EventHelper>(value.clone()) {{"
    )
    .unwrap();
    writeln!(output, "            Ok(helper) => Ok(helper.into()),").unwrap();
    writeln!(output, "            Err(_) => {{").unwrap();
    writeln!(
        output,
        "                // If body is missing, try with an empty body object"
    )
    .unwrap();
    writeln!(output, "                // (some events have all-optional body fields and adapters may omit the body entirely)").unwrap();
    writeln!(
        output,
        "                if let Some(obj) = value.as_object() {{"
    )
    .unwrap();
    writeln!(
        output,
        "                    if !obj.contains_key(\"body\") {{"
    )
    .unwrap();
    writeln!(
        output,
        "                        let mut patched = obj.clone();"
    )
    .unwrap();
    writeln!(output, "                        patched.insert(\"body\".to_string(), serde_json::Value::Object(Default::default()));").unwrap();
    writeln!(output, "                        if let Ok(helper) = serde_json::from_value::<EventHelper>(serde_json::Value::Object(patched)) {{").unwrap();
    writeln!(
        output,
        "                            return Ok(helper.into());"
    )
    .unwrap();
    writeln!(output, "                        }}").unwrap();
    writeln!(output, "                    }}").unwrap();
    writeln!(output, "                }}").unwrap();
    writeln!(output, "                Ok(Event::Unknown)").unwrap();
    writeln!(output, "            }}").unwrap();
    writeln!(output, "        }}").unwrap();
    writeln!(output, "    }}").unwrap();
    writeln!(output, "}}").unwrap();
    writeln!(output).unwrap();
}

// --- Type mapping ---

fn json_type_to_rust(field_def: &Value, is_required: bool) -> String {
    let base = json_type_to_rust_inner(field_def);
    if is_required {
        base
    } else {
        format!("Option<{base}>")
    }
}

fn json_type_to_rust_inner(field_def: &Value) -> String {
    // Check for $ref first
    if let Some(r) = field_def.get("$ref").and_then(|r| r.as_str()) {
        return ref_to_type_name(r);
    }

    // Check for enum/open enum on string type -> use String (unless it's a standalone type)
    // For inline enums in properties, just use String
    if field_def.get("type").and_then(|t| t.as_str()) == Some("string")
        && (field_def.get("enum").is_some() || field_def.get("_enum").is_some())
    {
        return "String".to_string();
    }

    let type_val = field_def.get("type");

    match type_val {
        Some(Value::String(s)) => match s.as_str() {
            "string" => "String".to_string(),
            "integer" => "i64".to_string(),
            "number" => "f64".to_string(),
            "boolean" => "bool".to_string(),
            "object" => {
                // Check for additionalProperties
                if let Some(ap) = field_def.get("additionalProperties") {
                    if ap.is_object() {
                        let value_type = json_type_to_rust_inner(ap);
                        return format!("std::collections::HashMap<String, {value_type}>");
                    }
                }
                "serde_json::Value".to_string()
            }
            "array" => {
                if let Some(items) = field_def.get("items") {
                    let item_type = json_type_to_rust_inner(items);
                    format!("Vec<{item_type}>")
                } else {
                    "Vec<serde_json::Value>".to_string()
                }
            }
            "null" => "serde_json::Value".to_string(),
            _ => "serde_json::Value".to_string(),
        },
        Some(Value::Array(_)) => {
            // Multi-type → serde_json::Value
            "serde_json::Value".to_string()
        }
        None => {
            // No type specified
            if field_def.get("$ref").is_some() {
                // Already handled above, but just in case
                let r = field_def["$ref"].as_str().unwrap();
                ref_to_type_name(r)
            } else if field_def.get("oneOf").is_some() || field_def.get("anyOf").is_some() {
                "serde_json::Value".to_string()
            } else {
                "serde_json::Value".to_string()
            }
        }
        _ => "serde_json::Value".to_string(),
    }
}

// --- Naming helpers ---

fn ref_to_type_name(r: &str) -> String {
    r.strip_prefix("#/definitions/").unwrap_or(r).to_string()
}

fn to_rust_field_name(name: &str) -> String {
    // Handle leading underscores (e.g., __restart)
    let stripped = name.trim_start_matches('_');
    let base = camel_to_snake(stripped);
    let rust_name = if name.starts_with('_') {
        base.clone()
    } else {
        base
    };

    // Handle Rust keywords
    match rust_name.as_str() {
        "type" => "r#type".to_string(),
        "ref" => "r#ref".to_string(),
        "mod" => "r#mod".to_string(),
        "enum" => "r#enum".to_string(),
        _ => rust_name,
    }
}

fn serde_rename_attr(json_name: &str, rust_field: &str) -> String {
    // Strip r# prefix for comparison
    let clean_rust = rust_field.strip_prefix("r#").unwrap_or(rust_field);

    // What camelCase would produce from rust_field
    let expected_camel = snake_to_camel(clean_rust);

    if expected_camel != json_name {
        format!("#[serde(rename = {json_name:?})]")
    } else {
        String::new()
    }
}

fn request_variant_serde_attr(cmd: &str, variant: &str) -> String {
    // Check if to_pascal_case(cmd) when lowered back to camelCase matches cmd
    let expected = pascal_to_camel(variant);
    if expected != cmd {
        format!("#[serde(rename = {cmd:?})]")
    } else {
        String::new()
    }
}

fn camel_to_snake(s: &str) -> String {
    let mut result = String::new();
    let mut prev_was_upper = false;
    let mut prev_was_separator = true;

    for (i, c) in s.char_indices() {
        if c.is_uppercase() {
            // Check if next char is lowercase (to handle acronyms like "ID")
            let next_is_lower = s[i + c.len_utf8()..]
                .chars()
                .next()
                .map_or(false, |nc| nc.is_lowercase());

            if !prev_was_separator && (!prev_was_upper || next_is_lower) {
                result.push('_');
            }
            result.push(c.to_lowercase().next().unwrap());
            prev_was_upper = true;
        } else {
            result.push(c);
            prev_was_upper = false;
        }
        prev_was_separator = false;
    }
    result
}

fn snake_to_camel(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize = false;
    for (i, c) in s.char_indices() {
        if c == '_' {
            capitalize = true;
        } else if capitalize {
            result.push(c.to_uppercase().next().unwrap());
            capitalize = false;
        } else if i == 0 {
            result.push(c); // Keep first char lowercase
        } else {
            result.push(c);
        }
    }
    result
}

fn to_pascal_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize = true;
    for c in s.chars() {
        if c == '_' || c == ' ' || c == '-' {
            capitalize = true;
        } else if capitalize {
            result.push(c.to_uppercase().next().unwrap());
            capitalize = false;
        } else {
            result.push(c);
        }
    }
    result
}

fn pascal_to_camel(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => {
            let mut result: String = c.to_lowercase().collect();
            result.extend(chars);
            result
        }
    }
}

fn enum_variant_name(s: &str) -> String {
    // Convert enum string values to PascalCase variant names
    // Handle spaces and special characters
    to_pascal_case(s)
}

fn write_doc_comment(output: &mut String, desc: &str, indent: &str) {
    for line in desc.lines() {
        if line.trim().is_empty() {
            writeln!(output, "{indent}///").unwrap();
        } else {
            writeln!(output, "{indent}/// {line}").unwrap();
        }
    }
}
