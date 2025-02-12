use debugger::{AttachArguments, Event, LaunchArguments, PausedFrame};
use launch_configuration::{ChosenLaunchConfiguration, LaunchConfiguration};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use std::path::PathBuf;
use std::{collections::HashMap, path::Path};
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
        todo!()
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
    /// Show the source code around the current execution position
    fn show(&self) -> PyResult<()> {
        let source = self.paused_frame.stack().source()?;
        let line = self.paused_frame.stack().line();

        let contents = std::fs::read_to_string(&source)
            .map_err(|e| PyRuntimeError::new_err(format!("error reading from file {}", e)))?;
        let line_text = contents.split('\n').nth(line - 1).unwrap();
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
pub(crate) struct Debugger {
    internal_debugger: debugger::Debugger,
    launched: bool,
}

#[pymethods]
impl Debugger {
    #[new]
    #[pyo3(signature = (/, breakpoints, config_path, config_name=None, file=None, program=None))]
    pub fn new(
        breakpoints: Vec<usize>,
        config_path: PathBuf,
        config_name: Option<String>,
        file: Option<PathBuf>,
        program: Option<PathBuf>,
    ) -> PyResult<Self> {
        Self::internal_new(None, breakpoints, config_path, config_name, file, program)
    }

    #[staticmethod]
    #[pyo3(signature = (/, port, breakpoints, config_path, config_name=None, file=None, program=None))]
    pub fn new_on_port(
        port: u16,
        breakpoints: Vec<usize>,
        config_path: PathBuf,
        config_name: Option<String>,
        file: Option<PathBuf>,
        program: Option<PathBuf>,
    ) -> PyResult<Self> {
        Self::internal_new(
            Some(port),
            breakpoints,
            config_path,
            config_name,
            file,
            program,
        )
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
        tracing::trace!("waiting for paused or ended event");
        match self.internal_debugger.wait_for_event(|evt| {
            matches!(evt, Event::Paused { .. }) || matches!(evt, Event::Ended)
        }) {
            Event::Paused(debugger::ProgramState {
                stack,
                paused_frame,
                ..
            }) => {
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
        tracing::trace!("waiting for paused or ended event");
        match self.internal_debugger.wait_for_event(|evt| {
            matches!(evt, Event::Paused { .. }) || matches!(evt, Event::Ended)
        }) {
            Event::Paused(debugger::ProgramState {
                stack,
                paused_frame,
                ..
            }) => {
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
        config_path: impl AsRef<Path>,
        config_name: Option<String>,
        file: Option<PathBuf>,
        program: Option<PathBuf>,
    ) -> PyResult<Self> {
        let port = port.unwrap_or(5678);
        tracing::debug!(%port, "creating Python debugger");

        let config_path = config_path.as_ref();
        let mut config =
            match launch_configuration::load_from_path(config_name.as_ref(), config_path).map_err(
                |e| PyRuntimeError::new_err(format!("loading launch configuration: {e}")),
            )? {
                ChosenLaunchConfiguration::Specific(config) => config,
                ChosenLaunchConfiguration::NotFound => {
                    return Err(PyRuntimeError::new_err("no matching configuration found"));
                }
                ChosenLaunchConfiguration::ToBeChosen(configurations) => {
                    eprintln!("Configuration name not specified");
                    eprintln!("Available options:");
                    for config in &configurations {
                        eprintln!("- {config}");
                    }
                    // TODO: best option?
                    std::process::exit(1);
                }
            };
        tracing::debug!(config = ?config, "chosen config");
        let root = config_path
            .parent()
            .expect("getting parent for config path");
        config.resolve(root);

        let mut debug_root_dir = std::env::current_dir().unwrap();

        let debugger = match config {
            LaunchConfiguration::Debugpy(launch_configuration::Debugpy {
                request,
                cwd,
                connect,
                path_mappings,
                ..
            }) => {
                if let Some(dir) = cwd {
                    debug_root_dir = debugger::utils::normalise_path(&dir).into_owned();
                }
                let debugger = match request.as_str() {
                    "attach" => {
                        let launch_arguments = AttachArguments {
                            working_directory: debug_root_dir.to_owned().to_path_buf(),
                            port: connect.map(|c| c.port),
                            language: debugger::Language::DebugPy,
                            path_mappings,
                        };

                        tracing::debug!(?launch_arguments, "generated launch configuration");

                        debugger::Debugger::on_port(port, launch_arguments).map_err(|e| {
                            PyRuntimeError::new_err(format!("creating internal debugger: {e}"))
                        })?
                    }
                    "launch" => {
                        let launch_arguments = LaunchArguments {
                            program: program.ok_or_else(|| {
                                PyRuntimeError::new_err("program is a required argument")
                            })?,
                            working_directory: Some(debug_root_dir.to_owned().to_path_buf()),
                            language: debugger::Language::DebugPy,
                        };

                        tracing::debug!(?launch_arguments, "generated launch configuration");
                        debugger::Debugger::on_port(port, launch_arguments).map_err(|e| {
                            PyRuntimeError::new_err(format!("creating internal debugger: {e}"))
                        })?
                    }
                    other => todo!("Configuration type: '{other}' not implemented yet, or invalid"),
                };
                debugger
            }
            other => todo!("{other:?}"),
        };

        tracing::trace!("waiting for initialised event");
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
