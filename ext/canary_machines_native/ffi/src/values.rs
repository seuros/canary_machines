//! Parsers for the values the core returns, so a value built by hand is held
//! to the core's constructors exactly like one the gem returned, and the
//! readers the core gives them.
use canary_machines::{Bounds, Counts, Fraction, Interval, Look, Recommendation, Status};
use magnus::{Error, Ruby, Value, prelude::*};
use matryona_machines::ruby::{duration, exact_struct, integer};

use crate::classes::classes;
use crate::{build, convert, range_error, symbols};

/// An Integer within 128 bits; `TypeError` for anything else.
pub fn wide(value: Value, name: &str) -> Result<i128, Error> {
    integer(value, name)
}

/// The sibling `sliderule_machines::Interval` has public fields and no
/// constructor; an interval holds its value only when `lo <= hi`.
pub fn interval(ruby: &Ruby, value: Value) -> Result<Interval, Error> {
    let message = "expected a CanaryMachines::Interval";
    let held = exact_struct(ruby, value, classes(ruby)?.interval.get(ruby), message)?;
    let (lo, hi) = (wide(held.aref(0)?, "lo")?, wide(held.aref(1)?, "hi")?);
    if lo > hi {
        return Err(range_error(ruby, "an interval needs lo <= hi"));
    }
    Ok(Interval { lo, hi })
}

pub fn fraction(ruby: &Ruby, value: Value) -> Result<Fraction, Error> {
    let message = "expected a CanaryMachines::Fraction";
    let held = exact_struct(ruby, value, classes(ruby)?.fraction.get(ruby), message)?;
    let (num, den) = (
        integer(held.aref(0)?, "num")?,
        integer(held.aref(1)?, "den")?,
    );
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
    let held = exact_struct(ruby, value, class, "expected a CanaryMachines::Bounds")?;
    Bounds::new(bound(ruby, held.aref(0)?)?, bound(ruby, held.aref(1)?)?)
        .ok_or_else(|| range_error(ruby, "no plan guarantees these bounds"))
}

pub fn look(ruby: &Ruby, value: Value) -> Result<Look, Error> {
    let class = classes(ruby)?.look.get(ruby);
    let held = exact_struct(ruby, value, class, "expected a CanaryMachines::Look")?;
    let (hits, misses) = (
        integer(held.aref(1)?, "hits")?,
        integer(held.aref(2)?, "misses")?,
    );
    let samples = integer(held.aref(3)?, "samples")?;
    Look::new(
        symbols::parse(ruby, held.aref(0)?)?,
        hits,
        misses,
        samples,
        integer(held.aref(4)?, "llr_micro")?,
    )
    .ok_or_else(|| range_error(ruby, "no test produces this look"))
}

pub fn status(ruby: &Ruby, value: Value) -> Result<Status, Error> {
    let class = classes(ruby)?.status.get(ruby);
    let held = exact_struct(ruby, value, class, "expected a CanaryMachines::Status")?;
    let decided_at: Value = held.aref(1)?;
    let decided_at = if decided_at.is_nil() {
        None
    } else {
        Some(duration(decided_at, "decided_at")?)
    };
    Status::new(look(ruby, held.aref(0)?)?, decided_at).ok_or_else(|| {
        range_error(
            ruby,
            "decided_at is present exactly when the look is decided",
        )
    })
}

pub fn recommendation(ruby: &Ruby, value: Value) -> Result<Recommendation, Error> {
    let message = "expected a CanaryMachines::Recommendation";
    let held = exact_struct(
        ruby,
        value,
        classes(ruby)?.recommendation.get(ruby),
        message,
    )?;
    let test: Value = held.aref(2)?;
    let test = if test.is_nil() {
        None
    } else {
        Some(crate::index(integer(test, "test")?))
    };
    Recommendation::new(
        symbols::parse(ruby, held.aref(0)?)?,
        symbols::parse(ruby, held.aref(1)?)?,
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
