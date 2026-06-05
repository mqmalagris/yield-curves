//! Holiday calendars, business-day adjustment, and BUS/252 — Phase 0.
//!
//! Curve construction needs to know which days are *business days*: coupon
//! dates roll off non-business days, and the Brazilian BUS/252 day count
//! divides business days by 252. This module provides a [`Calendar`] trait with
//! the adjustment machinery as default methods, concrete market calendars, a
//! [`JoinCalendar`] for multi-currency curves, and the BUS/252 year fraction.
//!
//! # Calendars
//!
//! - [`Brazil`] — ANBIMA national financial calendar (the basis for BUS/252 DI
//!   and NTN-B curves). Fixed national holidays plus Easter-derived Carnival,
//!   Good Friday, and Corpus Christi.
//! - [`Target2`] — Eurosystem TARGET2 settlement calendar.
//! - [`WeekendsOnly`] — Saturdays and Sundays only; a dependency-free baseline.
//!
//! `Brazil` follows the ANBIMA national list, **not** B3 exchange-only
//! closures (e.g. Dec 24 / Dec 31), because curve work uses the ANBIMA basis.
//!
//! # Business-day conventions
//!
//! [`BusinessDayConvention`] adjusts a date that lands on a non-business day:
//! `Following`, `ModifiedFollowing`, `Preceding`, `ModifiedPreceding`, and
//! `Unadjusted`. The modified variants avoid crossing a month boundary.
//!
//! # Example
//!
//! ```
//! use yield_curves::date::Date;
//! use yield_curves::calendar::{Brazil, Calendar, BusinessDayConvention};
//!
//! let cal = Brazil;
//! // 2025-04-21 is Tiradentes (a national holiday).
//! assert!(!cal.is_business_day(Date::new(2025, 4, 21).unwrap()));
//! // Roll a Saturday forward to the next business day.
//! let sat = Date::new(2025, 5, 31).unwrap();
//! let adj = cal.adjust(sat, BusinessDayConvention::Following);
//! assert!(cal.is_business_day(adj));
//! ```

use crate::date::Date;

/// How to move a date that falls on a non-business day onto a business day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BusinessDayConvention {
    /// Leave the date unchanged.
    Unadjusted,
    /// Roll forward to the next business day.
    Following,
    /// Roll forward, unless that crosses into the next month — then roll back.
    ModifiedFollowing,
    /// Roll backward to the previous business day.
    Preceding,
    /// Roll backward, unless that crosses into the previous month — then forward.
    ModifiedPreceding,
}

/// A holiday calendar: which dates are business days, plus date arithmetic that
/// respects them.
///
/// Implementors only need [`name`](Calendar::name) and
/// [`is_business_day`](Calendar::is_business_day); the rest are provided. The
/// trait is object-safe, so calendars compose via [`JoinCalendar`].
pub trait Calendar {
    /// Stable identifier, e.g. `"Brazil"`, `"TARGET2"`.
    fn name(&self) -> &'static str;

    /// True if `date` is a trading/settlement business day for this market
    /// (neither a weekend nor a holiday).
    fn is_business_day(&self, date: Date) -> bool;

    /// True if `date` is a non-weekend holiday. Weekends are not holidays.
    fn is_holiday(&self, date: Date) -> bool {
        !date.is_weekend() && !self.is_business_day(date)
    }

    /// Adjusts `date` onto a business day per `conv`. A date that is already a
    /// business day is returned unchanged.
    fn adjust(&self, date: Date, conv: BusinessDayConvention) -> Date {
        match conv {
            BusinessDayConvention::Unadjusted => date,
            BusinessDayConvention::Following => self.roll(date, 1),
            BusinessDayConvention::Preceding => self.roll(date, -1),
            BusinessDayConvention::ModifiedFollowing => {
                let rolled = self.roll(date, 1);
                if rolled.month() != date.month() {
                    self.roll(date, -1)
                } else {
                    rolled
                }
            }
            BusinessDayConvention::ModifiedPreceding => {
                let rolled = self.roll(date, -1);
                if rolled.month() != date.month() {
                    self.roll(date, 1)
                } else {
                    rolled
                }
            }
        }
    }

    /// Advances `date` by `n` business days (negative `n` moves backward).
    /// `n == 0` returns `date` unchanged, even if it is not a business day.
    fn advance(&self, date: Date, n: i32) -> Date {
        if n == 0 {
            return date;
        }
        let step = if n > 0 { 1 } else { -1 };
        let mut remaining = n.abs();
        let mut d = date;
        while remaining > 0 {
            d = d.add_days(step);
            if self.is_business_day(d) {
                remaining -= 1;
            }
        }
        d
    }

    /// Number of business days in the half-open interval `[start, end)` —
    /// includes `start`, excludes `end` (the QuantLib default). Returns the
    /// negative count when `end < start`, and `0` when the dates are equal.
    fn business_days_between(&self, start: Date, end: Date) -> i32 {
        if start == end {
            return 0;
        }
        if end < start {
            return -self.business_days_between(end, start);
        }
        let mut count = 0;
        let mut d = start;
        while d < end {
            if self.is_business_day(d) {
                count += 1;
            }
            d = d.add_days(1);
        }
        count
    }

    /// BUS/252 year fraction: business days in `[start, end)` divided by 252.
    ///
    /// This is the Brazilian fixed-income convention. It lives on the calendar
    /// (not [`crate::daycount::DayCount`]) because it needs the holiday set.
    fn year_fraction_252(&self, start: Date, end: Date) -> f64 {
        f64::from(self.business_days_between(start, end)) / 252.0
    }

    /// Internal: roll `date` in `step` direction until it is a business day.
    /// Returns `date` unchanged if it is already a business day. Not intended
    /// for direct use — call [`adjust`](Calendar::adjust) instead.
    #[doc(hidden)]
    fn roll(&self, date: Date, step: i32) -> Date {
        let mut d = date;
        while !self.is_business_day(d) {
            d = d.add_days(step);
        }
        d
    }
}

/// Gregorian Easter Sunday for `year` (Meeus/Jones/Butcher computus).
///
/// Carnival, Good Friday, Corpus Christi, and Easter Monday are all fixed
/// offsets from this date.
#[must_use]
pub fn easter(year: i32) -> Date {
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let month = (h + l - 7 * m + 114) / 31; // 3 = March, 4 = April
    let day = ((h + l - 7 * m + 114) % 31) + 1;
    Date::new(year, month as u32, day as u32).expect("computus yields a valid date")
}

/// Saturdays and Sundays are non-business; no named holidays.
#[derive(Debug, Clone, Copy, Default)]
pub struct WeekendsOnly;

impl Calendar for WeekendsOnly {
    fn name(&self) -> &'static str {
        "WeekendsOnly"
    }
    fn is_business_day(&self, date: Date) -> bool {
        !date.is_weekend()
    }
}

/// ANBIMA national financial calendar for Brazil — the BUS/252 basis.
///
/// Holidays: New Year (Jan 1), Tiradentes (Apr 21), Labour (May 1),
/// Independence (Sep 7), Our Lady of Aparecida (Oct 12), All Souls (Nov 2),
/// Republic (Nov 15), Black Awareness (Nov 20, national from 2024), Christmas
/// (Dec 25); plus Carnival Monday/Tuesday, Good Friday, and Corpus Christi
/// derived from [`easter`].
#[derive(Debug, Clone, Copy, Default)]
pub struct Brazil;

impl Brazil {
    fn is_named_holiday(date: Date) -> bool {
        let (y, m, d) = date.ymd();
        if matches!(
            (m, d),
            (1, 1) | (4, 21) | (5, 1) | (9, 7) | (10, 12) | (11, 2) | (11, 15) | (12, 25)
        ) {
            return true;
        }
        // Black Awareness Day became a national holiday in 2024 (Law 14.759/2023).
        if m == 11 && d == 20 && y >= 2024 {
            return true;
        }
        let easter = easter(y);
        let s = date.serial();
        s == easter.add_days(-48).serial() // Carnival Monday
            || s == easter.add_days(-47).serial() // Carnival Tuesday
            || s == easter.add_days(-2).serial() // Good Friday
            || s == easter.add_days(60).serial() // Corpus Christi
    }
}

impl Calendar for Brazil {
    fn name(&self) -> &'static str {
        "Brazil"
    }
    fn is_business_day(&self, date: Date) -> bool {
        !date.is_weekend() && !Self::is_named_holiday(date)
    }
}

/// Eurosystem TARGET2 settlement calendar.
///
/// Holidays: New Year (Jan 1), Good Friday, Easter Monday, Labour (May 1),
/// Christmas (Dec 25), and Dec 26.
#[derive(Debug, Clone, Copy, Default)]
pub struct Target2;

impl Target2 {
    fn is_named_holiday(date: Date) -> bool {
        let (_, m, d) = date.ymd();
        if matches!((m, d), (1, 1) | (5, 1) | (12, 25) | (12, 26)) {
            return true;
        }
        let easter = easter(date.year());
        let s = date.serial();
        s == easter.add_days(-2).serial() // Good Friday
            || s == easter.add_days(1).serial() // Easter Monday
    }
}

impl Calendar for Target2 {
    fn name(&self) -> &'static str {
        "TARGET2"
    }
    fn is_business_day(&self, date: Date) -> bool {
        !date.is_weekend() && !Self::is_named_holiday(date)
    }
}

/// How to combine the business days of several calendars.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JoinRule {
    /// A date is a business day only if it is a business day in **every**
    /// calendar (the union of holidays). Standard for multi-currency curves.
    JoinHolidays,
    /// A date is a business day if it is a business day in **any** calendar
    /// (the intersection of holidays).
    JoinBusinessDays,
}

/// Combines multiple calendars under a [`JoinRule`].
///
/// Used for cross-currency curves where a payment must avoid the holidays of
/// both currencies (`JoinHolidays`).
pub struct JoinCalendar {
    calendars: Vec<Box<dyn Calendar>>,
    rule: JoinRule,
}

impl JoinCalendar {
    /// Builds a joint calendar from `calendars` under `rule`.
    ///
    /// At least one calendar should be supplied; an empty set degenerates
    /// (`JoinHolidays` makes every day a business day, `JoinBusinessDays` makes
    /// none).
    #[must_use]
    pub fn new(calendars: Vec<Box<dyn Calendar>>, rule: JoinRule) -> Self {
        Self { calendars, rule }
    }
}

impl Calendar for JoinCalendar {
    fn name(&self) -> &'static str {
        "Joint"
    }
    fn is_business_day(&self, date: Date) -> bool {
        match self.rule {
            JoinRule::JoinHolidays => self.calendars.iter().all(|c| c.is_business_day(date)),
            JoinRule::JoinBusinessDays => self.calendars.iter().any(|c| c.is_business_day(date)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> Date {
        Date::new(y, m, day).unwrap()
    }

    #[test]
    fn easter_known_values() {
        assert_eq!(easter(2024), d(2024, 3, 31));
        assert_eq!(easter(2025), d(2025, 4, 20));
        assert_eq!(easter(2000), d(2000, 4, 23));
        assert_eq!(easter(2027), d(2027, 3, 28));
    }

    #[test]
    fn weekends_only_marks_weekends() {
        let cal = WeekendsOnly;
        assert!(!cal.is_business_day(d(2025, 1, 4))); // Saturday
        assert!(!cal.is_business_day(d(2025, 1, 5))); // Sunday
        assert!(cal.is_business_day(d(2025, 1, 6))); // Monday
        assert!(cal.is_business_day(d(2025, 1, 1))); // New Year is a business day here
    }

    #[test]
    fn brazil_fixed_holidays() {
        let cal = Brazil;
        assert!(!cal.is_business_day(d(2025, 1, 1))); // New Year (Wed)
        assert!(!cal.is_business_day(d(2025, 4, 21))); // Tiradentes (Mon)
        assert!(cal.is_holiday(d(2025, 4, 21)));
        assert!(cal.is_business_day(d(2025, 1, 2))); // Thursday, ordinary day
    }

    #[test]
    fn brazil_moving_holidays_2025() {
        let cal = Brazil;
        // Easter 2025 = Apr 20.
        assert!(!cal.is_business_day(d(2025, 3, 3))); // Carnival Monday
        assert!(!cal.is_business_day(d(2025, 3, 4))); // Carnival Tuesday
        assert!(!cal.is_business_day(d(2025, 4, 18))); // Good Friday
        assert!(!cal.is_business_day(d(2025, 6, 19))); // Corpus Christi
    }

    #[test]
    fn brazil_black_awareness_from_2024() {
        let cal = Brazil;
        // 2023-11-20 is a Monday and an ordinary business day (pre-national).
        assert!(cal.is_business_day(d(2023, 11, 20)));
        // 2024-11-20 is a Wednesday and now a national holiday.
        assert!(!cal.is_business_day(d(2024, 11, 20)));
    }

    #[test]
    fn target2_holidays_2025() {
        let cal = Target2;
        assert!(!cal.is_business_day(d(2025, 1, 1)));
        assert!(!cal.is_business_day(d(2025, 4, 18))); // Good Friday
        assert!(!cal.is_business_day(d(2025, 4, 21))); // Easter Monday
        assert!(!cal.is_business_day(d(2025, 5, 1)));
        assert!(!cal.is_business_day(d(2025, 12, 26)));
        assert!(cal.is_business_day(d(2025, 12, 24))); // ordinary in TARGET2
    }

    #[test]
    fn following_rolls_forward() {
        let cal = WeekendsOnly;
        // 2025-01-04 is Saturday → Monday Jan 6.
        let adj = cal.adjust(d(2025, 1, 4), BusinessDayConvention::Following);
        assert_eq!(adj, d(2025, 1, 6));
    }

    #[test]
    fn preceding_rolls_backward() {
        let cal = WeekendsOnly;
        // 2025-01-05 is Sunday → Friday Jan 3.
        let adj = cal.adjust(d(2025, 1, 5), BusinessDayConvention::Preceding);
        assert_eq!(adj, d(2025, 1, 3));
    }

    #[test]
    fn modified_following_stays_in_month() {
        let cal = WeekendsOnly;
        // 2025-05-31 is Saturday. Following → Jun 2 (next month) →
        // ModifiedFollowing rolls back to Fri May 30.
        let saturday = d(2025, 5, 31);
        assert_eq!(
            cal.adjust(saturday, BusinessDayConvention::Following),
            d(2025, 6, 2)
        );
        let mf = cal.adjust(saturday, BusinessDayConvention::ModifiedFollowing);
        assert_eq!(mf, d(2025, 5, 30));
        assert_eq!(mf.month(), 5);
    }

    #[test]
    fn modified_preceding_stays_in_month() {
        let cal = WeekendsOnly;
        // 2025-06-01 is Sunday. Preceding → May 30 (prev month) →
        // ModifiedPreceding rolls forward to Mon Jun 2.
        let sunday = d(2025, 6, 1);
        assert_eq!(
            cal.adjust(sunday, BusinessDayConvention::Preceding),
            d(2025, 5, 30)
        );
        let mp = cal.adjust(sunday, BusinessDayConvention::ModifiedPreceding);
        assert_eq!(mp, d(2025, 6, 2));
        assert_eq!(mp.month(), 6);
    }

    #[test]
    fn unadjusted_is_identity() {
        let cal = Brazil;
        let holiday = d(2025, 1, 1);
        assert_eq!(
            cal.adjust(holiday, BusinessDayConvention::Unadjusted),
            holiday
        );
    }

    #[test]
    fn adjust_business_day_unchanged() {
        let cal = WeekendsOnly;
        let wed = d(2025, 1, 8);
        assert_eq!(cal.adjust(wed, BusinessDayConvention::Following), wed);
        assert_eq!(cal.adjust(wed, BusinessDayConvention::Preceding), wed);
    }

    #[test]
    fn advance_business_days() {
        let cal = WeekendsOnly;
        // Friday Jan 3 2025 + 1 business day = Monday Jan 6.
        assert_eq!(cal.advance(d(2025, 1, 3), 1), d(2025, 1, 6));
        // Monday Jan 6 - 1 business day = Friday Jan 3.
        assert_eq!(cal.advance(d(2025, 1, 6), -1), d(2025, 1, 3));
        // Zero is identity even on a weekend.
        assert_eq!(cal.advance(d(2025, 1, 4), 0), d(2025, 1, 4));
    }

    #[test]
    fn business_days_between_half_open() {
        let cal = WeekendsOnly;
        // [Mon Jan 6, Fri Jan 10) = Mon,Tue,Wed,Thu = 4.
        assert_eq!(cal.business_days_between(d(2025, 1, 6), d(2025, 1, 10)), 4);
        // Equal dates = 0; reversed = negative.
        assert_eq!(cal.business_days_between(d(2025, 1, 6), d(2025, 1, 6)), 0);
        assert_eq!(cal.business_days_between(d(2025, 1, 10), d(2025, 1, 6)), -4);
    }

    #[test]
    fn business_days_between_differs_by_calendar() {
        // [2025-01-01, 2025-01-08): Jan 1 Wed,2,3 Fri, 4 Sat,5 Sun,6 Mon,7 Tue.
        let start = d(2025, 1, 1);
        let end = d(2025, 1, 8);
        // WeekendsOnly counts 1,2,3,6,7 = 5.
        assert_eq!(WeekendsOnly.business_days_between(start, end), 5);
        // Brazil drops Jan 1 (holiday) → 4.
        assert_eq!(Brazil.business_days_between(start, end), 4);
    }

    #[test]
    fn bus252_year_fraction() {
        let cal = WeekendsOnly;
        // 5 business days / 252.
        let yf = cal.year_fraction_252(d(2025, 1, 1), d(2025, 1, 8));
        assert!((yf - 5.0 / 252.0).abs() < 1e-12);
    }

    #[test]
    fn join_holidays_is_intersection_of_business_days() {
        let joint = JoinCalendar::new(
            vec![Box::new(Brazil), Box::new(Target2)],
            JoinRule::JoinHolidays,
        );
        // Corpus Christi 2025-06-19 (Thu): Brazil holiday, TARGET2 business.
        // JoinHolidays → not a business day.
        assert!(!joint.is_business_day(d(2025, 6, 19)));
        // Dec 26 2025 (Fri): TARGET2 holiday, Brazil business → not business.
        assert!(!joint.is_business_day(d(2025, 12, 26)));
        // An ordinary weekday in both is a business day.
        assert!(joint.is_business_day(d(2025, 1, 2)));
    }

    #[test]
    fn join_business_days_is_union_of_business_days() {
        let joint = JoinCalendar::new(
            vec![Box::new(Brazil), Box::new(Target2)],
            JoinRule::JoinBusinessDays,
        );
        // Corpus Christi: business in TARGET2 → business under JoinBusinessDays.
        assert!(joint.is_business_day(d(2025, 6, 19)));
        // Dec 26: business in Brazil → business.
        assert!(joint.is_business_day(d(2025, 12, 26)));
        // New Year is a holiday in both → not a business day.
        assert!(!joint.is_business_day(d(2025, 1, 1)));
        // Weekend remains non-business under either rule.
        assert!(!joint.is_business_day(d(2025, 1, 4)));
    }
}
