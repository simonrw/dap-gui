# dap-iced POC Design

This document outlines the design for a proof-of-concept debugger GUI using iced 0.14.

## Overview

The `dap-iced` crate implements a debugger GUI using iced's Elm-style architecture. It integrates with the async `debugger` crate via iced's `Task` and `Subscription` primitives for native async support.

## Scope

### In Scope (POC)
- Source code view with line numbers
- Current execution line highlighting
- Breakpoint display and toggle (click on gutter)
- Control panel (Continue, Step Over/In/Out, Stop)
- Real source file loading from disk
- Debugger event subscription

### Out of Scope (for now)
- AST node selection
- Syntax highlighting
- Full call stack interaction
- Variable inspection
- Console/output panel (placeholder only)

## Architecture

### Iced 0.14 Elm-Style Pattern

```
State -> view() -> Element<Message>
          |
          v
      User Interaction
          |
          v
Message -> update() -> (State, Task<Message>)
```

Key components:
- **Task**: For async operations like `debugger.step_over()`, `debugger.evaluate()`
- **Subscription**: For continuous event streams from the debugger
- **Message**: Enum connecting UI events to state changes

### Integration with AsyncDebugger

The `AsyncDebugger` API is already async, making it a natural fit for iced's `Task`:

```rust
// Example: Step Over button clicked
Message::StepOver => {
    let debugger = self.debugger.clone();
    Task::perform(
        async move { debugger.step_over().await },
        |result| Message::CommandResult(result.map_err(|e| e.to_string()))
    )
}
```

For continuous debugger events, use `Subscription::run` with a stream:

```rust
fn subscription(&self) -> Subscription<Message> {
    if self.debugger.is_some() {
        Subscription::run(debugger_event_stream)
    } else {
        Subscription::none()
    }
}
```

## Crate Structure

```
crates/dap-iced/
  Cargo.toml
  ICED_GUI_POC_DESIGN.md   # This file
  src/
    main.rs                 # Entry point, App struct, update/view/subscription
    message.rs              # Message enum
    state.rs                # UI state definitions
    debugger_bridge.rs      # Subscription for debugger events, command helpers
    widgets/
      mod.rs
      control_bar.rs        # Debug control buttons
      source_view.rs        # Code display with line numbers, breakpoints
      call_stack.rs         # Placeholder
      variables.rs          # Placeholder
      console.rs            # Placeholder
```

## Dependencies (Cargo.toml)

```toml
[package]
name = "dap-iced"
version = "0.1.0"
edition = "2024"

[dependencies]
iced = { version = "0.14", features = ["tokio"] }
debugger = { path = "../debugger" }
transport = { path = "../transport" }
server = { path = "../server" }
launch_configuration = { path = "../launch_configuration" }
tokio = { version = "1.48", features = ["sync", "rt-multi-thread", "fs"] }
eyre = "0.6"
tracing = "0.1"
tracing-subscriber = "0.3"
clap = { version = "4", features = ["derive"] }
```

**Note**: This crate is NOT added to the workspace `members` to avoid dependency conflicts during development. It will be added later once stable.

## Core Types

### Message Enum (`message.rs`)

```rust
#[derive(Debug, Clone)]
pub enum Message {
    // Debugger commands
    Continue,
    StepOver,
    StepIn,
    StepOut,
    Stop,
    
    // Breakpoint management
    ToggleBreakpoint(usize),  // line number (1-indexed)
    
    // Debugger lifecycle
    DebuggerConnected,
    DebuggerEvent(debugger::Event),
    CommandResult(Result<(), String>),
    
    // Source file loading
    SourceLoaded(Result<String, String>),
    LoadSource(PathBuf),
}
```

### App State (`state.rs`)

```rust
use std::collections::HashSet;
use std::path::PathBuf;

pub struct AppState {
    // Debugger connection
    pub connected: bool,
    pub is_running: bool,
    
    // Source view
    pub current_file: Option<PathBuf>,
    pub source_content: String,
    pub current_line: Option<usize>,
    
    // Breakpoints (line numbers, 1-indexed)
    pub breakpoints: HashSet<usize>,
    
    // Placeholders for future panels
    pub stack_frames: Vec<StackFrame>,
    pub variables: Vec<Variable>,
    pub console_output: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct StackFrame {
    pub name: String,
    pub file: String,
    pub line: usize,
}

#[derive(Clone, Debug)]
pub struct Variable {
    pub name: String,
    pub value: String,
    pub var_type: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            connected: false,
            is_running: false,
            current_file: None,
            source_content: String::new(),
            current_line: None,
            breakpoints: HashSet::new(),
            stack_frames: Vec::new(),
            variables: Vec::new(),
            console_output: Vec::new(),
        }
    }
}
```

## Debugger Integration (`debugger_bridge.rs`)

### Event Subscription

Uses iced's `Subscription::run` to create a stream from debugger events:

```rust
use iced::Subscription;
use iced::futures::stream;
use tokio::sync::mpsc;
use crate::message::Message;

/// Creates a subscription that listens to debugger events
pub fn debugger_events(
    mut event_rx: mpsc::UnboundedReceiver<debugger::Event>
) -> impl iced::futures::Stream<Item = Message> {
    stream::channel(100, |mut output| async move {
        loop {
            match event_rx.recv().await {
                Some(event) => {
                    let _ = output.send(Message::DebuggerEvent(event)).await;
                }
                None => break,
            }
        }
    })
}
```

### Command Execution

For sending commands, use `Task::perform`:

```rust
use iced::Task;
use std::sync::Arc;
use debugger::TcpAsyncDebugger;

pub enum DebuggerCommand {
    Continue,
    StepOver,
    StepIn,
    StepOut,
    Stop,
}

pub fn send_command(
    debugger: Arc<TcpAsyncDebugger>, 
    cmd: DebuggerCommand
) -> Task<Message> {
    Task::perform(
        async move {
            match cmd {
                DebuggerCommand::Continue => debugger.continue_().await,
                DebuggerCommand::StepOver => debugger.step_over().await,
                DebuggerCommand::StepIn => debugger.step_in().await,
                DebuggerCommand::StepOut => debugger.step_out().await,
                DebuggerCommand::Stop => debugger.terminate().await,
            }.map_err(|e| e.to_string())
        },
        Message::CommandResult
    )
}
```

### Source File Loading

```rust
pub fn load_source_file(path: PathBuf) -> Task<Message> {
    Task::perform(
        async move {
            tokio::fs::read_to_string(&path)
                .await
                .map_err(|e| format!("Failed to load {}: {}", path.display(), e))
        },
        Message::SourceLoaded
    )
}
```

## Widgets

### Control Bar (`widgets/control_bar.rs`)

```rust
use iced::widget::{button, row, text};
use iced::Element;
use crate::message::Message;

pub fn control_bar<'a>(is_running: bool, connected: bool) -> Element<'a, Message> {
    let can_step = connected && !is_running;
    
    row![
        button(text("Continue"))
            .on_press_maybe(can_step.then_some(Message::Continue)),
        button(text("Step Over"))
            .on_press_maybe(can_step.then_some(Message::StepOver)),
        button(text("Step In"))
            .on_press_maybe(can_step.then_some(Message::StepIn)),
        button(text("Step Out"))
            .on_press_maybe(can_step.then_some(Message::StepOut)),
        button(text("Stop"))
            .on_press_maybe(connected.then_some(Message::Stop)),
    ]
    .spacing(10)
    .padding(10)
    .into()
}
```

### Source View (`widgets/source_view.rs`)

The main focus of the POC - displays source code with:
- Line numbers (monospace, right-aligned)
- Breakpoint indicators (red circle, clickable to toggle)
- Current execution line marker (yellow arrow)
- Current line background highlight

```rust
use iced::widget::{column, container, mouse_area, row, scrollable, text};
use iced::{Color, Element, Fill, Font};
use std::collections::HashSet;
use crate::message::Message;

pub fn source_view<'a>(
    content: &'a str,
    current_line: Option<usize>,
    breakpoints: &'a HashSet<usize>,
) -> Element<'a, Message> {
    let lines: Vec<Element<Message>> = content
        .lines()
        .enumerate()
        .map(|(idx, line_text)| {
            let line_num = idx + 1;  // 1-indexed
            let is_current = current_line == Some(line_num);
            let has_breakpoint = breakpoints.contains(&line_num);
            
            source_line(line_num, line_text, is_current, has_breakpoint)
        })
        .collect();
    
    scrollable(
        column(lines).spacing(0)
    )
    .height(Fill)
    .into()
}

fn source_line<'a>(
    line_num: usize,
    content: &'a str,
    is_current: bool,
    has_breakpoint: bool,
) -> Element<'a, Message> {
    let bp_indicator = if has_breakpoint { "●" } else { " " };
    let current_marker = if is_current { ">" } else { " " };
    
    // Line number gutter (clickable for breakpoint toggle)
    let gutter = mouse_area(
        row![
            text(format!("{:4} ", line_num))
                .font(Font::MONOSPACE)
                .color(Color::from_rgb(0.5, 0.5, 0.5)),
            text(bp_indicator)
                .font(Font::MONOSPACE)
                .color(Color::from_rgb(1.0, 0.2, 0.2)),
            text(format!("{} ", current_marker))
                .font(Font::MONOSPACE)
                .color(Color::from_rgb(1.0, 1.0, 0.0)),
        ]
    )
    .on_press(Message::ToggleBreakpoint(line_num));
    
    // Code content
    let code = text(content).font(Font::MONOSPACE);
    
    let line_row = row![gutter, code];
    
    // Apply background highlight for current line
    if is_current {
        container(line_row)
            .style(|_| container::Style {
                background: Some(Color::from_rgb(0.15, 0.15, 0.05).into()),
                ..Default::default()
            })
            .width(Fill)
            .into()
    } else {
        container(line_row)
            .width(Fill)
            .into()
    }
}
```

### Placeholder Widgets

#### Call Stack (`widgets/call_stack.rs`)
```rust
pub fn call_stack_panel<'a>(frames: &[StackFrame]) -> Element<'a, Message> {
    container(
        column![
            text("Call Stack").size(14),
            // TODO: List of stack frames
            text("(placeholder)").color(Color::from_rgb(0.5, 0.5, 0.5)),
        ]
    )
    .padding(10)
    .width(200)
    .into()
}
```

#### Variables (`widgets/variables.rs`)
```rust
pub fn variables_panel<'a>(variables: &[Variable]) -> Element<'a, Message> {
    container(
        column![
            text("Variables").size(14),
            // TODO: Variable tree/list
            text("(placeholder)").color(Color::from_rgb(0.5, 0.5, 0.5)),
        ]
    )
    .padding(10)
    .width(250)
    .into()
}
```

#### Console (`widgets/console.rs`)
```rust
pub fn console_panel<'a>(output: &[String]) -> Element<'a, Message> {
    container(
        column![
            text("Console").size(14),
            // TODO: Scrollable console output
            text("(placeholder)").color(Color::from_rgb(0.5, 0.5, 0.5)),
        ]
    )
    .padding(10)
    .height(150)
    .into()
}
```

## Main Application (`main.rs`)

```rust
use iced::{application, Element, Task, Subscription, Theme, Fill};
use iced::widget::{column, row, container};
use std::sync::Arc;
use std::path::PathBuf;
use tokio::sync::mpsc;
use clap::Parser;

mod message;
mod state;
mod debugger_bridge;
mod widgets;

use message::Message;
use state::AppState;
use debugger::TcpAsyncDebugger;

#[derive(Parser)]
struct Args {
    /// Path to launch configuration file
    config_path: Option<PathBuf>,
    
    /// Name of the configuration to use
    #[clap(short, long)]
    name: Option<String>,
}

struct App {
    state: AppState,
    debugger: Option<Arc<TcpAsyncDebugger>>,
}

impl App {
    fn new(args: Args) -> (Self, Task<Message>) {
        let app = Self {
            state: AppState::default(),
            debugger: None,
        };
        
        // TODO: Parse args and initiate debugger connection
        let task = Task::none();
        
        (app, task)
    }
    
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Continue => {
                self.send_debugger_command(debugger_bridge::DebuggerCommand::Continue)
            }
            Message::StepOver => {
                self.send_debugger_command(debugger_bridge::DebuggerCommand::StepOver)
            }
            Message::StepIn => {
                self.send_debugger_command(debugger_bridge::DebuggerCommand::StepIn)
            }
            Message::StepOut => {
                self.send_debugger_command(debugger_bridge::DebuggerCommand::StepOut)
            }
            Message::Stop => {
                self.send_debugger_command(debugger_bridge::DebuggerCommand::Stop)
            }
            
            Message::ToggleBreakpoint(line) => {
                if self.state.breakpoints.contains(&line) {
                    self.state.breakpoints.remove(&line);
                } else {
                    self.state.breakpoints.insert(line);
                }
                // TODO: Send breakpoint update to debugger
                Task::none()
            }
            
            Message::DebuggerConnected => {
                self.state.connected = true;
                self.state.console_output.push("Debugger connected".into());
                Task::none()
            }
            
            Message::DebuggerEvent(event) => {
                self.handle_debugger_event(event)
            }
            
            Message::CommandResult(result) => {
                if let Err(e) = result {
                    self.state.console_output.push(format!("Error: {}", e));
                }
                Task::none()
            }
            
            Message::LoadSource(path) => {
                self.state.current_file = Some(path.clone());
                debugger_bridge::load_source_file(path)
            }
            
            Message::SourceLoaded(result) => {
                match result {
                    Ok(content) => {
                        self.state.source_content = content;
                    }
                    Err(e) => {
                        self.state.console_output.push(format!("Error: {}", e));
                    }
                }
                Task::none()
            }
        }
    }
    
    fn send_debugger_command(&self, cmd: debugger_bridge::DebuggerCommand) -> Task<Message> {
        if let Some(debugger) = &self.debugger {
            debugger_bridge::send_command(debugger.clone(), cmd)
        } else {
            Task::none()
        }
    }
    
    fn handle_debugger_event(&mut self, event: debugger::Event) -> Task<Message> {
        use debugger::Event;
        
        match event {
            Event::Paused(program_state) => {
                self.state.is_running = false;
                self.state.console_output.push("Paused".into());
                
                // Update stack frames
                self.state.stack_frames = program_state.stack
                    .iter()
                    .map(|f| state::StackFrame {
                        name: f.name.clone(),
                        file: f.source.as_ref()
                            .and_then(|s| s.path.as_ref())
                            .map(|p| p.display().to_string())
                            .unwrap_or_default(),
                        line: f.line,
                    })
                    .collect();
                
                // Update current line and load source if needed
                let frame = &program_state.paused_frame.frame;
                self.state.current_line = Some(frame.line);
                
                if let Some(source) = &frame.source {
                    if let Some(path) = &source.path {
                        // Load source file if it's different
                        if self.state.current_file.as_ref() != Some(path) {
                            return Task::done(Message::LoadSource(path.clone()));
                        }
                    }
                }
                
                Task::none()
            }
            
            Event::Running => {
                self.state.is_running = true;
                self.state.current_line = None;
                self.state.console_output.push("Running...".into());
                Task::none()
            }
            
            Event::Ended => {
                self.state.connected = false;
                self.state.is_running = false;
                self.state.console_output.push("Debug session ended".into());
                Task::none()
            }
            
            Event::Initialised => {
                self.state.console_output.push("Debugger initialized".into());
                Task::none()
            }
            
            Event::ScopeChange(_) | Event::Uninitialised => {
                Task::none()
            }
        }
    }
    
    fn view(&self) -> Element<Message> {
        column![
            // Control bar at top
            widgets::control_bar::control_bar(
                self.state.is_running, 
                self.state.connected
            ),
            
            // Main content area
            row![
                // Left: Call stack (placeholder)
                widgets::call_stack::call_stack_panel(&self.state.stack_frames),
                
                // Center: Source view (main focus)
                container(
                    widgets::source_view::source_view(
                        &self.state.source_content,
                        self.state.current_line,
                        &self.state.breakpoints,
                    )
                ).width(Fill),
                
                // Right: Variables (placeholder)
                widgets::variables::variables_panel(&self.state.variables),
            ]
            .height(Fill),
            
            // Bottom: Console (placeholder)
            widgets::console::console_panel(&self.state.console_output),
        ]
        .into()
    }
    
    fn subscription(&self) -> Subscription<Message> {
        // TODO: Wire up debugger event subscription when connected
        Subscription::none()
    }
    
    fn theme(&self) -> Theme {
        Theme::Dark
    }
}

fn main() -> iced::Result {
    tracing_subscriber::fmt::init();
    
    let args = Args::parse();
    
    application("DAP Debugger", App::update, App::view)
        .subscription(App::subscription)
        .theme(App::theme)
        .run_with(move || App::new(args))
}
```

## Implementation Phases

### Phase 1: Basic Skeleton
- [x] Design document
- [x] Create crate with Cargo.toml (excluded from workspace)
- [x] Implement minimal App with hardcoded mock state
- [x] Basic layout: control bar, source view, placeholder panels
- [x] Source view with line numbers and mock content

### Phase 2: Source View Features
- [ ] Current line highlighting
- [x] Breakpoint display (red circles)
- [x] Breakpoint toggle on gutter click
- [x] Real source file loading from disk

### Phase 3: Debugger Integration  
- [ ] Add AsyncDebugger connection logic
- [ ] Implement Subscription for debugger events
- [ ] Wire up Continue, Step Over/In/Out, Stop buttons
- [ ] Handle Paused/Running/Ended events
- [ ] Load source file when paused frame changes

### Phase 4: Polish
- [ ] Keyboard shortcuts (F5, F10, F11)
- [ ] Error handling and display in console
- [ ] Proper scrolling behavior for source view
- [ ] Status bar showing connection state

## Future Enhancements (Post-POC)

- Syntax highlighting (using `iced_highlighter` or custom spans)
- Full call stack interaction (click to change scope)
- Variable inspection with expand/collapse
- Console output from debugger
- Breakpoint conditions
- Watch expressions
- AST node selection mode
- Multiple file tabs
