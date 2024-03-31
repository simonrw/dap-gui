use iced::widget::{button, column, text};
use iced::{Alignment, Sandbox, Settings};

struct Counter {
    value: i64,
}

impl Sandbox for Counter {
    fn update(&mut self, message: Message) {
        match message {
            Message::Increment => self.value += 1,
            Message::Decrement => self.value -= 1,
        }
    }

    type Message = Message;

    fn new() -> Self {
        Self { value: 0 }
    }

    fn title(&self) -> String {
        "Counter".to_string()
    }

    fn view(&self) -> iced::Element<'_, Self::Message> {
        column![
            button("increment").on_press(Message::Increment),
            text(self.value).size(50),
            button("decrement").on_press(Message::Decrement),
        ]
        .padding(20)
        .align_items(Alignment::Center)
        .into()
    }
}

#[derive(Debug, Clone)]
enum Message {
    Increment,
    Decrement,
}

fn main() -> iced::Result {
    Counter::run(Settings::default())
}
