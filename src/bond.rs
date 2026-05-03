//! Bond pricing primitives — Tier 2 of the crate's evolution (v0.3).
//!
//! Operates on a list of cash flows plus a yield-to-maturity (YTM). Use
//! [`Compounding::Continuous`] or [`Compounding::Periodic`]; bond pricing
//! with simple interest is not standard and is rejected up front.
//!
//! Rates and the YTM passed in must be in **decimal form** (`0.07`, not `7`).
//! The functions are convention-agnostic: time is in years and the YTM is
//! consumed as-is by [`crate::compounding::discount_factor`].
//!
//! # Functions
//!
//! - [`price`] — Σ CF · DF(t, ytm)
//! - [`macaulay_duration`] — weighted-average time-to-cashflow
//! - [`modified_duration`] — `−(1/P)·dP/dy`
//! - [`convexity`] — `(1/P)·d²P/dy²`
//! - [`par_yield`] — given a curve, the coupon that prices a bond at par
//!
//! # Example
//!
//! ```
//! use std::num::NonZeroU32;
//! use yield_curves::bond::{macaulay_duration, CashFlow};
//! use yield_curves::compounding::Compounding;
//!
//! // 2-year bond, 5% annual coupon, principal 100, semi-annual payments.
//! let flows = [
//!     CashFlow { t_years: 0.5, amount: 2.5 },
//!     CashFlow { t_years: 1.0, amount: 2.5 },
//!     CashFlow { t_years: 1.5, amount: 2.5 },
//!     CashFlow { t_years: 2.0, amount: 102.5 },
//! ];
//! let ytm = 0.05;
//! let comp = Compounding::Periodic(NonZeroU32::new(2).unwrap());
//! let d = macaulay_duration(&flows, ytm, comp).unwrap();
//! assert!((1.85..=1.95).contains(&d));
//! ```

use std::num::NonZeroU32;

use crate::{
    compounding::{discount_factor, Compounding},
    YieldCurveError, YieldCurveInterpolator,
};

/// One cash flow of a bond: time-to-payment in years, amount in the same
/// scale as the principal (no normalization performed).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CashFlow {
    pub t_years: f64,
    pub amount: f64,
}

/// Present value `P = Σ CF_i · DF(t_i, ytm)`.
///
/// Errors with [`YieldCurveError::InvalidPoint`] for non-finite inputs,
/// negative `t_years`, or [`Compounding::Simple`] (not standard for bonds).
pub fn price(flows: &[CashFlow], ytm: f64, comp: Compounding) -> Result<f64, YieldCurveError> {
    validate_inputs(flows, ytm, comp)?;
    let mut p = 0.0;
    for cf in flows {
        p += cf.amount * discount_factor(ytm, cf.t_years, comp);
    }
    if !p.is_finite() {
        return Err(YieldCurveError::InvalidPoint(
            "computed price is non-finite".into(),
        ));
    }
    Ok(p)
}

/// Macaulay duration: `Σ t · CF · DF / P`. Units: years.
///
/// Same formula across continuous and periodic compounding — duration is the
/// weighted-average time at which the holder receives the cash, weights are
/// each flow's PV share of total price.
pub fn macaulay_duration(
    flows: &[CashFlow],
    ytm: f64,
    comp: Compounding,
) -> Result<f64, YieldCurveError> {
    let p = price(flows, ytm, comp)?;
    if p == 0.0 {
        return Err(YieldCurveError::InvalidPoint(
            "price is zero, duration undefined".into(),
        ));
    }
    let mut weighted = 0.0;
    for cf in flows {
        weighted += cf.t_years * cf.amount * discount_factor(ytm, cf.t_years, comp);
    }
    Ok(weighted / p)
}

/// Modified duration: `−(1/P) · dP/dy`. Units: years.
///
/// Compounding-dependent:
/// - [`Compounding::Continuous`]: equals Macaulay.
/// - [`Compounding::Periodic`] (`n`): `D_mac / (1 + ytm/n)`.
pub fn modified_duration(
    flows: &[CashFlow],
    ytm: f64,
    comp: Compounding,
) -> Result<f64, YieldCurveError> {
    let d_mac = macaulay_duration(flows, ytm, comp)?;
    let factor = match comp {
        Compounding::Continuous => 1.0,
        Compounding::Periodic(n) => 1.0 + ytm / f64::from(n.get()),
        Compounding::Simple => unreachable!("simple rejected in price()"),
    };
    Ok(d_mac / factor)
}

/// Convexity: `(1/P) · d²P/dy²`. Units: years².
///
/// Compounding-dependent:
/// - [`Compounding::Continuous`]: `Σ t² · CF · DF / P`.
/// - [`Compounding::Periodic`] (`n`): `Σ t · (t + 1/n) · CF · DF / [(1 + ytm/n)² · P]`.
pub fn convexity(flows: &[CashFlow], ytm: f64, comp: Compounding) -> Result<f64, YieldCurveError> {
    let p = price(flows, ytm, comp)?;
    if p == 0.0 {
        return Err(YieldCurveError::InvalidPoint(
            "price is zero, convexity undefined".into(),
        ));
    }
    let mut sum = 0.0;
    let result = match comp {
        Compounding::Continuous => {
            for cf in flows {
                sum += cf.t_years * cf.t_years * cf.amount * discount_factor(ytm, cf.t_years, comp);
            }
            sum / p
        }
        Compounding::Periodic(n) => {
            let n_f = f64::from(n.get());
            let one_plus = 1.0 + ytm / n_f;
            for cf in flows {
                sum += cf.t_years
                    * (cf.t_years + 1.0 / n_f)
                    * cf.amount
                    * discount_factor(ytm, cf.t_years, comp);
            }
            sum / (one_plus * one_plus * p)
        }
        Compounding::Simple => unreachable!("simple rejected in price()"),
    };
    if !result.is_finite() {
        return Err(YieldCurveError::InvalidPoint(
            "computed convexity is non-finite".into(),
        ));
    }
    Ok(result)
}

/// Par yield: the annual coupon `c` (in decimal) such that a coupon-paying
/// bond with principal 1 and `freq` payments per year prices at par given
/// the supplied spot curve.
///
/// Formula: `c = freq · (1 − DF(T)) / Σ DF(t_k)`, where `t_k = k/freq` for
/// `k = 1..=⌊T·freq⌋`.
///
/// Curve rates are consumed via [`YieldCurveInterpolator::rate_at`] and
/// passed to [`discount_factor`] without unit conversion — supply the curve
/// in the same units as `comp` expects (decimal).
pub fn par_yield<C: YieldCurveInterpolator>(
    curve: &C,
    t_years: f64,
    freq: NonZeroU32,
    comp: Compounding,
) -> Result<f64, YieldCurveError> {
    if !t_years.is_finite() || t_years <= 0.0 {
        return Err(YieldCurveError::InvalidPoint(format!(
            "t_years must be positive and finite: {t_years}"
        )));
    }
    if matches!(comp, Compounding::Simple) {
        return Err(YieldCurveError::InvalidPoint(
            "par yield requires Continuous or Periodic compounding".into(),
        ));
    }
    let n = f64::from(freq.get());
    let total_periods = (t_years * n).round() as u32;
    if total_periods == 0 {
        return Err(YieldCurveError::InvalidPoint(format!(
            "t_years × freq must round to ≥ 1 (t={t_years}, freq={n})"
        )));
    }

    let mut sum_df = 0.0;
    for k in 1..=total_periods {
        let t_k = f64::from(k) / n;
        let r_k = curve.rate_at(t_k);
        sum_df += discount_factor(r_k, t_k, comp);
    }
    if sum_df <= 0.0 {
        return Err(YieldCurveError::InvalidPoint(
            "sum of discount factors is non-positive".into(),
        ));
    }

    let r_t = curve.rate_at(t_years);
    let df_t = discount_factor(r_t, t_years, comp);
    let par_coupon = n * (1.0 - df_t) / sum_df;
    if !par_coupon.is_finite() {
        return Err(YieldCurveError::InvalidPoint(
            "computed par yield is non-finite".into(),
        ));
    }
    Ok(par_coupon)
}

fn validate_inputs(flows: &[CashFlow], ytm: f64, comp: Compounding) -> Result<(), YieldCurveError> {
    if !ytm.is_finite() {
        return Err(YieldCurveError::InvalidPoint(format!(
            "ytm not finite: {ytm}"
        )));
    }
    if matches!(comp, Compounding::Simple) {
        return Err(YieldCurveError::InvalidPoint(
            "bond pricing requires Continuous or Periodic compounding".into(),
        ));
    }
    for cf in flows {
        if !cf.t_years.is_finite() || !cf.amount.is_finite() {
            return Err(YieldCurveError::InvalidPoint(format!(
                "cash flow not finite (t={}, amount={})",
                cf.t_years, cf.amount
            )));
        }
        if cf.t_years < 0.0 {
            return Err(YieldCurveError::InvalidPoint(format!(
                "negative t_years: {}",
                cf.t_years
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LinearCurve;

    fn approx_eq(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() < eps
    }

    fn zero_coupon(t: f64) -> [CashFlow; 1] {
        [CashFlow {
            t_years: t,
            amount: 100.0,
        }]
    }

    /// 5% annual coupon bond, principal 100, 4 years, semi-annual payments.
    fn semi_annual_5pct_4y() -> Vec<CashFlow> {
        let mut v = Vec::with_capacity(8);
        for k in 1..=8 {
            let t = f64::from(k) / 2.0;
            let amount = if k == 8 { 102.5 } else { 2.5 };
            v.push(CashFlow { t_years: t, amount });
        }
        v
    }

    #[test]
    fn price_zero_coupon_continuous_matches_df() {
        let flows = zero_coupon(2.0);
        let p = price(&flows, 0.05, Compounding::Continuous).unwrap();
        let expected = 100.0 * (-0.05f64 * 2.0).exp();
        assert!(
            approx_eq(p, expected, 1e-12),
            "got {}, expected {}",
            p,
            expected
        );
    }

    #[test]
    fn macaulay_of_zero_coupon_equals_maturity() {
        let flows = zero_coupon(3.5);
        let d = macaulay_duration(&flows, 0.04, Compounding::Continuous).unwrap();
        assert!(approx_eq(d, 3.5, 1e-12));
        let d = macaulay_duration(&flows, 0.04, Compounding::annual()).unwrap();
        assert!(approx_eq(d, 3.5, 1e-12));
    }

    #[test]
    fn modified_equals_macaulay_for_continuous() {
        let flows = semi_annual_5pct_4y();
        let m = macaulay_duration(&flows, 0.05, Compounding::Continuous).unwrap();
        let d = modified_duration(&flows, 0.05, Compounding::Continuous).unwrap();
        assert!(approx_eq(m, d, 1e-12));
    }

    #[test]
    fn modified_lt_macaulay_for_periodic() {
        let flows = semi_annual_5pct_4y();
        let comp = Compounding::semi_annual();
        let m = macaulay_duration(&flows, 0.05, comp).unwrap();
        let d = modified_duration(&flows, 0.05, comp).unwrap();
        assert!(d < m);
        // D_mod = D_mac / (1 + 0.05/2) = D_mac / 1.025
        assert!(approx_eq(d, m / 1.025, 1e-12));
    }

    #[test]
    fn convexity_zero_coupon_continuous() {
        // Convexity of a zero-coupon under continuous compounding = T².
        let flows = zero_coupon(4.0);
        let c = convexity(&flows, 0.05, Compounding::Continuous).unwrap();
        assert!(approx_eq(c, 16.0, 1e-10));
    }

    #[test]
    fn convexity_periodic_zero_coupon_known_form() {
        // Periodic(n) zero-coupon at maturity T: C = T·(T + 1/n) / (1 + y/n)²
        let flows = zero_coupon(2.0);
        let comp = Compounding::semi_annual();
        let y = 0.06;
        let n = 2.0;
        let one_plus: f64 = 1.0 + y / n;
        let expected = 2.0 * (2.0 + 1.0 / n) / (one_plus * one_plus);
        let c = convexity(&flows, y, comp).unwrap();
        assert!(approx_eq(c, expected, 1e-10));
    }

    #[test]
    fn rejects_simple_compounding() {
        let flows = zero_coupon(1.0);
        let err = price(&flows, 0.05, Compounding::Simple).unwrap_err();
        assert!(matches!(err, YieldCurveError::InvalidPoint(_)));
    }

    #[test]
    fn rejects_negative_time() {
        let flows = [CashFlow {
            t_years: -1.0,
            amount: 100.0,
        }];
        let err = price(&flows, 0.05, Compounding::Continuous).unwrap_err();
        assert!(matches!(err, YieldCurveError::InvalidPoint(_)));
    }

    #[test]
    fn par_yield_flat_curve_equals_curve_rate_continuous() {
        // Flat 5% spot curve under continuous compounding → par yield ≈ 5%
        // (limit as freq → ∞; for any finite n it differs slightly because
        // continuous DF + periodic coupon dates compose into a slightly
        // different par coupon).
        let curve = LinearCurve::fit(&[(0.5, 0.05), (10.0, 0.05)]).unwrap();
        let par = par_yield(
            &curve,
            5.0,
            NonZeroU32::new(2).unwrap(),
            Compounding::Continuous,
        )
        .unwrap();
        // Should be within 0.5% of the flat rate (semi-annual, continuous DF).
        assert!((par - 0.05).abs() < 0.005, "par={par}");
    }

    #[test]
    fn par_yield_flat_curve_periodic_matches_curve_rate() {
        // Flat 6% with periodic compounding matching coupon freq:
        // par yield should equal the flat curve rate exactly.
        let curve = LinearCurve::fit(&[(0.5, 0.06), (10.0, 0.06)]).unwrap();
        let n = NonZeroU32::new(2).unwrap();
        let par = par_yield(&curve, 5.0, n, Compounding::Periodic(n)).unwrap();
        assert!(approx_eq(par, 0.06, 1e-12), "par={par}");
    }

    #[test]
    fn par_yield_rejects_non_positive_t() {
        let curve = LinearCurve::fit(&[(0.5, 0.05), (10.0, 0.05)]).unwrap();
        let err = par_yield(
            &curve,
            0.0,
            NonZeroU32::new(2).unwrap(),
            Compounding::Continuous,
        )
        .unwrap_err();
        assert!(matches!(err, YieldCurveError::InvalidPoint(_)));
    }

    #[test]
    fn par_yield_rejects_simple_compounding() {
        let curve = LinearCurve::fit(&[(0.5, 0.05), (10.0, 0.05)]).unwrap();
        let err = par_yield(
            &curve,
            5.0,
            NonZeroU32::new(2).unwrap(),
            Compounding::Simple,
        )
        .unwrap_err();
        assert!(matches!(err, YieldCurveError::InvalidPoint(_)));
    }
}
