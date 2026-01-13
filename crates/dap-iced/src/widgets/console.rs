use iced::widget::{column, container, text};
use iced::{Color, Element};

use crate::message::Message;

/// Renders the console output panel (placeholder).
pub fn console_panel(_output: &[String]) -> Element<'static, Message> {
    container(column![
        text("Console").size(14),
        text("(placeholder)").color(Color::from_rgb(0.5, 0.5, 0.5)),
    ])
    .padding(10)
    .height(150)
    .into()
}
