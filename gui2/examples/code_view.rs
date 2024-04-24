use iced::{
    mouse,
    widget::{
        canvas::{Frame, Path, Program},
        column, row, scrollable,
        text_editor::Content,
    },
    Application, Color, Command, Length, Point, Settings,
};

const LINE_HEIGHT: f32 = 20.8;
const OFFSET: u8 = 6;

#[derive(Debug, Clone)]
enum Message {
    LineHeightChanged(f32),
    OffsetChanged(u8),
    CanvasClicked(mouse::Button),
    MouseMoved(Point),
}

struct App {
    content: Content,
    breakpoints: Vec<usize>,
    line_height: f32,
    offset: u8,
    cursor_pos: Point,
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
                breakpoints: vec![1, 8, 20],
                line_height: LINE_HEIGHT,
                offset: OFFSET,
                cursor_pos: Point::default(),
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        "".to_string()
    }

    fn update(&mut self, message: Self::Message) -> Command<Message> {
        match message {
            Message::LineHeightChanged(value) => {
                self.line_height = value;
            }
            Message::OffsetChanged(value) => {
                self.offset = value;
            }
            Message::CanvasClicked(button) => {
                println!("{:?} {:?}", button, self.cursor_pos);
            }
            Message::MouseMoved(point) => self.cursor_pos = point,
        }
        Command::none()
    }

    fn view(&self) -> iced::Element<'_, Self::Message> {
        column![
            code_viewer(
                &self.content,
                self.line_height,
                self.offset,
                &self.breakpoints
            ),
            row![
                iced::widget::slider(0.0..=100.0, self.line_height, Message::LineHeightChanged)
                    .step(0.1),
                iced::widget::text(format!("{:.2}", self.line_height)),
            ]
            .spacing(16)
            .padding(8),
            row![
                iced::widget::slider(0..=255, self.offset, Message::OffsetChanged),
                iced::widget::text(format!("{:.2}", self.offset)),
            ]
            .spacing(16)
            .padding(8),
        ]
        .into()
    }

    fn theme(&self) -> iced::Theme {
        iced::Theme::Nord
    }
}

struct RenderBreakpoints<'b> {
    breakpoints: &'b [usize],
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
    breakpoints: &'a [usize],
) -> iced::Element<'a, Message> {
    let render_breakpoints = RenderBreakpoints {
        breakpoints,
        line_height,
        offset,
    };
    let gutter = iced::widget::canvas(render_breakpoints)
        .height(Length::Fill)
        .width(Length::Fixed(16.0));

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
    .into()
}

fn main() -> iced::Result {
    App::run(Settings::default())
}
