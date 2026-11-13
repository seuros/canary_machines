//! Thin binding to the sequential canary tests.
mod canary;
mod init;

use canary_machines::{OddsRatio, Rate, Refusal};
use magnus::{Error, Integer, RModule, Ruby, Symbol, TryConvert, Value, prelude::*};

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

/// The numerator and denominator of a Ruby `Rational`, each within 32 bits.
fn parts(ruby: &Ruby, value: Value) -> Result<(u32, u32), Error> {
    if !value.is_kind_of(ruby.class_rational()) {
        return Err(Error::new(
            ruby.exception_type_error(),
            "expected a Rational",
        ));
    }
    let part = |name| -> Result<u32, Error> {
        let number: Number = value.funcall(name, ())?;
        u32::try_from(number.0).map_err(|_| {
            Error::new(
                ruby.exception_range_error(),
                "Rational numerator and denominator must fit in 0..2**32-1",
            )
        })
    };
    Ok((part("numerator")?, part("denominator")?))
}

fn rate(ruby: &Ruby, value: Value) -> Result<Rate, Error> {
    let (num, den) = parts(ruby, value)?;
    Rate::new(num, den).ok_or_else(|| {
        Error::new(
            ruby.exception_range_error(),
            "a rate must be strictly between 0 and 1",
        )
    })
}

fn odds(ruby: &Ruby, value: Value) -> Result<OddsRatio, Error> {
    let (num, den) = parts(ruby, value)?;
    OddsRatio::new(num, den).ok_or_else(|| {
        Error::new(
            ruby.exception_range_error(),
            "an odds ratio must be positive",
        )
    })
}

/// A symbol argument's name, or `TypeError` for anything else.
fn symbol_name(ruby: &Ruby, value: Value, message: &'static str) -> Result<String, Error> {
    let symbol = Symbol::from_value(value)
        .ok_or_else(|| Error::new(ruby.exception_type_error(), message))?;
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
        _ => "refused",
    })
}

/// A value object class defined on the Ruby side with `Data.define`.
fn data_class(ruby: &Ruby, name: &str) -> Result<Value, Error> {
    ruby.class_object()
        .const_get::<_, RModule>("CanaryMachines")?
        .const_get(name)
}
