use iced::widget::{column, container, row, text, Container};
use iced::{executor, Application, Color, Command, Element, Length, Settings};
use iced_aw::Tabs;

#[derive(Debug, Clone)]
enum Message {
    TabSelected(TabId),
}

fn title<'a, Message>(input: impl ToString) -> Container<'a, Message> {
    container(text(input).size(30)).padding(20)
}

#[derive(Clone, PartialEq, Eq, Debug)]
enum TabId {
    Variables,
    Repl,
}

struct DebuggerApp {
    active_tab: TabId,
}

impl DebuggerApp {
    // view helper methods
    fn view_call_stack(&self) -> iced::Element<'_, Message> {
        title("Call Stack").width(Length::Fill).into()
    }

    fn view_breakpoints(&self) -> iced::Element<'_, Message> {
        title("Breakpoints").width(Length::Fill).into()
    }

    fn view_main_content(&self) -> iced::Element<'_, Message> {
        title("main content")
            .height(Length::Fill)
            .width(Length::Fill)
            .into()
    }

    fn view_bottom_panel(&self) -> iced::Element<'_, Message> {
        Tabs::new(Message::TabSelected)
            .tab_icon_position(iced_aw::tabs::Position::Top)
            .push(
                TabId::Variables,
                iced_aw::TabLabel::Text("Variables".to_string()),
                text("variables"),
            )
            .push(
                TabId::Repl,
                iced_aw::TabLabel::Text("Repl".to_string()),
                text("repl"),
            )
            .set_active_tab(&self.active_tab)
            .into()
    }
}

impl Application for DebuggerApp {
    type Executor = executor::Default;
    type Theme = iced::Theme;
    type Flags = ();
    type Message = Message;

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let this = Self {
            active_tab: TabId::Variables,
        };
        (this, Command::none())
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::TabSelected(selected) => self.active_tab = selected,
        }
        Command::none()
    }

    fn title(&self) -> String {
        "DebuggerApp".to_string()
    }

    fn view(&self) -> iced::Element<'_, Self::Message> {
        let sidebar = column![self.view_call_stack(), self.view_breakpoints(),]
            .height(Length::Fill)
            .width(Length::Fill);

        let main_content =
            column![self.view_main_content(), self.view_bottom_panel(),].height(Length::Fill);

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
        iced::Theme::Dark
        // match dark_light::detect() {
        //     Mode::Dark | Mode::Default => iced::Theme::Dark,
        //     Mode::Light => iced::Theme::Light,
        // }
    }
}

fn main() -> iced::Result {
    DebuggerApp::run(Settings::default())
}
