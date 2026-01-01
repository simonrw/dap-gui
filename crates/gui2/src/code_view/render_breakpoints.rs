use std::collections::HashSet;

use iced::{
    Color, Point, Renderer, mouse,
    widget::canvas::{Frame, Geometry, Path, Program},
};

use super::Event;

pub(crate) struct RenderBreakpoints<'b> {
    pub(crate) breakpoints: &'b HashSet<usize>,
    pub(crate) gutter_highlight: Option<usize>,
}

impl Program<Event> for RenderBreakpoints<'_> {
    type State = ();

    #[tracing::instrument(skip(self, renderer, _theme, bounds, _cursor))]
    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &iced::Theme,
        bounds: iced::Rectangle,
        _cursor: iced::advanced::mouse::Cursor,
    ) -> Vec<Geometry> {
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
        event: &iced::Event,
        _bounds: iced::Rectangle,
        _cursor: iced::advanced::mouse::Cursor,
    ) -> Option<iced::widget::Action<Event>> {
        tracing::trace!("program event");
        use iced::Event as IcedEvent;
        match event {
            IcedEvent::Mouse(mouse::Event::ButtonReleased(button)) => {
                Some(iced::widget::Action::publish(Event::CanvasClicked(*button)))
            }
            IcedEvent::Mouse(mouse::Event::CursorMoved { position }) => {
                Some(iced::widget::Action::publish(Event::MouseMoved(*position)))
            }
            _ => None,
        }
    }
}
