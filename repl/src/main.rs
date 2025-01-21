use std::{path::PathBuf, rc::Rc, sync::Mutex, time::Duration};

use clap::Parser;
use color_eyre::{eyre, eyre::Context};
use crossbeam_channel::{Receiver, TryRecvError};
use crossterm::event::{self, Event, KeyCode};
use debugger::{Breakpoint, Debugger};
use ratatui::{
    layout::{Constraint, Layout, Position},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, List, ListItem, Paragraph},
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
    messages: Vec<String>,
    events: Receiver<debugger::Event>,
    should_terminate: bool,
}

impl App {
    fn new(debugger: Debugger) -> Self {
        let events = debugger.events();
        Self {
            debugger,
            input: String::new(),
            character_index: 0,
            messages: Vec::new(),
            events,
            should_terminate: false,
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
        let command: Vec<_> = self.input.split_whitespace().collect();
        eyre::ensure!(!command.is_empty(), "no command given");
        match command[0] {
            "c" => {
                tracing::debug!("executing continue command");
                self.debugger.r#continue().context("continuing execution")?;
            }
            "n" => {
                tracing::debug!("executing step_over command");
                self.debugger.step_over().context("stepping over")?;
            }
            "i" => {
                tracing::debug!("executing step_in command");
                self.debugger.step_in().context("stepping in")?;
            }
            "o" => {
                tracing::debug!("executing step_out command");
                self.debugger.step_out().context("stepping out")?;
            }
            "p" => {
                eyre::ensure!(command.len() > 1, "no variables given to print");
                for variable_name in command.iter().skip(1) {
                    tracing::warn!(name = %*variable_name, "todo: printing variable");
                }
            }
            other => tracing::warn!(%other, "unhandled command"),
        }

        self.input.clear();
        self.reset_cursor();
        Ok(())
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
        loop {
            if self.should_terminate {
                tracing::info!("terminating application");
                return Ok(());
            }

            // debugger events
            // TODO: try to handle multiple events?
            // TODO: select over keyboard and debugger events to prevent blocking
            match self.events.try_recv() {
                Ok(event) => self
                    .handle_debugger_event(event)
                    .context("handling debugger event")?,
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    tracing::info!("debugger disconnected, terminating application");
                    return Ok(());
                }
            }

            // rendering
            terminal.draw(|frame| self.draw(frame))?;

            // user input, but only if there is a queued event
            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    match key.code {
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
                    }
                }
            }
        }
    }

    #[tracing::instrument(skip(self))]
    fn handle_debugger_event(&mut self, event: debugger::Event) -> eyre::Result<()> {
        tracing::debug!("got debugger event");
        match event {
            debugger::Event::Paused {
                stack,
                breakpoints,
                paused_frame,
            } => {}
            debugger::Event::ScopeChange {
                stack,
                breakpoints,
                paused_frame,
            } => {}
            debugger::Event::Running => {}
            debugger::Event::Ended => self.should_terminate = true,
            _ => {}
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        // TODO: coloured bar at the bottom showing debugging state
        let vertical = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]);
        let [input_area, messages_area] = vertical.areas(frame.area());

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

        let messages: Vec<ListItem> = self
            .messages
            .iter()
            .enumerate()
            .map(|(i, m)| {
                let content = Line::from(Span::raw(format!("{i}: {m}")));
                ListItem::new(content)
            })
            .collect();
        let messages = List::new(messages).block(Block::bordered().title("Messages"));
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
