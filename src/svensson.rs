//! Svensson (1994) parametric yield curve.
//!
//! Y(t) = β₀ + β₁·L(t/τ₁) + β₂·(L(t/τ₁) − e^(-t/τ₁)) + β₃·(L(t/τ₂) − e^(-t/τ₂))
//!
//! Extends Nelson-Siegel with a second curvature term, allowing two humps
//! — enough flexibility to fit the shapes typically observed in sovereign
//! yield curves. Six parameters. Official model used by BCB, ANBIMA, and
//! the ECB's AAA-rated euro-area curve.

use crate::{
    nelder_mead::nelder_mead, validate::validate_and_sort, YieldCurveError, YieldCurveInterpolator,
};

/// Svensson formula. `t_years` is time to maturity in years.
#[allow(clippy::too_many_arguments)]
fn svensson_rate(
    beta0: f64,
    beta1: f64,
    beta2: f64,
    beta3: f64,
    tau1: f64,
    tau2: f64,
    t_years: f64,
) -> f64 {
    if t_years < 1e-12 {
        return beta0 + beta1;
    }
    let x1 = t_years / tau1;
    let x2 = t_years / tau2;
    let ex1 = (-x1).exp();
    let ex2 = (-x2).exp();
    let load1 = (1.0 - ex1) / x1;
    let load2 = (1.0 - ex2) / x2;
    beta0 + beta1 * load1 + beta2 * (load1 - ex1) + beta3 * (load2 - ex2)
}

/// Svensson parametric fit (β₀, β₁, β₂, β₃, τ₁, τ₂) via Nelder-Mead on SSE.
#[derive(Debug, Clone)]
pub struct SvenssonCurve {
    beta0: f64,
    beta1: f64,
    beta2: f64,
    beta3: f64,
    tau1: f64,
    tau2: f64,
    observed_min: f64,
    observed_max: f64,
}

impl SvenssonCurve {
    /// Fits Svensson parameters to `(t_years, rate)` points.
    ///
    /// Requires at least 6 points (6 parameters). Returns
    /// [`YieldCurveError::FitFailed`] if the optimizer fails to converge or
    /// produces implausible parameters. Svensson is numerically fragile with
    /// few points or noisy data — a spline method is often a better choice
    /// in those cases.
    pub fn fit(points: &[(f64, f64)]) -> Result<Self, YieldCurveError> {
        let sorted = validate_and_sort(points, "svensson", 6)?;
        let observed_min = sorted.first().unwrap().0;
        let observed_max = sorted.last().unwrap().0;
        let y_short = sorted.first().unwrap().1;
        let y_long = sorted.last().unwrap().1;

        let initial = vec![y_long, y_short - y_long, 0.0, 0.0, 1.0, 5.0];
        let step = vec![0.5, 0.5, 0.5, 0.5, 0.3, 1.0];

        let cost = |p: &[f64]| -> f64 {
            let tau1 = p[4].max(0.01);
            let tau2 = p[5].max(0.01);
            sorted
                .iter()
                .map(|(t, y)| {
                    let pred = svensson_rate(p[0], p[1], p[2], p[3], tau1, tau2, *t);
                    (pred - y).powi(2)
                })
                .sum()
        };

        let (best, _) = nelder_mead(cost, initial, step, 20_000, 1e-12)?;

        let (b0, b1, b2, b3, t1, t2) = (best[0], best[1], best[2], best[3], best[4], best[5]);
        let max_abs_beta = b0.abs().max(b1.abs()).max(b2.abs()).max(b3.abs());
        if !(0.01..=50.0).contains(&t1) || !(0.01..=50.0).contains(&t2) || max_abs_beta > 50.0 {
            return Err(YieldCurveError::FitFailed(format!(
                "Svensson fit produced implausible parameters (β0={b0:.2}, β1={b1:.2}, β2={b2:.2}, β3={b3:.2}, τ1={t1:.4}, τ2={t2:.4}); dataset likely unsuitable for parametric fit, try cubic spline"
            )));
        }

        Ok(Self {
            beta0: b0,
            beta1: b1,
            beta2: b2,
            beta3: b3,
            tau1: t1.max(0.01),
            tau2: t2.max(0.01),
            observed_min,
            observed_max,
        })
    }

    /// Fitted `(β₀, β₁, β₂, β₃, τ₁, τ₂)` — same layout as ANBIMA/ECB publications.
    pub fn parameters(&self) -> (f64, f64, f64, f64, f64, f64) {
        (
            self.beta0, self.beta1, self.beta2, self.beta3, self.tau1, self.tau2,
        )
    }
}

impl YieldCurveInterpolator for SvenssonCurve {
    fn rate_at(&self, t_years: f64) -> f64 {
        let clamped = t_years.clamp(self.observed_min, self.observed_max);
        svensson_rate(
            self.beta0, self.beta1, self.beta2, self.beta3, self.tau1, self.tau2, clamped,
        )
    }

    fn method_name(&self) -> &'static str {
        "svensson"
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
        let y = svensson_rate(13.0, -2.0, 1.0, 0.5, 1.2, 5.0, 0.0);
        assert!(approx_eq(y, 11.0, 1e-10));
    }

    #[test]
    fn fit_recovers_known_parameters() {
        let (b0, b1, b2, b3, t1, t2) = (13.0, -2.0, 1.5, 0.8, 1.2, 5.0);
        let vertices_years = [0.25, 0.5, 1.0, 2.0, 4.0, 10.0, 20.0, 30.0];
        let points: Vec<(f64, f64)> = vertices_years
            .iter()
            .map(|&t| (t, svensson_rate(b0, b1, b2, b3, t1, t2, t)))
            .collect();
        let curve = SvenssonCurve::fit(&points).unwrap();
        for (t, expected) in &points {
            let got = curve.rate_at(*t);
            assert!(
                approx_eq(got, *expected, 0.1),
                "at t={t}: got {got}, expected {expected}"
            );
        }
    }

    #[test]
    fn rejects_insufficient_points() {
        let err = SvenssonCurve::fit(&[
            (0.25, 13.0),
            (0.5, 13.5),
            (0.75, 14.0),
            (1.0, 13.8),
            (2.0, 13.5),
        ])
        .unwrap_err();
        assert!(matches!(
            err,
            YieldCurveError::InsufficientData {
                need: 6,
                got: 5,
                ..
            }
        ));
    }

    #[test]
    fn method_name_stable() {
        let (b0, b1, b2, b3, t1, t2) = (13.0, -2.0, 1.5, 0.8, 1.2, 5.0);
        let points: Vec<(f64, f64)> = [0.25, 0.5, 1.0, 2.0, 4.0, 10.0]
            .iter()
            .map(|&t| (t, svensson_rate(b0, b1, b2, b3, t1, t2, t)))
            .collect();
        let curve = SvenssonCurve::fit(&points).unwrap();
        assert_eq!(curve.method_name(), "svensson");
    }
}
