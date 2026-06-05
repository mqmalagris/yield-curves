//! End-to-end usage tests for the Phase 0 date toolkit (date, day count,
//! calendar, schedule) and its interplay with the curve, compounding, and bond
//! modules.
//!
//! These compile against the crate exactly as a downstream user would, so they
//! also guard the public re-export surface: if a type stops being re-exported
//! from the crate root, this file fails to compile.

use yield_curves::bond::{self, CashFlow};
use yield_curves::{
    discount_factor, easter, forward_rate, third_wednesday, Brazil, BusinessDayConvention,
    Calendar, Compounding, CubicSplineCurve, Date, DateGeneration, DayCount, JoinCalendar,
    JoinRule, LinearCurve, Period, Schedule, StubConvention, Target2, Unit, Weekday, WeekendsOnly,
    YieldCurveInterpolator,
};

fn approx(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() < eps
}

/// The Brazilian moat, end to end: build pillar dates on the B3/ANBIMA
/// calendar, convert to BUS/252 year fractions, fit a curve, and discount.
#[test]
fn brazilian_bus252_curve_workflow() {
    let cal = Brazil;
    let trade = Date::new(2025, 1, 2).unwrap(); // Jan 1 is a holiday; Jan 2 trades.
    assert!(cal.is_business_day(trade));

    // Pillar maturities as adjusted calendar offsets from the trade date.
    let tenors = [
        Period::months(3),
        Period::months(6),
        Period::years(1),
        Period::years(2),
        Period::years(5),
    ];
    let rates = [0.1315, 0.1330, 0.1345, 0.1360, 0.1380]; // decimal, ~Brazil DI level

    let mut points = Vec::new();
    let mut last_t = 0.0;
    for (tenor, &rate) in tenors.iter().zip(&rates) {
        let maturity = cal.adjust(trade + *tenor, BusinessDayConvention::Following);
        let t = cal.year_fraction_252(trade, maturity);
        // BUS/252 fractions must be strictly increasing across maturities.
        assert!(t > last_t, "t not increasing: {t} <= {last_t}");
        last_t = t;
        points.push((t, rate));
    }

    let curve = CubicSplineCurve::fit(&points).unwrap();

    // Interpolated rate between the 6M and 1Y pillars sits in a sane band.
    let mid_t = 0.5 * (points[1].0 + points[2].0);
    let r_mid = curve.rate_at(mid_t);
    assert!((0.13..=0.14).contains(&r_mid), "mid rate off: {r_mid}");

    // DI discount factors use annual compounding on the 252 time axis.
    let df_short = discount_factor(points[0].1, points[0].0, Compounding::annual());
    let df_long = discount_factor(points[4].1, points[4].0, Compounding::annual());
    assert!(df_short > 0.0 && df_short < 1.0);
    assert!(
        df_long > 0.0 && df_long < df_short,
        "discounts not monotone"
    );
}

/// A 2-year semiannual bond: generate the coupon schedule, derive accrual
/// times with a day count, then price and risk it with the bond module.
#[test]
fn bond_coupon_schedule_then_pricing() {
    let sched = Schedule::builder(
        Date::new(2025, 1, 15).unwrap(),
        Date::new(2027, 1, 15).unwrap(),
        Period::months(6),
    )
    .calendar(Box::new(WeekendsOnly))
    .convention(BusinessDayConvention::ModifiedFollowing)
    .build()
    .unwrap();

    // 5 dates → 4 semiannual coupons; none of the 15ths fall on a weekend.
    assert_eq!(sched.len(), 5);

    let settle = sched.effective_date();
    let dc = DayCount::Act365Fixed;
    let notional = 100.0;
    let coupon = 5.0; // 10% annual / 2

    let n = sched.len();
    let flows: Vec<CashFlow> = sched
        .dates()
        .iter()
        .enumerate()
        .skip(1) // settlement date carries no coupon
        .map(|(idx, &date)| {
            let amount = if idx == n - 1 {
                coupon + notional
            } else {
                coupon
            };
            CashFlow {
                t_years: dc.year_fraction(settle, date),
                amount,
            }
        })
        .collect();
    assert_eq!(flows.len(), 4);

    let ytm = 0.10;
    let comp = Compounding::semi_annual();
    let price = bond::price(&flows, ytm, comp).unwrap();
    // Coupon ≈ yield → priced near par.
    assert!((95.0..=105.0).contains(&price), "price off par: {price}");

    let mac = bond::macaulay_duration(&flows, ytm, comp).unwrap();
    let modd = bond::modified_duration(&flows, ytm, comp).unwrap();
    let cvx = bond::convexity(&flows, ytm, comp).unwrap();
    assert!(mac > 0.0 && mac < 2.0, "macaulay out of range: {mac}");
    assert!(modd < mac, "modified should be < macaulay for periodic");
    assert!(cvx > 0.0, "convexity should be positive");
}

/// Cross-currency payment: a date that is a holiday in either market must roll
/// off it under a joint calendar, and `advance` must skip it.
#[test]
fn multicurrency_joint_calendar() {
    let joint = JoinCalendar::new(
        vec![Box::new(Brazil), Box::new(Target2)],
        JoinRule::JoinHolidays,
    );

    // Corpus Christi (2025-06-19) is a Brazil holiday but a TARGET2 business day.
    let corpus = Date::new(2025, 6, 19).unwrap();
    assert!(Target2.is_business_day(corpus));
    assert!(
        !joint.is_business_day(corpus),
        "joint must drop Corpus Christi"
    );

    let rolled = joint.adjust(corpus, BusinessDayConvention::Following);
    assert!(joint.is_business_day(rolled));
    assert_eq!(rolled, Date::new(2025, 6, 20).unwrap());

    // Advancing one joint business day from the prior day skips Corpus Christi.
    let prev = Date::new(2025, 6, 18).unwrap();
    assert!(joint.is_business_day(prev));
    assert_eq!(joint.advance(prev, 1), Date::new(2025, 6, 20).unwrap());
}

/// Day-count year fractions are additive over a schedule's partition: the sum
/// over consecutive periods equals the fraction over the whole interval.
#[test]
fn daycount_telescopes_over_schedule() {
    let sched = Schedule::builder(
        Date::new(2023, 3, 31).unwrap(),
        Date::new(2026, 3, 31).unwrap(),
        Period::months(6),
    )
    .convention(BusinessDayConvention::Unadjusted)
    .termination_convention(BusinessDayConvention::Unadjusted)
    .build()
    .unwrap();

    let dates = sched.dates();
    let whole_start = sched.effective_date();
    let whole_end = sched.termination_date();

    for dc in [
        DayCount::Act365Fixed,
        DayCount::ActActIsda,
        DayCount::Act360,
    ] {
        let piecewise: f64 = dates.windows(2).map(|w| dc.year_fraction(w[0], w[1])).sum();
        let whole = dc.year_fraction(whole_start, whole_end);
        assert!(
            approx(piecewise, whole, 1e-12),
            "{} not additive: {piecewise} vs {whole}",
            dc.name()
        );
    }
}

/// Forward rate implied by two curve pillars is consistent with the zero rates
/// and lands between them for an upward-sloping curve.
#[test]
fn forward_rate_from_fitted_curve() {
    let points = [(0.5, 0.12), (1.0, 0.125), (2.0, 0.13), (5.0, 0.135)];
    let curve = LinearCurve::fit(&points).unwrap();

    let (t1, t2) = (1.0, 2.0);
    let r1 = curve.rate_at(t1);
    let r2 = curve.rate_at(t2);
    let fwd = forward_rate(r1, t1, r2, t2, Compounding::Continuous).unwrap();

    // Upward-sloping zeros ⇒ forward above the far zero rate.
    assert!(fwd > r2, "forward {fwd} should exceed long zero {r2}");
    // Continuous forward closed form: (r2·t2 − r1·t1)/(t2 − t1).
    assert!(approx(fwd, (r2 * t2 - r1 * t1) / (t2 - t1), 1e-12));
}

/// IMM futures strip: third-Wednesday generation lands every pillar on the
/// third Wednesday of its quarter.
#[test]
fn imm_third_wednesday_schedule() {
    let sched = Schedule::builder(
        Date::new(2025, 3, 19).unwrap(),
        Date::new(2026, 3, 18).unwrap(),
        Period::months(3),
    )
    .rule(DateGeneration::ThirdWednesday)
    .convention(BusinessDayConvention::Unadjusted)
    .termination_convention(BusinessDayConvention::Unadjusted)
    .stub(StubConvention::ShortFront)
    .build()
    .unwrap();

    for &date in sched.dates() {
        assert_eq!(date.weekday(), Weekday::Wednesday, "{date} not a Wednesday");
        assert_eq!(date, third_wednesday(date.year(), date.month()));
    }
}

/// Touches the remaining re-exported surface (Date arithmetic, Period units,
/// Easter, DayCount variants) so a dropped re-export breaks compilation.
#[test]
fn public_api_surface_smoke() {
    // Date arithmetic and accessors.
    let d = Date::new(2024, 2, 29).unwrap();
    assert!(d.is_leap_year());
    assert_eq!((d + Period::years(1)).ymd(), (2025, 2, 28)); // EOM clamp
    assert_eq!(Period::weeks(1).unit, Unit::Weeks);
    assert_eq!(d.weekday(), Weekday::Thursday);

    // Easter anchor and every DayCount variant is callable.
    assert_eq!(easter(2025), Date::new(2025, 4, 20).unwrap());
    let (a, b) = (
        Date::new(2025, 1, 1).unwrap(),
        Date::new(2025, 7, 1).unwrap(),
    );
    for dc in [
        DayCount::Act360,
        DayCount::Act365Fixed,
        DayCount::ActActIsda,
        DayCount::Thirty360Us,
        DayCount::ThirtyE360,
    ] {
        assert!(dc.year_fraction(a, b) > 0.0);
    }
}
