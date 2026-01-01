use std::collections::HashSet;

use iced::{
    Element, Length, Point, Size, mouse,
    widget::{
        Component, component, row, scrollable,
        scrollable::Viewport,
        text_editor::{Action, Content},
    },
};

mod render_breakpoints;

use render_breakpoints::RenderBreakpoints;

pub const LINE_HEIGHT: f32 = 20.8;
pub const OFFSET: u8 = 6;
pub const GUTTER_WIDTH: f32 = 16.0;

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
    EditorAction(Action),
    ScrollCommand { offset: scrollable::AbsoluteOffset },
}

pub struct CodeViewer<'a, Message> {
    content: &'a Content,
    breakpoints: &'a HashSet<usize>,
    scrollable_id: iced::widget::Id,
    on_change: Box<dyn Fn(CodeViewerAction) -> Message + 'static>,
}

impl<'a, Message> CodeViewer<'a, Message> {
    pub fn new(
        content: &'a Content,
        breakpoints: &'a HashSet<usize>,
        scrollable_id: iced::widget::Id,
        start_line: usize,
        on_change: impl Fn(CodeViewerAction) -> Message + 'static,
    ) -> Self {
        let on_change = Box::new(on_change);

        // emit scroll to event to scroll the current line into view
        (on_change)(CodeViewerAction::ScrollCommand {
            offset: scrollable::AbsoluteOffset {
                x: 0.0,
                y: (start_line as f32) / LINE_HEIGHT,
            },
        });

        Self {
            content,
            breakpoints,
            scrollable_id,
            on_change,
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

#[derive(Debug, Default)]
pub struct State {
    mouse_position: Point,
    scroll_position: f32,
    gutter_highlight: Option<usize>,
}

impl<Message> Component<Message> for CodeViewer<'_, Message> {
    type State = State;

    type Event = Event;

    #[tracing::instrument(skip(self))]
    fn update(&mut self, state: &mut Self::State, event: Event) -> Option<Message> {
        tracing::trace!("component event");
        match event {
            Event::MouseMoved(point) => {
                state.mouse_position = point;

                if point.x < GUTTER_WIDTH {
                    state.gutter_highlight =
                        Some(((point.y + state.scroll_position) / LINE_HEIGHT).floor() as _);
                } else {
                    state.gutter_highlight = None;
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
                // (self.on_change)(CodeViewerAction::ScrollCommand {
                //     scrollable_id: self.scrollable_id.clone(),
                //     offset: scrollable::AbsoluteOffset {
                //         x: 0.0,
                //         y: state.scroll_position,
                //     },
                // });
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
                    state.scroll_position = state.scroll_position.max(0.0);
                    return Some((self.on_change)(CodeViewerAction::ScrollCommand {
                        offset: scrollable::AbsoluteOffset {
                            x: 0.0,
                            y: state.scroll_position,
                        },
                    }));
                    // forward the event
                    // Some((self.on_change)(CodeViewerAction::EditorAction(action)))
                }
                action => Some((self.on_change)(CodeViewerAction::EditorAction(action))), // text_editor::Action::Select(_) => todo!(),
                                                                                          // text_editor::Action::SelectWord => todo!(),
                                                                                          // text_editor::Action::SelectLine => todo!(),
                                                                                          // text_editor::Action::Drag(_) => todo!(),
            },
        }
    }

    fn view(&self, state: &Self::State) -> iced::Element<'_, Event> {
        let render_breakpoints = RenderBreakpoints {
            breakpoints: self.breakpoints,
            gutter_highlight: state.gutter_highlight,
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
        .id(self.scrollable_id.clone())
        .into()
    }

    fn size_hint(&self) -> iced::Size<Length> {
        Size {
            width: Length::Fill,
            height: Length::Fill,
        }
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
        let content = Content::new();
        let breakpoints = HashSet::new();
        let scrollable_id = iced::widget::Id::unique();

        enum TestMessage {
            Event(CodeViewerAction),
        }

        let mut code_view =
            CodeViewer::new(&content, &breakpoints, scrollable_id, 0, TestMessage::Event);

        // move the mouse to the gutter

        let mut state = State::default();

        let TestMessage::Event(CodeViewerAction::BreakpointChanged(bp)) =
            move_and_click(&mut code_view, &mut state, Point { x: 5.0, y: 93.6 }).unwrap()
        else {
            unreachable!()
        };
        assert_eq!(bp, 4);

        assert!(move_and_click(&mut code_view, &mut state, Point { x: 100.0, y: 10.0 }).is_none());
    }
}
