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
#[derive(Clone)]
pub struct PyStackFrame(StackFrame);

impl From<StackFrame> for PyStackFrame {
    fn from(value: StackFrame) -> Self {
        Self(value)
    }
}

#[pymethods]
impl PyStackFrame {
    #[getter]
    fn name(&self) -> String {
        self.0.name.clone()
    }

    #[getter]
    fn line(&self) -> usize {
        self.0.line
    }

    #[getter]
    fn source(&self) -> PyResult<PathBuf> {
        if let Some(path) = self.0.source.as_ref().and_then(|s| s.path.clone()) {
            return Ok(path);
        }

        todo!()
    }

    fn __repr__(&self) -> String {
        format!("{}:{}", self.name(), self.line())
    }
}

#[pyclass]
pub struct ProgramState {
    #[pyo3(get)]
    pub stack: Vec<PyStackFrame>,
    //#[pyo3(get)]
    //pub paused_frame: PausedFrame,
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

        tracing::debug!("waiting for debugee to run");
        self.internal_debugger
            .wait_for_event(|evt| matches!(evt, Event::Running { .. }));

        // wait for stopped or terminated event
        match self.internal_debugger.wait_for_event(|evt| {
            matches!(evt, Event::Paused { .. }) || matches!(evt, Event::Ended)
        }) {
            Event::Paused {
                stack,
                //paused_frame,
                ..
            } => {
                tracing::debug!("paused");
                Ok(Some(ProgramState {
                    stack: stack.into_iter().map(From::from).collect(),
                    //paused_frame,
                }))
            }
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

        debugger.wait_for_event(|e| matches!(e, debugger::Event::Initialised));

        if let Some(file_path) = file {
            let file_path = file_path
                .canonicalize()
                .map_err(|_| PyRuntimeError::new_err("invalid file path given"))?;
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
