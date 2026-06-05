//! Schedule generation — coupon/pillar date sequences. Phase 0 finisher.
//!
//! A [`Schedule`] is the ordered set of dates that partition a deal's life into
//! accrual periods: coupon dates for a bond, fixing/payment dates for a swap
//! leg, pillar dates for a bootstrap. It composes everything earlier in Phase
//! 0 — [`Date`], [`Period`], a [`Calendar`], and a [`BusinessDayConvention`].
//!
//! # Generation
//!
//! Dates are generated **from an anchor** (`termination` for backward rules,
//! `effective` for forward) as `anchor ± i·tenor`, never iteratively, so they
//! do not drift when month arithmetic clamps an end-of-month day. The raw grid
//! is then adjusted onto business days and consecutive duplicates are removed.
//!
//! - [`DateGeneration::Backward`] — regular dates measured back from
//!   termination; an uneven first period becomes a front stub.
//! - [`DateGeneration::Forward`] — measured forward from effective; an uneven
//!   final period becomes a back stub.
//! - [`DateGeneration::Zero`] — `[effective, termination]` only (tenor ignored).
//! - [`DateGeneration::ThirdWednesday`] — each date snapped to the third
//!   Wednesday of its month (IMM/futures dates).
//!
//! # Stubs
//!
//! When the tenor does not divide the interval evenly, [`StubConvention`]
//! controls the odd period: `ShortFront`/`LongFront` for backward generation,
//! `ShortBack`/`LongBack` for forward.
//!
//! # Example
//!
//! ```
//! use yield_curves::date::{Date, Period};
//! use yield_curves::calendar::{Brazil, BusinessDayConvention};
//! use yield_curves::schedule::Schedule;
//!
//! let sched = Schedule::builder(
//!     Date::new(2024, 1, 15).unwrap(),
//!     Date::new(2025, 1, 15).unwrap(),
//!     Period::months(6),
//! )
//! .calendar(Box::new(Brazil))
//! .convention(BusinessDayConvention::ModifiedFollowing)
//! .build()
//! .unwrap();
//!
//! assert_eq!(sched.len(), 3); // 2024-01-15, 2024-07-15, 2025-01-15
//! assert_eq!(sched.effective_date(), Date::new(2024, 1, 15).unwrap());
//! ```

use std::fmt;
use std::ops::Index;

use crate::calendar::{BusinessDayConvention, Calendar, WeekendsOnly};
use crate::date::{Date, Period, Unit};

/// How the regular date grid is generated and where a stub may fall.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DateGeneration {
    /// Regular dates measured backward from the termination date.
    Backward,
    /// Regular dates measured forward from the effective date.
    Forward,
    /// No intermediate dates: just effective and termination.
    Zero,
    /// Like [`Backward`](Self::Backward) but each date is snapped to the third
    /// Wednesday of its month (IMM dates).
    ThirdWednesday,
}

/// Placement and length of the irregular period when the tenor does not divide
/// the interval evenly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StubConvention {
    /// Short irregular first period (backward generation).
    ShortFront,
    /// Long irregular first period (backward generation).
    LongFront,
    /// Short irregular final period (forward generation).
    ShortBack,
    /// Long irregular final period (forward generation).
    LongBack,
}

/// Errors from schedule construction.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScheduleError {
    /// `termination` is not strictly after `effective`.
    EmptyRange { effective: Date, termination: Date },
    /// The tenor's magnitude is not positive.
    NonPositiveTenor(i32),
    /// The stub convention's direction does not match the generation rule
    /// (e.g. a back stub with backward generation).
    StubDirectionMismatch {
        rule: DateGeneration,
        stub: StubConvention,
    },
}

impl fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyRange {
                effective,
                termination,
            } => write!(
                f,
                "termination {termination} must be after effective {effective}"
            ),
            Self::NonPositiveTenor(n) => write!(f, "tenor must be positive, got {n}"),
            Self::StubDirectionMismatch { rule, stub } => {
                write!(f, "stub {stub:?} is incompatible with rule {rule:?}")
            }
        }
    }
}

impl std::error::Error for ScheduleError {}

/// The third Wednesday of `(year, month)` — the standard IMM/futures roll date.
#[must_use]
pub fn third_wednesday(year: i32, month: u32) -> Date {
    let first = Date::new(year, month, 1).expect("first of month is valid");
    // Days from the 1st to the first Wednesday (ISO Wednesday = 3), then +2 weeks.
    let offset = (3 + 7 - first.weekday().number()) % 7;
    first.add_days(offset as i32 + 14)
}

/// A generated sequence of dates partitioning `[effective, termination]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedule {
    dates: Vec<Date>,
}

impl Schedule {
    /// Starts a [`ScheduleBuilder`] for the interval `[effective, termination]`
    /// with the regular period `tenor`.
    #[must_use]
    pub fn builder(effective: Date, termination: Date, tenor: Period) -> ScheduleBuilder {
        ScheduleBuilder::new(effective, termination, tenor)
    }

    /// The generated dates in ascending order.
    #[must_use]
    pub fn dates(&self) -> &[Date] {
        &self.dates
    }

    /// Number of dates (one more than the number of periods).
    #[must_use]
    pub fn len(&self) -> usize {
        self.dates.len()
    }

    /// Always false — a valid schedule has at least two dates.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.dates.is_empty()
    }

    /// The first (effective) date.
    #[must_use]
    pub fn effective_date(&self) -> Date {
        self.dates[0]
    }

    /// The last (termination) date.
    #[must_use]
    pub fn termination_date(&self) -> Date {
        self.dates[self.dates.len() - 1]
    }

    /// Iterator over the dates.
    pub fn iter(&self) -> std::slice::Iter<'_, Date> {
        self.dates.iter()
    }
}

impl Index<usize> for Schedule {
    type Output = Date;
    fn index(&self, i: usize) -> &Date {
        &self.dates[i]
    }
}

impl<'a> IntoIterator for &'a Schedule {
    type Item = &'a Date;
    type IntoIter = std::slice::Iter<'a, Date>;
    fn into_iter(self) -> Self::IntoIter {
        self.dates.iter()
    }
}

/// Builder for a [`Schedule`]. Defaults: [`WeekendsOnly`] calendar,
/// `ModifiedFollowing` for both conventions, `Backward` generation, no
/// end-of-month rolling, and the natural short stub for the rule.
pub struct ScheduleBuilder {
    effective: Date,
    termination: Date,
    tenor: Period,
    calendar: Box<dyn Calendar>,
    convention: BusinessDayConvention,
    termination_convention: BusinessDayConvention,
    end_of_month: bool,
    rule: DateGeneration,
    stub: Option<StubConvention>,
}

impl ScheduleBuilder {
    fn new(effective: Date, termination: Date, tenor: Period) -> Self {
        Self {
            effective,
            termination,
            tenor,
            calendar: Box::new(WeekendsOnly),
            convention: BusinessDayConvention::ModifiedFollowing,
            termination_convention: BusinessDayConvention::ModifiedFollowing,
            end_of_month: false,
            rule: DateGeneration::Backward,
            stub: None,
        }
    }

    /// Sets the holiday calendar used for adjustment.
    #[must_use]
    pub fn calendar(mut self, calendar: Box<dyn Calendar>) -> Self {
        self.calendar = calendar;
        self
    }

    /// Sets the business-day convention for all dates except termination.
    #[must_use]
    pub fn convention(mut self, convention: BusinessDayConvention) -> Self {
        self.convention = convention;
        self
    }

    /// Sets the business-day convention applied to the termination date.
    #[must_use]
    pub fn termination_convention(mut self, convention: BusinessDayConvention) -> Self {
        self.termination_convention = convention;
        self
    }

    /// Enables end-of-month rolling: when the anchor is the last day of its
    /// month, every generated month/year date is rolled to its month end.
    #[must_use]
    pub fn end_of_month(mut self, eom: bool) -> Self {
        self.end_of_month = eom;
        self
    }

    /// Sets the date-generation rule.
    #[must_use]
    pub fn rule(mut self, rule: DateGeneration) -> Self {
        self.rule = rule;
        self
    }

    /// Forces a stub convention (otherwise the rule's natural short stub).
    #[must_use]
    pub fn stub(mut self, stub: StubConvention) -> Self {
        self.stub = Some(stub);
        self
    }

    /// Generates the schedule.
    ///
    /// # Errors
    ///
    /// - [`ScheduleError::EmptyRange`] if `termination <= effective`.
    /// - [`ScheduleError::NonPositiveTenor`] if the tenor magnitude is `<= 0`
    ///   (except under [`DateGeneration::Zero`], which ignores the tenor).
    /// - [`ScheduleError::StubDirectionMismatch`] if the stub direction
    ///   contradicts the rule.
    pub fn build(self) -> Result<Schedule, ScheduleError> {
        if self.termination <= self.effective {
            return Err(ScheduleError::EmptyRange {
                effective: self.effective,
                termination: self.termination,
            });
        }

        // Zero rule: a single period, tenor irrelevant.
        if self.rule == DateGeneration::Zero {
            return Ok(self.adjust_and_finish(vec![self.effective, self.termination]));
        }

        if self.tenor.num <= 0 {
            return Err(ScheduleError::NonPositiveTenor(self.tenor.num));
        }

        let forward = self.rule == DateGeneration::Forward;
        let stub = self.resolve_stub(forward)?;

        let mut unadjusted = if forward {
            self.generate_forward(stub)
        } else {
            self.generate_backward(stub)
        };

        if self.rule == DateGeneration::ThirdWednesday {
            for date in &mut unadjusted {
                *date = third_wednesday(date.year(), date.month());
            }
        }

        Ok(self.adjust_and_finish(unadjusted))
    }

    /// Picks/validates the stub for the chosen direction.
    fn resolve_stub(&self, forward: bool) -> Result<StubConvention, ScheduleError> {
        match self.stub {
            None => Ok(if forward {
                StubConvention::ShortBack
            } else {
                StubConvention::ShortFront
            }),
            Some(s) => {
                let ok = matches!(
                    (forward, s),
                    (true, StubConvention::ShortBack | StubConvention::LongBack)
                        | (
                            false,
                            StubConvention::ShortFront | StubConvention::LongFront
                        )
                );
                if ok {
                    Ok(s)
                } else {
                    Err(ScheduleError::StubDirectionMismatch {
                        rule: self.rule,
                        stub: s,
                    })
                }
            }
        }
    }

    /// `anchor` shifted by `mult` tenors, with optional end-of-month rolling.
    fn seed(&self, anchor: Date, mult: i32) -> Date {
        let shifted = anchor.add_period(Period {
            num: self.tenor.num * mult,
            unit: self.tenor.unit,
        });
        if self.end_of_month
            && matches!(self.tenor.unit, Unit::Months | Unit::Years)
            && anchor.is_end_of_month()
        {
            shifted.end_of_month()
        } else {
            shifted
        }
    }

    /// Backward grid: termination, termination−tenor, … down toward effective.
    fn generate_backward(&self, stub: StubConvention) -> Vec<Date> {
        let mut tmp = Vec::new();
        let mut i = 0;
        loop {
            let d = self.seed(self.termination, -i);
            if d < self.effective {
                break;
            }
            tmp.push(d);
            if d == self.effective {
                break;
            }
            i += 1;
        }
        // tmp is descending; its last element is the smallest regular date >= effective.
        let exact = tmp.last() == Some(&self.effective);
        if !exact {
            if stub == StubConvention::LongFront && tmp.len() >= 2 {
                tmp.pop(); // merge the first regular period into the stub
            }
            tmp.push(self.effective);
        }
        tmp.reverse();
        tmp
    }

    /// Forward grid: effective, effective+tenor, … up toward termination.
    fn generate_forward(&self, stub: StubConvention) -> Vec<Date> {
        let mut tmp = Vec::new();
        let mut i = 0;
        loop {
            let d = self.seed(self.effective, i);
            if d > self.termination {
                break;
            }
            tmp.push(d);
            if d == self.termination {
                break;
            }
            i += 1;
        }
        // tmp is ascending; its last element is the largest regular date <= termination.
        let exact = tmp.last() == Some(&self.termination);
        if !exact {
            if stub == StubConvention::LongBack && tmp.len() >= 2 {
                tmp.pop(); // merge the last regular period into the stub
            }
            tmp.push(self.termination);
        }
        tmp
    }

    /// Adjusts the raw grid onto business days and removes adjacent duplicates.
    fn adjust_and_finish(&self, unadjusted: Vec<Date>) -> Schedule {
        let n = unadjusted.len();
        let mut dates: Vec<Date> = unadjusted
            .into_iter()
            .enumerate()
            .map(|(idx, d)| {
                let conv = if idx == n - 1 {
                    self.termination_convention
                } else {
                    self.convention
                };
                self.calendar.adjust(d, conv)
            })
            .collect();
        dates.dedup();
        Schedule { dates }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::Brazil;

    fn d(y: i32, m: u32, day: u32) -> Date {
        Date::new(y, m, day).unwrap()
    }

    fn unadjusted_builder(eff: Date, term: Date, tenor: Period) -> ScheduleBuilder {
        Schedule::builder(eff, term, tenor)
            .convention(BusinessDayConvention::Unadjusted)
            .termination_convention(BusinessDayConvention::Unadjusted)
    }

    #[test]
    fn third_wednesday_known() {
        // March 2025: Wednesdays are 5, 12, 19, 26 → third is the 19th.
        assert_eq!(third_wednesday(2025, 3), d(2025, 3, 19));
        // December 2025: Wednesdays 3, 10, 17, 24, 31 → third is the 17th.
        assert_eq!(third_wednesday(2025, 12), d(2025, 12, 17));
    }

    #[test]
    fn backward_even_periods_no_stub() {
        let s = unadjusted_builder(d(2024, 1, 15), d(2025, 1, 15), Period::months(6))
            .build()
            .unwrap();
        assert_eq!(s.dates(), &[d(2024, 1, 15), d(2024, 7, 15), d(2025, 1, 15)]);
        assert_eq!(s.len(), 3);
        assert_eq!(s.effective_date(), d(2024, 1, 15));
        assert_eq!(s.termination_date(), d(2025, 1, 15));
    }

    #[test]
    fn backward_short_front_stub() {
        // 2024-02-10 → 2026-01-15, 6M backward.
        let s = unadjusted_builder(d(2024, 2, 10), d(2026, 1, 15), Period::months(6))
            .build()
            .unwrap();
        assert_eq!(
            s.dates(),
            &[
                d(2024, 2, 10), // short front stub
                d(2024, 7, 15),
                d(2025, 1, 15),
                d(2025, 7, 15),
                d(2026, 1, 15),
            ]
        );
    }

    #[test]
    fn backward_long_front_stub() {
        let s = unadjusted_builder(d(2024, 2, 10), d(2026, 1, 15), Period::months(6))
            .stub(StubConvention::LongFront)
            .build()
            .unwrap();
        // First regular date (2024-07-15) merged into a long initial period.
        assert_eq!(
            s.dates(),
            &[
                d(2024, 2, 10),
                d(2025, 1, 15),
                d(2025, 7, 15),
                d(2026, 1, 15),
            ]
        );
    }

    #[test]
    fn forward_short_back_stub() {
        // 2024-01-15 → 2025-04-10, 6M forward.
        let s = unadjusted_builder(d(2024, 1, 15), d(2025, 4, 10), Period::months(6))
            .rule(DateGeneration::Forward)
            .build()
            .unwrap();
        assert_eq!(
            s.dates(),
            &[
                d(2024, 1, 15),
                d(2024, 7, 15),
                d(2025, 1, 15),
                d(2025, 4, 10), // short back stub
            ]
        );
    }

    #[test]
    fn forward_long_back_stub() {
        let s = unadjusted_builder(d(2024, 1, 15), d(2025, 4, 10), Period::months(6))
            .rule(DateGeneration::Forward)
            .stub(StubConvention::LongBack)
            .build()
            .unwrap();
        // Last regular date (2025-01-15) merged into a long final period.
        assert_eq!(s.dates(), &[d(2024, 1, 15), d(2024, 7, 15), d(2025, 4, 10)]);
    }

    #[test]
    fn zero_rule_is_endpoints_only() {
        let s = unadjusted_builder(d(2024, 1, 15), d(2034, 1, 15), Period::months(6))
            .rule(DateGeneration::Zero)
            .build()
            .unwrap();
        assert_eq!(s.dates(), &[d(2024, 1, 15), d(2034, 1, 15)]);
    }

    #[test]
    fn end_of_month_rolling() {
        // Anchor 2024-07-31 is month-end; monthly backward dates roll to EOM.
        let s = unadjusted_builder(d(2024, 1, 31), d(2024, 7, 31), Period::months(1))
            .end_of_month(true)
            .build()
            .unwrap();
        assert_eq!(
            s.dates(),
            &[
                d(2024, 1, 31),
                d(2024, 2, 29), // leap-year February end
                d(2024, 3, 31),
                d(2024, 4, 30),
                d(2024, 5, 31),
                d(2024, 6, 30),
                d(2024, 7, 31),
            ]
        );
    }

    #[test]
    fn third_wednesday_rule_snaps_dates() {
        // Quarterly IMM dates measured back from 2025-12-17 (3rd Wed Dec).
        let s = unadjusted_builder(d(2025, 3, 19), d(2025, 12, 17), Period::months(3))
            .rule(DateGeneration::ThirdWednesday)
            .build()
            .unwrap();
        assert_eq!(
            s.dates(),
            &[
                d(2025, 3, 19),
                d(2025, 6, 18),
                d(2025, 9, 17),
                d(2025, 12, 17),
            ]
        );
    }

    #[test]
    fn adjustment_moves_dates_to_business_days() {
        // 2024-01-15 → 2025-01-15 with Brazil + Following.
        // 2024-07-15 is a Monday and a business day; pick a case that adjusts:
        // generate quarterly so a date lands on a holiday/weekend.
        let s = Schedule::builder(d(2024, 1, 13), d(2024, 7, 13), Period::months(3))
            .calendar(Box::new(Brazil))
            .convention(BusinessDayConvention::Following)
            .build()
            .unwrap();
        for &date in s.dates() {
            assert!(Brazil.is_business_day(date), "{date} should be adjusted");
        }
    }

    #[test]
    fn rejects_empty_range() {
        let err = unadjusted_builder(d(2025, 1, 15), d(2025, 1, 15), Period::months(6))
            .build()
            .unwrap_err();
        assert!(matches!(err, ScheduleError::EmptyRange { .. }));
    }

    #[test]
    fn rejects_non_positive_tenor() {
        let err = unadjusted_builder(d(2024, 1, 15), d(2025, 1, 15), Period::months(0))
            .build()
            .unwrap_err();
        assert!(matches!(err, ScheduleError::NonPositiveTenor(0)));
    }

    #[test]
    fn rejects_stub_direction_mismatch() {
        let err = unadjusted_builder(d(2024, 1, 15), d(2025, 1, 15), Period::months(6))
            .rule(DateGeneration::Backward)
            .stub(StubConvention::ShortBack)
            .build()
            .unwrap_err();
        assert!(matches!(err, ScheduleError::StubDirectionMismatch { .. }));
    }

    #[test]
    fn iteration_and_indexing() {
        let s = unadjusted_builder(d(2024, 1, 15), d(2025, 1, 15), Period::months(6))
            .build()
            .unwrap();
        assert_eq!(s[0], d(2024, 1, 15));
        let collected: Vec<Date> = s.iter().copied().collect();
        assert_eq!(collected, s.dates().to_vec());
        assert_eq!((&s).into_iter().count(), 3);
    }
}
