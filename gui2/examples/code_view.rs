use std::collections::HashSet;

use gui2::code_view::{CodeViewer, CodeViewerAction};
use iced::{
    widget::{column, text, text_editor::Content},
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
    gutter_highlight: Option<usize>,
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
                gutter_highlight: None,
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        "Code Viewer example".to_string()
    }

    fn update(&mut self, message: Self::Message) -> Command<Message> {
        match message {
            Message::CodeViewer(CodeViewerAction::BreakpointChanged(bp)) => {
                tracing::debug!(?bp, "updating breakpoint");
            }
            Message::CodeViewer(CodeViewerAction::EditorAction(action)) => {
                self.content.perform(action)
            }
        }
        Command::none()
    }

    fn view(&self) -> iced::Element<'_, Self::Message> {
        column![
            text("Hello world"),
            CodeViewer::new(
                &self.content,
                &self.breakpoints,
                self.scrollable_id.clone(),
                self.gutter_highlight.as_ref(),
                Message::CodeViewer,
            )
        ]
        .into()
    }

    fn theme(&self) -> iced::Theme {
        iced::Theme::Dark
    }
}

fn main() -> iced::Result {
    tracing_subscriber::fmt::init();

    App::run(Settings::default())
}
