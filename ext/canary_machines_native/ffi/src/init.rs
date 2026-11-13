//! Private native entry point.
use canary_machines::FRACTION_BITS;
use magnus::{Error, RClass, Ruby, Value, function, method, prelude::*};

use crate::{CAPACITY, canary::Canary, convert, data_class, sprt::Sprt};

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

fn plan_upper(ruby: &Ruby, this: Value) -> Result<Value, Error> {
    convert::interval_value(ruby, convert::plan(ruby, this)?.upper())
}

fn plan_lower(ruby: &Ruby, this: Value) -> Result<Value, Error> {
    convert::interval_value(ruby, convert::plan(ruby, this)?.lower())
}

fn plan_hit(ruby: &Ruby, this: Value) -> Result<Value, Error> {
    convert::interval_value(ruby, convert::plan(ruby, this)?.hit())
}

fn plan_miss(ruby: &Ruby, this: Value) -> Result<Value, Error> {
    convert::interval_value(ruby, convert::plan(ruby, this)?.miss())
}

fn plan_upper_micro(ruby: &Ruby, this: Value) -> Result<i64, Error> {
    Ok(convert::plan(ruby, this)?.upper_micro())
}

fn plan_lower_micro(ruby: &Ruby, this: Value) -> Result<i64, Error> {
    Ok(convert::plan(ruby, this)?.lower_micro())
}

fn plan_bounds(ruby: &Ruby, this: Value) -> Result<Value, Error> {
    convert::bounds_value(ruby, convert::plan(ruby, this)?.bounds())
}

fn values(ruby: &Ruby) -> Result<(), Error> {
    let class = |name| -> Result<RClass, Error> { data_class(ruby, name) };
    class("Rate")?.define_private_method("__validate", method!(validate_rate, 0))?;
    class("Rate")?.define_singleton_method("failure_of", function!(convert::failure_of, 1))?;
    class("OddsRatio")?.define_private_method("__validate", method!(validate_odds, 0))?;
    class("Counts")?.define_private_method("__validate", method!(validate_counts, 0))?;
    class("Pairs")?.define_private_method("__validate", method!(validate_pairs, 0))?;
    for name in ["Model::Reference", "Model::Paired"] {
        class(name)?.define_private_method("__validate", method!(validate_model, 0))?;
    }
    let plan = class("Plan")?;
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
    values(ruby)?;

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
