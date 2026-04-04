use std::path::PathBuf;

use clap::Parser;
use eyre::Context;
use launch_configuration::LaunchConfiguration;
use state::StateManager;

/// CLI arguments shared by both TUI and GUI frontends.
#[derive(Parser, Clone)]
pub struct Args {
    /// Path to a launch.json or VS Code workspace file.
    pub config_path: PathBuf,

    /// Select a specific configuration by name.
    #[clap(short, long)]
    pub name: Option<String>,

    /// Initial breakpoints in `file:line` format.
    #[clap(short, long)]
    pub breakpoints: Vec<String>,
}

/// The result of bootstrapping the application.
///
/// Contains all the shared state needed by both frontends to start up.
pub struct BootstrapResult {
    pub configs: Vec<LaunchConfiguration>,
    pub config_names: Vec<String>,
    pub selected_config_index: usize,
    pub debug_root_dir: PathBuf,
    pub state_manager: StateManager,
    pub initial_breakpoints: Vec<debugger::Breakpoint>,
}

/// Perform shared application bootstrap: load configurations, set up
/// the state manager, resolve the debug root directory, parse CLI breakpoints,
/// and select the initial configuration.
///
/// This is the common startup logic shared by both TUI and GUI frontends.
pub fn bootstrap(args: &Args) -> eyre::Result<BootstrapResult> {
    // Load configurations
    let configs = launch_configuration::load_all_from_path(&args.config_path)
        .wrap_err("loading launch configurations")?;
    if configs.is_empty() {
        eyre::bail!("no configurations found in {}", args.config_path.display());
    }
    let config_names: Vec<String> = configs.iter().map(|c| c.name().to_string()).collect();

    let debug_root_dir = std::env::current_dir()
        .and_then(|p| std::fs::canonicalize(&p))
        .wrap_err("resolving current directory")?;

    // State manager for breakpoint persistence and UI preferences
    let state_path = dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("dapgui")
        .join("state.json");
    if !state_path.parent().unwrap().is_dir() {
        std::fs::create_dir_all(state_path.parent().unwrap())
            .context("creating state directory")?;
    }
    let state_manager = StateManager::new(state_path).wrap_err("loading state")?;
    state_manager.save().wrap_err("saving initial state")?;

    // Select config by --name or restore last used
    let selected_config_index = if let Some(ref name) = args.name {
        config_names
            .iter()
            .position(|n| n == name)
            .ok_or_else(|| eyre::eyre!("no configuration named '{name}' found"))?
    } else if let Some(ref last) = state_manager.current().last_selected_config {
        config_names.iter().position(|n| n == last).unwrap_or(0)
    } else {
        0
    };

    // Parse CLI breakpoints
    let initial_breakpoints: Vec<debugger::Breakpoint> = args
        .breakpoints
        .iter()
        .map(|bp_str| debugger::Breakpoint::parse(bp_str, &debug_root_dir))
        .collect::<eyre::Result<Vec<_>>>()
        .wrap_err("parsing --breakpoint arguments")?;

    Ok(BootstrapResult {
        configs,
        config_names,
        selected_config_index,
        debug_root_dir,
        state_manager,
        initial_breakpoints,
    })
}
