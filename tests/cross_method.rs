//! Sanity: with the input anchors, every method should reproduce the
//! observed rates at the anchor vertices (within each method's tolerance).

use yield_curves::{
    CubicSplineCurve, LinearCurve, NelsonSiegelCurve, SvenssonCurve, YieldCurveInterpolator,
};

fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() < eps
}

#[test]
fn all_methods_agree_on_anchors() {
    let points = [
        (0.25, 13.0),
        (0.5, 13.5),
        (0.75, 13.8),
        (1.5, 14.0),
        (3.0, 13.7),
        (7.0, 13.5),
    ];
    let lin = LinearCurve::fit(&points).unwrap();
    let cub = CubicSplineCurve::fit(&points).unwrap();
    let ns = NelsonSiegelCurve::fit(&points).unwrap();
    let sv = SvenssonCurve::fit(&points).unwrap();
    for (t, y) in &points {
        assert!(approx_eq(lin.rate_at(*t), *y, 1e-10), "linear off at {t}");
        assert!(approx_eq(cub.rate_at(*t), *y, 1e-8), "cubic off at {t}");
        assert!(
            approx_eq(ns.rate_at(*t), *y, 0.1),
            "NS off at {t}: got {}",
            ns.rate_at(*t)
        );
        assert!(
            approx_eq(sv.rate_at(*t), *y, 0.1),
            "Svensson off at {t}: got {}",
            sv.rate_at(*t)
        );
    }
    assert_eq!(lin.method_name(), "linear");
    assert_eq!(cub.method_name(), "cubic_spline");
    assert_eq!(ns.method_name(), "nelson_siegel");
    assert_eq!(sv.method_name(), "svensson");
}
