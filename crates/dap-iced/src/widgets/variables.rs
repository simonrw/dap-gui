use iced::widget::{column, container, text};
use iced::{Color, Element};

use crate::message::Message;
use crate::state::Variable;

/// Renders the variables panel (placeholder).
pub fn variables_panel(_variables: &[Variable]) -> Element<'static, Message> {
    container(column![
        text("Variables").size(14),
        text("(placeholder)").color(Color::from_rgb(0.5, 0.5, 0.5)),
    ])
    .padding(10)
    .width(250)
    .into()
}
