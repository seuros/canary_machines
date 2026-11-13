//! Thin binding to the sequential canary tests.
mod canary;
mod convert;
mod init;
mod sprt;

use std::time::Duration;

use canary_machines::Refusal;
use magnus::{Error, Integer, RClass, RModule, Ruby, Symbol, TryConvert, Value, prelude::*};

/// Tests one canary can hold.
const CAPACITY: usize = 8;

#[derive(Clone, Copy)]
struct Number(u64);

impl TryConvert for Number {
    fn try_convert(value: Value) -> Result<Self, Error> {
        if Integer::from_value(value).is_none() {
            return Err(Error::new(
                Ruby::get_with(value).exception_type_error(),
                "expected an Integer (unsigned 64-bit)",
            ));
        }
        u64::try_convert(value).map(Self)
    }
}

#[derive(Clone, Copy)]
struct Small(u32);

impl TryConvert for Small {
    fn try_convert(value: Value) -> Result<Self, Error> {
        let number = Number::try_convert(value)?;
        u32::try_from(number.0).map(Self).map_err(|_| {
            Error::new(
                Ruby::get_with(value).exception_range_error(),
                "expected an Integer in 0..2**32-1",
            )
        })
    }
}

/// An instant: Integer nanoseconds in `0..2**64-1`.
#[derive(Clone, Copy)]
struct Nanos(Duration);

impl TryConvert for Nanos {
    fn try_convert(value: Value) -> Result<Self, Error> {
        if Integer::from_value(value).is_none() {
            return Err(Error::new(
                Ruby::get_with(value).exception_type_error(),
                "expected Integer nanoseconds",
            ));
        }
        u64::try_convert(value).map(|nanos| Self(Duration::from_nanos(nanos)))
    }
}

/// A duration as Integer nanoseconds, exact however large.
fn nanos(ruby: &Ruby, duration: Duration) -> Integer {
    ruby.integer_from_u128(duration.as_nanos())
}

/// A test index. One that does not fit in `usize` names no test.
fn index(test: Number) -> usize {
    usize::try_from(test.0).unwrap_or(usize::MAX)
}

fn type_error(ruby: &Ruby, message: &'static str) -> Error {
    Error::new(ruby.exception_type_error(), message)
}

fn range_error(ruby: &Ruby, message: &'static str) -> Error {
    Error::new(ruby.exception_range_error(), message)
}

/// A symbol argument's name, or `TypeError` for anything else.
fn symbol_name(ruby: &Ruby, value: Value, message: &'static str) -> Result<String, Error> {
    let symbol = Symbol::from_value(value).ok_or_else(|| type_error(ruby, message))?;
    Ok(symbol.name()?.into_owned())
}

fn refusal(ruby: &Ruby, refused: Refusal) -> Symbol {
    ruby.to_symbol(match refused {
        Refusal::StaleGeneration => "stale_generation",
        Refusal::TimeRegression => "time_regression",
        Refusal::Full => "full",
        Refusal::UnknownTest => "unknown_test",
        Refusal::WrongModel => "wrong_model",
        Refusal::Concluded => "concluded",
        Refusal::Overflow => "overflow",
    })
}

/// A value object class defined on the Ruby side with `Data.define`; `name`
/// may be nested (`Model::Reference`).
fn data_class(ruby: &Ruby, name: &str) -> Result<RClass, Error> {
    let mut scope: RModule = ruby.class_object().const_get("CanaryMachines")?;
    let mut parts = name.split("::").peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            return scope.const_get(part);
        }
        scope = scope.const_get(part)?;
    }
    scope.const_get(name)
}

/// A `Data` value built by positional `new`.
fn data<A: magnus::ArgList>(ruby: &Ruby, name: &str, fields: A) -> Result<Value, Error> {
    data_class(ruby, name)?.funcall("new", fields)
}

/// Insist that `value` is the `Data` class `name`, so its readers are the
/// validated fields.
fn expect(ruby: &Ruby, value: Value, name: &str, message: &'static str) -> Result<(), Error> {
    if value.is_kind_of(data_class(ruby, name)?) {
        return Ok(());
    }
    Err(type_error(ruby, message))
}
