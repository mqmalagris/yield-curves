//! Calendar dates, weekdays, and date periods — Phase 0 foundations.
//!
//! Real-world curve construction needs dates before anything else: day-count
//! year fractions, holiday calendars, and coupon schedules all build on a
//! [`Date`] primitive. This module provides one with **zero dependencies**,
//! preserving the library's hard zero-dep constraint (the `time`/`chrono`
//! crates would each pull a dependency tree for a surface we barely use).
//!
//! # Representation
//!
//! A [`Date`] is a proleptic Gregorian calendar date stored as a single
//! `i32` *serial number* — the count of days since the Unix epoch
//! (1970-01-01). Serial storage makes comparison, ordering, and day
//! arithmetic trivial and exact, and makes [`Date`] `Copy`.
//!
//! Conversions between `(year, month, day)` and the serial number use Howard
//! Hinnant's `days_from_civil` / `civil_from_days` algorithms, which are exact
//! over the full `i32` serial range (roughly ±5.8 million days, ≈ ±16 000
//! years around 1970).
//!
//! # Example
//!
//! ```
//! use yield_curves::date::{Date, Period, Weekday};
//!
//! let settle = Date::new(2025, 1, 31).unwrap();
//! assert_eq!(settle.weekday(), Weekday::Friday);
//!
//! // Month arithmetic clamps to end-of-month: Jan 31 + 1 month = Feb 28.
//! let next = settle + Period::months(1);
//! assert_eq!(next, Date::new(2025, 2, 28).unwrap());
//!
//! // Day counts come straight off the serial difference.
//! assert_eq!(next.serial() - settle.serial(), 28);
//! ```

use std::fmt;
use std::ops::{Add, Sub};

/// Errors from date construction and arithmetic.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DateError {
    /// `(year, month, day)` is not a valid Gregorian calendar date.
    InvalidDate { year: i32, month: u32, day: u32 },
    /// A serial number or arithmetic result fell outside the supported range.
    OutOfRange(String),
}

impl fmt::Display for DateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidDate { year, month, day } => {
                write!(f, "invalid date: {year:04}-{month:02}-{day:02}")
            }
            Self::OutOfRange(msg) => write!(f, "date out of range: {msg}"),
        }
    }
}

impl std::error::Error for DateError {}

/// Day of the week, ISO order (Monday = 1 … Sunday = 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Weekday {
    /// ISO weekday number, Monday = 1 … Sunday = 7.
    #[must_use]
    pub fn number(self) -> u32 {
        match self {
            Self::Monday => 1,
            Self::Tuesday => 2,
            Self::Wednesday => 3,
            Self::Thursday => 4,
            Self::Friday => 5,
            Self::Saturday => 6,
            Self::Sunday => 7,
        }
    }

    /// True for Saturday and Sunday. Holiday calendars layer real non-business
    /// days on top of this; the weekend itself is calendar-independent.
    #[must_use]
    pub fn is_weekend(self) -> bool {
        matches!(self, Self::Saturday | Self::Sunday)
    }
}

/// Unit of a [`Period`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unit {
    Days,
    Weeks,
    Months,
    Years,
}

/// A signed span of calendar time, e.g. `3 months` or `-2 weeks`.
///
/// `Days`/`Weeks` add a fixed number of days. `Months`/`Years` are *calendar*
/// arithmetic: the day-of-month is preserved where possible and clamped to the
/// last day of the target month otherwise (so Jan 31 + 1 month = Feb 28, and
/// Feb 29 + 1 year = Feb 28 in a non-leap year).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Period {
    pub num: i32,
    pub unit: Unit,
}

impl Period {
    /// `n` days.
    #[must_use]
    pub fn days(n: i32) -> Self {
        Self {
            num: n,
            unit: Unit::Days,
        }
    }

    /// `n` weeks (7 days each).
    #[must_use]
    pub fn weeks(n: i32) -> Self {
        Self {
            num: n,
            unit: Unit::Weeks,
        }
    }

    /// `n` calendar months.
    #[must_use]
    pub fn months(n: i32) -> Self {
        Self {
            num: n,
            unit: Unit::Months,
        }
    }

    /// `n` calendar years.
    #[must_use]
    pub fn years(n: i32) -> Self {
        Self {
            num: n,
            unit: Unit::Years,
        }
    }
}

impl fmt::Display for Period {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let suffix = match self.unit {
            Unit::Days => 'D',
            Unit::Weeks => 'W',
            Unit::Months => 'M',
            Unit::Years => 'Y',
        };
        write!(f, "{}{}", self.num, suffix)
    }
}

/// A proleptic Gregorian calendar date stored as days since 1970-01-01.
///
/// Ordering and equality are by serial number, so they coincide with calendar
/// order. The type is `Copy` and cheap to pass by value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Date {
    serial: i32,
}

impl Date {
    /// Constructs a date from a calendar `(year, month, day)`.
    ///
    /// `month` is 1–12, `day` is 1–`days_in_month`. Returns
    /// [`DateError::InvalidDate`] for any out-of-range or nonexistent date
    /// (e.g. month 13, day 0, or February 30).
    ///
    /// # Errors
    ///
    /// Returns [`DateError::InvalidDate`] if the triple is not a real date.
    pub fn new(year: i32, month: u32, day: u32) -> Result<Self, DateError> {
        if !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
            return Err(DateError::InvalidDate { year, month, day });
        }
        Ok(Self {
            serial: days_from_civil(year, month, day),
        })
    }

    /// Constructs a date directly from its serial number (days since
    /// 1970-01-01). Always valid; provided for round-tripping and arithmetic.
    #[must_use]
    pub fn from_serial(serial: i32) -> Self {
        Self { serial }
    }

    /// The serial number: days since 1970-01-01 (negative before the epoch).
    ///
    /// Day-count year fractions for actual/N conventions are
    /// `(b.serial() - a.serial())` divided by the convention's basis.
    #[must_use]
    pub fn serial(self) -> i32 {
        self.serial
    }

    /// Calendar year.
    #[must_use]
    pub fn year(self) -> i32 {
        civil_from_days(self.serial).0
    }

    /// Calendar month, 1–12.
    #[must_use]
    pub fn month(self) -> u32 {
        civil_from_days(self.serial).1
    }

    /// Day of month, 1–31.
    #[must_use]
    pub fn day(self) -> u32 {
        civil_from_days(self.serial).2
    }

    /// `(year, month, day)` in one call (one serial conversion instead of three).
    #[must_use]
    pub fn ymd(self) -> (i32, u32, u32) {
        civil_from_days(self.serial)
    }

    /// Day of the week.
    #[must_use]
    pub fn weekday(self) -> Weekday {
        // 1970-01-01 (serial 0) was a Thursday. Shift so Monday maps to 0.
        match (self.serial + 3).rem_euclid(7) {
            0 => Weekday::Monday,
            1 => Weekday::Tuesday,
            2 => Weekday::Wednesday,
            3 => Weekday::Thursday,
            4 => Weekday::Friday,
            5 => Weekday::Saturday,
            _ => Weekday::Sunday,
        }
    }

    /// True if the date falls on a Saturday or Sunday. Holiday calendars (a
    /// later phase) extend this with named holidays per market.
    #[must_use]
    pub fn is_weekend(self) -> bool {
        self.weekday().is_weekend()
    }

    /// True if the date's year is a Gregorian leap year.
    #[must_use]
    pub fn is_leap_year(self) -> bool {
        is_leap(self.year())
    }

    /// Returns this date advanced by `n` days (negative moves backward).
    #[must_use]
    pub fn add_days(self, n: i32) -> Self {
        Self {
            serial: self.serial + n,
        }
    }

    /// Signed number of days from `self` to `other` (`other - self`).
    #[must_use]
    pub fn days_until(self, other: Self) -> i32 {
        other.serial - self.serial
    }

    /// The last day of this date's month, preserving year and month.
    #[must_use]
    pub fn end_of_month(self) -> Self {
        let (y, m, _) = self.ymd();
        Self {
            serial: days_from_civil(y, m, days_in_month(y, m)),
        }
    }

    /// True if this date is the last day of its month.
    #[must_use]
    pub fn is_end_of_month(self) -> bool {
        let (y, m, d) = self.ymd();
        d == days_in_month(y, m)
    }

    /// Returns this date advanced by `period` (negative periods move backward).
    ///
    /// Days/weeks add a fixed day count. Months/years use calendar arithmetic
    /// with end-of-month clamping (see [`Period`]).
    #[must_use]
    pub fn add_period(self, period: Period) -> Self {
        match period.unit {
            Unit::Days => self.add_days(period.num),
            Unit::Weeks => self.add_days(period.num * 7),
            Unit::Months => self.add_months(period.num),
            Unit::Years => self.add_months(period.num * 12),
        }
    }

    /// Adds `n` calendar months with end-of-month clamping.
    fn add_months(self, n: i32) -> Self {
        let (y, m, d) = self.ymd();
        // Zero-based month index from year 0, shifted by n.
        let total = (i64::from(y) * 12 + i64::from(m) - 1) + i64::from(n);
        let new_year = total.div_euclid(12) as i32;
        let new_month = (total.rem_euclid(12) + 1) as u32;
        let new_day = d.min(days_in_month(new_year, new_month));
        Self {
            serial: days_from_civil(new_year, new_month, new_day),
        }
    }
}

impl Add<Period> for Date {
    type Output = Date;
    fn add(self, period: Period) -> Date {
        self.add_period(period)
    }
}

impl Sub<Period> for Date {
    type Output = Date;
    fn sub(self, period: Period) -> Date {
        self.add_period(Period {
            num: -period.num,
            unit: period.unit,
        })
    }
}

impl Sub<Date> for Date {
    /// Difference in days (`self - rhs`).
    type Output = i32;
    fn sub(self, rhs: Date) -> i32 {
        self.serial - rhs.serial
    }
}

impl fmt::Display for Date {
    /// ISO 8601 `YYYY-MM-DD`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (y, m, d) = self.ymd();
        write!(f, "{y:04}-{m:02}-{d:02}")
    }
}

/// True if `year` is a Gregorian leap year.
#[must_use]
pub fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Number of days in `(year, month)`. `month` must be 1–12; out-of-range
/// months return 0.
#[must_use]
pub fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Days since 1970-01-01 for a valid civil date (Howard Hinnant's algorithm).
///
/// Correct for any date in the proleptic Gregorian calendar. The caller must
/// pass a valid `(year, month, day)`; [`Date::new`] validates before calling.
fn days_from_civil(year: i32, month: u32, day: u32) -> i32 {
    let y = i64::from(year) - i64::from(month <= 2);
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400; // [0, 399]
    let m = i64::from(month);
    let mp = if m > 2 { m - 3 } else { m + 9 }; // March = 0 … February = 11
    let doy = (153 * mp + 2) / 5 + i64::from(day) - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    (era * 146097 + doe - 719468) as i32
}

/// Civil `(year, month, day)` from days since 1970-01-01 (inverse of
/// [`days_from_civil`], Howard Hinnant's algorithm).
fn civil_from_days(serial: i32) -> (i32, u32, u32) {
    let z = i64::from(serial) + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    ((y + i64::from(m <= 2)) as i32, m as u32, d as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn epoch_serial_is_zero() {
        assert_eq!(Date::new(1970, 1, 1).unwrap().serial(), 0);
        assert_eq!(Date::new(1970, 1, 2).unwrap().serial(), 1);
        assert_eq!(Date::new(1969, 12, 31).unwrap().serial(), -1);
    }

    #[test]
    fn ymd_roundtrips_over_wide_range() {
        // Walk every day across two centuries and check the serial round-trips
        // through civil_from_days back to the same (y, m, d).
        let start = Date::new(1900, 1, 1).unwrap().serial();
        let end = Date::new(2100, 12, 31).unwrap().serial();
        for s in start..=end {
            let (y, m, d) = civil_from_days(s);
            assert_eq!(
                days_from_civil(y, m, d),
                s,
                "roundtrip failed at serial {s}"
            );
        }
    }

    #[test]
    fn leap_year_rules() {
        assert!(is_leap(2000)); // divisible by 400
        assert!(!is_leap(1900)); // divisible by 100, not 400
        assert!(is_leap(2024));
        assert!(!is_leap(2023));
        assert!(Date::new(2024, 2, 29).unwrap().is_leap_year());
    }

    #[test]
    fn days_in_month_handles_february() {
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2023, 2), 28);
        assert_eq!(days_in_month(2024, 4), 30);
        assert_eq!(days_in_month(2024, 12), 31);
        assert_eq!(days_in_month(2024, 13), 0);
    }

    #[test]
    fn weekday_known_anchors() {
        assert_eq!(Date::new(1970, 1, 1).unwrap().weekday(), Weekday::Thursday);
        assert_eq!(Date::new(2000, 1, 1).unwrap().weekday(), Weekday::Saturday);
        assert_eq!(Date::new(2024, 2, 29).unwrap().weekday(), Weekday::Thursday);
        assert_eq!(Date::new(2025, 6, 5).unwrap().weekday(), Weekday::Thursday);
    }

    #[test]
    fn weekend_detection() {
        assert!(Date::new(2000, 1, 1).unwrap().is_weekend()); // Saturday
        assert!(Date::new(2000, 1, 2).unwrap().is_weekend()); // Sunday
        assert!(!Date::new(2000, 1, 3).unwrap().is_weekend()); // Monday
    }

    #[test]
    fn rejects_invalid_dates() {
        assert!(Date::new(2023, 2, 29).is_err()); // not a leap year
        assert!(Date::new(2024, 0, 1).is_err()); // month 0
        assert!(Date::new(2024, 13, 1).is_err()); // month 13
        assert!(Date::new(2024, 1, 0).is_err()); // day 0
        assert!(Date::new(2024, 4, 31).is_err()); // April has 30
        assert!(Date::new(2024, 2, 29).is_ok()); // leap-year Feb 29 is fine
    }

    #[test]
    fn day_arithmetic_and_difference() {
        let a = Date::new(2024, 1, 1).unwrap();
        let b = a.add_days(31);
        assert_eq!(b, Date::new(2024, 2, 1).unwrap());
        assert_eq!(a.days_until(b), 31);
        assert_eq!(b - a, 31);
        assert_eq!(a.add_days(-1), Date::new(2023, 12, 31).unwrap());
    }

    #[test]
    fn add_months_clamps_end_of_month() {
        let jan31 = Date::new(2021, 1, 31).unwrap();
        assert_eq!(jan31 + Period::months(1), Date::new(2021, 2, 28).unwrap());

        let jan31_leap = Date::new(2020, 1, 31).unwrap();
        assert_eq!(
            jan31_leap + Period::months(1),
            Date::new(2020, 2, 29).unwrap()
        );

        // Crossing a year boundary.
        assert_eq!(
            Date::new(2024, 11, 30).unwrap() + Period::months(3),
            Date::new(2025, 2, 28).unwrap()
        );
    }

    #[test]
    fn add_years_handles_leap_day() {
        let leap = Date::new(2020, 2, 29).unwrap();
        assert_eq!(leap + Period::years(1), Date::new(2021, 2, 28).unwrap());
        assert_eq!(leap + Period::years(4), Date::new(2024, 2, 29).unwrap());
    }

    #[test]
    fn subtract_period_moves_backward() {
        let d = Date::new(2025, 3, 31).unwrap();
        assert_eq!(d - Period::months(1), Date::new(2025, 2, 28).unwrap());
        assert_eq!(d - Period::days(1), Date::new(2025, 3, 30).unwrap());
        assert_eq!(d - Period::weeks(1), Date::new(2025, 3, 24).unwrap());
    }

    #[test]
    fn add_period_weeks_and_days() {
        let d = Date::new(2025, 1, 1).unwrap();
        assert_eq!(d + Period::weeks(2), Date::new(2025, 1, 15).unwrap());
        assert_eq!(d + Period::days(10), Date::new(2025, 1, 11).unwrap());
    }

    #[test]
    fn end_of_month_helpers() {
        let mid = Date::new(2024, 2, 15).unwrap();
        assert_eq!(mid.end_of_month(), Date::new(2024, 2, 29).unwrap());
        assert!(!mid.is_end_of_month());
        assert!(Date::new(2024, 2, 29).unwrap().is_end_of_month());
        assert!(Date::new(2025, 4, 30).unwrap().is_end_of_month());
    }

    #[test]
    fn ordering_matches_calendar() {
        let a = Date::new(2024, 1, 1).unwrap();
        let b = Date::new(2024, 6, 1).unwrap();
        let c = Date::new(2025, 1, 1).unwrap();
        assert!(a < b);
        assert!(b < c);
        let mut v = vec![c, a, b];
        v.sort();
        assert_eq!(v, vec![a, b, c]);
    }

    #[test]
    fn display_is_iso() {
        assert_eq!(Date::new(2025, 6, 5).unwrap().to_string(), "2025-06-05");
        assert_eq!(Date::new(999, 1, 9).unwrap().to_string(), "0999-01-09");
        assert_eq!(Period::months(3).to_string(), "3M");
        assert_eq!(Period::days(-5).to_string(), "-5D");
    }

    #[test]
    fn serial_roundtrip_via_from_serial() {
        let d = Date::new(2030, 7, 4).unwrap();
        assert_eq!(Date::from_serial(d.serial()), d);
    }
}
