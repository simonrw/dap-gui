//! Launch configuration management
//!
//! This crate handles parsing the launch configurations, primarily of VS Code.
//!
//! The path-based loaders ([`load_from_path`] and [`load_all_from_path`]) automatically
//! resolve VS Code variable placeholders before returning configurations:
//! - `${workspaceFolder}` — the workspace root directory
//! - `${workspaceFolder:name}` — named workspace folder (from `.code-workspace` files)
//! - `${env:VARNAME}` — environment variable lookup
//!
//! The reader-based [`load`] function does **not** resolve variables because it has no
//! file path context to derive the workspace root from.

use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
};

use eyre::Context;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PathMapping {
    pub local_root: String,
    pub remote_root: String,
}

/// Context needed to resolve VS Code variable placeholders.
#[derive(Debug, Clone)]
struct ResolutionContext {
    /// The workspace root directory (for `${workspaceFolder}`)
    workspace_root: PathBuf,
    /// Named folder mappings (for `${workspaceFolder:name}`)
    named_folders: HashMap<String, PathBuf>,
}

impl ResolutionContext {
    /// Build context for a `.vscode/launch.json` file.
    ///
    /// The workspace root is the grandparent of the file (parent of `.vscode/`).
    fn from_vscode_launch(config_path: &Path) -> Self {
        let workspace_root = config_path
            .parent() // .vscode/
            .and_then(|p| p.parent()) // project root
            .unwrap_or(Path::new("."))
            .to_path_buf();
        Self {
            workspace_root,
            named_folders: HashMap::new(),
        }
    }

    /// Build context for a `.code-workspace` file.
    ///
    /// The workspace root is the directory containing the file. Named folders are
    /// built from the `folders` array, with relative paths resolved against the
    /// workspace root.
    fn from_workspace_file(config_path: &Path, folders: &[Folder]) -> Self {
        let workspace_root = config_path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let mut named_folders = HashMap::new();
        for folder in folders {
            let folder_path = if Path::new(&folder.path).is_absolute() {
                PathBuf::from(&folder.path)
            } else {
                workspace_root.join(&folder.path)
            };
            // Use explicit name if provided, otherwise use directory basename
            let name = folder.name.clone().unwrap_or_else(|| {
                Path::new(&folder.path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
            });
            named_folders.entry(name).or_insert(folder_path);
        }
        Self {
            workspace_root,
            named_folders,
        }
    }

    /// Resolve all VS Code variable placeholders in a string.
    fn resolve_string(&self, input: &str) -> String {
        let mut result = input.to_string();

        // Resolve ${workspaceFolder:name} first (more specific pattern)
        while let Some(start) = result.find("${workspaceFolder:") {
            let after = &result[start + "${workspaceFolder:".len()..];
            let Some(end) = after.find('}') else {
                break;
            };
            let name = after[..end].to_string();
            let replacement = self
                .named_folders
                .get(&name)
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| format!("${{workspaceFolder:{name}}}"));
            let pattern = format!("${{workspaceFolder:{name}}}");
            result = result.replacen(&pattern, &replacement, 1);

            // If the replacement is the same as the pattern (unknown name), break
            // to avoid infinite loop
            if replacement == pattern {
                break;
            }
        }

        // Resolve ${workspaceFolder}
        result = result.replace(
            "${workspaceFolder}",
            &self.workspace_root.display().to_string(),
        );

        // Resolve ${env:VARNAME}
        while let Some(start) = result.find("${env:") {
            let after = &result[start + "${env:".len()..];
            let Some(end) = after.find('}') else {
                break;
            };
            let var_name = after[..end].to_string();
            let replacement = std::env::var(&var_name).unwrap_or_default();
            let pattern = format!("${{env:{var_name}}}");
            result = result.replacen(&pattern, &replacement, 1);
        }

        result
    }

    fn resolve_pathbuf(&self, path: &Path) -> PathBuf {
        PathBuf::from(self.resolve_string(&path.display().to_string()))
    }
}

/// Trait for types that can resolve VS Code variable placeholders in their fields.
trait Resolve {
    fn resolve(&mut self, ctx: &ResolutionContext);
}

/// Handle choosing a specific launch configuration, or if the user has not specified one, then
/// present a list of launch configurations they can choose from
#[allow(clippy::large_enum_variant)]
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
        #[serde(default)]
        folders: Vec<Folder>,
        launch: VsCodeLaunchConfiguration,
    },
}

#[derive(Debug, Clone, Deserialize)]
struct Folder {
    path: String,
    name: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LaunchConfiguration {
    Debugpy(Debugpy),
    Python(Debugpy),
    LLDB(LLDB),
    Go(Delve),
}

impl LaunchConfiguration {
    pub fn name(&self) -> &str {
        match self {
            LaunchConfiguration::Debugpy(d) | LaunchConfiguration::Python(d) => &d.name,
            LaunchConfiguration::LLDB(l) => &l.name,
            LaunchConfiguration::Go(d) => &d.name,
        }
    }

    pub fn cwd(&self) -> Option<&Path> {
        match self {
            LaunchConfiguration::Debugpy(d) | LaunchConfiguration::Python(d) => d.cwd.as_deref(),
            LaunchConfiguration::LLDB(l) => l.cwd.as_deref().map(Path::new),
            LaunchConfiguration::Go(d) => d.cwd.as_deref(),
        }
    }
}

impl Resolve for LaunchConfiguration {
    fn resolve(&mut self, ctx: &ResolutionContext) {
        match self {
            LaunchConfiguration::Debugpy(debugpy) | LaunchConfiguration::Python(debugpy) => {
                debugpy.resolve(ctx);
            }
            LaunchConfiguration::LLDB(lldb) => lldb.resolve(ctx),
            LaunchConfiguration::Go(delve) => delve.resolve(ctx),
        }
    }
}

/// Load a launch configuration from a reader.
///
/// **Note:** Variables like `${workspaceFolder}` are NOT resolved because there
/// is no file path context. Use [`load_from_path`] for automatic resolution.
pub fn load(
    name: Option<&String>,
    mut r: impl std::io::Read,
) -> eyre::Result<ChosenLaunchConfiguration> {
    let mut contents = String::new();
    r.read_to_string(&mut contents)
        .wrap_err("reading configuration contents")?;
    let parsed = parse_config(&contents).wrap_err("parsing launch configuration")?;
    Ok(select_config(name, parsed.configurations))
}

struct ParsedConfig {
    configurations: Vec<LaunchConfiguration>,
    folders: Vec<Folder>,
    is_workspace: bool,
}

fn parse_config(contents: &str) -> eyre::Result<ParsedConfig> {
    let config = jsonc_to_serde(contents)?;
    match config {
        ConfigFormat::VsCode(v) => Ok(ParsedConfig {
            configurations: v.configurations,
            folders: vec![],
            is_workspace: false,
        }),
        ConfigFormat::VsCodeWorkspace { folders, launch } => Ok(ParsedConfig {
            configurations: launch.configurations,
            folders,
            is_workspace: true,
        }),
    }
}

fn select_config(
    name: Option<&String>,
    configurations: Vec<LaunchConfiguration>,
) -> ChosenLaunchConfiguration {
    if let Some(name) = name {
        for config in configurations {
            if config.name() == name {
                return ChosenLaunchConfiguration::Specific(config);
            }
        }
        ChosenLaunchConfiguration::NotFound
    } else {
        let names = configurations
            .iter()
            .map(|c| c.name().to_string())
            .collect();
        ChosenLaunchConfiguration::ToBeChosen(names)
    }
}

fn resolve_configs(configs: &mut [LaunchConfiguration], ctx: &ResolutionContext) {
    for config in configs {
        config.resolve(ctx);
    }
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

/// Load a launch configuration from a file path, resolving all variable
/// placeholders to absolute paths.
pub fn load_from_path(
    name: Option<&String>,
    path: impl AsRef<Path>,
) -> eyre::Result<ChosenLaunchConfiguration> {
    let path = path.as_ref();
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

    let mut contents = String::new();
    std::fs::File::open(path)
        .wrap_err("opening input path")?
        .read_to_string(&mut contents)
        .wrap_err("reading configuration contents")?;

    let parsed = parse_config(&contents).wrap_err("parsing launch configuration")?;
    let ctx = if parsed.is_workspace {
        ResolutionContext::from_workspace_file(&canonical, &parsed.folders)
    } else {
        ResolutionContext::from_vscode_launch(&canonical)
    };

    let mut configurations = parsed.configurations;
    resolve_configs(&mut configurations, &ctx);
    Ok(select_config(name, configurations))
}

/// Load all launch configurations from a file path, resolving all variable
/// placeholders to absolute paths.
pub fn load_all_from_path(path: impl AsRef<Path>) -> eyre::Result<Vec<LaunchConfiguration>> {
    let path = path.as_ref();
    let canonical = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());

    let mut contents = String::new();
    std::fs::File::open(path)
        .wrap_err("opening input path")?
        .read_to_string(&mut contents)
        .wrap_err("reading configuration contents")?;

    let parsed = parse_config(&contents).wrap_err("parsing launch configuration")?;
    let ctx = if parsed.is_workspace {
        ResolutionContext::from_workspace_file(&canonical, &parsed.folders)
    } else {
        ResolutionContext::from_vscode_launch(&canonical)
    };

    let mut configurations = parsed.configurations;
    resolve_configs(&mut configurations, &ctx);
    Ok(configurations)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Debugpy {
    pub name: String,
    pub request: String,
    pub connect: Option<ConnectionDetails>,
    pub program: Option<PathBuf>,
    pub module: Option<String>,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub env_file: Option<PathBuf>,
    pub path_mappings: Option<Vec<PathMapping>>,
    pub just_my_code: Option<bool>,
    pub stop_on_entry: Option<bool>,
    pub cwd: Option<PathBuf>,
}

impl Resolve for Debugpy {
    fn resolve(&mut self, ctx: &ResolutionContext) {
        if let Some(mappings) = &mut self.path_mappings {
            for m in mappings {
                m.local_root = ctx.resolve_string(&m.local_root);
                m.remote_root = ctx.resolve_string(&m.remote_root);
            }
        }
        if let Some(ref mut cwd) = self.cwd {
            *cwd = ctx.resolve_pathbuf(cwd);
        }
        if let Some(ref mut program) = self.program {
            *program = ctx.resolve_pathbuf(program);
        }
        if let Some(ref mut env) = self.env {
            for value in env.values_mut() {
                *value = ctx.resolve_string(value);
            }
        }
        if let Some(ref mut env_file) = self.env_file {
            *env_file = ctx.resolve_pathbuf(env_file);
        }
        if let Some(ref mut args) = self.args {
            for arg in args {
                *arg = ctx.resolve_string(arg);
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LLDB {
    pub name: String,
    pub request: String,
    pub connect: Option<ConnectionDetails>,
    pub cargo: CargoConfig,
    pub cwd: Option<String>,
}

impl Resolve for LLDB {
    fn resolve(&mut self, ctx: &ResolutionContext) {
        if let Some(ref mut cwd) = self.cwd {
            *cwd = ctx.resolve_string(cwd);
        }
        for arg in &mut self.cargo.args {
            *arg = ctx.resolve_string(arg);
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Delve {
    pub name: String,
    pub request: String,
    pub mode: Option<String>,
    pub program: Option<PathBuf>,
    pub args: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
    pub env_file: Option<PathBuf>,
    pub cwd: Option<PathBuf>,
    pub host: Option<String>,
    pub port: Option<u16>,
    pub build_flags: Option<String>,
    pub substitute_path: Option<Vec<SubstitutePath>>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct SubstitutePath {
    pub from: String,
    pub to: String,
}

impl Resolve for Delve {
    fn resolve(&mut self, ctx: &ResolutionContext) {
        if let Some(ref mut cwd) = self.cwd {
            *cwd = ctx.resolve_pathbuf(cwd);
        }
        if let Some(ref mut program) = self.program {
            *program = ctx.resolve_pathbuf(program);
        }
        if let Some(ref mut env) = self.env {
            for value in env.values_mut() {
                *value = ctx.resolve_string(value);
            }
        }
        if let Some(ref mut env_file) = self.env_file {
            *env_file = ctx.resolve_pathbuf(env_file);
        }
        if let Some(ref mut args) = self.args {
            for arg in args {
                *arg = ctx.resolve_string(arg);
            }
        }
        if let Some(ref mut paths) = self.substitute_path {
            for p in paths {
                p.from = ctx.resolve_string(&p.from);
                p.to = ctx.resolve_string(&p.to);
            }
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConnectionDetails {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CargoConfig {
    pub args: Vec<String>,
    pub filter: CargoFilter,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CargoFilter {
    pub kind: String,
}
