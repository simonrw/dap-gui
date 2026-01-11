use eframe::egui;
use egui_extras::syntax_highlighting::{self, CodeTheme};
use tree_sitter::Tree;

mod ast;
use ast::SelectedNode;

mod async_bridge;
use async_bridge::{AsyncBridge, StateUpdate, UiCommand};

#[derive(PartialEq, Clone, Copy, Default)]
enum EditorMode {
    #[default]
    Normal,
    NodeSelect,
}

#[derive(PartialEq, Clone, Copy)]
enum BottomPanelTab {
    Variables,
    Breakpoints,
    Console,
}

struct UiState {
    // Debugger state
    is_running: bool,
    current_file: String,
    current_line: usize,

    // Call stack
    stack_frames: Vec<StackFrame>,
    selected_frame: usize,
    current_frame_id: Option<i64>,

    // Variables
    variables: Vec<Variable>,

    // Breakpoints
    breakpoints: Vec<Breakpoint>,

    // Console output
    console_output: Vec<String>,

    // UI state
    selected_tab: BottomPanelTab,

    // Editor mode state
    editor_mode: EditorMode,
    selected_node: Option<SelectedNode>,
    parsed_tree: Option<Tree>,
    source_code: String,
    last_evaluation: Option<String>,
}

#[derive(Clone)]
struct StackFrame {
    name: String,
    file: String,
    line: usize,
}

#[derive(Clone)]
struct Variable {
    name: String,
    value: String,
    var_type: String,
}

#[derive(Clone)]
struct Breakpoint {
    file: String,
    line: usize,
    enabled: bool,
}

// The mock Python source code (lines start at 40 in the display)
const MOCK_SOURCE: &str = r#"def main():
    print('Hello, debugger!')
    x = 42
    name = 'Alice'
    items = [1, 2, 3, 4, 5]
    result = process_data(items)  # <- Current line
    print(f'Result: {result}')
    return result

def process_data(data):
    total = 0
    for item in data:
        total += item
    return total"#;

impl Default for UiState {
    fn default() -> Self {
        // Parse the source code with tree-sitter
        let mut parser = ast::create_parser();
        let parsed_tree = parser.parse(MOCK_SOURCE, None);

        Self {
            is_running: false,
            current_file: "example.py".to_string(),
            current_line: 42,
            stack_frames: vec![
                StackFrame {
                    name: "main".to_string(),
                    file: "example.py".to_string(),
                    line: 42,
                },
                StackFrame {
                    name: "process_data".to_string(),
                    file: "example.py".to_string(),
                    line: 28,
                },
                StackFrame {
                    name: "calculate".to_string(),
                    file: "utils.py".to_string(),
                    line: 15,
                },
            ],
            selected_frame: 0,
            current_frame_id: None,
            variables: vec![
                Variable {
                    name: "x".to_string(),
                    value: "42".to_string(),
                    var_type: "int".to_string(),
                },
                Variable {
                    name: "name".to_string(),
                    value: "\"Alice\"".to_string(),
                    var_type: "str".to_string(),
                },
                Variable {
                    name: "items".to_string(),
                    value: "[1, 2, 3, 4, 5]".to_string(),
                    var_type: "list".to_string(),
                },
            ],
            breakpoints: vec![
                Breakpoint {
                    file: "example.py".to_string(),
                    line: 42,
                    enabled: true,
                },
                Breakpoint {
                    file: "example.py".to_string(),
                    line: 28,
                    enabled: true,
                },
                Breakpoint {
                    file: "utils.py".to_string(),
                    line: 15,
                    enabled: false,
                },
            ],
            console_output: vec![
                "Debugger started".to_string(),
                "Breakpoint hit at example.py:42".to_string(),
                "Paused in main()".to_string(),
            ],
            selected_tab: BottomPanelTab::Console,
            editor_mode: EditorMode::Normal,
            selected_node: None,
            parsed_tree,
            source_code: MOCK_SOURCE.to_string(),
            last_evaluation: None,
        }
    }
}

struct App {
    ui_state: UiState,
    bridge: Option<AsyncBridge>,
}

impl App {
    fn new(_cc: &eframe::CreationContext) -> Self {
        Self {
            ui_state: UiState::default(),
            bridge: None,
        }
    }

    fn process_updates(&mut self) {
        if let Some(bridge) = &mut self.bridge {
            for update in bridge.poll_updates() {
                match update {
                    StateUpdate::DebuggerEvent(event) => {
                        self.handle_debugger_event(event);
                    }
                    StateUpdate::EvaluateResult(result) => {
                        self.ui_state.last_evaluation = Some(if result.error {
                            format!("Error: {}", result.output)
                        } else {
                            result.output
                        });
                        self.ui_state.console_output.push(format!(
                            "Evaluated: {}",
                            self.ui_state.last_evaluation.as_ref().unwrap()
                        ));
                    }
                    StateUpdate::VariablesResult(vars) => {
                        self.ui_state.variables = vars
                            .iter()
                            .map(|v| Variable {
                                name: v.name.clone(),
                                value: v.value.clone(),
                                var_type: v.r#type.clone().unwrap_or_default(),
                            })
                            .collect();
                    }
                    StateUpdate::Error(msg) => {
                        self.ui_state.console_output.push(format!("Error: {}", msg));
                    }
                }
            }
        }
    }

    fn handle_debugger_event(&mut self, event: debugger::Event) {
        use debugger::Event;
        match event {
            Event::Paused(state) => {
                self.ui_state.is_running = false;
                self.ui_state.console_output.push("Paused".to_string());

                // Update stack frames
                self.ui_state.stack_frames = state
                    .stack
                    .iter()
                    .map(|f| StackFrame {
                        name: f.name.clone(),
                        file: f
                            .source
                            .as_ref()
                            .and_then(|s: &transport::types::Source| s.path.as_ref())
                            .and_then(|p: &std::path::PathBuf| p.to_str())
                            .unwrap_or("unknown")
                            .to_string(),
                        line: f.line,
                    })
                    .collect();

                // Update current frame
                let frame = &state.paused_frame.frame;
                self.ui_state.current_frame_id = Some(frame.id);
                if let Some(source) = &frame.source
                    && let Some(path) = &source.path
                {
                    self.ui_state.current_file = path.to_str().unwrap_or("unknown").to_string();
                }
                self.ui_state.current_line = frame.line;

                // Update variables
                self.ui_state.variables = state
                    .paused_frame
                    .variables
                    .iter()
                    .map(|v| Variable {
                        name: v.name.clone(),
                        value: v.value.clone(),
                        var_type: v.r#type.clone().unwrap_or_default(),
                    })
                    .collect();

                // Update breakpoints
                self.ui_state.breakpoints = state
                    .breakpoints
                    .iter()
                    .map(|bp| Breakpoint {
                        file: bp.path.to_str().unwrap_or("unknown").to_string(),
                        line: bp.line,
                        enabled: true,
                    })
                    .collect();
            }
            Event::Running => {
                self.ui_state.is_running = true;
                self.ui_state.console_output.push("Running...".to_string());
            }
            Event::Ended => {
                self.ui_state
                    .console_output
                    .push("Debugging session ended".to_string());
                self.bridge = None;
            }
            Event::Initialised => {
                self.ui_state
                    .console_output
                    .push("Debugger initialized".to_string());
            }
            Event::ScopeChange(state) => {
                // Update variables for new scope
                self.ui_state.variables = state
                    .paused_frame
                    .variables
                    .iter()
                    .map(|v| Variable {
                        name: v.name.clone(),
                        value: v.value.clone(),
                        var_type: v.r#type.clone().unwrap_or_default(),
                    })
                    .collect();
            }
            Event::Uninitialised => {}
        }
    }

    fn handle_keyboard_input(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            // Enter node selection mode with 'v' (like vim visual)
            if i.key_pressed(egui::Key::V) && self.ui_state.editor_mode == EditorMode::Normal {
                self.ui_state.editor_mode = EditorMode::NodeSelect;
                self.select_initial_node();
            }

            // Node selection mode keys
            if self.ui_state.editor_mode == EditorMode::NodeSelect {
                // Navigation (vim-style)
                if i.key_pressed(egui::Key::H) {
                    self.navigate_prev_sibling();
                }
                if i.key_pressed(egui::Key::L) {
                    self.navigate_next_sibling();
                }
                if i.key_pressed(egui::Key::K) {
                    self.navigate_to_parent();
                }
                if i.key_pressed(egui::Key::J) {
                    self.navigate_to_child();
                }

                // Evaluate with Enter or 'e'
                if i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::E) {
                    self.evaluate_selected_node();
                }

                // Exit mode with Escape
                if i.key_pressed(egui::Key::Escape) {
                    self.ui_state.editor_mode = EditorMode::Normal;
                    self.ui_state.selected_node = None;
                    self.ui_state.last_evaluation = None;
                }
            }

            // Global debugger shortcuts (always active)
            if i.key_pressed(egui::Key::F5) {
                if self.bridge.is_some() {
                    if self.ui_state.is_running {
                        // TODO: Implement pause
                    } else if let Some(bridge) = &self.bridge {
                        bridge.send_command(UiCommand::Continue);
                    }
                } else {
                    self.ui_state.is_running = !self.ui_state.is_running;
                    self.ui_state
                        .console_output
                        .push(if self.ui_state.is_running {
                            "Running...".to_string()
                        } else {
                            "Paused".to_string()
                        });
                }
            }
            if i.key_pressed(egui::Key::F10) && !self.ui_state.is_running {
                if let Some(bridge) = &self.bridge {
                    bridge.send_command(UiCommand::StepOver);
                } else {
                    self.ui_state.current_line += 1;
                    self.ui_state
                        .console_output
                        .push(format!("Stepped to line {}", self.ui_state.current_line));
                }
            }
            if i.key_pressed(egui::Key::F11)
                && !self.ui_state.is_running
                && let Some(bridge) = &self.bridge
            {
                if i.modifiers.shift {
                    bridge.send_command(UiCommand::StepOut);
                } else {
                    bridge.send_command(UiCommand::StepIn);
                }
            }
        });
    }

    fn select_initial_node(&mut self) {
        if let Some(ref tree) = self.ui_state.parsed_tree {
            // Convert display line (40-based) to 0-based tree-sitter line
            let tree_line = self.ui_state.current_line.saturating_sub(40);
            if let Some(node) =
                ast::find_first_evaluatable_on_line(tree, &self.ui_state.source_code, tree_line)
            {
                self.ui_state.selected_node = Some(node);
            }
        }
    }

    fn navigate_prev_sibling(&mut self) {
        if let (Some(tree), Some(current)) =
            (&self.ui_state.parsed_tree, &self.ui_state.selected_node)
        {
            if let Some(prev) = ast::get_prev_sibling(tree, &self.ui_state.source_code, current) {
                self.ui_state.console_output.push(format!(
                    "Prev: {} '{}' -> {} '{}'",
                    current.kind, current.text, prev.kind, prev.text
                ));
                self.ui_state.selected_node = Some(prev);
            } else {
                self.ui_state.console_output.push(format!(
                    "No prev sibling for {} '{}'",
                    current.kind, current.text
                ));
            }
        }
    }

    fn navigate_next_sibling(&mut self) {
        if let (Some(tree), Some(current)) =
            (&self.ui_state.parsed_tree, &self.ui_state.selected_node)
        {
            if let Some(next) = ast::get_next_sibling(tree, &self.ui_state.source_code, current) {
                self.ui_state.console_output.push(format!(
                    "Next: {} '{}' -> {} '{}'",
                    current.kind, current.text, next.kind, next.text
                ));
                self.ui_state.selected_node = Some(next);
            } else {
                self.ui_state.console_output.push(format!(
                    "No next sibling for {} '{}'",
                    current.kind, current.text
                ));
            }
        }
    }

    fn navigate_to_parent(&mut self) {
        if let (Some(tree), Some(current)) =
            (&self.ui_state.parsed_tree, &self.ui_state.selected_node)
        {
            if let Some(parent) = ast::get_parent_node(tree, &self.ui_state.source_code, current) {
                self.ui_state.console_output.push(format!(
                    "Parent: {} -> {} ({})",
                    current.kind, parent.kind, parent.text
                ));
                self.ui_state.selected_node = Some(parent);
            } else {
                self.ui_state.console_output.push(format!(
                    "No parent found for {} '{}'",
                    current.kind, current.text
                ));
            }
        }
    }

    fn navigate_to_child(&mut self) {
        if let (Some(tree), Some(current)) =
            (&self.ui_state.parsed_tree, &self.ui_state.selected_node)
        {
            if let Some(child) =
                ast::get_first_child_node(tree, &self.ui_state.source_code, current)
            {
                self.ui_state.console_output.push(format!(
                    "Child: {} -> {} ({})",
                    current.kind, child.kind, child.text
                ));
                self.ui_state.selected_node = Some(child);
            } else {
                self.ui_state.console_output.push(format!(
                    "No child found for {} '{}'",
                    current.kind, current.text
                ));
            }
        }
    }

    fn evaluate_selected_node(&mut self) {
        if let Some(ref node) = self.ui_state.selected_node {
            // Mock evaluation based on node type and known variables
            let result = match node.kind.as_str() {
                "identifier" => {
                    // Look up in our mock variables
                    let var_name = &node.text;
                    if let Some(var) = self.ui_state.variables.iter().find(|v| &v.name == var_name)
                    {
                        format!("{}: {} = {}", var.var_type, var.name, var.value)
                    } else {
                        format!("{} = <unknown>", var_name)
                    }
                }
                "call" => format!("{} → <function result>", node.text),
                "integer" => format!("int: {}", node.text),
                "string" => format!("str: {}", node.text),
                "list" => format!("list: {}", node.text),
                "assignment" | "expression_statement" => format!("Statement: {}", node.text),
                _ => format!("[{}] {}", node.kind, node.text),
            };

            self.ui_state.last_evaluation = Some(result.clone());
            self.ui_state
                .console_output
                .push(format!("Evaluated: {}", result));
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process async updates
        self.process_updates();

        // Request repaint if we have a bridge (to poll for updates)
        if self.bridge.is_some() {
            ctx.request_repaint();
        }

        // Handle keyboard input
        self.handle_keyboard_input(ctx);

        // Top panel - Control buttons
        egui::TopBottomPanel::top("control_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("DAP Debugger POC");
                ui.separator();

                // Mode indicator
                use egui::Color32;
                match self.ui_state.editor_mode {
                    EditorMode::Normal => {
                        ui.label(
                            egui::RichText::new("NORMAL")
                                .color(Color32::GRAY)
                                .monospace(),
                        );
                    }
                    EditorMode::NodeSelect => {
                        ui.label(
                            egui::RichText::new("NODE SELECT")
                                .color(Color32::YELLOW)
                                .strong()
                                .monospace(),
                        );
                        if let Some(ref node) = self.ui_state.selected_node {
                            ui.label(
                                egui::RichText::new(format!("[{}]", node.kind))
                                    .color(Color32::LIGHT_BLUE)
                                    .monospace(),
                            );
                        }
                    }
                }
                ui.separator();

                if self.ui_state.is_running {
                    if ui.button("⏸ Pause").clicked() {
                        // TODO: Implement pause command
                        self.ui_state.is_running = false;
                        self.ui_state.console_output.push("Paused".to_string());
                    }
                } else if ui.button("▶ Continue").clicked() {
                    if let Some(bridge) = &self.bridge {
                        bridge.send_command(UiCommand::Continue);
                    } else {
                        self.ui_state.is_running = true;
                        self.ui_state.console_output.push("Running...".to_string());
                    }
                }

                if ui.button("⏭ Step Over").clicked() {
                    if let Some(bridge) = &self.bridge {
                        bridge.send_command(UiCommand::StepOver);
                    } else {
                        self.ui_state.current_line += 1;
                        self.ui_state
                            .console_output
                            .push(format!("Stepped to line {}", self.ui_state.current_line));
                    }
                }

                if ui.button("⏬ Step Into").clicked() {
                    if let Some(bridge) = &self.bridge {
                        bridge.send_command(UiCommand::StepIn);
                    } else {
                        self.ui_state
                            .console_output
                            .push("Stepped into function".to_string());
                    }
                }

                if ui.button("⏫ Step Out").clicked() {
                    if let Some(bridge) = &self.bridge {
                        bridge.send_command(UiCommand::StepOut);
                    } else {
                        self.ui_state
                            .console_output
                            .push("Stepped out of function".to_string());
                    }
                }

                ui.separator();

                if ui.button("⏹ Stop").clicked() {
                    if let Some(bridge) = &self.bridge {
                        bridge.send_command(UiCommand::Terminate);
                    }
                    self.ui_state
                        .console_output
                        .push("Debugger stopped".to_string());
                }

                if ui.button("🔄 Restart").clicked() {
                    self.ui_state = UiState::default();
                    self.ui_state
                        .console_output
                        .push("Debugger restarted".to_string());
                }
            });
        });

        // Left panel - Call stack
        egui::SidePanel::left("call_stack")
            .default_width(200.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Call Stack");
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for (i, frame) in self.ui_state.stack_frames.iter().enumerate() {
                        let is_selected = i == self.ui_state.selected_frame;
                        if ui.selectable_label(is_selected, &frame.name).clicked() {
                            self.ui_state.selected_frame = i;
                        }
                        ui.label(format!("  {}:{}", frame.file, frame.line));
                    }
                });
            });

        // Right panel - Breakpoints
        egui::SidePanel::right("breakpoints")
            .default_width(250.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.heading("Breakpoints");
                ui.separator();

                egui::ScrollArea::vertical().show(ui, |ui| {
                    for bp in &mut self.ui_state.breakpoints {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut bp.enabled, "");
                            ui.label(format!("{}:{}", bp.file, bp.line));
                        });
                    }

                    ui.separator();
                    if ui.button("+ Add Breakpoint").clicked() {
                        self.ui_state.breakpoints.push(Breakpoint {
                            file: self.ui_state.current_file.clone(),
                            line: self.ui_state.current_line,
                            enabled: true,
                        });
                    }
                });
            });

        // Bottom panel - Variables/Console tabs
        egui::TopBottomPanel::bottom("bottom_panel")
            .default_height(200.0)
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.ui_state.selected_tab,
                        BottomPanelTab::Variables,
                        "Variables",
                    );
                    ui.selectable_value(
                        &mut self.ui_state.selected_tab,
                        BottomPanelTab::Breakpoints,
                        "Breakpoints",
                    );
                    ui.selectable_value(
                        &mut self.ui_state.selected_tab,
                        BottomPanelTab::Console,
                        "Console",
                    );
                });

                ui.separator();

                match self.ui_state.selected_tab {
                    BottomPanelTab::Variables => {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            egui::Grid::new("variables_grid")
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.label("Name");
                                    ui.label("Value");
                                    ui.label("Type");
                                    ui.end_row();

                                    for var in &self.ui_state.variables {
                                        ui.label(&var.name);
                                        ui.label(&var.value);
                                        ui.label(&var.var_type);
                                        ui.end_row();
                                    }
                                });
                        });
                    }
                    BottomPanelTab::Breakpoints => {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            for bp in &self.ui_state.breakpoints {
                                ui.label(format!(
                                    "{} at {}:{}",
                                    if bp.enabled { "✓" } else { "✗" },
                                    bp.file,
                                    bp.line
                                ));
                            }
                        });
                    }
                    BottomPanelTab::Console => {
                        egui::ScrollArea::vertical()
                            .auto_shrink([false, false])
                            .stick_to_bottom(true)
                            .show(ui, |ui| {
                                for msg in &self.ui_state.console_output {
                                    ui.label(msg);
                                }
                            });
                    }
                }
            });

        // Evaluation result popup
        if let Some(ref result) = self.ui_state.last_evaluation {
            use egui::Color32;
            egui::Window::new("Evaluation")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::RIGHT_TOP, [-10.0, 50.0])
                .show(ctx, |ui| {
                    if let Some(ref node) = self.ui_state.selected_node {
                        ui.label(
                            egui::RichText::new(&node.text)
                                .monospace()
                                .color(Color32::LIGHT_BLUE),
                        );
                    }
                    ui.label(egui::RichText::new(result).monospace());
                });
        }

        // Central panel - Code view
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(&self.ui_state.current_file);

            // Keyboard shortcuts help
            if self.ui_state.editor_mode == EditorMode::NodeSelect {
                ui.horizontal(|ui| {
                    use egui::Color32;
                    ui.label(
                        egui::RichText::new(
                            "h/l: sibling  j/k: child/parent  e: evaluate  Esc: exit",
                        )
                        .small()
                        .color(Color32::GRAY),
                    );
                });
            } else {
                ui.horizontal(|ui| {
                    use egui::Color32;
                    ui.label(
                        egui::RichText::new("v: enter node select mode  F5: run/pause  F10: step")
                            .small()
                            .color(Color32::GRAY),
                    );
                });
            }
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                use egui::Color32;

                // Get syntax highlighting theme
                let theme = CodeTheme::from_memory(ctx, &ctx.style());

                // Use the source code from state
                let code_lines: Vec<&str> = self.ui_state.source_code.lines().collect();

                for (i, line) in code_lines.iter().enumerate() {
                    let line_num = i + 40; // Start at line 40 (display offset)
                    let tree_line = i; // 0-based line for tree-sitter

                    ui.horizontal(|ui| {
                        // Line number
                        ui.label(
                            egui::RichText::new(format!("{:4}", line_num))
                                .color(Color32::GRAY)
                                .monospace(),
                        );

                        // Breakpoint indicator
                        if self
                            .ui_state
                            .breakpoints
                            .iter()
                            .any(|bp| bp.line == line_num)
                        {
                            ui.label(egui::RichText::new("🔴").color(Color32::RED));
                        } else {
                            ui.label("  ");
                        }

                        // Current line marker
                        if line_num == self.ui_state.current_line {
                            ui.label(egui::RichText::new("→").color(Color32::YELLOW));
                        } else {
                            ui.label(" ");
                        }

                        // Check if this line contains the selected node
                        let line_has_selection =
                            self.ui_state.selected_node.as_ref().is_some_and(|node| {
                                tree_line >= node.start_line && tree_line <= node.end_line
                            });

                        if line_has_selection {
                            // Render line with selection highlight
                            self.render_line_with_selection(ui, ctx, &theme, line, tree_line);
                        } else {
                            // Normal syntax highlighted code
                            let mut layout_job = syntax_highlighting::highlight(
                                ctx,
                                &ctx.style(),
                                &theme,
                                line,
                                "py",
                            );

                            // Apply background highlight for current execution line
                            if line_num == self.ui_state.current_line {
                                let bg_color = Color32::from_rgb(50, 50, 0);
                                for section in &mut layout_job.sections {
                                    section.format.background = bg_color;
                                }
                            }

                            ui.label(layout_job);
                        }
                    });
                }
            });
        });
    }
}

impl App {
    fn render_line_with_selection(
        &self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        theme: &CodeTheme,
        line: &str,
        tree_line: usize,
    ) {
        use egui::Color32;

        let Some(ref node) = self.ui_state.selected_node else {
            // No selection, just render normally
            let layout_job = syntax_highlighting::highlight(ctx, &ctx.style(), theme, line, "py");
            ui.label(layout_job);
            return;
        };

        // For single-line selections, highlight the specific columns
        if node.start_line == node.end_line && node.start_line == tree_line {
            let start_col = node.start_col;
            let end_col = node.end_col;

            // Split line into before, selected, after
            let before = &line[..start_col.min(line.len())];
            let selected = &line[start_col.min(line.len())..end_col.min(line.len())];
            let after = &line[end_col.min(line.len())..];

            // Render before (normal highlighting)
            if !before.is_empty() {
                let layout_job =
                    syntax_highlighting::highlight(ctx, &ctx.style(), theme, before, "py");
                ui.label(layout_job);
            }

            // Render selected with highlight background
            if !selected.is_empty() {
                let mut layout_job =
                    syntax_highlighting::highlight(ctx, &ctx.style(), theme, selected, "py");
                let selection_color = Color32::from_rgba_unmultiplied(0, 150, 255, 100);
                for section in &mut layout_job.sections {
                    section.format.background = selection_color;
                }
                ui.label(layout_job);
            }

            // Render after (normal highlighting)
            if !after.is_empty() {
                let layout_job =
                    syntax_highlighting::highlight(ctx, &ctx.style(), theme, after, "py");
                ui.label(layout_job);
            }
        } else {
            // Multi-line selection: highlight the entire line
            let mut layout_job =
                syntax_highlighting::highlight(ctx, &ctx.style(), theme, line, "py");
            let selection_color = Color32::from_rgba_unmultiplied(0, 150, 255, 80);
            for section in &mut layout_job.sections {
                section.format.background = selection_color;
            }
            ui.label(layout_job);
        }
    }
}

fn main() {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_title("DAP Debugger - Proof of Concept"),
        ..Default::default()
    };

    eframe::run_native(
        "DAP Debugger POC",
        native_options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
    .unwrap();
}
