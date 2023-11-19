use debugger::Event;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[pyclass]
struct Debugger {
    internal: debugger::Debugger,
    events: Arc<Mutex<Vec<Event>>>,
    launched: bool,
}

#[pymethods]
impl Debugger {
    #[new]
    #[pyo3(signature = (/, breakpoints, file=None))]
    fn new(breakpoints: Vec<usize>, file: Option<PathBuf>) -> PyResult<Self> {
        // TODO: start server
        let (tx, rx) = spmc::channel();
        let stream = TcpStream::connect(format!("127.0.0.1:5678"))
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
            events,
            launched: false,
        })
    }

    fn resume(&mut self) -> PyResult<()> {
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

        // wait for stopped event
        let Event::Paused { stack, source } = self
            .internal
            .wait_for_event(|evt| matches!(evt, Event::Paused { .. }))
        else {
            unreachable!()
        };
        eprintln!("Stopped {stack:?} {source:?}");
        Ok(())
    }
}

#[pymodule]
fn pythondap(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<Debugger>()?;
    Ok(())
}
