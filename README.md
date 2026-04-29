# yield-curves

Yield curve interpolation and parametric fitting for fixed income, in pure
Rust with **zero dependencies**.

## Interpolation methods

- **Linear** — piecewise linear, transparent baseline.
- **Cubic spline** — natural cubic spline (C² continuous) via Thomas algorithm.
- **PCHIP** *(new in 0.2)* — Fritsch-Carlson monotone cubic Hermite. C¹
  continuous; preserves monotonicity and never overshoots adjacent anchors.
  Use when natural cubic spline produces spurious humps with sparse data.
- **Nelson-Siegel** (1987) — 4-parameter parametric fit.
- **Svensson** (1994) — 6-parameter parametric fit; official model used by
  BCB (Brazil), ANBIMA, and the ECB's AAA-rated euro-area curve.

## Compounding & forward rates *(new in 0.2)*

The `compounding` module turns interpolated rates into **discount factors**
and **forward rates** under any of: continuous, periodic (`Periodic(n)`
covers annual / semi / quarterly / monthly / Brazil-252), and simple
compounding.

```rust
use yield_curves::{CubicSplineCurve, YieldCurveInterpolator};
use yield_curves::compounding::{discount_factor, forward_rate, Compounding};

let curve = CubicSplineCurve::fit(&[(1.0, 13.0), (2.0, 13.5), (5.0, 13.8)]).unwrap();
let rate_pct = curve.rate_at(3.0);

// Discount factor for 3 years under continuous compounding.
// Caller is responsible for converting percent → decimal.
let df = discount_factor(rate_pct / 100.0, 3.0, Compounding::Continuous);

// Implied forward rate between t1 = 1y and t2 = 5y.
let fwd = forward_rate(
    curve.rate_at(1.0) / 100.0, 1.0,
    curve.rate_at(5.0) / 100.0, 5.0,
    Compounding::Continuous,
).unwrap();
```

Functions live outside the `YieldCurveInterpolator` trait on purpose: rate
unit (% vs decimal) and compounding convention are caller concerns, not
properties of the curve shape.

No dependency on `ndarray`, `argmin`, or any numerical crate. The Nelder-Mead
simplex optimizer used by the parametric fits is implemented internally.

## Quick start

```rust
use yield_curves::{CubicSplineCurve, NelsonSiegelCurve, YieldCurveInterpolator};

// Brazilian nominal yield curve from LTNs / NTN-Fs.
// x is time in years, y is the observed yield in percent.
let points = [
    (1.0, 13.98),
    (2.5, 13.51),
    (4.0, 13.45),
    (7.0, 13.57),
    (10.0, 13.80),
];

let cubic = CubicSplineCurve::fit(&points).unwrap();
let rate_5y = cubic.rate_at(5.0);

let ns = NelsonSiegelCurve::fit(&points).unwrap();
let (beta0, beta1, beta2, tau) = ns.parameters();
```

## Conventions

The x-axis is **time in years**. Convert from calendar / business days at
the call site:

| Market                     | Convention            |
| -------------------------- | --------------------- |
| Brazil (LTN/NTN-F/NTN-B)   | `days / 252.0` (DU)   |
| US Treasury (CMT)          | `days / 365.25`       |
| ISDA actual/365            | `days / 365.0`        |

Extrapolation is **flat** outside the observed range — the rate of the
nearest observed anchor is returned. Parametric models in particular diverge
quickly outside the fitted range, so flat extrapolation is the safer default
for financial use.

## When to pick what

- **Linear** — transparent, monotonic, used as a baseline or when anchors are
  already smoothed. Not C¹.
- **Cubic spline** — smoothest interpolation that still passes through every
  anchor exactly. Good default when you trust your anchor points.
- **PCHIP** — pick this over cubic spline when sparse anchors produce
  visible overshoots/oscillations, or when monotonicity must be preserved
  (e.g. an inflation index). C¹ continuous (less smooth than spline) but
  shape-preserving.
- **Nelson-Siegel** — parsimonious 4-parameter fit. Produces monotonic or
  single-hump curves only. Use when you want a smooth parametric form for
  research or when your anchors are noisy.
- **Svensson** — adds a second hump to NS. Standard for sovereign curves
  (BCB/ANBIMA/ECB publish Svensson). Needs at least 6 anchor points and
  benefits from regularly spaced maturities.

Both parametric methods perform a sanity check on the fitted parameters and
return [`YieldCurveError::FitFailed`] if the optimizer lands on an implausible
mode (typical symptom with few anchors or anchors that don't match the
parametric shape). In that case, fall back to the cubic spline.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache License, Version 2.0](LICENSE-APACHE)
at your option.
