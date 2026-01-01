use std::collections::HashSet;

use debugger::Debugger;
use iced::widget::text_editor;
use transport::types::StackFrame;

use crate::{args::Args, message::TabId};

#[derive(Debug)]
pub enum Phase {
    #[allow(dead_code)]
    Initialising,
    #[allow(dead_code)]
    Running { breakpoints: HashSet<usize> },
    Paused {
        args: Args,
        active_tab: TabId,
        content: text_editor::Content,
        breakpoints: HashSet<usize>,
        scrollable_id: iced::widget::Id,
        stack: Vec<StackFrame>,
    },
    #[allow(dead_code)]
    Terminated,
}

pub struct AppState {
    pub phase: Phase,
    pub debugger: Debugger,
}

impl AppState {
    pub fn new(debugger: Debugger) -> Self {
        Self {
            debugger,
            phase: Phase::Running {
                breakpoints: HashSet::new(),
            },
        }
    }
}
