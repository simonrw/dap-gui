use std::collections::HashSet;

use iced::{
    Element, Length, Point, mouse,
    widget::{
        row, scrollable,
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
pub enum CodeViewerAction {
    BreakpointChanged(usize),
    EditorAction(Action),
    ScrollCommand { offset: scrollable::AbsoluteOffset },
    MouseMoved(Point),
    Scrolled(Viewport),
    CanvasClicked(mouse::Button),
}

pub fn code_viewer<'a, Message>(
    content: &'a Content,
    breakpoints: &'a HashSet<usize>,
    mouse_position: Point,
    scroll_position: f32,
    scrollable_id: iced::widget::Id,
    on_action: impl Fn(CodeViewerAction) -> Message + 'a + Clone,
) -> Element<'a, Message>
where
    Message: 'a + Clone,
{
    // Compute gutter highlight from mouse position
    let gutter_highlight = if mouse_position.x < GUTTER_WIDTH {
        Some(((mouse_position.y + scroll_position) / LINE_HEIGHT).floor() as usize)
    } else {
        None
    };

    let render_breakpoints = RenderBreakpoints {
        breakpoints,
        gutter_highlight,
    };

    let on_action_clone1 = on_action.clone();
    let on_action_clone2 = on_action.clone();

    let gutter: Element<'_, CodeViewerAction> = iced::widget::canvas(render_breakpoints)
        .height(Length::Fill)
        .width(Length::Fixed(GUTTER_WIDTH))
        .into();
    let gutter: Element<'_, Message> = gutter.map(move |action| on_action(action));

    let editor = iced::widget::text_editor(content)
        .padding(16)
        .height(Length::Fill)
        .on_action(move |action| match action {
            Action::Edit(_) => {
                // Don't allow editing
                on_action_clone1(CodeViewerAction::EditorAction(action))
            }
            Action::Scroll { lines } => {
                // Handle scroll via text editor
                let new_scroll = (scroll_position + (lines as f32) * LINE_HEIGHT).max(0.0);
                on_action_clone1(CodeViewerAction::ScrollCommand {
                    offset: scrollable::AbsoluteOffset {
                        x: 0.0,
                        y: new_scroll,
                    },
                })
            }
            action => on_action_clone1(CodeViewerAction::EditorAction(action)),
        });

    scrollable(
        row![gutter, editor]
            .width(Length::Fill)
            .height(Length::Fill),
    )
    .height(Length::Fill)
    .width(Length::Fill)
    .on_scroll(move |viewport| on_action_clone2(CodeViewerAction::Scrolled(viewport)))
    .id(scrollable_id)
    .into()
}
