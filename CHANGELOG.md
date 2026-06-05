# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] — 2026-06-05

### Added

- **`date` module** — zero-dependency proleptic Gregorian `Date` stored as an
  `i32` serial (days since 1970-01-01), with `Period` / `Unit`, `Weekday`,
  `is_leap` / `days_in_month` helpers, and end-of-month-clamping month/year
  arithmetic.
- **`daycount` module** — `DayCount` year fractions per ISDA 2006 §4.16:
  ACT/360, ACT/365 Fixed, ACT/ACT ISDA, 30/360 Bond Basis, and 30E/360
  Eurobond Basis.
- **`calendar` module** — object-safe `Calendar` trait with business-day
  adjustment (`Following`, `ModifiedFollowing`, `Preceding`,
  `ModifiedPreceding`, `Unadjusted`), `advance`, and `business_days_between`;
  `Brazil` (ANBIMA national financial calendar), `Target2`, and `WeekendsOnly`
  calendars; `JoinCalendar` (`JoinHolidays` / `JoinBusinessDays`); a Gregorian
  `easter` computus; and the **BUS/252** year fraction.
- **`schedule` module** — `Schedule` and a builder generating coupon/pillar
  date sequences (`Backward`, `Forward`, `Zero`, `ThirdWednesday` rules), stub
  handling (`ShortFront` / `LongFront` / `ShortBack` / `LongBack`),
  end-of-month rolling, and a `third_wednesday` IMM helper.
- End-to-end usage integration tests covering cross-module workflows
  (Brazilian BUS/252 curve, bond coupon schedule + pricing, joint calendars,
  day-count additivity, IMM strips).
- `CONTRIBUTING.md` with development setup, PR guidelines, and security
  reporting policy.
- `CHANGELOG.md`.
- README badges for CI, crates.io, docs.rs, license, MSRV, and SLSA Level 3.
- README hook line clarifying target audience (quant developers and risk
  engineers wanting curve fitting without the QuantLib build chain).

### Changed

- `Cargo.toml` `description` trimmed to fit crates.io listing previews and
  added `homepage` and `documentation` fields.

## [0.3.0] — 2026-04-29

### Added

- `bond` module: bond pricing, Macaulay duration, modified duration,
  convexity, and par yield from a list of cash flows.
- `par_yield` helper that takes a yield-curve interpolator and returns the
  coupon rate that prices a bond at par given its maturity and payment
  frequency.
- CI/CD pipeline: GitHub Actions running `cargo fmt`, `clippy`, `test`,
  doctest, and `cargo doc --no-deps` on every PR.
- Release workflow producing SLSA Level 3 build provenance attestations
  alongside the `.crate` artifact for every GitHub Release tag, verifiable
  with `slsa-verifier`.

### Changed

- Bond-pricing inputs are in **decimal form** (`0.07`, not `7`). Only
  `Compounding::Continuous` and `Compounding::Periodic(n)` are accepted by
  bond functions; `Compounding::Simple` is rejected because it isn't
  standard for multi-period bonds.

## [0.2.0] — 2026-03-18

### Added

- **PCHIP interpolation** (Fritsch-Carlson monotone cubic Hermite). C¹
  continuous; preserves monotonicity and never overshoots adjacent anchors.
  Useful when natural cubic spline produces spurious humps on sparse data.
- `compounding` module turning interpolated rates into discount factors and
  forward rates under continuous, periodic (`Periodic(n)` covers annual /
  semi / quarterly / monthly / Brazil-252), and simple compounding.
- `discount_factor`, `forward_rate`, and conversion helpers as free
  functions outside the curve trait — rate unit (% vs decimal) and
  compounding convention are caller concerns.

## [0.1.0] — 2026-02-21

### Added

- Initial release.
- `YieldCurveInterpolator` trait.
- Linear, Cubic Spline (natural, C² via Thomas algorithm),
  Nelson-Siegel (1987), and Svensson (1994) interpolation methods.
- Internal Nelder-Mead simplex optimizer for parametric fits — no
  dependency on `argmin` or `ndarray`.
- Sanity checks on fitted parametric parameters returning
  `YieldCurveError::FitFailed` when the optimizer lands on an implausible
  mode.
- Flat extrapolation outside the observed anchor range.
- Dual MIT / Apache-2.0 licensing.

[Unreleased]: https://github.com/mqmalagris/yield-curves/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/mqmalagris/yield-curves/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/mqmalagris/yield-curves/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/mqmalagris/yield-curves/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/mqmalagris/yield-curves/releases/tag/v0.1.0
