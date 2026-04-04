use debugger::{Breakpoint, Event, ProgramState};

type StackFrameId = i64;

/// The state of the debugger as seen by a UI frontend.
///
/// This is a frontend-agnostic representation of the debugger lifecycle.
/// Use [`SessionState::apply`] to update it from incoming [`Event`]s.
#[derive(Clone, Debug)]
pub enum SessionState {
    /// Session exists but hasn't reached the first pause yet.
    Running,
    /// Execution is paused at a breakpoint or step.
    Paused {
        stack: Vec<dap_types::StackFrame>,
        paused_frame: Box<debugger::PausedFrame>,
        breakpoints: Vec<Breakpoint>,
    },
    /// The debugee has terminated.
    Terminated,
}

impl SessionState {
    /// Apply a debugger event, returning the new state.
    ///
    /// Events that don't affect the core state (Output, Thread, Error, etc.)
    /// leave the state unchanged — unlike a `From<Event>` conversion, this
    /// preserves the previous state.
    pub fn apply(self, event: &Event) -> Self {
        match event {
            Event::Initialised | Event::Running => SessionState::Running,
            Event::Paused(ProgramState {
                stack,
                paused_frame,
                breakpoints,
            })
            | Event::ScopeChange(ProgramState {
                stack,
                paused_frame,
                breakpoints,
            }) => SessionState::Paused {
                stack: stack.clone(),
                paused_frame: Box::new(paused_frame.clone()),
                breakpoints: breakpoints.clone(),
            },
            Event::Ended => SessionState::Terminated,
            // Uninitialised, Output, Thread, Error don't change core state
            _ => self,
        }
    }

    /// Update the current frame ID based on the current state.
    ///
    /// Returns `Some(frame_id)` when paused, `None` when running or terminated.
    pub fn current_frame_id(&self) -> Option<StackFrameId> {
        match self {
            SessionState::Paused { paused_frame, .. } => Some(paused_frame.frame.id),
            _ => None,
        }
    }

    /// Returns `true` if the state is [`SessionState::Paused`].
    pub fn is_paused(&self) -> bool {
        matches!(self, SessionState::Paused { .. })
    }

    /// Returns `true` if the state is [`SessionState::Running`].
    pub fn is_running(&self) -> bool {
        matches!(self, SessionState::Running)
    }

    /// Returns `true` if the state is [`SessionState::Terminated`].
    pub fn is_terminated(&self) -> bool {
        matches!(self, SessionState::Terminated)
    }
}
