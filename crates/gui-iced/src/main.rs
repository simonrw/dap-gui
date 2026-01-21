use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use color_eyre::eyre::{self, Context};
use debugger::Debugger;
use iced::widget::{button, row, text};
use iced::{Element, Task};

#[derive(Clone)]
struct State {
    debugger: Arc<Debugger>,
    is_running: Arc<AtomicBool>,
}

#[derive(Debug, Clone)]
enum Message {
    Continue,
    Continued,
}

fn boot() -> State {
    let debugger = Debugger::from_launch_configuration("launch.json", "Launch")
        .context("loading debugger from launch configuration")
        .unwrap();
    State {
        debugger: Arc::new(debugger),
        is_running: Arc::new(AtomicBool::new(true)),
    }
}

fn update(state: &mut State, message: Message) -> Task<Message> {
    let state = state.clone();
    match message {
        Message::Continue => {
            return Task::future(async move {
                // TODO: async debugger
                state
                    .debugger
                    .r#continue()
                    .expect("could not continue execution");
                Message::Continued
            });
        }
        Message::Continued => todo!(),
    }
}

fn view(state: &State) -> Element<'_, Message> {
    // control bar
    row![button(text("Continue")).on_press(Message::Continue),]
        .spacing(10)
        .padding(10)
        .into()
}

pub fn main() -> iced::Result {
    iced::application(boot, update, view).run()
}
