//! Yield curve interpolation and parametric fitting for fixed income.
//!
//! Zero-dependency library. All curves accept `(t_years, rate)` pairs and
//! expose a uniform interface via [`YieldCurveInterpolator`].
//!
//! # Methods
//!
//! - [`LinearCurve`] — piecewise linear, transparent baseline.
//! - [`CubicSplineCurve`] — natural cubic spline (C² continuous), Thomas
//!   algorithm, no linear-algebra dependencies.
//! - [`PchipCurve`] — Fritsch-Carlson monotone cubic Hermite (C¹). Use when
//!   cubic spline produces overshoots or when monotonicity must be preserved.
//! - [`NelsonSiegelCurve`] — Nelson-Siegel (1987) 4-parameter parametric fit.
//! - [`SvenssonCurve`] — Nelson-Siegel-Svensson (1994) 6-parameter parametric
//!   fit (BCB/ANBIMA/ECB standard for sovereign yield curves).
//!
//! # Discount factors and forward rates
//!
//! See the [`compounding`] module for free-standing helpers that turn
//! interpolated rates into discount factors and implied forward rates under
//! any of: continuous, periodic, or simple compounding.
//!
//! # Bond pricing
//!
//! See the [`bond`] module for price, Macaulay/modified duration, convexity
//! and par yield computed from a cash-flow schedule plus a YTM.
//!
//! # Conventions
//!
//! The x-axis is **time in years**. Convert from calendar/business days at
//! the call site with the appropriate day count convention:
//!
//! - Brazil (business-day 252): `days / 252.0`
//! - US Treasury (actual/365): `days / 365.0`
//! - ISDA actual/365.25: `days / 365.25`
//!
//! Rates are in the same unit as the input (typically percent). The library
//! performs no unit conversion.
//!
//! # Extrapolation
//!
//! All curves extrapolate **flat** outside the observed range — the rate of
//! the nearest observed anchor is returned. Parametric methods (NS, Svensson)
//! in particular diverge quickly outside the fitted range, so flat extrapolation
//! is a sane default for financial use.
//!
//! # Example
//!
//! ```
//! use yield_curves::{CubicSplineCurve, YieldCurveInterpolator};
//!
//! // Brazilian nominal yield curve from LTNs/NTN-Fs (t in years, rate in %).
//! let points = [
//!     (1.0, 13.98),
//!     (2.5, 13.51),
//!     (4.0, 13.45),
//!     (7.0, 13.57),
//!     (10.0, 13.80),
//! ];
//!
//! let curve = CubicSplineCurve::fit(&points).unwrap();
//! let rate_5y = curve.rate_at(5.0);
//! assert!((13.4..=13.6).contains(&rate_5y));
//! ```

pub mod bond;
pub mod calendar;
pub mod compounding;
pub mod date;
pub mod daycount;
pub mod linear;
pub mod nelson_siegel;
pub mod pchip;
pub mod spline;
pub mod svensson;

mod error;
mod nelder_mead;
mod validate;

pub use calendar::{
    easter, Brazil, BusinessDayConvention, Calendar, JoinCalendar, JoinRule, Target2, WeekendsOnly,
};
pub use compounding::{discount_factor, forward_rate, Compounding};
pub use date::{Date, DateError, Period, Unit, Weekday};
pub use daycount::DayCount;
pub use error::YieldCurveError;
pub use linear::LinearCurve;
pub use nelson_siegel::NelsonSiegelCurve;
pub use pchip::PchipCurve;
pub use spline::CubicSplineCurve;
pub use svensson::SvenssonCurve;

/// Common interface for all yield curve methods.
///
/// Time is in years. Implementations clamp `t_years` to the observed range
/// before evaluating (flat extrapolation).
pub trait YieldCurveInterpolator {
    /// Interpolated rate at `t_years`. Returns the boundary rate if `t_years`
    /// falls outside the observed range.
    fn rate_at(&self, t_years: f64) -> f64;

    /// Stable identifier for the method (e.g. `"linear"`, `"cubic_spline"`,
    /// `"nelson_siegel"`, `"svensson"`). Useful for tagging response payloads.
    fn method_name(&self) -> &'static str;

    /// The `(min, max)` t_years range observed in the fitted data. Callers
    /// can use this to mark whether a requested vertex is an interpolation
    /// or an extrapolation.
    fn observed_range(&self) -> (f64, f64);
}
