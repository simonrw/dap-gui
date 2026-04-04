use std::collections::HashSet;
use std::path::Path;

use debugger::Breakpoint;
use state::StateManager;

/// Persist the given breakpoints to the state file for the specified project directory.
pub fn persist_breakpoints(
    state_manager: &mut StateManager,
    debug_root_dir: &Path,
    ui_breakpoints: &HashSet<Breakpoint>,
) {
    let breakpoints: Vec<_> = ui_breakpoints.iter().cloned().collect();
    if let Err(e) = state_manager.set_project_breakpoints(debug_root_dir.to_owned(), breakpoints) {
        tracing::warn!(error = %e, "failed to persist breakpoints");
    }
}

/// Collect breakpoints from persisted state for the given project directory.
///
/// Normalises and canonicalises paths so they match the debugger's expectations.
pub fn collect_persisted_breakpoints(
    state_manager: &StateManager,
    debug_root_dir: &Path,
) -> Vec<Breakpoint> {
    let mut bps = Vec::new();
    if let Some(project_state) = state_manager
        .current()
        .projects
        .iter()
        .find(|p| debugger::utils::normalise_path(&p.path) == debug_root_dir)
    {
        tracing::debug!("got project state");
        for breakpoint in &project_state.breakpoints {
            let mut bp = breakpoint.clone();
            let normalised = debugger::utils::normalise_path(&bp.path).into_owned();
            bp.path = std::fs::canonicalize(&normalised).unwrap_or(normalised);
            if !bps.contains(&bp) {
                bps.push(bp);
            }
        }
    } else {
        tracing::debug!("no project state found for persisted breakpoints");
    }
    bps
}

/// Collect all breakpoints (UI + persisted) for session start.
///
/// Merges the current UI breakpoints with any persisted breakpoints,
/// avoiding duplicates.
pub fn collect_all_breakpoints(
    state_manager: &StateManager,
    debug_root_dir: &Path,
    ui_breakpoints: &HashSet<Breakpoint>,
) -> Vec<Breakpoint> {
    let mut bps: Vec<Breakpoint> = ui_breakpoints.iter().cloned().collect();
    let persisted = collect_persisted_breakpoints(state_manager, debug_root_dir);
    for bp in persisted {
        if !bps.contains(&bp) {
            bps.push(bp);
        }
    }
    bps
}
