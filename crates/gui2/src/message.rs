use debugger::Event;
use iced::window;
use transport::types::StackFrameId;

use crate::code_view::CodeViewerAction;

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TabId {
    Variables,
    Repl,
}

#[derive(Debug, Clone)]
pub enum Message {
    TabSelected(TabId),
    CodeViewer(CodeViewerAction),
    DebuggerMessage(Box<Event>),
    Window(window::Id, window::Event),
    StackFrameChanged(StackFrameId),
    Quit,
}
