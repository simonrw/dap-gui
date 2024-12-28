use crate::debugger::{Debugger, ProgramState};
use launch_configuration::py_load_from_path;
use pyo3::prelude::*;

mod debugger;
mod launch_configuration;

#[pymodule]
fn pythondap(m: &Bound<'_, PyModule>) -> PyResult<()> {
    tracing_subscriber::fmt::init();

    // debugger
    m.add_class::<Debugger>()?;
    m.add_class::<ProgramState>()?;
    m.add_class::<debugger::PyPausedFrame>()?;

    // launch_configuration
    m.add_function(wrap_pyfunction!(py_load_from_path, m)?)?;
    Ok(())
}
