//! Private native entry point.
use canary_machines::FRACTION_BITS;
use magnus::value::Opaque;
use magnus::{Error, RClass, Ruby, Value, function, method, prelude::*};

use crate::classes::{self, Classes};
use crate::{CAPACITY, canary::Canary, convert, sprt::Sprt, values as parse};

/// `__validate` of a `Data` class: parse `self` as the core type and keep
/// nothing, so a value the core would not build never exists.
macro_rules! validator {
    ($name:ident, $parse:path) => {
        fn $name(ruby: &Ruby, this: Value) -> Result<(), Error> {
            $parse(ruby, this).map(|_| ())
        }
    };
}

validator!(validate_rate, convert::rate);
validator!(validate_odds, convert::odds);
validator!(validate_counts, convert::counts);
validator!(validate_pairs, convert::pairs);
validator!(validate_model, convert::model);
validator!(validate_plan, convert::plan);
validator!(validate_interval, parse::interval);
validator!(validate_fraction, parse::fraction);
validator!(validate_bounds, parse::bounds);
validator!(validate_look, parse::look);
validator!(validate_status, parse::status);
validator!(validate_recommendation, parse::recommendation);

/// A native `Plan` reader: rebuild the core plan from `self`, then read it.
macro_rules! plan_reader {
    ($name:ident -> $answer:ty, |$ruby:ident, $plan:ident| $read:expr) => {
        fn $name($ruby: &Ruby, this: Value) -> Result<$answer, Error> {
            let $plan = convert::plan($ruby, this)?;
            $read
        }
    };
}

plan_reader!(plan_upper -> Value, |ruby, plan| convert::interval_value(ruby, plan.upper()));
plan_reader!(plan_lower -> Value, |ruby, plan| convert::interval_value(ruby, plan.lower()));
plan_reader!(plan_hit -> Value, |ruby, plan| convert::interval_value(ruby, plan.hit()));
plan_reader!(plan_miss -> Value, |ruby, plan| convert::interval_value(ruby, plan.miss()));
plan_reader!(plan_upper_micro -> i64, |ruby, plan| Ok(plan.upper_micro()));
plan_reader!(plan_lower_micro -> i64, |ruby, plan| Ok(plan.lower_micro()));
plan_reader!(plan_bounds -> Value, |ruby, plan| convert::bounds_value(ruby, plan.bounds()));

fn values(ruby: &Ruby, classes: &Classes) -> Result<(), Error> {
    let class = |class: Opaque<RClass>| ruby.get_inner(class);
    class(classes.rate).define_private_method("__validate", method!(validate_rate, 0))?;
    class(classes.rate).define_singleton_method("failure_of", function!(convert::failure_of, 1))?;
    class(classes.odds).define_private_method("__validate", method!(validate_odds, 0))?;
    let counts = class(classes.counts);
    counts.define_private_method("__validate", method!(validate_counts, 0))?;
    counts.define_method("bad", method!(parse::bad, 0))?;
    counts.define_method("checked_add", method!(parse::checked_add, 1))?;
    class(classes.pairs).define_private_method("__validate", method!(validate_pairs, 0))?;
    for model in [classes.reference, classes.paired] {
        class(model).define_private_method("__validate", method!(validate_model, 0))?;
    }
    for (value, validate) in [
        (classes.interval, method!(validate_interval, 0)),
        (classes.fraction, method!(validate_fraction, 0)),
        (classes.bounds, method!(validate_bounds, 0)),
        (classes.look, method!(validate_look, 0)),
        (classes.status, method!(validate_status, 0)),
        (classes.recommendation, method!(validate_recommendation, 0)),
    ] {
        class(value).define_private_method("__validate", validate)?;
    }
    class(classes.look).define_method("decided?", method!(parse::look_decided, 0))?;
    class(classes.status).define_method("decided?", method!(parse::status_decided, 0))?;
    let plan = class(classes.plan);
    plan.define_private_method("__validate", method!(validate_plan, 0))?;
    plan.define_method("upper", method!(plan_upper, 0))?;
    plan.define_method("lower", method!(plan_lower, 0))?;
    plan.define_method("hit", method!(plan_hit, 0))?;
    plan.define_method("miss", method!(plan_miss, 0))?;
    plan.define_method("upper_micro", method!(plan_upper_micro, 0))?;
    plan.define_method("lower_micro", method!(plan_lower_micro, 0))?;
    plan.define_method("bounds", method!(plan_bounds, 0))?;
    Ok(())
}

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.define_module("CanaryMachines")?;
    module.const_set("CAPACITY", CAPACITY)?;
    module.const_set("FRACTION_BITS", FRACTION_BITS)?;
    module.define_singleton_method("micro_nats", function!(convert::micro, 1))?;
    module
        .define_module("Guardrail")?
        .define_singleton_method("from", function!(convert::guardrail_from, 1))?;
    values(ruby, classes::resolve(ruby, module)?)?;

    let class = module.define_class("Sprt", ruby.class_object())?;
    class.undef_default_alloc_func();
    class.define_singleton_method("new", function!(Sprt::new, 1))?;
    class.define_method("plan", method!(Sprt::plan, 0))?;
    class.define_method("outcome", method!(Sprt::outcome, 0))?;
    class.define_method("look", method!(Sprt::look, 0))?;
    class.define_method("observe_trials", method!(Sprt::observe_trials, 1))?;
    class.define_method("observe_pairs", method!(Sprt::observe_pairs, 1))?;
    class.define_method("reset", method!(Sprt::reset, 0))?;

    let class = module.define_class("Canary", ruby.class_object())?;
    class.undef_default_alloc_func();
    class.define_singleton_method("new", function!(Canary::new, 1))?;
    class.define_method("add", method!(Canary::add, 1))?;
    class.define_method("plan", method!(Canary::plan, 1))?;
    class.define_method("status", method!(Canary::status, 1))?;
    class.define_method("observe_trials", method!(Canary::observe_trials, 4))?;
    class.define_method("observe_pairs", method!(Canary::observe_pairs, 4))?;
    class.define_method("restart", method!(Canary::restart, 1))?;
    class.define_method("recommend", method!(Canary::recommend, 1))?;
    class.define_method("generation", method!(Canary::generation, 0))?;
    class.define_method("last", method!(Canary::last, 0))?;
    class.define_method("size", method!(Canary::size, 0))?;
    class.define_method("empty?", method!(Canary::is_empty, 0))?;
    class.define_method("capacity", method!(Canary::capacity, 0))?;
    Ok(())
}
