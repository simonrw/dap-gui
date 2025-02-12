use std::{path::PathBuf, sync::Mutex};

use clap::Parser;
use color_eyre::{eyre, eyre::Context};
use crossbeam_channel::Receiver;
use crossterm::event::{self, Event, KeyCode};
use debugger::{Breakpoint, Debugger, ProgramState};
use ratatui::{
    layout::{Constraint, Layout, Position},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph},
    DefaultTerminal, Frame,
};
use syntect::{
    easy::HighlightLines, highlighting::ThemeSet, parsing::SyntaxSet, util::LinesWithEndings,
};
use syntect_tui::into_span;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
struct Args {
    launch_configuration: PathBuf,

    #[clap(short, long)]
    name: String,

    #[clap(short, long)]
    breakpoints: Vec<Breakpoint>,
}

#[derive(Debug, Clone)]
enum DisplayMessage {
    Plain(String),
    Block(String),
}

struct App {
    debugger: Debugger,
    input: String,
    character_index: usize,
    events: Receiver<debugger::Event>,
    should_terminate: bool,
    messages: Vec<DisplayMessage>,
    program_description: Option<ProgramState>,

    // highlighting
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

impl App {
    fn new(debugger: Debugger) -> Self {
        let events = debugger.events();
        Self {
            debugger,
            input: String::new(),
            character_index: 0,
            events,
            should_terminate: false,
            messages: Vec::new(),
            program_description: None,
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        }
    }

    fn move_cursor_right(&mut self) {
        let cursor_moved_right = self.character_index.saturating_add(1);
        self.character_index = self.clamp_cursor(cursor_moved_right);
    }

    fn enter_char(&mut self, new_char: char) {
        let index = self.byte_index();
        self.input.insert(index, new_char);
        self.move_cursor_right();
    }

    fn byte_index(&self) -> usize {
        self.input
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.character_index)
            .unwrap_or(self.input.len())
    }

    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.input.chars().count())
    }

    fn run_command(&mut self) -> eyre::Result<()> {
        // TODO: execute debugger command
        let command: Vec<_> = self
            .input
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();
        eyre::ensure!(!command.is_empty(), "no command given");
        match command[0].as_str() {
            "c" => {
                tracing::debug!("executing continue command");
                self.add_message(DisplayMessage::Plain("Continuing execution".to_string()));
                self.debugger.r#continue().context("continuing execution")?;
            }
            "n" => {
                tracing::debug!("executing step_over command");
                self.add_message(DisplayMessage::Plain("Stepping over".to_string()));
                self.debugger.step_over().context("stepping over")?;
            }
            "i" => {
                tracing::debug!("executing step_in command");
                self.add_message(DisplayMessage::Plain("Stepping in".to_string()));
                self.debugger.step_in().context("stepping in")?;
            }
            "o" => {
                tracing::debug!("executing step_out command");
                self.add_message(DisplayMessage::Plain("Stepping out".to_string()));
                self.debugger.step_out().context("stepping out")?;
            }
            "q" => {
                tracing::debug!("executing quit command");
                self.should_terminate = true;
            }
            "w" => {
                let mut messages = Vec::new();
                if let Some(description) = &self.program_description {
                    for (i, frame) in description.stack.iter().enumerate() {
                        if i == 0 {
                            messages.push(format!(
                                "-> {}:{}",
                                frame
                                    .source
                                    .as_ref()
                                    .unwrap()
                                    .path
                                    .as_ref()
                                    .unwrap()
                                    .display(),
                                frame.line
                            ));
                        } else {
                            messages.push(format!(
                                "   {}:{}",
                                frame
                                    .source
                                    .as_ref()
                                    .unwrap()
                                    .path
                                    .as_ref()
                                    .unwrap()
                                    .display(),
                                frame.line
                            ));
                        }
                    }
                } else {
                    messages.push("???".to_string());
                }

                self.add_messages(messages.into_iter().map(DisplayMessage::Plain));
            }
            "l" => {
                let mut messages = Vec::new();
                if let Some(program_description) = &self.program_description {
                    let Some(source) = program_description.paused_frame.frame.source.as_ref()
                    else {
                        todo!()
                    };
                    eyre::ensure!(source.path.is_some());
                    let line = program_description.paused_frame.frame.line;

                    let contents = std::fs::read_to_string(source.path.as_ref().unwrap())
                        .context("reading file")?;
                    let line_text = contents.split('\n').nth(line - 1).unwrap();
                    let start = tree_sitter::Point {
                        row: line - 1,
                        column: 0,
                    };
                    let end = tree_sitter::Point {
                        row: line - 1,
                        column: line_text.len(),
                    };

                    // set up treesitter
                    let mut parser = tree_sitter::Parser::new();
                    parser
                        .set_language(&tree_sitter_python::LANGUAGE.into())
                        .context("setting parser language")?;
                    let tree = parser
                        .parse(contents.as_bytes(), None)
                        .ok_or(eyre::eyre!("error parsing file"))?;
                    let root = tree.root_node();
                    let descendant = root
                        .descendant_for_point_range(start, end)
                        .ok_or(eyre::eyre!("getting descendant"))?;

                    // find up until function body
                    let mut n = descendant;

                    loop {
                        tracing::debug!(node = ?n, "loop iteration");
                        if n.kind() == "function_definition" {
                            let s = n
                                .utf8_text(contents.as_bytes())
                                .context("extracting utf8 text from node")?;

                            let start = n.start_position();

                            // Get the cursor's line number (line - 1 since it's 0-based)
                            let cursor_line = line - 1;

                            // Convert node text to lines and find which line contains cursor
                            let node_start_line = start.row;
                            let node_lines = s.split('\n');

                            // Find the relative line number within the node's text
                            let relative_line = cursor_line - node_start_line;

                            for (i, line) in node_lines.enumerate() {
                                if i == relative_line {
                                    messages.push(format!("-> {}", line));
                                } else {
                                    messages.push(format!("   {}", line));
                                }
                            }

                            break;
                        }

                        let Some(parent) = n.parent() else {
                            eyre::bail!("no function body found");
                        };
                        n = parent;
                    }
                }

                self.add_message(DisplayMessage::Block(messages.join("\n")));
            }
            "p" => if let Some(ProgramState { .. }) = &self.program_description {},
            other => tracing::warn!(%other, "unhandled command"),
        }

        self.input.clear();
        self.reset_cursor();
        Ok(())
    }

    fn add_message(&mut self, message: impl Into<DisplayMessage>) {
        self.messages.push(message.into());
    }

    fn add_messages(&mut self, messages: impl IntoIterator<Item = DisplayMessage>) {
        self.messages.extend(messages);
    }

    fn delete_char(&mut self) {
        let is_not_cursor_leftmost = self.character_index != 0;
        if is_not_cursor_leftmost {
            // Method "remove" is not used on the saved text for deleting the selected char.
            // Reason: Using remove on String works on bytes instead of the chars.
            // Using remove would require special care because of char boundaries.

            let current_index = self.character_index;
            let from_left_to_current_index = current_index - 1;

            // Getting all characters before the selected character.
            let before_char_to_delete = self.input.chars().take(from_left_to_current_index);
            // Getting all characters after selected character.
            let after_char_to_delete = self.input.chars().skip(current_index);

            // Put all characters together except the selected one.
            // By leaving the selected one out, it is forgotten and therefore deleted.
            self.input = before_char_to_delete.chain(after_char_to_delete).collect();
            self.move_cursor_left();
        }
    }

    fn move_cursor_left(&mut self) {
        let cursor_moved_left = self.character_index.saturating_sub(1);
        self.character_index = self.clamp_cursor(cursor_moved_left);
    }

    fn reset_cursor(&mut self) {
        self.character_index = 0;
    }

    fn run(mut self, mut terminal: DefaultTerminal) -> eyre::Result<()> {
        // set up background thread polling for keyboard events
        let (tx, rx) = crossbeam_channel::unbounded();
        std::thread::spawn(move || loop {
            match event::read() {
                Ok(event) => {
                    let _ = tx.send(event);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "error reading from event stream");
                }
            }
        });

        loop {
            if self.should_terminate {
                tracing::info!("terminating application");
                return Ok(());
            }

            crossbeam_channel::select! {
                recv(self.events) -> msg => match msg {
                    Ok(event) => self
                        .handle_debugger_event(event)
                        .context("handling debugger event")?,
                    Err(e) => {
                        tracing::info!(error = %e, "error while reading event stream");
                    },
                },


                recv(rx) -> msg => {
                    #[allow(clippy::single_match)]
                    match msg? {
                        Event::Key(key) => match key.code {
                            KeyCode::Enter => {
                                if let Err(e) = self.run_command() {
                                    tracing::warn!(error = %e, "error running command");
                                }
                            }
                            KeyCode::Char(to_insert) => self.enter_char(to_insert),
                            KeyCode::Backspace => self.delete_char(),
                            KeyCode::Left => self.move_cursor_left(),
                            KeyCode::Right => self.move_cursor_right(),
                            KeyCode::Esc => break Ok(()),
                            _ => {}
                        },
                        _ => {},
                    }
                },
            }

            // debugger events
            // TODO: try to handle multiple events?
            // TODO: select over keyboard and debugger events to prevent blocking

            // rendering
            terminal.draw(|frame| self.draw(frame))?;
        }
    }

    #[tracing::instrument(skip(self))]
    fn handle_debugger_event(&mut self, event: debugger::Event) -> eyre::Result<()> {
        tracing::debug!("got debugger event");
        match event {
            debugger::Event::Paused(program_description) => {
                self.add_message(DisplayMessage::Plain(format!(
                    "program paused at {}:{}",
                    &program_description
                        .paused_frame
                        .frame
                        .source
                        .as_ref()
                        .unwrap()
                        .path
                        .as_ref()
                        .unwrap()
                        .display(),
                    program_description.paused_frame.frame.line
                )));
                self.program_description = Some(program_description);
            }
            debugger::Event::ScopeChange { .. } => {}
            debugger::Event::Running => {}
            debugger::Event::Ended => {
                self.add_message(DisplayMessage::Plain("program ended".to_string()));
                self.should_terminate = true;
            }
            _ => {}
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        // TODO: coloured bar at the bottom showing debugging state
        let vertical = Layout::vertical([Constraint::Length(3), Constraint::Min(10)]);
        let [input_area, messages_area] = vertical.areas(frame.area());

        // input box
        let input = Paragraph::new(self.input.as_str())
            .style(Style::default().fg(Color::Yellow))
            .block(Block::bordered().title("Input"));
        frame.render_widget(input, input_area);
        frame.set_cursor_position(Position::new(
            // Draw the cursor at the current position in the input field.
            // This position is can be controlled via the left and right arrow key
            input_area.x + self.character_index as u16 + 1,
            // Move one line down, from the border to the input line
            input_area.y + 1,
        ));

        // update messages
        let mut tui_lines = Vec::new();
        for message in &self.messages {
            match message {
                DisplayMessage::Plain(text) => tui_lines.push(Line::from(text.as_str())),
                DisplayMessage::Block(text) => {
                    let lines = syntax_highlight(&self.syntax_set, &self.theme_set, text)
                        .into_iter()
                        .map(|line| line.to_owned());
                    tui_lines.extend(lines);
                }
            }
        }
        let num_lines = tui_lines.len();
        let paragraph = Paragraph::new(tui_lines)
            .style(Style::default().fg(Color::White))
            .block(Block::bordered().title("Messages"))
            .scroll((
                num_lines.saturating_sub(messages_area.height as usize - 2) as u16,
                0,
            ));
        frame.render_widget(paragraph, messages_area);
    }
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let log_dir = dirs::data_dir()
        .ok_or_else(|| eyre::eyre!("Could not find data directory"))?
        .join("dap-gui");
    std::fs::create_dir_all(&log_dir)?;
    let log_file = std::fs::File::create(log_dir.join("debug.log"))?;
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(Mutex::new(log_file))
        .init();

    let args = Args::parse();
    let debugger = Debugger::from_launch_configuration(args.launch_configuration, args.name)
        .context("creating debugger")?;
    debugger.wait_for_event(|event| matches!(event, debugger::Event::Initialised));

    for breakpoint in args.breakpoints {
        tracing::debug!(?breakpoint, "adding breakpoint");
        debugger
            .add_breakpoint(&breakpoint)
            .context("adding breakpoint")?;
    }
    tracing::debug!("breakpoints added");
    debugger.start().context("starting debugger")?;
    tracing::debug!("debugger started");

    let terminal = ratatui::init();
    let app = App::new(debugger);
    let result = app.run(terminal);
    ratatui::restore();
    result
}

// TODO: use tree-sitter-highlight
// https://crates.io/crates/tree-sitter-highlight
fn syntax_highlight<'a>(ps: &'a SyntaxSet, ts: &'a ThemeSet, text: &'a str) -> Vec<Line<'a>> {
    // TODO: detect file language
    let syntax = ps.find_syntax_by_extension("py").unwrap();
    let mut h = HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]);

    let mut out = Vec::new();
    for line in LinesWithEndings::from(text) {
        // LinesWithEndings enables use of newlines mode
        let line_spans: Vec<Span> = h
            .highlight_line(line, ps)
            .unwrap()
            .into_iter()
            .filter_map(|segment| into_span(segment).ok())
            .collect();
        out.push(Line::from(line_spans).to_owned());
    }
    out
}
