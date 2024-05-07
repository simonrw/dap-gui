use std::collections::HashSet;

use iced::{
    mouse,
    widget::{
        canvas::{Frame, Path, Program},
        row, scrollable,
        text_editor::Content,
    },
    Color, Length, Point,
};

use crate::Message;

pub const LINE_HEIGHT: f32 = 20.8;
pub const OFFSET: u8 = 6;
pub const GUTTER_WIDTH: f32 = 16.0;

struct RenderBreakpoints<'b> {
    breakpoints: &'b HashSet<usize>,
    line_height: f32,
    offset: u8,
    gutter_highlight: Option<&'b usize>,
}

// TODO: make `Message` generic
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

        if let Some(highlight) = self.gutter_highlight {
            let mut frame = Frame::new(renderer, bounds.size());
            let center = Point::new(
                bounds.size().width / 2.0,
                (*highlight as f32) * self.line_height + (self.offset as f32),
            );
            let circle = Path::circle(center, 4.0);
            frame.fill(&circle, Color::from_rgb8(207, 120, 0));
            geometry.push(frame.into_geometry());
        }

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

pub fn code_viewer<'a>(
    content: &'a Content,
    line_height: f32,
    offset: u8,
    breakpoints: &'a HashSet<usize>,
    scrollable_id: iced::widget::scrollable::Id,
    gutter_highlight: Option<&'a usize>,
) -> iced::Element<'a, Message> {
    let render_breakpoints = RenderBreakpoints {
        breakpoints,
        line_height,
        offset,
        gutter_highlight,
    };
    let gutter = iced::widget::canvas(render_breakpoints)
        .height(Length::Fill)
        .width(Length::Fixed(GUTTER_WIDTH));

    let editor = iced::widget::text_editor(content)
        .padding(16)
        .height(Length::Fill)
        .on_action(Message::EditorActionPerformed);
    scrollable(
        row![gutter, editor]
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .height(Length::Fill)
    .width(Length::Fill)
    .on_scroll(Message::OnScroll)
    .id(scrollable_id)
    .into()
}
