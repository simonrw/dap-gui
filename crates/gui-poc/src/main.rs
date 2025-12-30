use eframe::egui;

#[derive(PartialEq, Clone, Copy)]
enum BottomPanelTab {
    Variables,
    Breakpoints,
    Console,
}

struct MockState {
    // Debugger state
    is_running: bool,
    current_file: String,
    current_line: usize,

    // Call stack
    stack_frames: Vec<StackFrame>,
    selected_frame: usize,

    // Variables
    variables: Vec<Variable>,

    // Breakpoints
    breakpoints: Vec<Breakpoint>,

    // Console output
    console_output: Vec<String>,

    // UI state
    selected_tab: BottomPanelTab,
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

impl Default for MockState {
    fn default() -> Self {
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
            selected_tab: BottomPanelTab::Variables,
        }
    }
}

struct App {
    state: MockState,
}

impl App {
    fn new(_cc: &eframe::CreationContext) -> Self {
        Self {
            state: MockState::default(),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Top panel - Control buttons
        egui::TopBottomPanel::top("control_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("DAP Debugger POC");
                ui.separator();

                if self.state.is_running {
                    if ui.button("⏸ Pause").clicked() {
                        self.state.is_running = false;
                        self.state.console_output.push("Paused".to_string());
                    }
                } else {
                    if ui.button("▶ Continue").clicked() {
                        self.state.is_running = true;
                        self.state.console_output.push("Running...".to_string());
                    }
                }

                if ui.button("⏭ Step Over").clicked() {
                    self.state.current_line += 1;
                    self.state
                        .console_output
                        .push(format!("Stepped to line {}", self.state.current_line));
                }

                if ui.button("⏬ Step Into").clicked() {
                    self.state
                        .console_output
                        .push("Stepped into function".to_string());
                }

                if ui.button("⏫ Step Out").clicked() {
                    self.state
                        .console_output
                        .push("Stepped out of function".to_string());
                }

                ui.separator();

                if ui.button("⏹ Stop").clicked() {
                    self.state
                        .console_output
                        .push("Debugger stopped".to_string());
                }

                if ui.button("🔄 Restart").clicked() {
                    self.state = MockState::default();
                    self.state
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
                    for (i, frame) in self.state.stack_frames.iter().enumerate() {
                        let is_selected = i == self.state.selected_frame;
                        if ui.selectable_label(is_selected, &frame.name).clicked() {
                            self.state.selected_frame = i;
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
                    for bp in &mut self.state.breakpoints {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut bp.enabled, "");
                            ui.label(format!("{}:{}", bp.file, bp.line));
                        });
                    }

                    ui.separator();
                    if ui.button("+ Add Breakpoint").clicked() {
                        self.state.breakpoints.push(Breakpoint {
                            file: self.state.current_file.clone(),
                            line: self.state.current_line,
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
                        &mut self.state.selected_tab,
                        BottomPanelTab::Variables,
                        "Variables",
                    );
                    ui.selectable_value(
                        &mut self.state.selected_tab,
                        BottomPanelTab::Breakpoints,
                        "Breakpoints",
                    );
                    ui.selectable_value(
                        &mut self.state.selected_tab,
                        BottomPanelTab::Console,
                        "Console",
                    );
                });

                ui.separator();

                match self.state.selected_tab {
                    BottomPanelTab::Variables => {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            egui::Grid::new("variables_grid")
                                .striped(true)
                                .show(ui, |ui| {
                                    ui.label("Name");
                                    ui.label("Value");
                                    ui.label("Type");
                                    ui.end_row();

                                    for var in &self.state.variables {
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
                            for bp in &self.state.breakpoints {
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
                                for msg in &self.state.console_output {
                                    ui.label(msg);
                                }
                            });
                    }
                }
            });

        // Central panel - Code view
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading(&self.state.current_file);
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                // Mock code display
                let code_lines = vec![
                    "def main():",
                    "    print('Hello, debugger!')",
                    "    x = 42",
                    "    name = 'Alice'",
                    "    items = [1, 2, 3, 4, 5]",
                    "    result = process_data(items)  # <- Current line",
                    "    print(f'Result: {result}')",
                    "    return result",
                    "",
                    "def process_data(data):",
                    "    total = 0",
                    "    for item in data:",
                    "        total += item",
                    "    return total",
                ];

                use egui::Color32;

                for (i, line) in code_lines.iter().enumerate() {
                    let line_num = i + 40; // Start at line 40

                    ui.horizontal(|ui| {
                        // Line number
                        ui.label(
                            egui::RichText::new(format!("{:4}", line_num))
                                .color(Color32::GRAY)
                                .monospace(),
                        );

                        // Breakpoint indicator
                        if self.state.breakpoints.iter().any(|bp| bp.line == line_num) {
                            ui.label(egui::RichText::new("🔴").color(Color32::RED));
                        } else {
                            ui.label("  ");
                        }

                        // Current line highlight
                        if line_num == self.state.current_line {
                            ui.label(egui::RichText::new("→").color(Color32::YELLOW));
                            ui.label(
                                egui::RichText::new(*line)
                                    .monospace()
                                    .background_color(Color32::from_rgb(50, 50, 0)),
                            );
                        } else {
                            ui.label(egui::RichText::new(*line).monospace());
                        }
                    });
                }
            });
        });
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
