use debugger::{Event, FileSource};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use transport::types::StackFrame;

#[pyclass]
struct ProgramState {
    _stack: Vec<StackFrame>,
    _source: FileSource,
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
    internal: debugger::Debugger,
    _events: Arc<Mutex<Vec<Event>>>,
    launched: bool,
}

#[pymethods]
impl Debugger {
    #[new]
    #[pyo3(signature = (/, breakpoints, file=None))]
    fn new(breakpoints: Vec<usize>, file: Option<PathBuf>) -> PyResult<Self> {
        // TODO: start server
        let (tx, rx) = spmc::channel();
        let stream = TcpStream::connect("127.0.0.1:5678")
            .map_err(|e| PyRuntimeError::new_err(format!("connecting to DAP server: {e}")))?;
        let client = transport::Client::new(stream, tx)
            .map_err(|e| PyRuntimeError::new_err(format!("creating transport client: {e}")))?;

        let events = Arc::new(Mutex::new(Vec::new()));

        let debugger = debugger::Debugger::new(client, rx)
            .map_err(|e| PyRuntimeError::new_err(format!("creating debugger: {e}")))?;

        if let Some(file_path) = file {
            debugger
                .initialise(debugger::LaunchArguments {
                    program: file_path.clone(),
                    working_directory: None,
                    language: debugger::Language::DebugPy,
                })
                .map_err(|e| PyRuntimeError::new_err(format!("initialising debugger: {e}")))?;

            // breakpoints
            for &line in &breakpoints {
                let breakpoint = debugger::Breakpoint {
                    name: None,
                    path: file_path.clone(),
                    line,
                };
                debugger.add_breakpoint(breakpoint);
            }
        }

        Ok(Self {
            internal: debugger,
            _events: events,
            launched: false,
        })
    }

    fn resume(&mut self) -> PyResult<Option<ProgramState>> {
        if !self.launched {
            self.launched = true;
            self.internal
                .launch()
                .map_err(|e| PyRuntimeError::new_err(format!("launching debugger: {e}")))?;
        } else {
            self.internal
                .r#continue()
                .map_err(|e| PyRuntimeError::new_err(format!("continuing execution: {e}")))?;
        }

        // wait for stopped or terminated event
        match self.internal.wait_for_event(|evt| {
            matches!(evt, Event::Paused { .. }) || matches!(evt, Event::Ended)
        }) {
            Event::Paused { stack, source } => Ok(Some(ProgramState {
                _stack: stack,
                _source: source,
            })),
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
