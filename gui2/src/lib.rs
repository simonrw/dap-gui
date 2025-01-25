use std::collections::HashSet;
use std::path::PathBuf;

use clap::Parser;
use code_view::{CodeViewer, CodeViewerAction};
use color_eyre::eyre::{self, Context};
use dark_light::Mode;
use debugger::{AttachArguments, Debugger, Event, ProgramDescription};
use iced::keyboard::{Key, Modifiers};
use iced::widget::{button, column, container, row, text, text_editor, Container};
use iced::{
    executor, subscription, Application, Color, Command, Element, Event as WindowEvent, Length,
    Subscription,
};
use iced_aw::Tabs;
use launch_configuration::{ChosenLaunchConfiguration, Debugpy, LaunchConfiguration};
use state::StateManager;
use transport::types::{StackFrame, StackFrameId};

pub mod code_view;
mod highlight;

#[derive(Debug, Parser, Default)]
pub struct Args {
    /// debug rendering
    #[clap(short, long)]
    debug: bool,

    /// Path to the config file
    config_path: PathBuf,

    /// Name of the launch configuration to choose
    #[clap(short, long)]
    name: Option<String>,
}

#[derive(Debug, Clone)]
pub enum Message {
    TabSelected(TabId),
    CodeViewer(CodeViewerAction),
    DebuggerMessage(Box<Event>),
    Window(WindowEvent),
    StackFrameChanged(StackFrameId),
    Quit,
}

fn title<'a, Message>(input: impl ToString) -> Container<'a, Message> {
    container(text(input).size(30)).padding(20)
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TabId {
    Variables,
    Repl,
}

#[derive(Debug)]
pub enum AppState {
    #[allow(dead_code)]
    Initialising,
    #[allow(dead_code)]
    Running { breakpoints: HashSet<usize> },
    Paused {
        args: Args,
        active_tab: TabId,
        content: text_editor::Content,
        breakpoints: HashSet<usize>,
        scrollable_id: iced::widget::scrollable::Id,
        stack: Vec<StackFrame>,
    },
    #[allow(dead_code)]
    Terminated,
}

pub struct DebuggerApp {
    state: AppState,
    debugger: Debugger,
}

impl DebuggerApp {
    // custom constructor method that is fallable, because the iced Application::new is not
    fn init() -> eyre::Result<Self> {
        let args = Args::parse();

        let state_path = dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("dapgui")
            .join("state.json");
        tracing::debug!(state_path = %state_path.display(), "loading state");
        if !state_path.parent().unwrap().is_dir() {
            std::fs::create_dir_all(state_path.parent().unwrap())
                .context("creating state directory")?;
        }
        let state_manager = StateManager::new(state_path)
            .wrap_err("loading state")?
            .save()
            .wrap_err("saving state")?;
        let persisted_state = state_manager.current();
        tracing::trace!(state = ?persisted_state, "loaded state");

        let config =
            match launch_configuration::load_from_path(args.name.as_ref(), &args.config_path)
                .wrap_err("loading launch configuration")?
            {
                ChosenLaunchConfiguration::Specific(config) => config,
                ChosenLaunchConfiguration::NotFound => {
                    eyre::bail!("no matching configuration found")
                }
                ChosenLaunchConfiguration::ToBeChosen(configurations) => {
                    eprintln!("Configuration name not specified");
                    eprintln!("Available options:");
                    for config in &configurations {
                        eprintln!("- {config}");
                    }
                    // TODO: best option?
                    std::process::exit(1);
                }
            };

        let mut debug_root_dir = std::env::current_dir().unwrap();

        let debugger = match config {
            LaunchConfiguration::Debugpy(Debugpy {
                request,
                cwd,
                connect,
                path_mappings,
                ..
            }) => {
                if let Some(dir) = cwd {
                    debug_root_dir = debugger::utils::normalise_path(&dir).into_owned();
                }
                let debugger = match request.as_str() {
                    "attach" => {
                        let launch_arguments = AttachArguments {
                            working_directory: debug_root_dir.to_owned().to_path_buf(),
                            port: connect.map(|c| c.port),
                            language: debugger::Language::DebugPy,
                            path_mappings,
                        };

                        tracing::debug!(?launch_arguments, "generated launch configuration");

                        Debugger::new(launch_arguments).context("creating internal debugger")?
                    }
                    _ => todo!(),
                };
                debugger
            }
        };

        debugger.wait_for_event(|e| matches!(e, debugger::Event::Initialised));

        if let Some(project_state) = state_manager
            .current()
            .projects
            .iter()
            .find(|p| debugger::utils::normalise_path(&p.path) == debug_root_dir)
        {
            tracing::debug!("got project state");
            for breakpoint in &project_state.breakpoints {
                {
                    let breakpoint_path = debugger::utils::normalise_path(&breakpoint.path);
                    if !breakpoint_path.starts_with(&debug_root_dir) {
                        continue;
                    }
                    tracing::debug!(?breakpoint, "adding breakpoint from state file");

                    let mut breakpoint = breakpoint.clone();
                    breakpoint.path = debugger::utils::normalise_path(&breakpoint.path)
                        .into_owned()
                        .to_path_buf();

                    debugger
                        .add_breakpoint(&breakpoint)
                        .context("adding breakpoint")?;
                }
            }
        } else {
            tracing::warn!("missing project state");
        }

        tracing::debug!("launching debugee");
        debugger.start().context("launching debugee")?;

        Ok(Self {
            state: AppState::Running {
                breakpoints: HashSet::new(),
            },
            debugger,
        })
    }

    // view helper methods
    fn view_call_stack(&self, stack: &[StackFrame]) -> iced::Element<'_, Message> {
        let mut column = column![title("Call Stack")].width(Length::Fill);

        for frame in stack {
            let elem =
                button(text(frame.name.clone())).on_press(Message::StackFrameChanged(frame.id));
            column = column.push(elem);
        }

        column.into()
    }

    fn view_breakpoints(&self) -> iced::Element<'_, Message> {
        title("Breakpoints").width(Length::Fill).into()
    }

    fn view_main_content(&self) -> iced::Element<'_, Message> {
        match &self.state {
            AppState::Initialising => todo!(),
            AppState::Running { .. } => todo!(),
            AppState::Paused {
                ref content,
                breakpoints,
                scrollable_id,
                ..
            } => CodeViewer::new(
                content,
                breakpoints,
                scrollable_id.clone(),
                0,
                Message::CodeViewer,
            )
            .into(),
            AppState::Terminated => todo!(),
        }
    }

    fn view_variables_content(&self) -> iced::Element<'_, Message> {
        text("variables").into()
    }

    fn view_repl_content(&self) -> iced::Element<'_, Message> {
        text("repl").into()
    }

    fn view_bottom_panel(&self) -> iced::Element<'_, Message> {
        if let AppState::Paused { active_tab, .. } = &self.state {
            Tabs::new(Message::TabSelected)
                .tab_icon_position(iced_aw::tabs::Position::Top)
                .push(
                    TabId::Variables,
                    iced_aw::TabLabel::Text("Variables".to_string()),
                    self.view_variables_content(),
                )
                .push(
                    TabId::Repl,
                    iced_aw::TabLabel::Text("Repl".to_string()),
                    self.view_repl_content(),
                )
                .set_active_tab(active_tab)
                .into()
        } else {
            panic!(
                "programming error: state {:?} should not have a bottom panel",
                self.state
            );
        }
    }
}

impl Application for DebuggerApp {
    type Executor = executor::Default;
    type Theme = iced::Theme;
    type Flags = ();
    type Message = Message;

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        match Self::init() {
            Ok(this) => (this, Command::none()),
            Err(e) => panic!("failed to initialise application: {e}"),
        }
    }

    #[tracing::instrument(skip(self))]
    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match &mut self.state {
            AppState::Running { .. } => match message {
                Message::DebuggerMessage(event) => match *event {
                    Event::Uninitialised => todo!(),
                    Event::Initialised => todo!(),
                    Event::Paused(ProgramDescription {
                        breakpoints, stack, ..
                    }) => {
                        self.state = AppState::Paused {
                            args: Args::default(),
                            active_tab: TabId::Variables,
                            content: text_editor::Content::with_text(include_str!("main.rs")),
                            breakpoints: breakpoints.iter().map(|bp| bp.line).collect(),
                            scrollable_id: iced::widget::scrollable::Id::unique(),
                            stack,
                        }
                    }
                    Event::ScopeChange { .. } => todo!(),
                    Event::Running => {}
                    Event::Ended => todo!(),
                },
                other => {
                    tracing::debug!(message = ?other, "unhandled message");
                }
            },
            AppState::Paused {
                active_tab,
                breakpoints,
                content,
                scrollable_id,
                ..
            } => match message {
                Message::TabSelected(selected) => *active_tab = selected,
                Message::CodeViewer(CodeViewerAction::BreakpointChanged(bp)) => {
                    if breakpoints.contains(&bp) {
                        breakpoints.remove(&bp);
                    } else {
                        breakpoints.insert(bp);
                    }
                }
                Message::CodeViewer(CodeViewerAction::EditorAction(action)) => {
                    content.perform(action)
                }
                Message::CodeViewer(CodeViewerAction::ScrollCommand { offset, .. }) => {
                    return iced::widget::scrollable::scroll_to(scrollable_id.clone(), offset);
                }
                Message::DebuggerMessage(event) => {
                    tracing::debug!(?event, "received event from debugger");
                }
                Message::Quit => {
                    tracing::info!("got quit event");
                    return iced::window::close(iced::window::Id::MAIN);
                }
                Message::Window(WindowEvent::Window(id, iced::window::Event::Closed)) => {
                    tracing::debug!(?id, "got window event");
                }
                Message::StackFrameChanged(stack_frame_id) => {
                    tracing::debug!(?stack_frame_id, "being asked to change stack frame context");
                    if let Err(e) = self.debugger.change_scope(stack_frame_id) {
                        tracing::warn!(error = %e, %stack_frame_id, "failed to change scope to new stack frame");
                    }
                }
                other => tracing::trace!(event = ?other, "unhandled event in paused state"),
            },
            other => {
                tracing::debug!(event = ?other, "unhandled event");
            }
        }
        Command::none()
    }

    fn title(&self) -> String {
        "DebuggerApp".to_string()
    }

    fn view(&self) -> iced::Element<'_, Self::Message> {
        match &self.state {
            AppState::Paused { args, stack, .. } => {
                let sidebar = column![self.view_call_stack(stack), self.view_breakpoints(),]
                    .height(Length::Fill)
                    .width(Length::Fill);

                let main_content = column![self.view_main_content(), self.view_bottom_panel(),]
                    .height(Length::Fill);

                let mut result = Element::from(row![
                    sidebar.width(Length::Fixed(300.0)),
                    main_content.width(Length::Fill),
                ]);

                if args.debug {
                    result = result.explain(Color::from_rgb(1.0, 0.0, 0.0));
                }
                result
            }
            AppState::Running { .. } => text("Running").into(),
            _ => todo!(),
        }
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        let events = self.debugger.events();
        let debugger_sub = subscription::unfold("id", events, move |rx| async move {
            let msg = rx.recv().unwrap();
            (Message::DebuggerMessage(Box::new(msg)), rx)
        });
        let events_sub = iced::keyboard::on_key_press(|key, mods| match (key, mods) {
            (Key::Character(c), Modifiers::CTRL) if c == "q" => Some(Message::Quit),
            _ => None,
        });
        let window_sub = iced::event::listen().map(Message::Window);

        subscription::Subscription::batch([debugger_sub, events_sub, window_sub])
    }

    fn theme(&self) -> Self::Theme {
        match dark_light::detect() {
            Mode::Dark | Mode::Default => iced::Theme::Dark,
            Mode::Light => iced::Theme::Light,
        }
    }
}
