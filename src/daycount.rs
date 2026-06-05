//! Day-count conventions — year fractions between two [`Date`]s.
//!
//! A day-count convention answers "how much of a year elapsed between two
//! dates?". Accrued interest, discount-factor time axes, and schedule year
//! fractions all depend on the convention, and the conventions disagree: the
//! same calendar interval can be 0.500, 0.503, or 0.497 of a year depending on
//! the rule. This module implements the conventions exactly per ISDA 2006
//! definitions (Section 4.16) so results reconcile with market systems.
//!
//! # Conventions
//!
//! - [`DayCount::Act360`] — actual days / 360 (money-market standard: USD/EUR
//!   deposits, most floating legs).
//! - [`DayCount::Act365Fixed`] — actual days / 365, ignoring leap years
//!   (GBP money market, many fixed legs).
//! - [`DayCount::ActActIsda`] — actual days, split across the calendar-year
//!   boundary so leap-year days count as 1/366 and others as 1/365.
//! - [`DayCount::Thirty360Us`] — 30/360 "Bond Basis" (ISDA 4.16(f)): each month
//!   treated as 30 days, year as 360 (US corporate/municipal bonds).
//! - [`DayCount::ThirtyE360`] — 30E/360 "Eurobond Basis" (ISDA 4.16(g)): like
//!   Bond Basis with a simpler day-31 rule (Eurobonds).
//!
//! BUS/252 (Brazilian business-day / 252) is intentionally **not** here: it
//! counts *business* days, which requires a holiday calendar. It lands with the
//! calendar module, where it can take a `&Calendar`.
//!
//! # Sign convention
//!
//! `year_fraction(start, end)` expects `start <= end` and returns a
//! non-negative fraction. A reversed pair returns the negative of the forward
//! fraction, so `yf(a, b) == -yf(b, a)`.
//!
//! # Example
//!
//! ```
//! use yield_curves::date::Date;
//! use yield_curves::daycount::DayCount;
//!
//! let start = Date::new(2025, 1, 1).unwrap();
//! let end = Date::new(2025, 7, 1).unwrap(); // 181 actual days
//!
//! assert!((DayCount::Act360.year_fraction(start, end) - 181.0 / 360.0).abs() < 1e-12);
//! assert!((DayCount::Thirty360Us.year_fraction(start, end) - 0.5).abs() < 1e-12);
//! ```

use std::fmt;

use crate::date::{is_leap, Date};

/// A day-count convention. See the [module docs](self) for the precise rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DayCount {
    /// Actual days / 360.
    Act360,
    /// Actual days / 365 (leap years ignored).
    Act365Fixed,
    /// ACT/ACT ISDA: actual days, leap days weighted 1/366, others 1/365.
    ActActIsda,
    /// 30/360 "Bond Basis" (ISDA 2006 §4.16(f)). Also called 30/360 US.
    Thirty360Us,
    /// 30E/360 "Eurobond Basis" (ISDA 2006 §4.16(g)).
    ThirtyE360,
}

impl DayCount {
    /// Year fraction between `start` and `end`.
    ///
    /// Non-negative when `start <= end`; the negative of the forward value when
    /// reversed. Returns `0.0` when the dates are equal.
    #[must_use]
    pub fn year_fraction(self, start: Date, end: Date) -> f64 {
        if start == end {
            return 0.0;
        }
        if end < start {
            return -self.year_fraction(end, start);
        }
        match self {
            Self::Act360 => f64::from(end - start) / 360.0,
            Self::Act365Fixed => f64::from(end - start) / 365.0,
            Self::ActActIsda => act_act_isda(start, end),
            Self::Thirty360Us => f64::from(days_30360_us(start, end)) / 360.0,
            Self::ThirtyE360 => f64::from(days_30e360(start, end)) / 360.0,
        }
    }

    /// The number of days the convention counts between `start` and `end`.
    ///
    /// For the actual conventions this is the calendar day difference; for the
    /// 30/360 family it is the adjusted 30-day-month count. ACT/ACT has no
    /// single integer day count independent of the year boundary, so it returns
    /// the plain calendar difference (use [`year_fraction`](Self::year_fraction)
    /// for ACT/ACT).
    #[must_use]
    pub fn day_count(self, start: Date, end: Date) -> i32 {
        match self {
            Self::Act360 | Self::Act365Fixed | Self::ActActIsda => end - start,
            Self::Thirty360Us => days_30360_us(start, end),
            Self::ThirtyE360 => days_30e360(start, end),
        }
    }

    /// Stable identifier, e.g. `"act/360"`, `"30e/360"`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Act360 => "act/360",
            Self::Act365Fixed => "act/365f",
            Self::ActActIsda => "act/act-isda",
            Self::Thirty360Us => "30/360-us",
            Self::ThirtyE360 => "30e/360",
        }
    }
}

impl fmt::Display for DayCount {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// ACT/ACT ISDA year fraction for `start < end` (ISDA 2006 §4.16(b)).
///
/// Splits the interval at calendar-year boundaries: the portion in each year is
/// divided by that year's length (366 for leap years, 365 otherwise), and whole
/// intervening years each contribute exactly 1.0.
fn act_act_isda(start: Date, end: Date) -> f64 {
    let y1 = start.year();
    let y2 = end.year();
    let denom = |y: i32| if is_leap(y) { 366.0 } else { 365.0 };

    if y1 == y2 {
        return f64::from(end - start) / denom(y1);
    }

    // Stub from `start` to the first day of the next year.
    let next_year_start = Date::new(y1 + 1, 1, 1).expect("Jan 1 is valid");
    let first = f64::from(next_year_start - start) / denom(y1);

    // Stub from the first day of `end`'s year to `end`.
    let end_year_start = Date::new(y2, 1, 1).expect("Jan 1 is valid");
    let last = f64::from(end - end_year_start) / denom(y2);

    // Whole calendar years strictly between y1 and y2 each count as 1.0.
    let whole = f64::from(y2 - y1 - 1);

    first + whole + last
}

/// 30/360 "Bond Basis" adjusted day count (ISDA 2006 §4.16(f)) for `start < end`.
///
/// - `d1` becomes 30 if it is 31.
/// - `d2` becomes 30 if it is 31 *and* `d1` (after its own adjustment) is > 29.
fn days_30360_us(start: Date, end: Date) -> i32 {
    let (y1, m1, mut d1) = (start.year(), start.month() as i32, start.day() as i32);
    let (y2, m2, mut d2) = (end.year(), end.month() as i32, end.day() as i32);

    if d1 == 31 {
        d1 = 30;
    }
    if d2 == 31 && d1 > 29 {
        d2 = 30;
    }

    360 * (y2 - y1) + 30 * (m2 - m1) + (d2 - d1)
}

/// 30E/360 "Eurobond Basis" adjusted day count (ISDA 2006 §4.16(g)) for
/// `start < end`. Both `d1` and `d2` become 30 if they are 31 — no condition on
/// the other day.
fn days_30e360(start: Date, end: Date) -> i32 {
    let (y1, m1, mut d1) = (start.year(), start.month() as i32, start.day() as i32);
    let (y2, m2, mut d2) = (end.year(), end.month() as i32, end.day() as i32);

    if d1 == 31 {
        d1 = 30;
    }
    if d2 == 31 {
        d2 = 30;
    }

    360 * (y2 - y1) + 30 * (m2 - m1) + (d2 - d1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> Date {
        Date::new(y, m, day).unwrap()
    }

    fn approx(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    #[test]
    fn act_360_basic() {
        // 2025-01-01 → 2025-07-01 is 181 actual days.
        let yf = DayCount::Act360.year_fraction(d(2025, 1, 1), d(2025, 7, 1));
        assert!(approx(yf, 181.0 / 360.0));
    }

    #[test]
    fn act_365_fixed_basic() {
        let yf = DayCount::Act365Fixed.year_fraction(d(2025, 1, 1), d(2025, 7, 1));
        assert!(approx(yf, 181.0 / 365.0));
    }

    #[test]
    fn act_365_fixed_ignores_leap() {
        // A full leap year is 366/365 > 1 under ACT/365F.
        let yf = DayCount::Act365Fixed.year_fraction(d(2024, 1, 1), d(2025, 1, 1));
        assert!(approx(yf, 366.0 / 365.0));
    }

    #[test]
    fn act_act_isda_same_year_non_leap() {
        let yf = DayCount::ActActIsda.year_fraction(d(2025, 1, 1), d(2025, 7, 1));
        assert!(approx(yf, 181.0 / 365.0));
    }

    #[test]
    fn act_act_isda_full_leap_year_is_one() {
        let yf = DayCount::ActActIsda.year_fraction(d(2024, 1, 1), d(2025, 1, 1));
        assert!(approx(yf, 1.0));
    }

    #[test]
    fn act_act_isda_isda_reference_example() {
        // ISDA 2006 worked example: 2003-11-01 → 2004-05-01 = 0.497724380...
        // 61 days of 2003 (/365) + 121 days of 2004 (/366).
        let yf = DayCount::ActActIsda.year_fraction(d(2003, 11, 1), d(2004, 5, 1));
        let expected = 61.0 / 365.0 + 121.0 / 366.0;
        assert!(approx(yf, expected));
        assert!(approx(yf, 0.497_724_380_165_289));
    }

    #[test]
    fn thirty_360_us_half_year() {
        // 30*(7-1) + (1-1) = 180 → 0.5.
        let yf = DayCount::Thirty360Us.year_fraction(d(2025, 1, 1), d(2025, 7, 1));
        assert!(approx(yf, 0.5));
    }

    #[test]
    fn thirty_360_us_adjusts_d1_31() {
        // 2025-01-31 → 2025-02-28: d1 31→30, d2 28. 30*1 + (28-30) = 28.
        let n = DayCount::Thirty360Us.day_count(d(2025, 1, 31), d(2025, 2, 28));
        assert_eq!(n, 28);
    }

    #[test]
    fn thirty_360_us_vs_30e_differ_on_d2_31() {
        // 2025-01-15 → 2025-03-31.
        // US: d1=15 (not 31, not >29 path), d2=31 but d1<=29 → d2 stays 31.
        //     30*2 + (31-15) = 76.
        // 30E: d2 31→30. 30*2 + (30-15) = 75.
        let us = DayCount::Thirty360Us.day_count(d(2025, 1, 15), d(2025, 3, 31));
        let e = DayCount::ThirtyE360.day_count(d(2025, 1, 15), d(2025, 3, 31));
        assert_eq!(us, 76);
        assert_eq!(e, 75);
    }

    #[test]
    fn thirty_e_360_adjusts_both_31() {
        // 2025-01-31 → 2025-03-31: both 31→30. 30*2 + 0 = 60.
        let n = DayCount::ThirtyE360.day_count(d(2025, 1, 31), d(2025, 3, 31));
        assert_eq!(n, 60);
    }

    #[test]
    fn same_date_is_zero() {
        for dc in [
            DayCount::Act360,
            DayCount::Act365Fixed,
            DayCount::ActActIsda,
            DayCount::Thirty360Us,
            DayCount::ThirtyE360,
        ] {
            assert_eq!(dc.year_fraction(d(2025, 3, 15), d(2025, 3, 15)), 0.0);
        }
    }

    #[test]
    fn reversed_is_negated() {
        let a = d(2024, 2, 10);
        let b = d(2025, 8, 20);
        for dc in [
            DayCount::Act360,
            DayCount::Act365Fixed,
            DayCount::ActActIsda,
            DayCount::Thirty360Us,
            DayCount::ThirtyE360,
        ] {
            assert!(approx(dc.year_fraction(a, b), -dc.year_fraction(b, a)));
        }
    }

    #[test]
    fn act_act_isda_multi_year_span() {
        // 2022-07-01 → 2025-07-01: partial 2022 + whole 2023, 2024 + partial 2025.
        let yf = DayCount::ActActIsda.year_fraction(d(2022, 7, 1), d(2025, 7, 1));
        // 2022-07-01→2023-01-01 = 184 days /365; +1 (2023) +1 (2024);
        // 2025-01-01→2025-07-01 = 181 days /365.
        let expected = 184.0 / 365.0 + 2.0 + 181.0 / 365.0;
        assert!(approx(yf, expected));
    }

    #[test]
    fn name_and_display() {
        assert_eq!(DayCount::Act360.name(), "act/360");
        assert_eq!(DayCount::ThirtyE360.to_string(), "30e/360");
    }
}
