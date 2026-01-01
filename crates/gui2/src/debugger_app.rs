use std::path::PathBuf;

use clap::Parser;
use color_eyre::eyre::{self, Context};
use debugger::{AttachArguments, Debugger, Event, ProgramState};
use iced::{
    Color, Element, Length, Point, Subscription, Task,
    keyboard::{Key, Modifiers},
    mouse,
    widget::{Container, button, column, container, row, text, text_editor},
};
use iced_aw::Tabs;
use launch_configuration::{ChosenLaunchConfiguration, Debugpy, LaunchConfiguration};
use state::StateManager;

use crate::{
    args::Args,
    code_view::{self, CodeViewerAction},
    message::{Message, TabId},
    state::{AppState, Phase},
};

fn title<'a, Message>(input: impl ToString) -> Container<'a, Message> {
    container(text(input.to_string()).size(30)).padding(20)
}

fn maybe_boot() -> eyre::Result<AppState> {
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

    let config = match launch_configuration::load_from_path(args.name.as_ref(), &args.config_path)
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
        other => todo!("{other:?}"),
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

    Ok(AppState::new(debugger))
}

pub fn boot() -> AppState {
    match maybe_boot() {
        Ok(state) => state,
        Err(err) => {
            tracing::error!("failed to boot debugger: {}", err);
            std::process::exit(1);
        }
    }
}

pub fn update(state: &mut AppState, message: Message) -> Task<Message> {
    match &mut state.phase {
        Phase::Running { .. } => match message {
            Message::DebuggerMessage(event) => match *event {
                Event::Uninitialised => todo!(),
                Event::Initialised => todo!(),
                Event::Paused(ProgramState {
                    breakpoints: bps,
                    stack,
                    ..
                }) => {
                    state.phase = Phase::Paused {
                        args: Args::default(),
                        active_tab: TabId::Variables,
                        content: text_editor::Content::with_text(include_str!("main.rs")),
                        breakpoints: bps.iter().map(|bp| bp.line).collect(),
                        scrollable_id: iced::widget::Id::unique(),
                        stack,
                        mouse_position: Point::default(),
                        scroll_position: 0.0,
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
        Phase::Paused {
            active_tab,
            breakpoints,
            content,
            scrollable_id,
            mouse_position,
            scroll_position,
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
            Message::CodeViewer(CodeViewerAction::EditorAction(action)) => content.perform(action),
            Message::CodeViewer(CodeViewerAction::ScrollCommand { offset }) => {
                return iced::widget::operation::scroll_to(scrollable_id.clone(), offset);
            }
            Message::CodeViewer(CodeViewerAction::MouseMoved(point)) => {
                *mouse_position = point;
            }
            Message::CodeViewer(CodeViewerAction::Scrolled(viewport)) => {
                let offset = viewport.absolute_offset();
                *scroll_position = offset.y;
            }
            Message::CodeViewer(CodeViewerAction::CanvasClicked(mouse::Button::Left)) => {
                use crate::code_view::{GUTTER_WIDTH, LINE_HEIGHT};
                if mouse_position.x < GUTTER_WIDTH {
                    let line_no =
                        ((mouse_position.y + *scroll_position) / LINE_HEIGHT).floor() as usize;
                    if breakpoints.contains(&line_no) {
                        breakpoints.remove(&line_no);
                    } else {
                        breakpoints.insert(line_no);
                    }
                }
            }
            Message::CodeViewer(CodeViewerAction::CanvasClicked(_)) => {}
            Message::DebuggerMessage(event) => {
                tracing::debug!(?event, "received event from debugger");
            }
            Message::Quit => {
                tracing::info!("got quit event");
                return iced::exit();
            }
            Message::Window(id, iced::window::Event::Closed) => {
                tracing::debug!(?id, "got window event");
            }
            Message::StackFrameChanged(stack_frame_id) => {
                tracing::debug!(?stack_frame_id, "being asked to change stack frame context");
                if let Err(e) = state.debugger.change_scope(stack_frame_id) {
                    tracing::warn!(error = %e, %stack_frame_id, "failed to change scope to new stack frame");
                }
            }
            other => tracing::trace!(event = ?other, "unhandled event in paused state"),
        },
        other => {
            tracing::debug!(event = ?other, "unhandled event");
        }
    }
    Task::none()
}

pub fn view(state: &AppState) -> impl Into<Element<'_, Message>> {
    match &state.phase {
        Phase::Paused { args, stack, .. } => {
            // Inline view_call_stack
            let mut call_stack_col = column![title("Call Stack")].width(Length::Fill);
            for frame in stack {
                let elem =
                    button(text(frame.name.clone())).on_press(Message::StackFrameChanged(frame.id));
                call_stack_col = call_stack_col.push(elem);
            }

            let sidebar = column![call_stack_col, title("Breakpoints").width(Length::Fill),]
                .height(Length::Fill)
                .width(Length::Fill);

            // Inline view_main_content
            let main_content_elem: Element<'_, Message> = match &state.phase {
                Phase::Paused {
                    content,
                    breakpoints,
                    scrollable_id,
                    mouse_position,
                    scroll_position,
                    ..
                } => code_view::code_viewer(
                    content,
                    breakpoints,
                    *mouse_position,
                    *scroll_position,
                    scrollable_id.clone(),
                    Message::CodeViewer,
                ),
                _ => text("").into(),
            };

            // Inline view_bottom_panel
            let bottom_panel_elem: iced::Element<_> = match &state.phase {
                Phase::Paused { active_tab, .. } => Tabs::new(Message::TabSelected)
                    .tab_icon_position(iced_aw::tabs::Position::Top)
                    .push::<iced::advanced::widget::Text<'_, _, _>>(
                        TabId::Variables,
                        iced_aw::TabLabel::Text("Variables".to_string()),
                        text("variables"),
                    )
                    .push(
                        TabId::Repl,
                        iced_aw::TabLabel::Text("Repl".to_string()),
                        text("repl"),
                    )
                    .set_active_tab(active_tab)
                    .into(),
                _ => text("").into(),
            };

            let main_content = column![main_content_elem, bottom_panel_elem].height(Length::Fill);

            let mut result = Element::from(row![
                sidebar.width(Length::Fixed(300.0)),
                main_content.width(Length::Fill),
            ]);

            if args.debug {
                result = result.explain(Color::from_rgb(1.0, 0.0, 0.0));
            }
            result
        }
        Phase::Running { .. } => text("Running").into(),
        _ => todo!(),
    }
}

pub fn subscription(_state: &AppState) -> Subscription<Message> {
    // TODO: Re-implement debugger event subscription
    // For now, just use keyboard events
    iced::event::listen_with(|event, _status, id| match event {
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
            key: Key::Character(c),
            modifiers: Modifiers::CTRL,
            ..
        }) if c == "q" => Some(Message::Quit),
        iced::Event::Window(window_event) => Some(Message::Window(id, window_event)),
        _ => None,
    })
}
