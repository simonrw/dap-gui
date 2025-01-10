use proc_macro2::TokenStream;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt::Display, path::PathBuf};

// spec types

#[derive(Deserialize, Serialize)]
struct Definition<'i> {
    #[serde(borrow, rename = "type")]
    r#type: Option<&'i str>,
}

#[derive(Deserialize, Serialize)]
struct Spec<'i> {
    #[serde(borrow)]
    definitions: HashMap<String, Definition<'i>>,
}

impl<'i> Display for Spec<'i> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let t = serde_json::to_string_pretty(self).unwrap();
        f.write_str(&t)
    }
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
    let spec: Spec = serde_json::from_str(&spec_content).unwrap();

    let out_dir = PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let out_path = out_dir.join("bindings.rs");

    let tokens = generate_struct("MyStruct");
    let text = format!("{tokens}");
    eprintln!("Generating text: {text}");
    std::fs::write(&out_path, text).unwrap();
}
