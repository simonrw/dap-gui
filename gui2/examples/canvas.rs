use iced::{
    Color, Length, Sandbox, Settings,
    widget::{
        Canvas,
        canvas::{Frame, Path, Program},
        row,
    },
};

#[derive(Debug, Clone)]
enum Message {}

struct App {}

impl Sandbox for App {
    type Message = Message;

    fn new() -> Self {
        Self {}
    }

    fn title(&self) -> String {
        "Canvas test".to_string()
    }

    fn update(&mut self, _message: Self::Message) {}

    fn view(&self) -> iced::Element<'_, Self::Message> {
        row![canvas(120.0f32).width(Length::Fill).height(Length::Fill),].into()
    }
}

// circle rendering
#[derive(Debug)]
pub(crate) struct Circle {
    radius: f32,
}

impl<Message> Program<Message> for Circle {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::advanced::mouse::Cursor,
    ) -> Vec<<iced::Renderer as iced::widget::canvas::Renderer>::Geometry> {
        let mut frame = Frame::new(renderer, bounds.size());
        let circle = Path::circle(frame.center(), self.radius);
        frame.fill(&circle, Color::from_rgb(1.0, 0.0, 0.0));
        vec![frame.into_geometry()]
    }
}

pub(crate) fn canvas<Message>(radius: f32) -> Canvas<Circle, Message> {
    Canvas::new(Circle { radius })
}

fn main() -> iced::Result {
    App::run(Settings::default())
}
