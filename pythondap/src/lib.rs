use debugger::{AttachArguments, Event, PausedFrame};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::collections::HashMap;
use std::env::current_dir;
use std::path::PathBuf;
use transport::types::StackFrame;
use tree_sitter::{Parser, Point};

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

#[pyclass(name = "PausedFrame")]
#[derive(Clone)]
pub struct PyPausedFrame(PausedFrame);

impl From<PausedFrame> for PyPausedFrame {
    fn from(value: PausedFrame) -> Self {
        Self(value)
    }
}

#[pymethods]
impl PyPausedFrame {
    #[getter]
    fn variables(&self) -> HashMap<String, PyVariable> {
        self.0
            .variables
            .iter()
            .cloned()
            .map(|v| (v.name.clone(), v.into()))
            .collect()
    }

    #[getter]
    fn stack(&self) -> PyStackFrame {
        self.0.frame.clone().into()
    }
}

#[pyclass(name = "Variable")]
#[derive(Clone)]
pub struct PyVariable(transport::types::Variable);

#[pymethods]
impl PyVariable {
    #[getter]
    fn name(&self) -> String {
        self.0.name.clone()
    }

    #[getter]
    fn value(&self) -> String {
        self.0.value.clone()
    }

    #[getter]
    fn r#type(&self) -> Option<String> {
        self.0.r#type.clone()
    }

    fn __repr__(&self) -> String {
        match &self.0.r#type {
            Some(ty) => {
                format!("<Variable {}={} ({})", self.0.name, self.0.value, ty)
            }
            None => {
                format!("<Variable {}={} (???)", self.0.name, self.0.value)
            }
        }
    }
}

impl From<transport::types::Variable> for PyVariable {
    fn from(value: transport::types::Variable) -> Self {
        Self(value)
    }
}

#[pyclass]
pub struct ProgramState {
    #[pyo3(get)]
    pub stack: Vec<PyStackFrame>,
    #[pyo3(get)]
    pub paused_frame: PyPausedFrame,
}

#[pymethods]
impl ProgramState {
    fn __getattr__(&self, name: &Bound<'_, PyAny>) -> PyResult<String> {
        let name: String = name.extract()?;
        Ok(name)
    }

    /// Show the source code around the current execution position
    fn show(&self) -> PyResult<()> {
        let source = self.paused_frame.stack().source()?;
        let line = self.paused_frame.stack().line();

        let contents = std::fs::read_to_string(&source)
            .map_err(|e| PyRuntimeError::new_err(format!("error reading from file {}", e)))?;
        let line_text = contents.split('\n').skip(line - 1).next().unwrap();
        let start = Point {
            row: line - 1,
            column: 0,
        };
        let end = Point {
            row: line - 1,
            column: line_text.len(),
        };

        // set up treesitter
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
            .map_err(|e| PyRuntimeError::new_err(format!("setting treesitter language: {e}")))?;
        let tree = parser
            .parse(contents.as_bytes(), None)
            .ok_or_else(|| PyRuntimeError::new_err("error parsing file".to_string()))?;
        let root = tree.root_node();
        let descendant = root
            .descendant_for_point_range(start, end)
            .ok_or_else(|| PyRuntimeError::new_err("getting descendant"))?;

        // find up until function body
        let mut n = descendant;

        loop {
            tracing::debug!(node = ?n, "loop iteration");
            if n.kind() == "function_definition" {
                let s = n.utf8_text(contents.as_bytes()).map_err(|e| {
                    PyRuntimeError::new_err(format!(
                        "error getting utf8 text from input source: {e}"
                    ))
                })?;

                println!("{s}");

                return Ok(());
            }

            match n.parent() {
                Some(parent) => {
                    n = parent;
                }
                None => {
                    return Err(PyRuntimeError::new_err(
                        "no function body found".to_string(),
                    ))
                }
            }
        }
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
                paused_frame,
                ..
            } => {
                tracing::debug!("paused");
                Ok(Some(ProgramState {
                    stack: stack.into_iter().map(From::from).collect(),
                    paused_frame: paused_frame.into(),
                }))
            }
            Event::Ended => {
                eprintln!("Debugee ended");
                Ok(None)
            }
            _ => unreachable!(),
        }
    }

    pub fn step_over(&mut self) -> PyResult<Option<ProgramState>> {
        self.internal_debugger
            .step_over()
            .map_err(|e| PyRuntimeError::new_err(format!("stepping debugee: {e}")))?;
        match self.internal_debugger.wait_for_event(|evt| {
            matches!(evt, Event::Paused { .. }) || matches!(evt, Event::Ended)
        }) {
            Event::Paused {
                stack,
                paused_frame,
                ..
            } => {
                tracing::debug!("paused");
                Ok(Some(ProgramState {
                    stack: stack.into_iter().map(From::from).collect(),
                    paused_frame: paused_frame.into(),
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
    m.add_class::<PyPausedFrame>()?;
    Ok(())
}
