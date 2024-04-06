use dark_light::Mode;
use iced::alignment::{Horizontal, Vertical};
use iced::widget::{button, column, container, row, text, Container, Text};
use iced::{executor, Alignment, Application, Border, Color, Command, Element, Length, Settings};

#[derive(Debug, Clone)]
enum Message {
    Increment,
    Decrement,
}

fn title<'a, Message>(input: impl ToString) -> Container<'a, Message> {
    container(text(input).size(30)).padding(20)
}

struct Counter {
    value: i64,
}

impl Counter {
    // view helper methods
    fn view_call_stack(&self) -> iced::Element<'_, Message> {
        title("Call Stack")
            .height(Length::Fill)
            .width(Length::Fill)
            .into()
    }
}

impl Application for Counter {
    type Executor = executor::Default;
    type Theme = iced::Theme;
    type Flags = ();
    type Message = Message;

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let this = Self { value: 0 };
        (this, Command::none())
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::Increment => self.value += 1,
            Message::Decrement => self.value -= 1,
        }
        Command::none()
    }

    fn title(&self) -> String {
        "Counter".to_string()
    }

    fn view(&self) -> iced::Element<'_, Self::Message> {
        let sidebar = column![self.view_call_stack(),]
            .height(Length::Fill)
            .width(Length::Fill);

        let main_content = column![
            title("main content")
                .height(Length::Fill)
                .width(Length::Fill),
            title("bottom panel")
                .height(Length::Fixed(300.0))
                .width(Length::Fill),
        ]
        .height(Length::Fill);

        let container: Element<_> = row![
            sidebar.width(Length::Fixed(300.0)),
            main_content.width(Length::Fill),
        ]
        .into();
        container.explain(Color::from_rgb(0.7, 0.7, 0.7))

        // let c: iced::Element<_> = container(
        //     column![
        //         button("increment").on_press(Message::Increment),
        //         text(self.value).size(50),
        //         button("decrement").on_press(Message::Decrement),
        //     ]
        //     .padding(20)
        //     .align_items(Alignment::Center),
        // )
        // .width(Length::Fill)
        // .height(Length::Fill)
        // .align_x(Horizontal::Center)
        // .align_y(Vertical::Center)
        // .center_x()
        // .center_y()
        // .style(container::Appearance {
        //     border: Border {
        //         width: 2.0,
        //         color: Color::BLACK,
        //         ..Default::default()
        //     },
        //     ..Default::default()
        // })
        // .into();
        // c.explain(Color::BLACK)
    }

    fn theme(&self) -> Self::Theme {
        match dark_light::detect() {
            Mode::Dark | Mode::Default => iced::Theme::Dark,
            Mode::Light => iced::Theme::Light,
        }
    }
}

fn main() -> iced::Result {
    Counter::run(Settings::default())
}
