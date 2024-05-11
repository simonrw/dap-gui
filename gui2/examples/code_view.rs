use std::collections::HashSet;

use gui2::code_view::{CodeViewer, GUTTER_WIDTH, LINE_HEIGHT, OFFSET};
use iced::{
    mouse::Button,
    widget::{
        column,
        scrollable::{self},
        text_editor::{Action, Content},
    },
    Application, Command, Point, Settings,
};

#[derive(Debug, Clone)]
enum Message {}

struct App {
    content: Content,
    breakpoints: HashSet<usize>,
    line_height: f32,
    offset: u8,
    cursor_pos: Point,
    scroll_position: f32,
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
                line_height: LINE_HEIGHT,
                offset: OFFSET,
                cursor_pos: Point::default(),
                scroll_position: 0.0,
                scrollable_id: iced::widget::scrollable::Id::unique(),
                gutter_highlight: None,
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        "".to_string()
    }

    fn update(&mut self, message: Self::Message) -> Command<Message> {
        match message {
            Message::CanvasClicked(Button::Left) => {
                if self.cursor_pos.x < GUTTER_WIDTH {
                    let line_no =
                        ((self.cursor_pos.y + self.scroll_position) / LINE_HEIGHT).floor() as usize;
                    if self.breakpoints.contains(&line_no) {
                        println!("Removing line {line_no}");
                        self.breakpoints.remove(&line_no);
                    } else {
                        println!("Adding line {line_no}");
                        self.breakpoints.insert(line_no);
                    }
                }
            }
            Message::CanvasClicked(_) => {}
            Message::MouseMoved(point) => {
                self.cursor_pos = point;

                if point.x < GUTTER_WIDTH {
                    self.gutter_highlight =
                        Some(((point.y + self.scroll_position) / LINE_HEIGHT).floor() as _);
                } else {
                    self.gutter_highlight = None;
                }
            }
            Message::OnScroll(viewport) => {
                let offset = viewport.absolute_offset();
                self.scroll_position = offset.y;
            }
            Message::EditorActionPerformed(action) => match action {
                Action::Edit(_) => {
                    // override edit action to make nothing happen
                }
                Action::Scroll { lines } => {
                    // override scroll action to make sure we don't break things
                    self.scroll_position += (lines as f32) * LINE_HEIGHT;
                    return iced::widget::scrollable::scroll_to(
                        self.scrollable_id.clone(),
                        scrollable::AbsoluteOffset {
                            x: 0.0,
                            y: self.scroll_position,
                        },
                    );
                }
                action => self.content.perform(action),
                // text_editor::Action::Select(_) => todo!(),
                // text_editor::Action::SelectWord => todo!(),
                // text_editor::Action::SelectLine => todo!(),
                // text_editor::Action::Drag(_) => todo!(),
                // text_editor::Action::Scroll { lines } => todo!(),
            },
            Message::TabSelected(_) => todo!(),
        }
        Command::none()
    }

    fn view(&self) -> iced::Element<'_, Self::Message> {
        column![code_viewer(
            &self.content,
            self.line_height,
            self.offset,
            &self.breakpoints,
            self.scrollable_id.clone(),
            self.gutter_highlight.as_ref(),
        )
        .map(Message::CodeViewer)]
        .into()
    }

    fn theme(&self) -> iced::Theme {
        iced::Theme::Dark
    }
}

fn main() -> iced::Result {
    App::run(Settings::default())
}
