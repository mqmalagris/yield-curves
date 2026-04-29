//! Compounding conventions, discount factors, and forward rates.
//!
//! Yield curve interpolation answers "what is the rate at time t?". This
//! module answers "what is that rate worth?" and "what does it imply about
//! shorter forward periods?".
//!
//! Functions are intentionally free-standing (not on the
//! [`crate::YieldCurveInterpolator`] trait) because:
//!
//! - Interpolation methods know nothing about whether their output is decimal
//!   or percent. The caller has that context.
//! - Compounding is a *separate concern* from curve shape. Bundling them on
//!   the trait would force every interpolator to grow a `Compounding`
//!   parameter on every call.
//!
//! Rates passed to these functions must be in **decimal form** (e.g. `0.135`
//! for 13.5%). Convert at the call site:
//!
//! ```
//! use yield_curves::{compounding::{discount_factor, Compounding}, CubicSplineCurve, YieldCurveInterpolator};
//!
//! let curve = CubicSplineCurve::fit(&[(1.0, 13.0), (5.0, 13.5), (10.0, 13.8)]).unwrap();
//! let rate_pct = curve.rate_at(3.0);
//! let df = discount_factor(rate_pct / 100.0, 3.0, Compounding::Continuous);
//! assert!(df > 0.0 && df < 1.0);
//! ```

use std::num::NonZeroU32;

use crate::YieldCurveError;

/// Compounding convention used to translate a yield into a discount factor or
/// to compose forward rates.
///
/// `Periodic(n)` covers the common cases:
/// - `Periodic(NonZeroU32::new(1).unwrap())` — annual compounding, `(1+r)^t`.
/// - `Periodic(NonZeroU32::new(2).unwrap())` — semi-annual, `(1+r/2)^(2t)`.
/// - `Periodic(NonZeroU32::new(12).unwrap())` — monthly.
/// - `Periodic(NonZeroU32::new(252).unwrap())` — Brazilian business-day
///   convention (one compounding per business day).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compounding {
    /// Continuous compounding: `DF = exp(-r*t)`.
    Continuous,
    /// Periodic compounding `n` times per year: `DF = (1 + r/n)^(-n*t)`.
    Periodic(NonZeroU32),
    /// Simple (linear) interest: `DF = 1 / (1 + r*t)`.
    Simple,
}

impl Compounding {
    /// Convenience constructor for annual compounding (`Periodic(1)`).
    #[must_use]
    pub fn annual() -> Self {
        Self::Periodic(NonZeroU32::new(1).expect("1 is non-zero"))
    }

    /// Convenience constructor for semi-annual compounding (`Periodic(2)`).
    #[must_use]
    pub fn semi_annual() -> Self {
        Self::Periodic(NonZeroU32::new(2).expect("2 is non-zero"))
    }
}

/// Discount factor for a rate that lasts `t_years` years.
///
/// Rate must be in **decimal form** (`0.135`, not `13.5`). Time must be
/// non-negative.
///
/// Returns `f64::NAN` if either input is non-finite, or `1.0` for `t = 0`.
///
/// This function is infallible: NaN/Inf inputs propagate as NaN outputs so it
/// composes cleanly with curves that hit a flat-extrapolation boundary.
#[must_use]
pub fn discount_factor(rate: f64, t_years: f64, comp: Compounding) -> f64 {
    if !rate.is_finite() || !t_years.is_finite() {
        return f64::NAN;
    }
    if t_years == 0.0 {
        return 1.0;
    }
    match comp {
        Compounding::Continuous => (-rate * t_years).exp(),
        Compounding::Periodic(n) => {
            let n = f64::from(n.get());
            (1.0 + rate / n).powf(-n * t_years)
        }
        Compounding::Simple => 1.0 / (1.0 + rate * t_years),
    }
}

/// Implied forward rate between `t1` and `t2`.
///
/// Solves: starting from a unit at time 0, accruing at `r1` to `t1` then at
/// the forward rate `f` for the period `(t1, t2)`, must equal accruing at
/// `r2` to `t2`. Equivalently, `DF(t1) * compound(f, t2-t1) = DF(t2)`.
///
/// Rates must be in **decimal form**. Returns an error if:
/// - `t1 < 0` or `t2 < 0`
/// - `t1 >= t2`
/// - any input is non-finite
/// - the implied forward is non-finite (e.g. negative discount factor under
///   simple compounding for an unrealistic input)
///
/// # Example
///
/// ```
/// use yield_curves::compounding::{forward_rate, Compounding};
/// // Spot 5% for 1y and 6% for 2y, continuous → forward(1y, 2y) ≈ 7%.
/// let f = forward_rate(0.05, 1.0, 0.06, 2.0, Compounding::Continuous).unwrap();
/// assert!((f - 0.07).abs() < 1e-12);
/// ```
pub fn forward_rate(
    r1: f64,
    t1: f64,
    r2: f64,
    t2: f64,
    comp: Compounding,
) -> Result<f64, YieldCurveError> {
    for (label, v) in [("r1", r1), ("t1", t1), ("r2", r2), ("t2", t2)] {
        if !v.is_finite() {
            return Err(YieldCurveError::InvalidTimeRange(format!(
                "{label} is not finite ({v})"
            )));
        }
    }
    if t1 < 0.0 || t2 < 0.0 {
        return Err(YieldCurveError::InvalidTimeRange(format!(
            "negative time (t1={t1}, t2={t2})"
        )));
    }
    if t1 >= t2 {
        return Err(YieldCurveError::InvalidTimeRange(format!(
            "t1 must be < t2 (t1={t1}, t2={t2})"
        )));
    }

    let dt = t2 - t1;
    let result = match comp {
        Compounding::Continuous => (r2 * t2 - r1 * t1) / dt,
        Compounding::Periodic(n) => {
            let n = f64::from(n.get());
            // ((1 + r2/n)^(n*t2) / (1 + r1/n)^(n*t1))^(1/(n*dt)) - 1, scaled by n
            let num = (1.0 + r2 / n).powf(n * t2);
            let den = (1.0 + r1 / n).powf(n * t1);
            let ratio = num / den;
            n * (ratio.powf(1.0 / (n * dt)) - 1.0)
        }
        Compounding::Simple => {
            let df1 = 1.0 / (1.0 + r1 * t1);
            let df2 = 1.0 / (1.0 + r2 * t2);
            // df1 * (1 + f*dt) = 1/df2_inv ... derived from spot relations
            (df1 / df2 - 1.0) / dt
        }
    };

    if !result.is_finite() {
        return Err(YieldCurveError::InvalidTimeRange(format!(
            "forward rate is non-finite (r1={r1}, t1={t1}, r2={r2}, t2={t2}, comp={comp:?})"
        )));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn df_continuous_zero_rate_is_one() {
        assert!(approx_eq(
            discount_factor(0.0, 5.0, Compounding::Continuous),
            1.0,
            1e-12
        ));
    }

    #[test]
    fn df_continuous_known_value() {
        // exp(-0.05 * 1.0) ≈ 0.9512294
        assert!(approx_eq(
            discount_factor(0.05, 1.0, Compounding::Continuous),
            (-0.05_f64).exp(),
            1e-12
        ));
    }

    #[test]
    fn df_annual_known_value() {
        // 1 / (1.05)^2 ≈ 0.9070295
        assert!(approx_eq(
            discount_factor(0.05, 2.0, Compounding::annual()),
            1.0 / 1.05_f64.powi(2),
            1e-12
        ));
    }

    #[test]
    fn df_semi_annual() {
        // (1 + 0.06/2)^(-2*1) = 1/1.03^2
        assert!(approx_eq(
            discount_factor(0.06, 1.0, Compounding::semi_annual()),
            1.0 / 1.03_f64.powi(2),
            1e-12
        ));
    }

    #[test]
    fn df_simple() {
        // 1 / (1 + 0.10 * 0.5) = 1 / 1.05
        assert!(approx_eq(
            discount_factor(0.10, 0.5, Compounding::Simple),
            1.0 / 1.05,
            1e-12
        ));
    }

    #[test]
    fn df_t_zero_is_one() {
        assert_eq!(discount_factor(0.5, 0.0, Compounding::Continuous), 1.0);
        assert_eq!(discount_factor(0.5, 0.0, Compounding::annual()), 1.0);
        assert_eq!(discount_factor(0.5, 0.0, Compounding::Simple), 1.0);
    }

    #[test]
    fn df_propagates_nan() {
        assert!(discount_factor(f64::NAN, 1.0, Compounding::Continuous).is_nan());
        assert!(discount_factor(0.05, f64::INFINITY, Compounding::annual()).is_nan());
    }

    #[test]
    fn forward_continuous_classic() {
        // 5% spot for 1y, 6% spot for 2y, continuous → forward = (0.06*2 - 0.05*1)/1 = 0.07
        let f = forward_rate(0.05, 1.0, 0.06, 2.0, Compounding::Continuous).unwrap();
        assert!(approx_eq(f, 0.07, 1e-12));
    }

    #[test]
    fn forward_annual_inverse_of_df() {
        // (1.06)^2 / (1.05)^1 = (1 + f)^1 → f ≈ 0.07009524
        let f = forward_rate(0.05, 1.0, 0.06, 2.0, Compounding::annual()).unwrap();
        let expected = 1.06_f64.powi(2) / 1.05 - 1.0;
        assert!(approx_eq(f, expected, 1e-12));
    }

    #[test]
    fn forward_simple_smoke() {
        // Just check it returns finite for sane inputs.
        let f = forward_rate(0.05, 0.5, 0.06, 1.0, Compounding::Simple).unwrap();
        assert!(f.is_finite());
        assert!(f > 0.0);
    }

    #[test]
    fn forward_rejects_t1_ge_t2() {
        let err = forward_rate(0.05, 2.0, 0.06, 1.0, Compounding::Continuous).unwrap_err();
        assert!(matches!(err, YieldCurveError::InvalidTimeRange(_)));
        let err = forward_rate(0.05, 1.0, 0.06, 1.0, Compounding::Continuous).unwrap_err();
        assert!(matches!(err, YieldCurveError::InvalidTimeRange(_)));
    }

    #[test]
    fn forward_rejects_negative_time() {
        let err = forward_rate(0.05, -0.5, 0.06, 1.0, Compounding::Continuous).unwrap_err();
        assert!(matches!(err, YieldCurveError::InvalidTimeRange(_)));
    }

    #[test]
    fn forward_rejects_nan() {
        let err = forward_rate(f64::NAN, 1.0, 0.06, 2.0, Compounding::Continuous).unwrap_err();
        assert!(matches!(err, YieldCurveError::InvalidTimeRange(_)));
    }
}
