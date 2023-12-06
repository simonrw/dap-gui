// imports
use std::{cell::RefCell, net::TcpStream, path::PathBuf};

use debugger::{Breakpoint, Language, LaunchArguments};
use iced::{
    executor,
    keyboard::{KeyCode, Modifiers},
    subscription,
    widget::{button, container, row, scrollable},
    widget::{
        column,
        scrollable::{Id, RelativeOffset},
        text, text_input, Row,
    },
    Application, Command, Element, Length, Settings, Subscription, Theme,
};

use message::Message;

// mods
mod components;
mod message;

type Receiver<T> = spmc::Receiver<T>;

enum AppState {
    Uninitialised,
    Ready,
    Running,
    Paused {
        stack: Vec<transport::types::StackFrame>,
        source: debugger::FileSource,
    },
}

struct Debugger {
    rx: RefCell<Option<Receiver<debugger::Event>>>,
    debugger: debugger::Debugger,
    // store for recorded events
    event_log: Vec<debugger::Event>,
    event_log_id: Id,
    state: AppState,

    // temporary while we get the UI
    new_breakpoint: String,
    breakpoints: Vec<Breakpoint>,
}

impl Application for Debugger {
    type Executor = executor::Default;
    type Message = Message;
    type Theme = Theme;
    type Flags = ();

    /// Create the debugger application
    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let (tx, rx) = spmc::channel();
        let stream = TcpStream::connect("127.0.0.1:5678").unwrap();
        let client = transport::Client::new(stream, tx).unwrap();
        let (dtx, drx) = spmc::channel();
        let debugger = debugger::Debugger::new(client, rx, dtx).unwrap();

        debugger
            .initialise(LaunchArguments::from_path("./test.py", Language::DebugPy))
            .unwrap();

        (
            Self {
                rx: RefCell::new(Some(drx)),
                debugger,
                event_log: Vec::new(),
                event_log_id: Id::unique(),
                state: AppState::Uninitialised,
                new_breakpoint: String::new(),
                breakpoints: Vec::new(),
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        String::from("My application")
    }

    #[tracing::instrument(skip(self))]
    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        tracing::trace!("got event");
        match message {
            Message::DefineBreakpoint(s) => match s.parse::<usize>() {
                Ok(_) => self.new_breakpoint = s,
                Err(_) => self.new_breakpoint.clear(),
            },
            Message::AddBreakpoint => {
                if let Ok(line) = self.new_breakpoint.parse() {
                    let breakpoint = Breakpoint {
                        path: PathBuf::from("./test.py").canonicalize().unwrap(),
                        line,
                        ..Default::default()
                    };
                    self.debugger.add_breakpoint(breakpoint.clone());
                    self.breakpoints.push(breakpoint);
                }
                self.new_breakpoint.clear();
            }
            Message::Launch => {
                self.debugger.launch().unwrap();
            }
            Message::Quit => std::process::exit(0),
            Message::Continue => {
                self.debugger.r#continue().unwrap();
                self.state = AppState::Running;
            }
            Message::DebuggerMessage(msg) => {
                tracing::debug!(?msg, "debugger message");
                match &msg {
                    debugger::Event::Initialised => {
                        self.state = AppState::Ready;
                    }
                    debugger::Event::Uninitialised => {}
                    debugger::Event::Paused { stack, source } => {
                        self.state = AppState::Paused {
                            stack: stack.clone(),
                            source: source.clone(),
                        };
                    }
                    debugger::Event::Running => {
                        self.state = AppState::Running;
                    }
                    debugger::Event::Ended => {
                        std::process::exit(0);
                    }
                }

                self.event_log.push(msg);
                return iced::widget::scrollable::snap_to(
                    self.event_log_id.clone(),
                    RelativeOffset { x: 0.0, y: 1.0 },
                );
            }
        }
        Command::none()
    }

    fn view(&self) -> iced::Element<'_, Self::Message, iced::Renderer<Self::Theme>> {
        let event_log = {
            let event_messages: Vec<String> =
                self.event_log.iter().map(|e| format!("{e:?}")).collect();
            let log_text = event_messages.join("\n");
            scrollable(text(log_text))
                .height(Length::Fixed(100.0))
                .id(self.event_log_id.clone())
        };

        let main_content: Element<'_, Message> = match &self.state {
            AppState::Uninitialised => column![].into(),
            AppState::Ready => {
                // show breakpoint values
                let breakpoint_text: String = self
                    .breakpoints
                    .iter()
                    .map(|b| format!("{}", b.line))
                    .collect::<Vec<_>>()
                    .join("\n");

                column![
                    components::launch_button(),
                    text_input("", &self.new_breakpoint)
                        .on_input(Message::DefineBreakpoint)
                        .on_submit(Message::AddBreakpoint)
                        .padding(10),
                    text("Breakpoints").size(20),
                    text(breakpoint_text),
                ]
                .into()
            }
            AppState::Running => text("Running").into(),
            AppState::Paused { stack, source } => {
                let stack = {
                    let items: Vec<_> = stack
                        .iter()
                        .map(|frame| frame.name.clone())
                        .map(text)
                        .map(From::from)
                        .collect();
                    Row::with_children(items)
                };

                // show the current line that the debugger has paused at
                let start_line = (source.line - 2).max(0) as usize;
                let end_line =
                    (source.line + 2).min(source.contents.lines().count() as isize) as usize;
                let lines: Vec<_> = source
                    .contents
                    .lines()
                    .skip(start_line)
                    .take(end_line - start_line)
                    .collect();
                let current_text = lines.join("\n");

                column![
                    // row![text(&current_line)].padding(10),
                    text(&current_text).size(20),
                    stack,
                    button("Continue").on_press(Message::Continue),
                ]
                .into()
            }
        };

        container(column![
            row![main_content].height(Length::Fill),
            row![event_log,],
        ])
        .into()
    }

    fn theme(&self) -> Self::Theme {
        Theme::Dark
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let debugger_sub = subscription::unfold("id", self.rx.take(), move |mut rx| async move {
            let msg = rx.as_mut().unwrap().recv().unwrap();
            (Message::DebuggerMessage(msg), rx)
        });

        let events_sub = iced::keyboard::on_key_press(|code, mods| match (code, mods) {
            (KeyCode::Q, Modifiers::CTRL) => Some(Message::Quit),
            _ => None,
        });

        subscription::Subscription::batch([debugger_sub, events_sub])
    }
}

#[cfg(feature = "sentry")]
macro_rules! setup_sentry {
    () => {
        tracing::info!("setting up sentry for crash reporting");
        let _guard = sentry::init((
            "https://f08b65bc9944ecbb855f1ebb2cadcb92@o366030.ingest.sentry.io/4505663159926784",
            sentry::ClientOptions {
                release: sentry::release_name!(),
                ..Default::default()
            },
        ));
    };
}

#[cfg(not(feature = "sentry"))]
macro_rules! setup_sentry {
    () => {};
}

fn main() -> iced::Result {
    setup_sentry!();
    tracing_subscriber::fmt::init();

    Debugger::run(Settings::default())
}
