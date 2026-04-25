//! Natural cubic spline interpolation.

use crate::{validate::validate_and_sort, YieldCurveError, YieldCurveInterpolator};

/// Natural cubic spline: piecewise cubic polynomials with continuous first
/// and second derivatives, and zero second derivative at the endpoints.
///
/// The tridiagonal system for the second derivatives is solved via the
/// Thomas algorithm (O(n)), avoiding any dependency on a linear-algebra
/// crate.
#[derive(Debug, Clone)]
pub struct CubicSplineCurve {
    /// Sorted x coordinates (t_years).
    x: Vec<f64>,
    /// Corresponding y values.
    y: Vec<f64>,
    /// Second derivatives at each knot.
    m: Vec<f64>,
}

impl CubicSplineCurve {
    /// Fits from `(t_years, rate)` anchor points. Requires at least 2 points.
    pub fn fit(points: &[(f64, f64)]) -> Result<Self, YieldCurveError> {
        let sorted = validate_and_sort(points, "cubic_spline", 2)?;
        let n = sorted.len();
        let x: Vec<f64> = sorted.iter().map(|p| p.0).collect();
        let y: Vec<f64> = sorted.iter().map(|p| p.1).collect();

        if n == 2 {
            return Ok(Self {
                x,
                y,
                m: vec![0.0; 2],
            });
        }

        let h: Vec<f64> = (0..n - 1).map(|i| x[i + 1] - x[i]).collect();

        let interior = n - 2;
        let mut a = vec![0.0; interior];
        let mut b = vec![0.0; interior];
        let mut c = vec![0.0; interior];
        let mut d = vec![0.0; interior];

        for i in 0..interior {
            let idx = i + 1;
            a[i] = if i == 0 { 0.0 } else { h[idx - 1] };
            b[i] = 2.0 * (h[idx - 1] + h[idx]);
            c[i] = if i == interior - 1 { 0.0 } else { h[idx] };
            d[i] = 6.0 * ((y[idx + 1] - y[idx]) / h[idx] - (y[idx] - y[idx - 1]) / h[idx - 1]);
        }

        for i in 1..interior {
            let factor = a[i] / b[i - 1];
            b[i] -= factor * c[i - 1];
            d[i] -= factor * d[i - 1];
        }
        let mut m_interior = vec![0.0; interior];
        m_interior[interior - 1] = d[interior - 1] / b[interior - 1];
        for i in (0..interior - 1).rev() {
            m_interior[i] = (d[i] - c[i] * m_interior[i + 1]) / b[i];
        }

        let mut m = vec![0.0; n];
        m[1..n - 1].copy_from_slice(&m_interior);
        Ok(Self { x, y, m })
    }
}

impl YieldCurveInterpolator for CubicSplineCurve {
    fn rate_at(&self, t_years: f64) -> f64 {
        let n = self.x.len();
        if t_years <= self.x[0] {
            return self.y[0];
        }
        if t_years >= self.x[n - 1] {
            return self.y[n - 1];
        }
        let i = self.x.partition_point(|&xi| xi < t_years).saturating_sub(1);
        let h = self.x[i + 1] - self.x[i];
        let a = (self.x[i + 1] - t_years) / h;
        let b = (t_years - self.x[i]) / h;
        let term1 = a * self.y[i] + b * self.y[i + 1];
        let term2 = ((a * a * a - a) * self.m[i] + (b * b * b - b) * self.m[i + 1]) * (h * h) / 6.0;
        term1 + term2
    }

    fn method_name(&self) -> &'static str {
        "cubic_spline"
    }

    fn observed_range(&self) -> (f64, f64) {
        (self.x[0], self.x[self.x.len() - 1])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linear::LinearCurve;

    fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn passes_through_anchors() {
        let anchors = [(0.25, 13.0), (0.5, 13.5), (1.5, 14.0), (3.0, 13.8)];
        let curve = CubicSplineCurve::fit(&anchors).unwrap();
        for &(x, y) in &anchors {
            assert!(
                approx_eq(curve.rate_at(x), y, 1e-8),
                "at x={x}: got {}, expected {y}",
                curve.rate_at(x)
            );
        }
    }

    #[test]
    fn monotonic_input_stays_bounded() {
        let curve =
            CubicSplineCurve::fit(&[(0.25, 13.0), (0.5, 13.5), (0.75, 13.8), (1.5, 14.0)]).unwrap();
        for t in [0.3, 0.6, 0.9, 1.2] {
            let v = curve.rate_at(t);
            assert!(
                (12.5..=14.5).contains(&v),
                "oscillation out of bounds: {v} at {t}"
            );
        }
    }

    #[test]
    fn two_points_reduces_to_linear() {
        let cubic = CubicSplineCurve::fit(&[(0.0, 10.0), (1.0, 20.0)]).unwrap();
        let linear = LinearCurve::fit(&[(0.0, 10.0), (1.0, 20.0)]).unwrap();
        for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
            assert!(
                approx_eq(cubic.rate_at(t), linear.rate_at(t), 1e-10),
                "cubic≠linear for n=2 at t={t}"
            );
        }
    }

    #[test]
    fn flat_extrapolation() {
        let curve = CubicSplineCurve::fit(&[(0.25, 13.0), (0.5, 13.5), (1.5, 14.0)]).unwrap();
        assert_eq!(curve.rate_at(0.1), 13.0);
        assert_eq!(curve.rate_at(5.0), 14.0);
    }

    #[test]
    fn observed_range_reflects_input() {
        let curve = CubicSplineCurve::fit(&[(0.25, 13.0), (0.5, 13.5), (1.5, 14.0)]).unwrap();
        assert_eq!(curve.observed_range(), (0.25, 1.5));
    }

    #[test]
    fn method_name_stable() {
        let curve = CubicSplineCurve::fit(&[(1.0, 1.0), (2.0, 2.0), (3.0, 3.0)]).unwrap();
        assert_eq!(curve.method_name(), "cubic_spline");
    }
}
