// TODO: VS code launch json files include comments

use std::path::{Path, PathBuf};

use eyre::Context;
use serde::Deserialize;

// re-export
pub use transport::requests::PathMapping;

#[derive(Deserialize)]
#[serde(untagged)]
enum ConfigFormat {
    VsCode {
        // TODO: probably have to handle versions for these configuration files
        #[serde(rename = "version")]
        _version: String,
        configurations: Vec<LaunchConfiguration>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum LaunchConfiguration {
    Debugpy(Debugpy),
}

pub fn load(
    name: impl AsRef<str>,
    mut r: impl std::io::Read,
) -> eyre::Result<Option<LaunchConfiguration>> {
    let mut contents = String::new();
    r.read_to_string(&mut contents)
        .wrap_err("reading configuration contents")?;
    let configuration = from_str(name, &contents).wrap_err("parsing launch configuration")?;
    Ok(configuration)
}

fn from_str(name: impl AsRef<str>, contents: &str) -> eyre::Result<Option<LaunchConfiguration>> {
    // let config: ConfigFormat = serde_json::from_reader(r).context("reading and deserialising")?;
    let config = jsonc_to_serde(contents).wrap_err("parsing jsonc configuration")?;
    let name = name.as_ref();
    match config {
        ConfigFormat::VsCode { configurations, .. } => {
            for configuration in configurations {
                match &configuration {
                    LaunchConfiguration::Debugpy(Debugpy {
                        name: config_name, ..
                    }) => {
                        if config_name == name {
                            return Ok(Some(configuration));
                        }
                    }
                }
            }
        }
    }
    Ok(None)
}

fn jsonc_to_serde(input: &str) -> eyre::Result<ConfigFormat> {
    let value = jsonc_parser::parse_to_serde_value(input, &Default::default())
        .wrap_err("parsing jsonc configuration")?;
    let Some(config_format_value) = value else {
        eyre::bail!("no configuration found");
    };
    let config_format = serde_json::from_value(config_format_value)
        .wrap_err("deserializing serde_json::Value value")?;
    Ok(config_format)
}

pub fn load_from_path(
    name: impl AsRef<str>,
    path: impl AsRef<Path>,
) -> eyre::Result<Option<LaunchConfiguration>> {
    let f = std::fs::File::open(path).wrap_err("opening input path")?;
    let config = crate::load(name, f).context("loading file from given path")?;
    Ok(config)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Debugpy {
    pub name: String,
    pub r#type: String,
    pub request: String,
    pub connect: ConnectionDetails,
    pub path_mappings: Option<Vec<PathMapping>>,
    pub just_my_code: Option<bool>,
    pub cwd: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub struct ConnectionDetails {
    pub host: String,
    pub port: u16,
}
