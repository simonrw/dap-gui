use iced::widget::{button, Button};

use crate::message::Message;

pub(crate) fn launch_button<'a>() -> Button<'a, Message> {
    button("Launch").on_press(Message::Launch).padding(10)
}
