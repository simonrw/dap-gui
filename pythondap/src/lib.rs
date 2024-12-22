use crossbeam_channel::Receiver;
use debugger::{AttachArguments, Event};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::env::current_dir;
use std::path::PathBuf;
use transport::types::StackFrame;

#[pyclass]
pub struct Breakpoint {
    pub line: usize,
    pub file: String,
}

#[pymethods]
impl Breakpoint {
    fn __repr__(&self) -> String {
        format!("'{}:{}'", self.file, self.line)
    }
}

impl From<debugger::Breakpoint> for Breakpoint {
    fn from(value: debugger::Breakpoint) -> Self {
        Self {
            line: value.line,
            file: format!("{}", value.path.display()),
        }
    }
}

#[pyclass]
struct ProgramState {
    _stack: Vec<StackFrame>,
}

#[pymethods]
impl ProgramState {
    fn __getattr__(&self, name: &Bound<'_, PyAny>) -> PyResult<String> {
        let name: String = name.extract()?;
        Ok(name)
    }
}

#[pyclass]
struct Debugger {
    internal_debugger: debugger::Debugger,
    launched: bool,
    events: Receiver<Event>,
}

#[pymethods]
impl Debugger {
    #[new]
    #[pyo3(signature = (/, breakpoints, file=None))]
    pub fn new(breakpoints: Vec<usize>, file: Option<PathBuf>) -> PyResult<Self> {
        Self::internal_new(None, breakpoints, file)
    }

    #[staticmethod]
    #[pyo3(signature = (/, port, breakpoints, file=None))]
    pub fn new_on_port(
        port: u16,
        breakpoints: Vec<usize>,
        file: Option<PathBuf>,
    ) -> PyResult<Self> {
        Self::internal_new(Some(port), breakpoints, file)
    }

    pub fn resume(&mut self) -> PyResult<Option<ProgramState>> {
        if !self.launched {
            self.launched = true;
            self.internal_debugger
                .start()
                .map_err(|e| PyRuntimeError::new_err(format!("launching debugger: {e}")))?;
        } else {
            self.internal_debugger
                .r#continue()
                .map_err(|e| PyRuntimeError::new_err(format!("continuing execution: {e}")))?;
        }


        tracing;:
        self.internal_debugger.wait_for_event(|evt| matches!(evt, Event::Running { .. }));

        // wait for stopped or terminated event
        match self.internal_debugger.wait_for_event(|evt| {
            matches!(evt, Event::Paused { .. }) || matches!(evt, Event::Ended)
        }) {
            Event::Paused { stack, .. } => {
                tracing::debug!("paused");
                Ok(Some(ProgramState { _stack: stack }))
            },
            Event::Ended => {
                eprintln!("Debugee ended");
                Ok(None)
            }
            _ => unreachable!(),
        }
    }

    // /// List the breakpoints the debugger knows about
    pub fn breakpoints(&mut self) -> Vec<Breakpoint> {
        let debugger_breakpoints = self.internal_debugger.breakpoints();
        debugger_breakpoints.into_iter().map(From::from).collect()
    }
}

impl Debugger {
    fn internal_new(
        port: Option<u16>,
        breakpoints: Vec<usize>,
        file: Option<PathBuf>,
    ) -> PyResult<Self> {
        let port = port.unwrap_or(5678);

        let args = AttachArguments {
            working_directory: current_dir().unwrap(),
            port: Some(port),
            language: debugger::Language::DebugPy,
            path_mappings: None,
        };
        let debugger = debugger::Debugger::on_port(port, args)
            .map_err(|e| PyRuntimeError::new_err(format!("creating debugger: {e}")))?;
        let drx = debugger.events();

        debugger.wait_for_event(|e| matches!(e, debugger::Event::Initialised));

        if let Some(file_path) = file {
            // breakpoints
            for &line in &breakpoints {
                let breakpoint = debugger::Breakpoint {
                    name: None,
                    path: file_path.clone(),
                    line,
                };
                debugger
                    .add_breakpoint(&breakpoint)
                    .map_err(|_| PyRuntimeError::new_err("adding breakpoint"))?;
            }
        }

        Ok(Self {
            internal_debugger: debugger,
            launched: false,
            events: drx,
        })
    }
}

#[pymodule]
fn pythondap(m: &Bound<'_, PyModule>) -> PyResult<()> {
    tracing_subscriber::fmt::init();

    tracing::info!("info");

    m.add_class::<Debugger>()?;
    m.add_class::<ProgramState>()?;
    Ok(())
}
