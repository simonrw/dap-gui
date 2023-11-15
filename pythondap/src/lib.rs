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

        let (dtx, drx) = spmc::channel();
        let background_events = Arc::clone(&events);
        std::thread::spawn(move || loop {
            let msg = drx.recv().unwrap();
            background_events.lock().unwrap().push(msg);
        });

        let debugger = debugger::Debugger::new(client, rx, dtx)
            .map_err(|e| PyRuntimeError::new_err(format!("creating debugger: {e}")))?;

        if let Some(file_path) = file {
            debugger
                .initialise(debugger::LaunchArguments {
                    program: file_path.clone(),
                    working_directory: None,
                    language: debugger::Language::DebugPy,
                })
                .map_err(|e| PyRuntimeError::new_err(format!("initialising debugger: {e}")))?;
        }

        Ok(Self {
            internal: debugger,
            events,
        })
    }

    fn resume(&mut self) -> PyResult<()> {
        self.internal
            .r#continue()
            .map_err(|e| PyRuntimeError::new_err(format!("continuing execution: {e}")))?;

        // wait for stopped event 
        Ok(())
    }
}

#[pymodule]
fn pythondap(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<Debugger>()?;
    Ok(())
}
