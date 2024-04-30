use std::collections::HashSet;

use iced::{
    mouse::{self, Button},
    widget::{
        canvas::{Frame, Path, Program},
        column, row, scrollable,
        scrollable::Viewport,
        text_editor::Content,
    },
    Application, Color, Command, Length, Point, Settings,
};

const LINE_HEIGHT: f32 = 20.8;
const OFFSET: u8 = 6;
const GUTTER_WIDTH: f32 = 16.0;

#[derive(Debug, Clone)]
enum Message {
    CanvasClicked(mouse::Button),
    MouseMoved(Point),
    OnScroll(Viewport),
}

struct App {
    content: Content,
    breakpoints: HashSet<usize>,
    line_height: f32,
    offset: u8,
    cursor_pos: Point,
    scroll_position: f32,
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
                breakpoints: (0..200).collect(),
                line_height: LINE_HEIGHT,
                offset: OFFSET,
                cursor_pos: Point::default(),
                scroll_position: 0.0,
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
            Message::MouseMoved(point) => self.cursor_pos = point,
            Message::OnScroll(viewport) => {
                let offset = viewport.absolute_offset();
                self.scroll_position = offset.y;
            }
        }
        Command::none()
    }

    fn view(&self) -> iced::Element<'_, Self::Message> {
        column![code_viewer(
            &self.content,
            self.line_height,
            self.offset,
            &self.breakpoints
        ),]
        .into()
    }

    fn theme(&self) -> iced::Theme {
        iced::Theme::Dark
    }
}

struct RenderBreakpoints<'b> {
    breakpoints: &'b HashSet<usize>,
    line_height: f32,
    offset: u8,
}

impl<'b> Program<Message> for RenderBreakpoints<'b> {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::advanced::mouse::Cursor,
    ) -> Vec<<iced::Renderer as iced::widget::canvas::Renderer>::Geometry> {
        let mut geometry = Vec::with_capacity(self.breakpoints.len());
        for b in self.breakpoints {
            let mut frame = Frame::new(renderer, bounds.size());
            let center = Point::new(
                bounds.size().width / 2.0,
                (*b as f32) * self.line_height + (self.offset as f32),
            );
            let circle = Path::circle(center, 4.0);
            frame.fill(&circle, Color::from_rgb8(255, 0, 0));
            geometry.push(frame.into_geometry());
        }
        geometry
    }

    fn update(
        &self,
        _state: &mut Self::State,
        event: iced::widget::canvas::Event,
        _bounds: iced::Rectangle,
        _cursor: iced::advanced::mouse::Cursor,
    ) -> (iced::widget::canvas::event::Status, Option<Message>) {
        match event {
            iced::widget::canvas::Event::Mouse(mouse::Event::ButtonReleased(button)) => (
                iced::widget::canvas::event::Status::Captured,
                Some(Message::CanvasClicked(button)),
            ),
            iced::widget::canvas::Event::Mouse(mouse::Event::CursorMoved { position }) => (
                iced::widget::canvas::event::Status::Captured,
                Some(Message::MouseMoved(position)),
            ),
            _ => (iced::widget::canvas::event::Status::Ignored, None),
        }
    }
}

fn code_viewer<'a>(
    content: &'a Content,
    line_height: f32,
    offset: u8,
    breakpoints: &'a HashSet<usize>,
) -> iced::Element<'a, Message> {
    let render_breakpoints = RenderBreakpoints {
        breakpoints,
        line_height,
        offset,
    };
    let gutter = iced::widget::canvas(render_breakpoints)
        .height(Length::Fill)
        .width(Length::Fixed(GUTTER_WIDTH));

    let editor = iced::widget::text_editor(content)
        .padding(16)
        .height(Length::Fill);
    scrollable(
        row![gutter, editor]
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .height(Length::Fill)
    .width(Length::Fill)
    .on_scroll(Message::OnScroll)
    .into()
}

fn main() -> iced::Result {
    App::run(Settings::default())
}
