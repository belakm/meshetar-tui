use std::fmt;

use pyo3::PyErr;
use thiserror::Error;

/// All errors generated in the barter::portfolio module.
#[derive(Error, Debug)]
pub enum StrategyError {
    #[error("Python error: {0}")]
    PythonError(PythonErrWrapper),
}

#[derive(Debug)]
pub struct PythonErrWrapper(pub PyErr);

impl fmt::Display for PythonErrWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl From<PyErr> for StrategyError {
    fn from(err: PyErr) -> Self {
        StrategyError::PythonError(PythonErrWrapper(err))
    }
}
