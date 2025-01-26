use pyo3::{exceptions::PyRuntimeError, prelude::*};

use launch_configuration::{load_from_path, ChosenLaunchConfiguration, LaunchConfiguration};

#[pyclass]
pub struct PyChosenLaunchConfiguration {
    #[pyo3(get)]
    pub name: String,
}

impl From<ChosenLaunchConfiguration> for PyChosenLaunchConfiguration {
    fn from(value: ChosenLaunchConfiguration) -> Self {
        match value {
            ChosenLaunchConfiguration::Specific(launch_configuration) => match launch_configuration
            {
                LaunchConfiguration::Debugpy(debugpy) => Self { name: debugpy.name },
                LaunchConfiguration::LLDB(lldb) => Self { name: lldb.name },
            },
            _ => todo!("unhandled case for chosen launch configuration",),
        }
    }
}

#[pyfunction(name = "load_from_path")]
#[pyo3(signature = (path, name=None))]
pub fn py_load_from_path(
    path: String,
    name: Option<String>,
) -> PyResult<PyChosenLaunchConfiguration> {
    let result = load_from_path(name.as_ref(), &path).map_err(|e| {
        PyRuntimeError::new_err(format!(
            "Error loading configuration from path {}: {}",
            path, e
        ))
    })?;
    Ok(result.into())
}
