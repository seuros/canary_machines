//! Thin binding to the sequential canary tests.
mod canary;
mod classes;
mod convert;
mod init;
mod sprt;
mod symbols;
mod values;

use canary_machines::Refusal;
use magnus::{Error, Ruby, Value, prelude::*};
use matryona_machines::ruby::{Class, Symbols};

/// Tests one canary can hold.
const CAPACITY: usize = 8;

/// A test index. One that does not fit in `usize` names no test.
fn index(test: u64) -> usize {
    usize::try_from(test).unwrap_or(usize::MAX)
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
        Err(refused) => Ok(symbols::Refusals::symbol(ruby, refused).as_value()),
    }
}

/// An instance of `class`, built by positional `new` so it validates.
fn build<A: magnus::ArgList>(ruby: &Ruby, class: Class, fields: A) -> Result<Value, Error> {
    class.get(ruby).funcall("new", fields)
}
