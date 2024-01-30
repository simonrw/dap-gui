use debugger::{AttachArguments, Event};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::env::current_dir;
use std::path::PathBuf;
use transport::types::StackFrame;

#[pyclass]
struct ProgramState {
    _stack: Vec<StackFrame>,
}

#[pymethods]
impl ProgramState {
    fn __getattr__(&self, py: Python, name: PyObject) -> PyResult<impl IntoPy<PyObject>> {
        let name: String = name.extract(py)?;
        Ok(name)
    }
}

#[pyclass]
struct Debugger {
    _internal: debugger::Debugger,
    _launched: bool,
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
}

impl Debugger {
    fn internal_new(
        port: Option<u16>,
        breakpoints: Vec<usize>,
        file: Option<PathBuf>,
    ) -> PyResult<Self> {
        let port = dbg!(port.unwrap_or(5678));

        let args = AttachArguments {
            working_directory: current_dir().unwrap(),
            port: Some(port),
            language: debugger::Language::DebugPy,
        };
        let debugger = debugger::Debugger::new(args)
            .map_err(|e| PyRuntimeError::new_err(format!("creating debugger: {e}")))?;

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
            _internal: debugger,
            _launched: false,
        })
    }

    pub fn resume(&mut self) -> PyResult<Option<ProgramState>> {
        if !self._launched {
            self._launched = true;
            self._internal
                .launch()
                .map_err(|e| PyRuntimeError::new_err(format!("launching debugger: {e}")))?;
        } else {
            self._internal
                .r#continue()
                .map_err(|e| PyRuntimeError::new_err(format!("continuing execution: {e}")))?;
        }

        // wait for stopped or terminated event
        match self._internal.wait_for_event(|evt| {
            matches!(evt, Event::Paused { .. }) || matches!(evt, Event::Ended)
        }) {
            Event::Paused {
                stack,
                paused_frame,
                ..
            } => Ok(Some(ProgramState { _stack: stack })),
            Event::Ended => {
                eprintln!("Debugee ended");
                return Ok(None);
            }
            _ => unreachable!(),
        }
    }
}

#[pymodule]
fn pythondap(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<Debugger>()?;
    m.add_class::<ProgramState>()?;
    Ok(())
}
