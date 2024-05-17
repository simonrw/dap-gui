use std::collections::HashSet;

use code_view::{CodeViewer, CodeViewerAction};
use dark_light::Mode;
use iced::widget::{column, container, row, text, text_editor, Container};
use iced::{executor, Application, Color, Command, Element, Length};
use iced_aw::Tabs;

pub mod code_view;
mod highlight;

#[derive(Debug, Clone)]
pub enum Message {
    TabSelected(TabId),
    CodeViewer(CodeViewerAction),
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
pub enum DebuggerApp {
    #[allow(dead_code)]
    Initialising,
    #[allow(dead_code)]
    Running { breakpoints: HashSet<usize> },
    Paused {
        active_tab: TabId,
        content: text_editor::Content,
        breakpoints: HashSet<usize>,
        scrollable_id: iced::widget::scrollable::Id,
    },
    #[allow(dead_code)]
    Terminated,
}

impl DebuggerApp {
    // view helper methods
    fn view_call_stack(&self) -> iced::Element<'_, Message> {
        title("Call Stack").width(Length::Fill).into()
    }

    fn view_breakpoints(&self) -> iced::Element<'_, Message> {
        title("Breakpoints").width(Length::Fill).into()
    }

    fn view_main_content(&self) -> iced::Element<'_, Message> {
        match self {
            DebuggerApp::Initialising => todo!(),
            DebuggerApp::Running { .. } => todo!(),
            DebuggerApp::Paused {
                ref content,
                breakpoints,
                scrollable_id,
                ..
            } => CodeViewer::new(
                content,
                breakpoints,
                scrollable_id.clone(),
                Message::CodeViewer,
            )
            .into(),
            DebuggerApp::Terminated => todo!(),
        }
    }

    fn view_variables_content(&self) -> iced::Element<'_, Message> {
        text("variables").into()
    }

    fn view_repl_content(&self) -> iced::Element<'_, Message> {
        text("repl").into()
    }

    fn view_bottom_panel(&self) -> iced::Element<'_, Message> {
        if let Self::Paused { active_tab, .. } = self {
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
            panic!("programming error: state {self:?} should not have a bottom panel");
        }
    }
}

impl Application for DebuggerApp {
    type Executor = executor::Default;
    type Theme = iced::Theme;
    type Flags = ();
    type Message = Message;

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let this = Self::Paused {
            active_tab: TabId::Variables,
            content: text_editor::Content::with_text(include_str!("main.rs")),
            breakpoints: HashSet::new(),
            scrollable_id: iced::widget::scrollable::Id::unique(),
        };
        (this, Command::none())
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        #[allow(clippy::single_match)]
        match self {
            Self::Paused {
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
                    tracing::debug!(?action, "got editor action");
                    content.perform(action)
                }
                Message::CodeViewer(CodeViewerAction::ScrollCommand { offset, .. }) => {
                    return iced::widget::scrollable::scroll_to(scrollable_id.clone(), offset);
                }
            },
            _ => {}
        }
        Command::none()
    }

    fn title(&self) -> String {
        "DebuggerApp".to_string()
    }

    fn view(&self) -> iced::Element<'_, Self::Message> {
        let sidebar = column![self.view_call_stack(), self.view_breakpoints(),]
            .height(Length::Fill)
            .width(Length::Fill);

        let main_content =
            column![self.view_main_content(), self.view_bottom_panel(),].height(Length::Fill);

        let container: Element<_> = row![
            sidebar.width(Length::Fixed(300.0)),
            main_content.width(Length::Fill),
        ]
        .into();
        container.explain(Color::from_rgb(0.7, 0.7, 0.7))

        // let c: iced::Element<_> = container(
        //     column![
        //         button("increment").on_press(Message::Increment),
        //         text(self.value).size(50),
        //         button("decrement").on_press(Message::Decrement),
        //     ]
        //     .padding(20)
        //     .align_items(Alignment::Center),
        // )
        // .width(Length::Fill)
        // .height(Length::Fill)
        // .align_x(Horizontal::Center)
        // .align_y(Vertical::Center)
        // .center_x()
        // .center_y()
        // .style(container::Appearance {
        //     border: Border {
        //         width: 2.0,
        //         color: Color::BLACK,
        //         ..Default::default()
        //     },
        //     ..Default::default()
        // })
        // .into();
        // c.explain(Color::BLACK)
    }

    fn theme(&self) -> Self::Theme {
        match dark_light::detect() {
            Mode::Dark | Mode::Default => iced::Theme::Dark,
            Mode::Light => iced::Theme::Light,
        }
    }
}
