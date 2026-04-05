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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Build a minimal ProgramState for testing.
    fn test_program_state() -> ProgramState {
        let frame = test_stack_frame(1, "main", 10);
        ProgramState {
            stack: vec![frame.clone()],
            paused_frame: debugger::PausedFrame {
                frame,
                variables: vec![],
            },
            breakpoints: vec![Breakpoint {
                name: None,
                path: PathBuf::from("/project/test.py"),
                line: 10,
            }],
        }
    }

    fn test_stack_frame(id: i64, name: &str, line: usize) -> dap_types::StackFrame {
        dap_types::StackFrame {
            id,
            name: name.to_string(),
            line,
            column: 0,
            source: Some(dap_types::Source {
                name: Some("test.py".to_string()),
                path: Some(PathBuf::from("/project/test.py")),
                ..Default::default()
            }),
            can_restart: None,
            end_column: None,
            end_line: None,
            instruction_pointer_reference: None,
            module_id: None,
            presentation_hint: None,
        }
    }

    // ── State transitions ────────────────────────────────────────────

    #[test]
    fn running_to_paused_on_paused_event() {
        let state = SessionState::Running;
        let ps = test_program_state();
        let new = state.apply(&Event::Paused(ps));
        assert!(new.is_paused());
    }

    #[test]
    fn running_to_paused_on_scope_change_event() {
        let state = SessionState::Running;
        let ps = test_program_state();
        let new = state.apply(&Event::ScopeChange(ps));
        assert!(new.is_paused());
    }

    #[test]
    fn running_to_terminated_on_ended() {
        let state = SessionState::Running;
        let new = state.apply(&Event::Ended);
        assert!(new.is_terminated());
    }

    #[test]
    fn paused_to_running_on_running_event() {
        let ps = test_program_state();
        let state = SessionState::Paused {
            stack: ps.stack.clone(),
            paused_frame: Box::new(ps.paused_frame.clone()),
            breakpoints: ps.breakpoints.clone(),
        };
        let new = state.apply(&Event::Running);
        assert!(new.is_running());
    }

    #[test]
    fn paused_to_terminated_on_ended() {
        let ps = test_program_state();
        let state = SessionState::Paused {
            stack: ps.stack.clone(),
            paused_frame: Box::new(ps.paused_frame.clone()),
            breakpoints: ps.breakpoints.clone(),
        };
        let new = state.apply(&Event::Ended);
        assert!(new.is_terminated());
    }

    #[test]
    fn terminated_to_running_on_initialised() {
        let state = SessionState::Terminated;
        let new = state.apply(&Event::Initialised);
        assert!(new.is_running());
    }

    #[test]
    fn terminated_to_running_on_running_event() {
        let state = SessionState::Terminated;
        let new = state.apply(&Event::Running);
        assert!(new.is_running());
    }

    // ── Non-transitioning events ─────────────────────────────────────

    #[test]
    fn output_event_preserves_running_state() {
        let state = SessionState::Running;
        let new = state.apply(&Event::Output {
            category: "stdout".to_string(),
            output: "hello\n".to_string(),
        });
        assert!(new.is_running());
    }

    #[test]
    fn thread_event_preserves_running_state() {
        let state = SessionState::Running;
        let new = state.apply(&Event::Thread {
            reason: "started".to_string(),
            thread_id: 1,
        });
        assert!(new.is_running());
    }

    #[test]
    fn error_event_preserves_paused_state() {
        let ps = test_program_state();
        let state = SessionState::Paused {
            stack: ps.stack.clone(),
            paused_frame: Box::new(ps.paused_frame.clone()),
            breakpoints: ps.breakpoints.clone(),
        };
        let new = state.apply(&Event::Error("something failed".to_string()));
        assert!(new.is_paused());
    }

    #[test]
    fn uninitialised_event_preserves_state() {
        let state = SessionState::Running;
        let new = state.apply(&Event::Uninitialised);
        assert!(new.is_running());
    }

    // ── Helper methods ───────────────────────────────────────────────

    #[test]
    fn current_frame_id_when_paused() {
        let ps = test_program_state();
        let state = SessionState::Paused {
            stack: ps.stack,
            paused_frame: Box::new(ps.paused_frame),
            breakpoints: ps.breakpoints,
        };
        assert_eq!(state.current_frame_id(), Some(1));
    }

    #[test]
    fn current_frame_id_when_running() {
        assert_eq!(SessionState::Running.current_frame_id(), None);
    }

    #[test]
    fn current_frame_id_when_terminated() {
        assert_eq!(SessionState::Terminated.current_frame_id(), None);
    }

    #[test]
    fn boolean_helpers() {
        assert!(SessionState::Running.is_running());
        assert!(!SessionState::Running.is_paused());
        assert!(!SessionState::Running.is_terminated());

        assert!(SessionState::Terminated.is_terminated());
        assert!(!SessionState::Terminated.is_running());
        assert!(!SessionState::Terminated.is_paused());
    }

    // ── Paused state captures correct data ───────────────────────────

    #[test]
    fn paused_state_captures_stack_and_breakpoints() {
        let state = SessionState::Running;
        let ps = test_program_state();
        let new = state.apply(&Event::Paused(ps));

        if let SessionState::Paused {
            stack,
            paused_frame,
            breakpoints,
        } = new
        {
            assert_eq!(stack.len(), 1);
            assert_eq!(stack[0].name, "main");
            assert_eq!(paused_frame.frame.line, 10);
            assert_eq!(breakpoints.len(), 1);
            assert_eq!(breakpoints[0].line, 10);
        } else {
            panic!("expected Paused state");
        }
    }
}
