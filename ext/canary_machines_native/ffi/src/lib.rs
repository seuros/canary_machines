//! Thin binding to the sequential canary tests.
mod canary;
mod classes;
mod convert;
mod init;
mod sprt;
mod symbols;
mod values;

use std::time::Duration;

use canary_machines::Refusal;
use magnus::value::Opaque;
use magnus::{Error, Integer, RClass, Ruby, TryConvert, Value, prelude::*};

/// Tests one canary can hold.
const CAPACITY: usize = 8;

/// `value` as an Integer, or `TypeError` with `message`.
fn integer(value: Value, message: &'static str) -> Result<Integer, Error> {
    Integer::from_value(value)
        .ok_or_else(|| Error::new(Ruby::get_with(value).exception_type_error(), message))
}

#[derive(Clone, Copy)]
struct Number(u64);

impl TryConvert for Number {
    fn try_convert(value: Value) -> Result<Self, Error> {
        integer(value, "expected an Integer (unsigned 64-bit)")?
            .to_u64()
            .map(Self)
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
        let nanos = integer(value, "expected Integer nanoseconds")?.to_u64()?;
        Ok(Self(Duration::from_nanos(nanos)))
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

/// A core answer as Ruby: the honoured value built by `honoured`, or the
/// refusal's symbol.
fn answer<T>(
    ruby: &Ruby,
    outcome: Result<T, Refusal>,
    honoured: impl FnOnce(T) -> Result<Value, Error>,
) -> Result<Value, Error> {
    match outcome {
        Ok(value) => honoured(value),
        Err(refused) => Ok(symbols::symbol(ruby, refused).as_value()),
    }
}

/// An instance of `class`, built by positional `new` so it validates.
fn build<A: magnus::ArgList>(
    ruby: &Ruby,
    class: Opaque<RClass>,
    fields: A,
) -> Result<Value, Error> {
    ruby.get_inner(class).funcall("new", fields)
}
