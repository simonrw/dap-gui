//! Launch configuration management
//!
//! This crate handles parsing the launch configurations, primarily of VS Code.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use eyre::Context;
use serde::Deserialize;

// re-export
pub use transport::requests::PathMapping;

/// Handle choosing a specific launch configuration, or if the user has not specified one, then
/// present a list of launch configurations they can choose from
pub enum ChosenLaunchConfiguration {
    /// A specific launch configuration is available
    Specific(LaunchConfiguration),
    /// The specified launch configuration was not found
    NotFound,
    /// The user did not request a specific launch configuration, so present available options
    ToBeChosen(Vec<String>),
}

#[derive(Deserialize)]
struct VsCodeLaunchConfiguration {
    #[serde(rename = "version")]
    _version: String,
    configurations: Vec<LaunchConfiguration>,
}

/// Deserializable model for the launch configuration
#[derive(Deserialize)]
#[serde(untagged)]
enum ConfigFormat {
    VsCode(VsCodeLaunchConfiguration),
    VsCodeWorkspace {
        folders: Vec<Folder>,
        settings: HashMap<String, serde_json::Value>,
        launch: VsCodeLaunchConfiguration,
    },
}

#[derive(Deserialize)]
struct Folder {
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum LaunchConfiguration {
    Debugpy(Debugpy),
}

pub fn load(
    name: Option<&String>,
    mut r: impl std::io::Read,
) -> eyre::Result<ChosenLaunchConfiguration> {
    let mut contents = String::new();
    r.read_to_string(&mut contents)
        .wrap_err("reading configuration contents")?;
    let configuration = from_str(name, &contents).wrap_err("parsing launch configuration")?;
    Ok(configuration)
}

fn from_str(name: Option<&String>, contents: &str) -> eyre::Result<ChosenLaunchConfiguration> {
    // let config: ConfigFormat = serde_json::from_reader(r).context("reading and deserialising")?;
    let config = jsonc_to_serde(contents).wrap_err("parsing jsonc configuration")?;

    match config {
        ConfigFormat::VsCode(VsCodeLaunchConfiguration { configurations, .. }) => {
            if let Some(name) = name {
                for configuration in configurations {
                    match &configuration {
                        LaunchConfiguration::Debugpy(Debugpy {
                            name: config_name, ..
                        }) => {
                            if config_name == name {
                                return Ok(ChosenLaunchConfiguration::Specific(configuration));
                            }
                        }
                    }
                }
            } else {
                let configuration_names: Vec<_> = configurations
                    .iter()
                    .map(|c| match &c {
                        LaunchConfiguration::Debugpy(Debugpy { name, .. }) => name.clone(),
                    })
                    .collect();
                return Ok(ChosenLaunchConfiguration::ToBeChosen(configuration_names));
            }
        }
        ConfigFormat::VsCodeWorkspace { .. } => todo!(),
    }
    Ok(ChosenLaunchConfiguration::NotFound)
}

fn jsonc_to_serde(input: &str) -> eyre::Result<ConfigFormat> {
    let value = jsonc_parser::parse_to_serde_value(input, &Default::default())
        .wrap_err("parsing jsonc configuration")?;
    let Some(config_format_value) = value else {
        eyre::bail!("no configuration found");
    };

    let config_format =
        serde_json::from_value(config_format_value).wrap_err("deserializing jsonc::Value value")?;
    Ok(config_format)
}

pub fn load_from_path(
    name: Option<&String>,
    path: impl AsRef<Path>,
) -> eyre::Result<ChosenLaunchConfiguration> {
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
    pub connect: Option<ConnectionDetails>,
    pub path_mappings: Option<Vec<PathMapping>>,
    pub just_my_code: Option<bool>,
    pub cwd: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
pub struct ConnectionDetails {
    pub host: String,
    pub port: u16,
}
