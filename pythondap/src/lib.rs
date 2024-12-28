use crate::debugger::{Debugger, ProgramState};
use pyo3::prelude::*;

mod debugger;

#[pymodule]
fn pythondap(m: &Bound<'_, PyModule>) -> PyResult<()> {
    tracing_subscriber::fmt::init();

    m.add_class::<Debugger>()?;
    m.add_class::<ProgramState>()?;
    m.add_class::<debugger::PyPausedFrame>()?;
    Ok(())
}
