//! Nelson-Siegel (1987) parametric yield curve.
//!
//! Y(t) = β₀ + β₁·L(t/τ) + β₂·(L(t/τ) − e^(-t/τ)),  where L(x) = (1 − e^(-x))/x.
//!
//! Four parameters. Widely used for central-bank monetary policy analysis
//! and as a parsimonious alternative to cubic splines when a smooth
//! parametric form is desired.

use crate::{
    nelder_mead::nelder_mead, validate::validate_and_sort, YieldCurveError, YieldCurveInterpolator,
};

/// Nelson-Siegel formula. `t_years` is time to maturity in years.
///
/// The limit at t → 0 is β₀ + β₁; the limit as t → ∞ is β₀.
fn ns_rate(beta0: f64, beta1: f64, beta2: f64, tau: f64, t_years: f64) -> f64 {
    if t_years < 1e-12 {
        return beta0 + beta1;
    }
    let x = t_years / tau;
    let ex = (-x).exp();
    let load = (1.0 - ex) / x;
    beta0 + beta1 * load + beta2 * (load - ex)
}

/// Nelson-Siegel parametric fit (β₀, β₁, β₂, τ) via Nelder-Mead on SSE.
#[derive(Debug, Clone)]
pub struct NelsonSiegelCurve {
    beta0: f64,
    beta1: f64,
    beta2: f64,
    tau: f64,
    observed_min: f64,
    observed_max: f64,
}

impl NelsonSiegelCurve {
    /// Fits Nelson-Siegel parameters to `(t_years, rate)` points.
    ///
    /// Requires at least 4 points (4 parameters). Returns
    /// [`YieldCurveError::FitFailed`] if the optimizer fails to converge or
    /// produces implausible parameters (τ outside [0.01, 50] or any β over 50
    /// in absolute value) — this typically indicates that the input data is
    /// not well-described by a Nelson-Siegel shape and a spline method would
    /// be more appropriate.
    pub fn fit(points: &[(f64, f64)]) -> Result<Self, YieldCurveError> {
        let sorted = validate_and_sort(points, "nelson_siegel", 4)?;
        let observed_min = sorted.first().unwrap().0;
        let observed_max = sorted.last().unwrap().0;

        let y_short = sorted.first().unwrap().1;
        let y_long = sorted.last().unwrap().1;

        let initial = vec![y_long, y_short - y_long, 0.0, 1.0];
        let step = vec![0.5, 0.5, 0.5, 0.3];

        let cost = |p: &[f64]| -> f64 {
            let tau = p[3].max(0.01);
            sorted
                .iter()
                .map(|(t, y)| {
                    let pred = ns_rate(p[0], p[1], p[2], tau, *t);
                    (pred - y).powi(2)
                })
                .sum()
        };

        let (best, _) = nelder_mead(cost, initial, step, 10_000, 1e-12)?;

        let (b0, b1, b2, tau) = (best[0], best[1], best[2], best[3]);
        let max_abs_beta = b0.abs().max(b1.abs()).max(b2.abs());
        if !(0.01..=50.0).contains(&tau) || max_abs_beta > 50.0 {
            return Err(YieldCurveError::FitFailed(format!(
                "Nelson-Siegel fit produced implausible parameters (β0={b0:.2}, β1={b1:.2}, β2={b2:.2}, τ={tau:.4}); dataset likely unsuitable for parametric fit, try cubic spline"
            )));
        }

        Ok(Self {
            beta0: b0,
            beta1: b1,
            beta2: b2,
            tau: tau.max(0.01),
            observed_min,
            observed_max,
        })
    }

    /// Fitted `(β₀, β₁, β₂, τ)` — β's in the same unit as the input rates,
    /// τ in years.
    pub fn parameters(&self) -> (f64, f64, f64, f64) {
        (self.beta0, self.beta1, self.beta2, self.tau)
    }
}

impl YieldCurveInterpolator for NelsonSiegelCurve {
    fn rate_at(&self, t_years: f64) -> f64 {
        let clamped = t_years.clamp(self.observed_min, self.observed_max);
        ns_rate(self.beta0, self.beta1, self.beta2, self.tau, clamped)
    }

    fn method_name(&self) -> &'static str {
        "nelson_siegel"
    }

    fn observed_range(&self) -> (f64, f64) {
        (self.observed_min, self.observed_max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn limit_at_zero() {
        let y = ns_rate(10.0, -3.0, 1.0, 1.5, 0.0);
        assert!(approx_eq(y, 7.0, 1e-10));
    }

    #[test]
    fn asymptote_long_term() {
        let y_far = ns_rate(10.0, -3.0, 1.0, 1.5, 1000.0);
        assert!(approx_eq(y_far, 10.0, 0.01), "y_far={y_far}");
    }

    #[test]
    fn fit_recovers_known_parameters() {
        let b0 = 13.0;
        let b1 = -2.0;
        let b2 = 1.5;
        let tau = 1.2;
        let vertices_years = [0.25, 0.5, 1.0, 2.0, 4.0, 10.0, 20.0];
        let points: Vec<(f64, f64)> = vertices_years
            .iter()
            .map(|&t| (t, ns_rate(b0, b1, b2, tau, t)))
            .collect();
        let curve = NelsonSiegelCurve::fit(&points).unwrap();
        for (t, expected) in &points {
            let got = curve.rate_at(*t);
            assert!(
                approx_eq(got, *expected, 0.05),
                "at t={t}: got {got}, expected {expected}"
            );
        }
    }

    #[test]
    fn rejects_insufficient_points() {
        let err = NelsonSiegelCurve::fit(&[(0.25, 13.0), (0.5, 13.5), (1.0, 14.0)]).unwrap_err();
        assert!(matches!(
            err,
            YieldCurveError::InsufficientData {
                need: 4,
                got: 3,
                ..
            }
        ));
    }

    #[test]
    fn clamps_outside_observed() {
        let points = [(0.25, 13.0), (1.0, 13.5), (2.0, 13.8), (5.0, 14.0)];
        let curve = NelsonSiegelCurve::fit(&points).unwrap();
        let lo = curve.rate_at(0.01);
        let hi = curve.rate_at(100.0);
        let at_lo = curve.rate_at(0.25);
        let at_hi = curve.rate_at(5.0);
        assert!(approx_eq(lo, at_lo, 1e-10));
        assert!(approx_eq(hi, at_hi, 1e-10));
    }

    #[test]
    fn method_name_stable() {
        let points = [(0.25, 13.0), (1.0, 13.5), (2.0, 13.8), (5.0, 14.0)];
        let curve = NelsonSiegelCurve::fit(&points).unwrap();
        assert_eq!(curve.method_name(), "nelson_siegel");
    }
}
