use iced::{
    Element,
    widget::{button, row, text},
};

#[derive(Debug, Default)]
struct State {
    value: i64,
}

#[derive(Clone, Copy)]
enum Message {
    Increment,
    Decrement,
}

fn update(state: &mut State, message: Message) {
    match message {
        Message::Increment => state.value += 1,
        Message::Decrement => state.value -= 1,
    }
}

fn view(state: &State) -> Element<'_, Message> {
    row![
        button(text("+".to_string())).on_press(Message::Increment),
        text(format!("{}", state.value)),
        button(text("-".to_string())).on_press(Message::Decrement),
    ]
    .padding(10)
    .spacing(10)
    .into()
}

fn main() -> iced::Result {
    iced::run(update, view)
}
