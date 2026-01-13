use iced::widget::{column, container, text};
use iced::{Color, Element};

use crate::message::Message;
use crate::state::StackFrame;

/// Renders the call stack panel (placeholder).
pub fn call_stack_panel(_frames: &[StackFrame]) -> Element<'static, Message> {
    container(column![
        text("Call Stack").size(14),
        text("(placeholder)").color(Color::from_rgb(0.5, 0.5, 0.5)),
    ])
    .padding(10)
    .width(200)
    .into()
}
