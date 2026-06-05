# yield-curves

[![CI](https://github.com/mqmalagris/yield-curves/actions/workflows/ci.yml/badge.svg)](https://github.com/mqmalagris/yield-curves/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/yield-curves.svg)](https://crates.io/crates/yield-curves)
[![docs.rs](https://img.shields.io/docsrs/yield-curves)](https://docs.rs/yield-curves)
[![License: MIT OR Apache-2.0](https://img.shields.io/crates/l/yield-curves.svg)](#license)
[![MSRV: 1.75](https://img.shields.io/badge/MSRV-1.75-blue.svg)](#)
[![SLSA Level 3](https://slsa.dev/images/gh-badge-level3.svg)](https://slsa.dev)

Yield curve interpolation and parametric fitting for fixed income, in pure
Rust with **zero dependencies**.

Built for quant developers and risk engineers who need curve fitting without
QuantLib's C++ build chain or a Python numerical stack as a transitive
dependency. Small, auditable, embeddable in WASM and serverless cold starts.

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

## Bond pricing *(new in 0.3)*

The `bond` module computes price, duration, convexity and par yield from a
list of cash flows plus a YTM. All inputs in **decimal form** (`0.07`, not
`7`). Only `Continuous` and `Periodic(n)` compounding are accepted —
`Simple` is rejected because it isn't standard for multi-period bonds.

```rust
use std::num::NonZeroU32;
use yield_curves::bond::{macaulay_duration, modified_duration, convexity, par_yield, CashFlow};
use yield_curves::compounding::Compounding;
use yield_curves::{CubicSplineCurve, YieldCurveInterpolator};

// 4-year, 5% annual coupon, principal 100, semi-annual payments.
let flows: Vec<CashFlow> = (1..=8)
    .map(|k| CashFlow {
        t_years: f64::from(k) / 2.0,
        amount: if k == 8 { 102.5 } else { 2.5 },
    })
    .collect();

let ytm = 0.05;
let comp = Compounding::Periodic(NonZeroU32::new(2).unwrap());

let d_mac = macaulay_duration(&flows, ytm, comp).unwrap();
let d_mod = modified_duration(&flows, ytm, comp).unwrap();
let c     = convexity(&flows, ytm, comp).unwrap();

// Par yield: coupon that prices a 5y semi-annual bond at par given a curve.
let curve = CubicSplineCurve::fit(&[(1.0, 0.05), (5.0, 0.055), (10.0, 0.06)]).unwrap();
let par   = par_yield(&curve, 5.0, NonZeroU32::new(2).unwrap(), comp).unwrap();
```

No dependency on `ndarray`, `argmin`, or any numerical crate. The Nelder-Mead
simplex optimizer used by the parametric fits is implemented internally.

## Dates, calendars & schedules *(new in 0.4)*

A zero-dependency date toolkit so you can build the curve's time axis without
pulling `chrono` / `time` or QuantLib:

- **`date`** — proleptic Gregorian `Date` (stored as an `i32` serial),
  `Period`, `Weekday`, end-of-month-aware arithmetic.
- **`daycount`** — `DayCount` year fractions per ISDA 2006 §4.16: ACT/360,
  ACT/365F, ACT/ACT-ISDA, 30/360 (Bond Basis), 30E/360.
- **`calendar`** — `Calendar` trait with business-day adjustment, `Brazil`
  (ANBIMA national), `Target2`, `WeekendsOnly`, `JoinCalendar`, and the
  **BUS/252** year fraction.
- **`schedule`** — coupon/pillar date generation with stubs, end-of-month
  rolling, and IMM (third-Wednesday) dates.

```rust
use yield_curves::{Brazil, BusinessDayConvention, Calendar, Date, DayCount, Period, Schedule};

let cal = Brazil;
let trade = Date::new(2025, 1, 2).unwrap();

// Roll a 6-month maturity onto a B3 business day, then get its BUS/252 time.
let maturity = cal.adjust(trade + Period::months(6), BusinessDayConvention::Following);
let t = cal.year_fraction_252(trade, maturity); // business days / 252

// Day-count year fraction for an accrual period.
let accrual = DayCount::Act365Fixed.year_fraction(trade, maturity);

// Semiannual coupon schedule for a 2-year bond.
let sched = Schedule::builder(
    Date::new(2025, 1, 15).unwrap(),
    Date::new(2027, 1, 15).unwrap(),
    Period::months(6),
)
.calendar(Box::new(Brazil))
.convention(BusinessDayConvention::ModifiedFollowing)
.build()
.unwrap();
assert_eq!(sched.len(), 5);
```

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

The x-axis is **time in years**. As of 0.4 the `calendar` and `daycount`
modules do this conversion for you (e.g. `Brazil.year_fraction_252(trade,
maturity)` or `DayCount::Act365Fixed.year_fraction(a, b)`); the manual factors
are:

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

## Supply chain — SLSA Level 3

Releases are built by GitHub Actions and ship a SLSA Level 3 provenance
attestation alongside the `.crate` artifact on every GitHub Release tag.
Verify with [`slsa-verifier`](https://github.com/slsa-framework/slsa-verifier):

```bash
slsa-verifier verify-artifact \
  --provenance-path yield-curves-provenance.intoto.jsonl \
  --source-uri github.com/mqmalagris/yield-curves \
  yield-curves-<version>.crate
```

The same `.crate` is what is uploaded to crates.io.

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache License, Version 2.0](LICENSE-APACHE)
at your option.
