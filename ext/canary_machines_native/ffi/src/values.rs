//! Parsers for the values the core returns, so a value built by hand is held
//! to the core's constructors exactly like one the gem returned, and the
//! readers the core gives them.
use canary_machines::{Bounds, Counts, Fraction, Interval, Look, Recommendation, Status};
use magnus::{Error, Ruby, Value, prelude::*};

use crate::classes::{classes, exact, field};
use crate::{Nanos, Number, build, convert, integer, range_error, symbols};

/// An Integer within 128 bits; `TypeError` for anything else.
pub fn wide(value: Value) -> Result<i128, Error> {
    integer(value, "expected an Integer")?.to_i128()
}

fn small_signed(ruby: &Ruby, value: Value) -> Result<i64, Error> {
    i64::try_from(wide(value)?)
        .map_err(|_| range_error(ruby, "expected an Integer in -2**63..2**63-1"))
}

/// The sibling `sliderule_machines::Interval` has public fields and no
/// constructor; an interval holds its value only when `lo <= hi`.
pub fn interval(ruby: &Ruby, value: Value) -> Result<Interval, Error> {
    let message = "expected a CanaryMachines::Interval";
    let held = exact(ruby, value, classes(ruby)?.interval, message)?;
    let (lo, hi) = (wide(field(held, 0)?)?, wide(field(held, 1)?)?);
    if lo > hi {
        return Err(range_error(ruby, "an interval needs lo <= hi"));
    }
    Ok(Interval { lo, hi })
}

pub fn fraction(ruby: &Ruby, value: Value) -> Result<Fraction, Error> {
    let message = "expected a CanaryMachines::Fraction";
    let held = exact(ruby, value, classes(ruby)?.fraction, message)?;
    let (num, den): (Number, Number) = (field(held, 0)?, field(held, 1)?);
    Fraction::new(num.0, den.0).ok_or_else(|| range_error(ruby, "a fraction needs den > 0"))
}

fn bound(ruby: &Ruby, value: Value) -> Result<Option<Fraction>, Error> {
    if value.is_nil() {
        return Ok(None);
    }
    fraction(ruby, value).map(Some)
}

pub fn bounds(ruby: &Ruby, value: Value) -> Result<Bounds, Error> {
    let held = exact(
        ruby,
        value,
        classes(ruby)?.bounds,
        "expected a CanaryMachines::Bounds",
    )?;
    Bounds::new(bound(ruby, field(held, 0)?)?, bound(ruby, field(held, 1)?)?)
        .ok_or_else(|| range_error(ruby, "no plan guarantees these bounds"))
}

pub fn look(ruby: &Ruby, value: Value) -> Result<Look, Error> {
    let held = exact(
        ruby,
        value,
        classes(ruby)?.look,
        "expected a CanaryMachines::Look",
    )?;
    let (hits, misses, samples): (Number, Number, Number) =
        (field(held, 1)?, field(held, 2)?, field(held, 3)?);
    Look::new(
        symbols::parse(ruby, field(held, 0)?)?,
        hits.0,
        misses.0,
        samples.0,
        small_signed(ruby, field(held, 4)?)?,
    )
    .ok_or_else(|| range_error(ruby, "no test produces this look"))
}

pub fn status(ruby: &Ruby, value: Value) -> Result<Status, Error> {
    let held = exact(
        ruby,
        value,
        classes(ruby)?.status,
        "expected a CanaryMachines::Status",
    )?;
    let decided_at: Option<Nanos> = field(held, 1)?;
    Status::new(look(ruby, field(held, 0)?)?, decided_at.map(|at| at.0)).ok_or_else(|| {
        range_error(
            ruby,
            "decided_at is present exactly when the look is decided",
        )
    })
}

pub fn recommendation(ruby: &Ruby, value: Value) -> Result<Recommendation, Error> {
    let message = "expected a CanaryMachines::Recommendation";
    let held = exact(ruby, value, classes(ruby)?.recommendation, message)?;
    let test: Option<Number> = field(held, 2)?;
    Recommendation::new(
        symbols::parse(ruby, field(held, 0)?)?,
        symbols::parse(ruby, field(held, 1)?)?,
        test.map(crate::index),
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
