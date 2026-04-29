//! Piecewise Cubic Hermite Interpolating Polynomial (PCHIP).
//!
//! Monotone cubic interpolation that **does not overshoot** between anchor
//! points. Useful for yield curves with sparse vertices where the natural
//! cubic spline (see [`crate::CubicSplineCurve`]) tends to oscillate or
//! introduce spurious humps.
//!
//! Slopes are chosen via the Fritsch-Carlson algorithm: the slope at each
//! interior knot is the harmonic-mean-like blend of the two adjacent secant
//! slopes (zero whenever the secants change sign), then a final pass enforces
//! monotonicity by clipping each interior slope to a sufficient condition
//! (Fritsch & Carlson, 1980).
//!
//! C¹ continuous (continuous first derivative). Not C² (second derivative
//! jumps at knots) — that is the trade for monotonicity.

use crate::{validate::validate_and_sort, YieldCurveError, YieldCurveInterpolator};

/// Monotone cubic Hermite curve (PCHIP / Fritsch-Carlson).
#[derive(Debug, Clone)]
pub struct PchipCurve {
    x: Vec<f64>,
    y: Vec<f64>,
    /// Slope (dy/dx) at each knot.
    m: Vec<f64>,
}

impl PchipCurve {
    /// Fits from `(t_years, rate)` anchor points. Requires at least 2 points.
    pub fn fit(points: &[(f64, f64)]) -> Result<Self, YieldCurveError> {
        let sorted = validate_and_sort(points, "pchip", 2)?;
        let n = sorted.len();
        let x: Vec<f64> = sorted.iter().map(|p| p.0).collect();
        let y: Vec<f64> = sorted.iter().map(|p| p.1).collect();

        // h_i = x[i+1] - x[i],  d_i = (y[i+1] - y[i]) / h_i
        let h: Vec<f64> = (0..n - 1).map(|i| x[i + 1] - x[i]).collect();
        let d: Vec<f64> = (0..n - 1).map(|i| (y[i + 1] - y[i]) / h[i]).collect();

        let mut m = vec![0.0; n];

        if n == 2 {
            // Line through two points — slope is the secant.
            m[0] = d[0];
            m[1] = d[0];
            return Ok(Self { x, y, m });
        }

        // Interior slopes (Fritsch-Carlson): weighted-harmonic-mean of secants,
        // zero when the two adjacent secants have opposite signs.
        for i in 1..n - 1 {
            if d[i - 1] * d[i] <= 0.0 {
                m[i] = 0.0;
            } else {
                let w1 = 2.0 * h[i] + h[i - 1];
                let w2 = h[i] + 2.0 * h[i - 1];
                // 1/m = (w1/d[i-1] + w2/d[i]) / (w1 + w2)
                m[i] = (w1 + w2) / (w1 / d[i - 1] + w2 / d[i]);
            }
        }

        // Endpoint slopes: non-centred parabolic, with shape-preserving clip.
        // Right endpoint uses the two rightmost intervals: h[n-2] and h[n-3].
        // (n >= 3 here, so n-3 is in range.)
        m[0] = endpoint_slope(h[0], h[1], d[0], d[1]);
        m[n - 1] = endpoint_slope(h[n - 2], h[n - 3], d[n - 2], d[n - 3]);

        // Monotonicity enforcement (Fritsch-Carlson alpha/beta clipping).
        for i in 0..n - 1 {
            if d[i] == 0.0 {
                m[i] = 0.0;
                m[i + 1] = 0.0;
                continue;
            }
            let alpha = m[i] / d[i];
            let beta = m[i + 1] / d[i];
            let s = alpha * alpha + beta * beta;
            if s > 9.0 {
                let tau = 3.0 / s.sqrt();
                m[i] = tau * alpha * d[i];
                m[i + 1] = tau * beta * d[i];
            }
        }

        Ok(Self { x, y, m })
    }
}

/// One-sided endpoint slope per Fritsch & Carlson, with sign-preserving clip
/// to avoid creating a local extremum at the boundary.
fn endpoint_slope(h0: f64, h1: f64, d0: f64, d1: f64) -> f64 {
    let m = ((2.0 * h0 + h1) * d0 - h0 * d1) / (h0 + h1);
    if m * d0 <= 0.0 {
        0.0
    } else if d0 * d1 < 0.0 && m.abs() > 3.0 * d0.abs() {
        3.0 * d0
    } else {
        m
    }
}

impl YieldCurveInterpolator for PchipCurve {
    fn rate_at(&self, t_years: f64) -> f64 {
        let n = self.x.len();
        if t_years <= self.x[0] {
            return self.y[0];
        }
        if t_years >= self.x[n - 1] {
            return self.y[n - 1];
        }
        let idx = self.x.partition_point(|&xi| xi < t_years);
        // idx is in [1, n-1]; segment is [idx-1, idx].
        let i = idx - 1;
        let h = self.x[i + 1] - self.x[i];
        let t = (t_years - self.x[i]) / h;
        // Hermite basis on [0, 1]:
        //   h00 =  2t^3 - 3t^2 + 1
        //   h10 =      t^3 - 2t^2 + t
        //   h01 = -2t^3 + 3t^2
        //   h11 =      t^3 -   t^2
        let t2 = t * t;
        let t3 = t2 * t;
        let h00 = 2.0 * t3 - 3.0 * t2 + 1.0;
        let h10 = t3 - 2.0 * t2 + t;
        let h01 = -2.0 * t3 + 3.0 * t2;
        let h11 = t3 - t2;
        h00 * self.y[i] + h10 * h * self.m[i] + h01 * self.y[i + 1] + h11 * h * self.m[i + 1]
    }

    fn method_name(&self) -> &'static str {
        "pchip"
    }

    fn observed_range(&self) -> (f64, f64) {
        (self.x[0], self.x[self.x.len() - 1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn passes_through_anchors() {
        let pts = [(1.0, 13.0), (2.0, 13.5), (5.0, 13.2), (10.0, 14.0)];
        let curve = PchipCurve::fit(&pts).unwrap();
        for (t, y) in pts {
            assert!(approx_eq(curve.rate_at(t), y, 1e-10), "anchor {t}");
        }
    }

    #[test]
    fn flat_extrapolation_outside() {
        let curve = PchipCurve::fit(&[(1.0, 13.0), (5.0, 13.5), (10.0, 14.0)]).unwrap();
        assert_eq!(curve.rate_at(0.5), 13.0);
        assert_eq!(curve.rate_at(20.0), 14.0);
    }

    #[test]
    fn rejects_single_point() {
        let err = PchipCurve::fit(&[(1.0, 13.0)]).unwrap_err();
        assert!(matches!(err, YieldCurveError::InsufficientData { .. }));
    }

    #[test]
    fn two_points_collapse_to_line() {
        // With only 2 anchors, m=secant on both endpoints → linear behaviour.
        let curve = PchipCurve::fit(&[(0.0, 10.0), (10.0, 20.0)]).unwrap();
        assert!(approx_eq(curve.rate_at(5.0), 15.0, 1e-10));
    }

    #[test]
    fn monotone_dataset_stays_monotone() {
        // Strictly increasing anchors — interpolated values must also increase.
        let pts = [
            (0.0, 0.0),
            (1.0, 0.5),
            (2.0, 1.0),
            (3.0, 4.0),
            (4.0, 5.0),
            (5.0, 6.0),
        ];
        let curve = PchipCurve::fit(&pts).unwrap();
        let mut prev = -f64::INFINITY;
        for k in 0..=500 {
            let t = (k as f64) * 5.0 / 500.0;
            let v = curve.rate_at(t);
            assert!(
                v >= prev - 1e-9,
                "monotonicity broken at t={t}: {v} < {prev}"
            );
            prev = v;
        }
    }

    #[test]
    fn no_overshoot_on_step_like_data() {
        // A near-step that defeats natural cubic spline (it overshoots).
        // PCHIP must keep the interpolated value within [0, 1] across the step.
        let pts = [(0.0, 0.0), (1.0, 0.0), (2.0, 1.0), (3.0, 1.0)];
        let curve = PchipCurve::fit(&pts).unwrap();
        for k in 0..=200 {
            let t = (k as f64) * 3.0 / 200.0;
            let v = curve.rate_at(t);
            assert!((-1e-9..=1.0 + 1e-9).contains(&v), "overshoot at t={t}: {v}");
        }
    }

    #[test]
    fn method_name_stable() {
        let curve = PchipCurve::fit(&[(0.0, 1.0), (1.0, 2.0)]).unwrap();
        assert_eq!(curve.method_name(), "pchip");
    }

    #[test]
    fn observed_range_reflects_input() {
        let curve = PchipCurve::fit(&[(0.5, 10.0), (1.0, 11.0), (3.0, 12.0)]).unwrap();
        assert_eq!(curve.observed_range(), (0.5, 3.0));
    }
}
