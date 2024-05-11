use std::{cell::RefCell, collections::HashSet, rc::Rc};

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
    BreakpointChanged(usize),
}

pub struct CodeViewer<'a, Message> {
    content: Rc<RefCell<Content>>,
    breakpoints: &'a HashSet<usize>,
    scrollable_id: iced::widget::scrollable::Id,
    gutter_highlight: Option<&'a usize>,
    on_change: Box<dyn Fn(CodeViewerAction) -> Message + 'static>,
}

impl<'a, Message> CodeViewer<'a, Message> {
    pub fn new(
        content: Rc<RefCell<Content>>,
        breakpoints: &'a HashSet<usize>,
        scrollable_id: iced::widget::scrollable::Id,
        gutter_highlight: Option<&'a usize>,
        on_change: impl Fn(CodeViewerAction) -> Message + 'static,
    ) -> Self {
        Self {
            content,
            breakpoints,
            scrollable_id,
            gutter_highlight,
            on_change: Box::new(on_change),
        }
    }
}

impl<'a, Message> From<CodeViewer<'a, Message>> for Element<'a, Message>
where
    Message: 'a,
{
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
            iced::widget::canvas::Event::Mouse(mouse::Event::ButtonReleased(_button)) => (
                iced::widget::canvas::event::Status::Captured,
                // Some(CodeViewerMessage::CanvasClicked(button)),
                //`TODO
                None,
            ),
            iced::widget::canvas::Event::Mouse(mouse::Event::CursorMoved {
                position: _position,
            }) => (
                iced::widget::canvas::event::Status::Captured,
                // Some(CodeViewerMessage::MouseMoved(position)),
                //`TODO
                None,
            ),
            _ => (iced::widget::canvas::event::Status::Ignored, None),
        }
    }
}

#[derive(Default)]
pub struct State {
    mouse_position: Point,
    scroll_position: f32,
    gutter_highlight: Option<usize>,
}

impl<'a, Message> Component<Message> for CodeViewer<'a, Message> {
    type State = State;

    type Event = Event;

    fn update(&mut self, state: &mut Self::State, event: Event) -> Option<Message> {
        match event {
            Event::MouseMoved(point) => {
                state.mouse_position = point;

                if point.x < GUTTER_WIDTH {
                    state.gutter_highlight =
                        Some(((point.y + state.scroll_position) / LINE_HEIGHT).floor() as _);
                } else {
                    self.gutter_highlight = None;
                }

                None
            }
            Event::CanvasClicked(mouse::Button::Left) => {
                if state.mouse_position.x < GUTTER_WIDTH {
                    let line_no = ((state.mouse_position.y + state.scroll_position) / LINE_HEIGHT)
                        .floor() as usize;
                    return Some((self.on_change)(CodeViewerAction::BreakpointChanged(
                        line_no,
                    )));
                }
                None
            }
            Event::CanvasClicked(_) => None,
            Event::OnScroll(viewport) => {
                let offset = viewport.absolute_offset();
                state.scroll_position = offset.y;
                None
            }
            Event::EditorActionPerformed(action) => match action {
                Action::Edit(_) => {
                    // override edit action to make nothing happen
                    None
                }
                Action::Scroll { lines } => {
                    // override scroll action to make sure we don't break things
                    state.scroll_position += (lines as f32) * LINE_HEIGHT;
                    // return iced::widget::scrollable::scroll_to(
                    //     self.scrollable_id.clone(),
                    //     scrollable::AbsoluteOffset {
                    //         x: 0.0,
                    //         y: self.scroll_position,
                    //     },
                    // );
                    None
                }
                action => {
                    let mut content = self.content.borrow_mut();
                    content.perform(action);
                    None
                } // text_editor::Action::Select(_) => todo!(),
                  // text_editor::Action::SelectWord => todo!(),
                  // text_editor::Action::SelectLine => todo!(),
                  // text_editor::Action::Drag(_) => todo!(),
                  // text_editor::Action::Scroll { lines } => todo!(),
            },
        }
    }

    fn view(&self, _state: &Self::State) -> iced::Element<'_, Event> {
        let render_breakpoints = RenderBreakpoints {
            breakpoints: self.breakpoints,
            gutter_highlight: self.gutter_highlight,
        };
        let gutter = iced::widget::canvas(render_breakpoints)
            .height(Length::Fill)
            .width(Length::Fixed(GUTTER_WIDTH));

        let editor = iced::widget::text_editor(&*self.content.borrow())
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
        .id(self.scrollable_id.clone())
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn move_and_click<Message>(
        code_view: &mut CodeViewer<'_, Message>,
        state: &mut State,
        position: Point,
    ) -> Option<Message> {
        code_view.update(state, Event::MouseMoved(position));
        code_view.update(state, Event::CanvasClicked(mouse::Button::Left))
    }

    #[test]
    fn add_breakpoints() {
        let mut content = Content::new();
        let breakpoints = HashSet::new();
        let scrollable_id = iced::widget::scrollable::Id::unique();

        enum TestMessage {
            Event(CodeViewerAction),
        }

        let mut code_view = CodeViewer::new(
            &mut content,
            &breakpoints,
            scrollable_id,
            None,
            TestMessage::Event,
        );

        // move the mouse to the gutter

        let mut state = State::default();

        let TestMessage::Event(CodeViewerAction::BreakpointChanged(bp)) =
            move_and_click(&mut code_view, &mut state, Point { x: 5.0, y: 93.6 }).unwrap();
        assert_eq!(bp, 4);

        assert!(move_and_click(&mut code_view, &mut state, Point { x: 100.0, y: 10.0 }).is_none());
    }
}
