use proc_macro2::TokenStream;
use serde::Deserialize;
use std::{collections::HashMap, fmt::Display, path::PathBuf};

// spec types

#[derive(Deserialize, Debug)]
struct ArrayItemDefinition {
    r#type: String,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Property {
    Integer {
        description: String,
    },
    String {
        description: String,
        #[serde(rename = "_enum")]
        allowed_values: Option<Vec<String>>,
    },
    Array {
        items: ArrayItemDefinition,
        description: String,
    },
    Object {
        description: String,
    },
    Boolean {
        description: String,
    },
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum Definition {
    Root {
        r#type: String,
        title: Option<String>,
        description: String,
        properties: HashMap<String, Property>,
    },
    AllOf {
        #[serde(rename = "allOf")]
        all_of: Vec<serde_json::Value>,
    },
}

#[derive(Deserialize, Debug)]
struct Spec {
    definitions: HashMap<String, Definition>,
}

// helper methods

fn generate_struct(name: impl AsRef<str>) -> TokenStream {
    let name = quote::format_ident!("{}", name.as_ref());
    quote::quote! {
        pub struct #name;
    }
}

fn main() {
    let spec_path = PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("..")
        .join("spec")
        .join("microsoft.github.io")
        .join("debug-adapter-protocol")
        .join("debugAdapterProtocol.json")
        .canonicalize()
        .unwrap();
    let spec_content = std::fs::read_to_string(spec_path).unwrap();
    // let spec: Spec = serde_json::from_str(&spec_content).unwrap();
    let jd = &mut serde_json::Deserializer::from_str(&spec_content);
    let spec: Result<Spec, _> = serde_path_to_error::deserialize(jd);
    match spec {
        Ok(spec) => eprintln!("got spec: {spec:?}"),
        Err(e) => {
            let _path = e.path().to_string();
            // TODO: panic!("parse error at {}: {}", path, e.inner());
        }
    }

    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let out_path = out_dir.join("bindings.rs");

    let tokens = generate_struct("MyStruct");
    let text = format!("{tokens}");
    eprintln!("Generating text: {text}");
    std::fs::write(&out_path, text).unwrap();
}
