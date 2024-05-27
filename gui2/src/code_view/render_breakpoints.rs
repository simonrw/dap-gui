use std::collections::HashSet;

use iced::{
    mouse,
    widget::canvas::{Frame, Path, Program},
    Color, Point,
};

use super::Event;

pub(crate) struct RenderBreakpoints<'b> {
    pub(crate) breakpoints: &'b HashSet<usize>,
    pub(crate) gutter_highlight: Option<usize>,
}

impl<'b> Program<Event> for RenderBreakpoints<'b> {
    type State = ();

    #[tracing::instrument(skip(self, renderer, _theme, bounds, _cursor))]
    fn draw(
        &self,
        _state: &Self::State,
        renderer: &iced::Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::advanced::mouse::Cursor,
    ) -> Vec<<iced::Renderer as iced::widget::canvas::Renderer>::Geometry> {
        tracing::trace!("program draw");
        let mut geometry = Vec::with_capacity(self.breakpoints.len());

        if let Some(highlight) = self.gutter_highlight {
            let mut frame = Frame::new(renderer, bounds.size());
            let center = Point::new(
                bounds.size().width / 2.0,
                (highlight as f32) * super::LINE_HEIGHT + (super::OFFSET as f32),
            );
            let circle = Path::circle(center, 4.0);
            frame.fill(&circle, Color::from_rgb8(207, 120, 0));
            geometry.push(frame.into_geometry());
        }

        for b in self.breakpoints {
            let mut frame = Frame::new(renderer, bounds.size());
            let center = Point::new(
                bounds.size().width / 2.0,
                (*b as f32) * super::LINE_HEIGHT + (super::OFFSET as f32),
            );
            let circle = Path::circle(center, 4.0);
            frame.fill(&circle, Color::from_rgb8(255, 0, 0));
            geometry.push(frame.into_geometry());
        }
        geometry
    }

    #[tracing::instrument(skip(self, _bounds, _cursor))]
    fn update(
        &self,
        _state: &mut Self::State,
        event: iced::widget::canvas::Event,
        _bounds: iced::Rectangle,
        _cursor: iced::advanced::mouse::Cursor,
    ) -> (iced::widget::canvas::event::Status, Option<Event>) {
        tracing::trace!("program event");
        match event {
            iced::widget::canvas::Event::Mouse(mouse::Event::ButtonReleased(button)) => (
                iced::widget::canvas::event::Status::Captured,
                Some(Event::CanvasClicked(button)),
            ),
            iced::widget::canvas::Event::Mouse(mouse::Event::CursorMoved { position }) => (
                iced::widget::canvas::event::Status::Captured,
                Some(Event::MouseMoved(position)),
            ),
            _ => (iced::widget::canvas::event::Status::Ignored, None),
        }
    }
}
