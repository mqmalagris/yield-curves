//! Piecewise-linear interpolation with flat extrapolation.

use crate::{validate::validate_and_sort, YieldCurveError, YieldCurveInterpolator};

/// Piecewise-linear interpolation between anchor points. Extrapolates flat
/// outside the observed range.
///
/// O(log n) evaluation via binary search.
#[derive(Debug, Clone)]
pub struct LinearCurve {
    points: Vec<(f64, f64)>,
}

impl LinearCurve {
    /// Fits from `(t_years, rate)` anchor points. Requires at least 2 points.
    pub fn fit(points: &[(f64, f64)]) -> Result<Self, YieldCurveError> {
        let points = validate_and_sort(points, "linear", 2)?;
        Ok(Self { points })
    }
}

impl YieldCurveInterpolator for LinearCurve {
    fn rate_at(&self, t_years: f64) -> f64 {
        let pts = &self.points;
        if t_years <= pts[0].0 {
            return pts[0].1;
        }
        if t_years >= pts.last().unwrap().0 {
            return pts.last().unwrap().1;
        }
        let idx = pts.partition_point(|p| p.0 < t_years);
        let (x0, y0) = pts[idx - 1];
        let (x1, y1) = pts[idx];
        let t = (t_years - x0) / (x1 - x0);
        y0 + t * (y1 - y0)
    }

    fn method_name(&self) -> &'static str {
        "linear"
    }

    fn observed_range(&self) -> (f64, f64) {
        (self.points[0].0, self.points.last().unwrap().0)
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
        let curve = LinearCurve::fit(&[(0.25, 13.0), (0.5, 13.5), (1.0, 14.0)]).unwrap();
        assert!(approx_eq(curve.rate_at(0.25), 13.0, 1e-10));
        assert!(approx_eq(curve.rate_at(0.5), 13.5, 1e-10));
        assert!(approx_eq(curve.rate_at(1.0), 14.0, 1e-10));
    }

    #[test]
    fn midpoint_is_average() {
        let curve = LinearCurve::fit(&[(0.0, 10.0), (1.0, 20.0)]).unwrap();
        assert!(approx_eq(curve.rate_at(0.5), 15.0, 1e-10));
    }

    #[test]
    fn flat_extrapolation() {
        let curve = LinearCurve::fit(&[(0.25, 13.0), (1.0, 14.0)]).unwrap();
        assert_eq!(curve.rate_at(0.1), 13.0);
        assert_eq!(curve.rate_at(5.0), 14.0);
    }

    #[test]
    fn rejects_single_point() {
        let err = LinearCurve::fit(&[(0.25, 13.0)]).unwrap_err();
        assert!(matches!(err, YieldCurveError::InsufficientData { .. }));
    }

    #[test]
    fn rejects_nan() {
        let err = LinearCurve::fit(&[(0.25, f64::NAN), (0.5, 13.5)]).unwrap_err();
        assert!(matches!(err, YieldCurveError::InvalidPoint(_)));
    }

    #[test]
    fn rejects_duplicate_vertex() {
        let err = LinearCurve::fit(&[(0.25, 13.0), (0.25, 13.5)]).unwrap_err();
        assert!(matches!(err, YieldCurveError::InvalidPoint(_)));
    }

    #[test]
    fn method_name_stable() {
        let curve = LinearCurve::fit(&[(1.0, 1.0), (2.0, 2.0)]).unwrap();
        assert_eq!(curve.method_name(), "linear");
    }

    #[test]
    fn observed_range_reflects_input() {
        let curve = LinearCurve::fit(&[(0.25, 13.0), (0.5, 13.5), (3.0, 14.0)]).unwrap();
        assert_eq!(curve.observed_range(), (0.25, 3.0));
    }
}
