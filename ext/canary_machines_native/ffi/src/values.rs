//! Parsers for the values the core returns, so a value built by hand is held
//! to the core's constructors exactly like one the gem returned, and the
//! readers the core gives them.
use canary_machines::{Bounds, Counts, Fraction, Interval, Look, Recommendation, Status};
use core::time::Duration;
use magnus::{Error, Ruby, Value, prelude::*};
use matryona_machines::ruby::{Symbols, exact_fields, integer, optional};

use crate::classes::classes;
use crate::symbols::{Actions, Outcomes, Reasons};
use crate::{build, convert, range_error};

/// The sibling `sliderule_machines::Interval` has public fields and no
/// constructor; an interval holds its value only when `lo <= hi`.
pub fn interval(ruby: &Ruby, value: Value) -> Result<Interval, Error> {
    let message = "expected a CanaryMachines::Interval";
    let [lo, hi] = exact_fields::<2>(ruby, value, classes(ruby)?.interval.get(ruby), message)?;
    let (lo, hi): (i128, i128) = (integer(lo, "lo")?, integer(hi, "hi")?);
    if lo > hi {
        return Err(range_error(ruby, "an interval needs lo <= hi"));
    }
    Ok(Interval { lo, hi })
}

pub fn fraction(ruby: &Ruby, value: Value) -> Result<Fraction, Error> {
    let message = "expected a CanaryMachines::Fraction";
    let [num, den] = exact_fields::<2>(ruby, value, classes(ruby)?.fraction.get(ruby), message)?;
    let (num, den) = (integer(num, "num")?, integer(den, "den")?);
    Fraction::new(num, den).ok_or_else(|| range_error(ruby, "a fraction needs den > 0"))
}

fn bound(ruby: &Ruby, value: Value) -> Result<Option<Fraction>, Error> {
    if value.is_nil() {
        return Ok(None);
    }
    fraction(ruby, value).map(Some)
}

pub fn bounds(ruby: &Ruby, value: Value) -> Result<Bounds, Error> {
    let class = classes(ruby)?.bounds.get(ruby);
    let [false_rollback, false_promote] =
        exact_fields::<2>(ruby, value, class, "expected a CanaryMachines::Bounds")?;
    Bounds::new(bound(ruby, false_rollback)?, bound(ruby, false_promote)?)
        .ok_or_else(|| range_error(ruby, "no plan guarantees these bounds"))
}

pub fn look(ruby: &Ruby, value: Value) -> Result<Look, Error> {
    let class = classes(ruby)?.look.get(ruby);
    let [outcome, hits, misses, samples, llr_micro] =
        exact_fields::<5>(ruby, value, class, "expected a CanaryMachines::Look")?;
    let (hits, misses) = (integer(hits, "hits")?, integer(misses, "misses")?);
    let samples = integer(samples, "samples")?;
    Look::new(
        Outcomes::parse(ruby, outcome)?,
        hits,
        misses,
        samples,
        integer(llr_micro, "llr_micro")?,
    )
    .ok_or_else(|| range_error(ruby, "no test produces this look"))
}

pub fn status(ruby: &Ruby, value: Value) -> Result<Status, Error> {
    let class = classes(ruby)?.status.get(ruby);
    let [held_look, decided_at] =
        exact_fields::<2>(ruby, value, class, "expected a CanaryMachines::Status")?;
    let decided_at = optional(decided_at, "decided_at")?.map(Duration::from_nanos);
    Status::new(look(ruby, held_look)?, decided_at).ok_or_else(|| {
        range_error(
            ruby,
            "decided_at is present exactly when the look is decided",
        )
    })
}

pub fn recommendation(ruby: &Ruby, value: Value) -> Result<Recommendation, Error> {
    let message = "expected a CanaryMachines::Recommendation";
    let class = classes(ruby)?.recommendation.get(ruby);
    let [action, reason, test] = exact_fields::<3>(ruby, value, class, message)?;
    let test = optional(test, "test")?.map(crate::index);
    Recommendation::new(
        Actions::parse(ruby, action)?,
        Reasons::parse(ruby, reason)?,
        test,
    )
    .ok_or_else(|| range_error(ruby, "recommend never gives this combination"))
}

/// `Counts#checked_add`: the sum as this gem's `Counts`, `nil` on overflow.
/// `other` may be any counts value, a `BudgetMachines::Counts` included.
pub fn checked_add(ruby: &Ruby, this: Value, other: Value) -> Result<Option<Value>, Error> {
    let sum = convert::counts(ruby, this)?.checked_add(convert::counts(ruby, other)?);
    let class = classes(ruby)?.counts;
    sum.map(|sum: Counts| build(ruby, class, (sum.good(), sum.total())))
        .transpose()
}

/// `Counts#bad`.
pub fn bad(ruby: &Ruby, this: Value) -> Result<u64, Error> {
    Ok(convert::counts(ruby, this)?.bad())
}

/// `Look#decided?`: the core's `Look::is_decided`.
pub fn look_decided(ruby: &Ruby, this: Value) -> Result<bool, Error> {
    Ok(look(ruby, this)?.is_decided())
}

/// `Status#decided?`: the core's `Status::is_decided`.
pub fn status_decided(ruby: &Ruby, this: Value) -> Result<bool, Error> {
    Ok(status(ruby, this)?.is_decided())
}
