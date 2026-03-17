//! File-picker startup mode.
//!
//! When the user launches without `-b/--breakpoints`, the app starts here
//! showing only a file picker sidebar and a syntax-highlighted code viewer.

use std::{
    collections::{HashMap, HashSet},
    fs::create_dir_all,
    path::PathBuf,
};

use eframe::egui::{self, Context, Key, TextEdit};
use eyre::WrapErr;
use state::StateManager;

use crate::code_view::CodeView;

pub(crate) struct BrowseAppState {
    // File picker state
    pub(crate) file_picker_input: String,
    pub(crate) file_picker_cursor: usize,
    pub(crate) file_picker_results: Vec<fuzzy::FuzzyMatch>,
    pub(crate) git_files: Vec<fuzzy::TrackedFile>,
    pub(crate) git_files_loaded: bool,

    // Currently displayed file
    pub(crate) file_override: Option<PathBuf>,

    // File content cache
    pub(crate) file_cache: HashMap<PathBuf, String>,

    // Breakpoints (local only, no debugger)
    pub(crate) ui_breakpoints: HashSet<debugger::Breakpoint>,

    // Persistence
    pub(crate) state_manager: StateManager,
    pub(crate) debug_root_dir: PathBuf,
}

impl BrowseAppState {
    fn persist_breakpoints(&mut self) {
        let breakpoints: Vec<_> = self.ui_breakpoints.iter().cloned().collect();
        if let Err(e) = self
            .state_manager
            .set_project_breakpoints(self.debug_root_dir.clone(), breakpoints)
        {
            tracing::warn!(error = %e, "failed to persist breakpoints");
        }
    }
}

pub(crate) struct BrowseApp {
    state: BrowseAppState,
}

impl BrowseApp {
    pub(crate) fn new(
        debug_root_dir: PathBuf,
        cc: &eframe::CreationContext<'_>,
    ) -> eyre::Result<Self> {
        let _ = cc; // used for future customization

        let state_path = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("dapgui")
            .join("state.json");
        tracing::debug!(state_path = %state_path.display(), "loading state");
        if !state_path.parent().unwrap().is_dir() {
            create_dir_all(state_path.parent().unwrap()).context("creating state directory")?;
        }
        let state_manager = StateManager::new(state_path).wrap_err("loading state")?;
        state_manager.save().wrap_err("saving state")?;

        // Load persisted breakpoints for this project
        let mut ui_breakpoints = HashSet::new();
        if let Some(project_state) = state_manager
            .current()
            .projects
            .iter()
            .find(|p| debugger::utils::normalise_path(&p.path) == debug_root_dir)
        {
            for bp in &project_state.breakpoints {
                let bp_path = debugger::utils::normalise_path(&bp.path);
                if bp_path.starts_with(&debug_root_dir) {
                    let mut bp = bp.clone();
                    let normalised = debugger::utils::normalise_path(&bp.path).into_owned();
                    bp.path = std::fs::canonicalize(&normalised).unwrap_or(normalised);
                    ui_breakpoints.insert(bp);
                }
            }
        }

        Ok(Self {
            state: BrowseAppState {
                file_picker_input: String::new(),
                file_picker_cursor: 0,
                file_picker_results: Vec::new(),
                git_files: Vec::new(),
                git_files_loaded: false,
                file_override: None,
                file_cache: HashMap::new(),
                ui_breakpoints,
                state_manager,
                debug_root_dir,
            },
        })
    }
}

impl eframe::App for BrowseApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        let state = &mut self.state;

        // Lazy-load git files
        if !state.git_files_loaded {
            state.git_files_loaded = true;
            match fuzzy::list_git_files(&state.debug_root_dir) {
                Ok(files) => state.git_files = files,
                Err(e) => {
                    tracing::warn!(error = %e, "failed to list git files");
                    // Fall back to repo root detection
                    if let Some(root) = fuzzy::find_repo_root() {
                        if let Ok(files) = fuzzy::list_git_files(&root) {
                            state.git_files = files;
                        }
                    }
                }
            }
            state.file_picker_results =
                fuzzy::fuzzy_filter(&state.git_files, &state.file_picker_input);
        }

        // Handle keyboard: arrow keys and enter for file picker
        ctx.input(|i| {
            if i.key_pressed(Key::ArrowDown) && !state.file_picker_results.is_empty() {
                state.file_picker_cursor =
                    (state.file_picker_cursor + 1).min(state.file_picker_results.len() - 1);
            }
            if i.key_pressed(Key::ArrowUp) {
                state.file_picker_cursor = state.file_picker_cursor.saturating_sub(1);
            }
            if i.key_pressed(Key::Enter) && !state.file_picker_results.is_empty() {
                let selected = &state.file_picker_results[state.file_picker_cursor];
                state.file_override = Some(
                    std::fs::canonicalize(&selected.file.absolute_path)
                        .unwrap_or_else(|_| selected.file.absolute_path.clone()),
                );
            }
        });

        // Left sidebar: file picker
        egui::SidePanel::left("browse-file-picker")
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.heading("Files");
                ui.separator();

                // Search input
                let input_response = ui.add(
                    TextEdit::singleline(&mut state.file_picker_input)
                        .hint_text("Search files...")
                        .desired_width(f32::INFINITY),
                );

                input_response.request_focus();

                // Re-filter
                let prev_len = state.file_picker_results.len();
                state.file_picker_results =
                    fuzzy::fuzzy_filter(&state.git_files, &state.file_picker_input);

                if state.file_picker_results.is_empty() {
                    state.file_picker_cursor = 0;
                } else {
                    if state.file_picker_results.len() != prev_len {
                        state.file_picker_cursor = 0;
                    }
                    state.file_picker_cursor = state
                        .file_picker_cursor
                        .min(state.file_picker_results.len() - 1);
                }

                ui.separator();

                // Results list
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, m) in state.file_picker_results.iter().enumerate() {
                        let is_selected = i == state.file_picker_cursor;
                        let path_str = m.file.relative_path.to_string_lossy();

                        let mut job = eframe::epaint::text::LayoutJob::default();
                        let base_color = if is_selected {
                            egui::Color32::WHITE
                        } else {
                            ui.visuals().text_color()
                        };
                        let match_color = egui::Color32::from_rgb(255, 200, 50);

                        for (ci, ch) in path_str.char_indices() {
                            let color = if m.matched_indices.contains(&ci) {
                                match_color
                            } else {
                                base_color
                            };
                            let mut buf = [0u8; 4];
                            job.append(
                                ch.encode_utf8(&mut buf),
                                0.0,
                                egui::TextFormat {
                                    color,
                                    ..Default::default()
                                },
                            );
                        }

                        let bg = if is_selected {
                            ui.visuals().selection.bg_fill
                        } else {
                            egui::Color32::TRANSPARENT
                        };

                        let frame = egui::Frame::new().fill(bg).inner_margin(4.0);
                        let response = frame
                            .show(ui, |ui| {
                                ui.label(job);
                            })
                            .response;

                        if response.clicked() {
                            state.file_override = Some(
                                std::fs::canonicalize(&m.file.absolute_path)
                                    .unwrap_or_else(|_| m.file.absolute_path.clone()),
                            );
                            state.file_picker_cursor = i;
                        }
                    }
                });
            });

        // Central panel: code viewer
        egui::CentralPanel::default().show(ctx, |ui| {
            let Some(ref display_path) = state.file_override else {
                ui.centered_and_justified(|ui| {
                    ui.label("Select a file to view");
                });
                return;
            };
            let display_path = display_path.clone();

            // File breadcrumb
            let display_name = display_path
                .strip_prefix(&state.debug_root_dir)
                .unwrap_or(&display_path)
                .to_string_lossy()
                .to_string();
            ui.label(&display_name);
            ui.separator();

            // Read file contents with caching
            let contents = state
                .file_cache
                .entry(display_path.clone())
                .or_insert_with(|| {
                    std::fs::read_to_string(&display_path)
                        .unwrap_or_else(|e| format!("Error reading file: {e}"))
                })
                .clone();

            // Filter breakpoints for current file
            let mut file_breakpoints: HashSet<_> = state
                .ui_breakpoints
                .iter()
                .filter(|b| b.path == display_path.as_path())
                .cloned()
                .collect();

            let breakpoints_before = file_breakpoints.clone();
            let is_dark = ui.visuals().dark_mode;
            let jump = false;

            ui.add(CodeView::new(
                &contents,
                1,
                false,
                &mut file_breakpoints,
                &jump,
                display_path.clone(),
                is_dark,
                14.0,
            ));

            // Detect breakpoint changes from gutter clicks
            for added in file_breakpoints.difference(&breakpoints_before) {
                state.ui_breakpoints.insert(added.clone());
            }
            for removed in breakpoints_before.difference(&file_breakpoints) {
                state.ui_breakpoints.remove(removed);
            }

            if file_breakpoints != breakpoints_before {
                state.persist_breakpoints();
            }
        });
    }
}
