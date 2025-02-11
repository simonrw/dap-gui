use std::{path::PathBuf, sync::Mutex};

use clap::Parser;
use color_eyre::{eyre, eyre::Context};
use crossbeam_channel::Receiver;
use crossterm::event::{self, Event, KeyCode};
use debugger::{Breakpoint, Debugger, ProgramState};
use ratatui::{
    layout::{Constraint, Layout, Position},
    style::{Color, Style},
    widgets::{Block, Paragraph},
    DefaultTerminal, Frame,
};
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
struct Args {
    launch_configuration: PathBuf,

    #[clap(short, long)]
    name: String,

    #[clap(short, long)]
    breakpoints: Vec<Breakpoint>,
}
struct App {
    debugger: Debugger,
    input: String,
    character_index: usize,
    events: Receiver<debugger::Event>,
    should_terminate: bool,
    messages: Vec<String>,
    program_description: Option<ProgramState>,
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
                self.add_message("Continuing execution");
                self.debugger.r#continue().context("continuing execution")?;
            }
            "n" => {
                tracing::debug!("executing step_over command");
                self.add_message("Stepping over");
                self.debugger.step_over().context("stepping over")?;
            }
            "i" => {
                tracing::debug!("executing step_in command");
                self.add_message("Stepping in");
                self.debugger.step_in().context("stepping in")?;
            }
            "o" => {
                tracing::debug!("executing step_out command");
                self.add_message("Stepping out");
                self.debugger.step_out().context("stepping out")?;
            }
            "p" => {
                eyre::ensure!(command.len() > 1, "no variables given to print");
                for variable_name in command.iter().skip(1) {
                    tracing::warn!(name = %*variable_name, "todo: printing variable");
                    if let Some(ProgramState { paused_frame, .. }) = &self.program_description {
                        if let Some(var) = paused_frame
                            .variables
                            .iter()
                            .find(|v| v.name == *variable_name)
                        {
                            let msg = format!(". {} = {}", var.name, var.value);
                            self.add_message(msg);
                        } else {
                            let msg =
                                format!("Variable '{}' not found in current scope", variable_name);
                            self.add_message(msg);
                        }
                    } else {
                        println!("???");
                    }
                }
            }
            other => tracing::warn!(%other, "unhandled command"),
        }

        self.input.clear();
        self.reset_cursor();
        Ok(())
    }

    fn add_message(&mut self, message: impl Into<String>) {
        self.messages.push(message.into());
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
                self.add_message(format!(
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
                ));
                self.program_description = Some(program_description);
            }
            debugger::Event::ScopeChange { .. } => {}
            debugger::Event::Running => {}
            debugger::Event::Ended => {
                self.add_message("program ended");
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
        let messages = self.messages.clone().join("\n");
        let messages = Paragraph::new(messages.as_str())
            .style(Style::default().fg(Color::White))
            .block(Block::bordered().title("Messages"));
        frame.render_widget(messages, messages_area);
    }
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;
    let log_file = std::fs::File::create("log.log")?;
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
