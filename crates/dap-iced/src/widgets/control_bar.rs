use iced::Element;
use iced::widget::{button, row, text};

use crate::message::Message;

/// Renders the debug control bar with Continue, Step, and Stop buttons.
pub fn control_bar(is_running: bool, connected: bool) -> Element<'static, Message> {
    let can_step = connected && !is_running;

    row![
        button(text("Continue")).on_press_maybe(can_step.then_some(Message::Continue)),
        button(text("Step Over")).on_press_maybe(can_step.then_some(Message::StepOver)),
        button(text("Step In")).on_press_maybe(can_step.then_some(Message::StepIn)),
        button(text("Step Out")).on_press_maybe(can_step.then_some(Message::StepOut)),
        button(text("Stop")).on_press_maybe(connected.then_some(Message::Stop)),
    ]
    .spacing(10)
    .padding(10)
    .into()
}
