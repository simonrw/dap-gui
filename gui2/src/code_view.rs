use std::collections::HashSet;

use iced::{
    mouse,
    widget::{
        canvas::{Frame, Path, Program},
        component, row, scrollable,
        scrollable::Viewport,
        text_editor::{Action, Content},
        Component,
    },
    Color, Element, Length, Point,
};

pub const LINE_HEIGHT: f32 = 20.8;
pub const OFFSET: u8 = 6;
pub const GUTTER_WIDTH: f32 = 16.0;

// #[derive(Debug, Clone)]
// pub enum Event {
//     CanvasClicked(mouse::Button),
//     MouseMoved(Point),
//     EditorActionPerformed(Action),
//     OnScroll(Viewport),
// }

// struct RenderBreakpoints<'b> {
//     breakpoints: &'b HashSet<usize>,
//     gutter_highlight: Option<&'b usize>,
// }

// impl<'b, Message> Program<Message> for RenderBreakpoints<'b> {
//     type State = ();

//     fn draw(
//         &self,
//         _state: &Self::State,
//         renderer: &iced::Renderer,
//         _theme: &iced::Theme,
//         bounds: iced::Rectangle,
//         _cursor: iced::advanced::mouse::Cursor,
//     ) -> Vec<<iced::Renderer as iced::widget::canvas::Renderer>::Geometry> {
//         let mut geometry = Vec::with_capacity(self.breakpoints.len());

//         if let Some(highlight) = self.gutter_highlight {
//             let mut frame = Frame::new(renderer, bounds.size());
//             let center = Point::new(
//                 bounds.size().width / 2.0,
//                 (*highlight as f32) * LINE_HEIGHT + (OFFSET as f32),
//             );
//             let circle = Path::circle(center, 4.0);
//             frame.fill(&circle, Color::from_rgb8(207, 120, 0));
//             geometry.push(frame.into_geometry());
//         }

//         for b in self.breakpoints {
//             let mut frame = Frame::new(renderer, bounds.size());
//             let center = Point::new(
//                 bounds.size().width / 2.0,
//                 (*b as f32) * LINE_HEIGHT + (OFFSET as f32),
//             );
//             let circle = Path::circle(center, 4.0);
//             frame.fill(&circle, Color::from_rgb8(255, 0, 0));
//             geometry.push(frame.into_geometry());
//         }
//         geometry
//     }

//     fn update(
//         &self,
//         _state: &mut Self::State,
//         event: iced::widget::canvas::Event,
//         _bounds: iced::Rectangle,
//         _cursor: iced::advanced::mouse::Cursor,
//     ) -> (
//         iced::widget::canvas::event::Status,
//         Option<CodeViewerMessage>,
//     ) {
//         match event {
//             iced::widget::canvas::Event::Mouse(mouse::Event::ButtonReleased(button)) => (
//                 iced::widget::canvas::event::Status::Captured,
//                 Some(CodeViewerMessage::CanvasClicked(button)),
//             ),
//             iced::widget::canvas::Event::Mouse(mouse::Event::CursorMoved { position }) => (
//                 iced::widget::canvas::event::Status::Captured,
//                 Some(CodeViewerMessage::MouseMoved(position)),
//             ),
//             _ => (iced::widget::canvas::event::Status::Ignored, None),
//         }
//     }
// }

// pub fn code_viewer<'a>(
//     content: &'a Content,
//     breakpoints: &'a HashSet<usize>,
//     scrollable_id: iced::widget::scrollable::Id,
//     gutter_highlight: Option<&'a usize>,
//     // TODO: on_change customisation
// ) -> CodeViewer<'a> {
//     CodeViewer {
//         content,
//         breakpoints,
//         scrollable_id,
//         gutter_highlight,
//     }
// }

// pub struct CodeViewer<'a> {
//     content: &'a Content,
//     breakpoints: &'a HashSet<usize>,
//     scrollable_id: iced::widget::scrollable::Id,
//     gutter_highlight: Option<&'a usize>,
// }

// // impl<'a, Message> From<CodeViewer> for iced::Element<'a, Message> {
// //     fn from(value: CodeViewer) -> Self {
// //         component(value)
// //     }
// // }

// impl<'a, Message> Component<Message> for CodeViewer<'a> {
//     type State = ();
//     type Event = Event;

//     fn update(&mut self, _state: &mut Self::State, event: Self::Event) -> Option<Message> {
//         todo!()
//     }

//     fn view(
//         &self,
//         _state: &Self::State,
//     ) -> iced::Element<'_, Self::Event, iced::Theme, iced::Renderer> {
//         let render_breakpoints = RenderBreakpoints {
//             breakpoints: self.breakpoints,
//             gutter_highlight: self.gutter_highlight,
//         };
//         let gutter = iced::widget::canvas(render_breakpoints)
//             .height(Length::Fill)
//             .width(Length::Fixed(GUTTER_WIDTH));

//         let editor = iced::widget::text_editor(self.content)
//             .padding(16)
//             .height(Length::Fill)
//             .on_action(CodeViewerMessage::EditorActionPerformed);

//         scrollable(
//             row![gutter, editor]
//                 .width(Length::Fill)
//                 .height(Length::Fill),
//         )
//         .height(Length::Fill)
//         .width(Length::Fill)
//         .on_scroll(Event::OnScroll)
//         .id(self.scrollable_id)
//         .into()
//     }
// }

#[derive(Debug, Clone)]
pub enum Event {
    CanvasClicked(mouse::Button),
    MouseMoved(Point),
    EditorActionPerformed(Action),
    OnScroll(Viewport),
}

#[derive(Debug, Clone)]
pub enum CodeViewerAction {
    BreakpointAdded(usize),
    BreakpointRemoved(usize),
}

pub struct CodeViewer<'a, Message> {
    content: &'a Content,
    breakpoints: &'a HashSet<usize>,
    scrollable_id: iced::widget::scrollable::Id,
    gutter_highlight: Option<&'a usize>,
    on_change: Option<Box<dyn Fn(CodeViewerAction) -> Message>>,
}

impl<'a, Message> CodeViewer<'a, Message> {
    pub fn new(
        content: &'a Content,
        breakpoints: &'a HashSet<usize>,
        scrollable_id: iced::widget::scrollable::Id,
        gutter_highlight: Option<&'a usize>,
    ) -> Self {
        Self {
            content,
            breakpoints,
            scrollable_id,
            gutter_highlight,
            on_change: None,
        }
    }

    pub fn on_change(mut self, on_change: impl Fn(CodeViewerAction) -> Message) -> Self {
        self.on_change = Some(Box::new(on_change));
        self
    }
}

impl<'a, Message> From<CodeViewer<'a, Message>> for Element<'a, Message> {
    fn from(value: CodeViewer<'a, Message>) -> Self {
        component(value)
    }
}

struct RenderBreakpoints<'b> {
    breakpoints: &'b HashSet<usize>,
    gutter_highlight: Option<&'b usize>,
}

impl<'b, Message> Program<Message> for RenderBreakpoints<'b> {
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
                (*highlight as f32) * LINE_HEIGHT + (OFFSET as f32),
            );
            let circle = Path::circle(center, 4.0);
            frame.fill(&circle, Color::from_rgb8(207, 120, 0));
            geometry.push(frame.into_geometry());
        }

        for b in self.breakpoints {
            let mut frame = Frame::new(renderer, bounds.size());
            let center = Point::new(
                bounds.size().width / 2.0,
                (*b as f32) * LINE_HEIGHT + (OFFSET as f32),
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
                // Some(CodeViewerMessage::CanvasClicked(button)),
                //`TODO
                None,
            ),
            iced::widget::canvas::Event::Mouse(mouse::Event::CursorMoved { position }) => (
                iced::widget::canvas::event::Status::Captured,
                // Some(CodeViewerMessage::MouseMoved(position)),
                //`TODO
                None,
            ),
            _ => (iced::widget::canvas::event::Status::Ignored, None),
        }
    }
}

impl<'a, Message> Component<Message> for CodeViewer<'a, Message> {
    type State = ();

    type Event = Event;

    fn update(&mut self, _state: &mut Self::State, event: Event) -> Option<Message> {
        todo!()
    }

    fn view(&self, _state: &Self::State) -> iced::Element<'_, Event> {
        let render_breakpoints = RenderBreakpoints {
            breakpoints: self.breakpoints,
            gutter_highlight: self.gutter_highlight,
        };
        let gutter = iced::widget::canvas(render_breakpoints)
            .height(Length::Fill)
            .width(Length::Fixed(GUTTER_WIDTH));

        let editor = iced::widget::text_editor(self.content)
            .padding(16)
            .height(Length::Fill)
            .on_action(Self::Event::EditorActionPerformed);

        scrollable(
            row![gutter, editor]
                .width(Length::Fill)
                .height(Length::Fill),
        )
        .height(Length::Fill)
        .width(Length::Fill)
        .on_scroll(Event::OnScroll)
        .id(self.scrollable_id)
        .into()
    }
}
