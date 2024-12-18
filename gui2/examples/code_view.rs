use std::collections::HashSet;

use dark_light::Mode;
use gui2::code_view::{CodeViewer, CodeViewerAction};
use iced::{
    widget::{column, text_editor::Content},
    Application, Command, Settings,
};

#[derive(Debug, Clone)]
enum Message {
    CodeViewer(CodeViewerAction),
}

struct App {
    content: Content,
    breakpoints: HashSet<usize>,
    scrollable_id: iced::widget::scrollable::Id,
}

impl Application for App {
    type Message = Message;
    type Executor = iced::futures::executor::ThreadPool;
    type Theme = iced::Theme;
    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, Command<Message>) {
        (
            Self {
                content: Content::with_text(include_str!("code_view.rs")),
                breakpoints: Default::default(),
                scrollable_id: iced::widget::scrollable::Id::unique(),
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        "Code Viewer example".to_string()
    }

    #[tracing::instrument(skip(self))]
    fn update(&mut self, message: Self::Message) -> Command<Message> {
        match message {
            Message::CodeViewer(CodeViewerAction::BreakpointChanged(bp)) => {
                if self.breakpoints.contains(&bp) {
                    self.breakpoints.remove(&bp);
                } else {
                    self.breakpoints.insert(bp);
                }
            }
            Message::CodeViewer(CodeViewerAction::EditorAction(action)) => {
                tracing::debug!(?action, "got editor action");
                self.content.perform(action)
            }
            Message::CodeViewer(CodeViewerAction::ScrollCommand { offset, .. }) => {
                return iced::widget::scrollable::scroll_to(self.scrollable_id.clone(), offset);
            }
        }
        Command::none()
    }

    fn view(&self) -> iced::Element<'_, Self::Message> {
        column![CodeViewer::new(
            &self.content,
            &self.breakpoints,
            self.scrollable_id.clone(),
            0,
            Message::CodeViewer,
        )]
        .into()
    }

    fn theme(&self) -> iced::Theme {
        match dark_light::detect() {
            Mode::Dark | Mode::Default => iced::Theme::Dark,
            Mode::Light => iced::Theme::Light,
        }
    }
}

fn main() -> iced::Result {
    tracing_subscriber::fmt::init();

    App::run(Settings::default())
}
