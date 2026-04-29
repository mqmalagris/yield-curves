use std::fmt;

/// Errors returned by curve construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum YieldCurveError {
    /// Not enough points for the requested method.
    InsufficientData {
        method: &'static str,
        need: usize,
        got: usize,
    },
    /// A point failed validation (NaN/infinite, negative x, duplicate x).
    InvalidPoint(String),
    /// Parametric fit (Nelson-Siegel, Svensson) did not converge or produced
    /// implausible parameters.
    FitFailed(String),
    /// Forward-rate computation received `t1 >= t2`, non-finite times, or
    /// produced a non-finite result (e.g. taking a negative discount factor
    /// to a fractional power).
    InvalidTimeRange(String),
}

impl fmt::Display for YieldCurveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InsufficientData { method, need, got } => {
                write!(f, "{method} requires at least {need} points, got {got}")
            }
            Self::InvalidPoint(msg) => write!(f, "invalid point: {msg}"),
            Self::FitFailed(msg) => write!(f, "fit failed to converge: {msg}"),
            Self::InvalidTimeRange(msg) => write!(f, "invalid time range: {msg}"),
        }
    }
}

impl std::error::Error for YieldCurveError {}
