//! Ruby `Data` values to core types and back.
use canary_machines::{
    Bounds, Counts, Fraction, Guardrail, Interval, Invalid, Look, Model, Objective, OddsRatio,
    OnLimit, Pairs, Plan, Rate, Standing, State, Status, micro_nats,
};
use magnus::{Error, Ruby, Symbol, Value, prelude::*};

use crate::classes::{classes, exact, field, is};
use crate::{Number, Small, build, nanos, range_error, symbols, type_error, values};

pub fn rate(ruby: &Ruby, value: Value) -> Result<Rate, Error> {
    let held = exact(
        ruby,
        value,
        classes(ruby)?.rate,
        "expected a CanaryMachines::Rate",
    )?;
    let (num, den): (Small, Small) = (field(held, 0)?, field(held, 1)?);
    Rate::new(num.0, den.0).ok_or_else(|| range_error(ruby, "a rate needs 0 < num < den"))
}

fn rate_value(ruby: &Ruby, rate: Rate) -> Result<Value, Error> {
    build(ruby, classes(ruby)?.rate, (rate.num(), rate.den()))
}

pub fn odds(ruby: &Ruby, value: Value) -> Result<OddsRatio, Error> {
    let message = "expected a CanaryMachines::OddsRatio";
    let held = exact(ruby, value, classes(ruby)?.odds, message)?;
    let (num, den): (Small, Small) = (field(held, 0)?, field(held, 1)?);
    OddsRatio::new(num.0, den.0)
        .ok_or_else(|| range_error(ruby, "an odds ratio needs num > 0 and den > 0"))
}

fn odds_value(ruby: &Ruby, odds: OddsRatio) -> Result<Value, Error> {
    build(ruby, classes(ruby)?.odds, (odds.num(), odds.den()))
}

/// Counts are the sibling `budget_machines` type. This gem's own `Counts`
/// must be exactly that class (a subclass of it is a forgery) and is read
/// from its fields; anything else with `good` and `total` readers is a
/// sibling's value, a `BudgetMachines::Counts` included, and is read by duck
/// typing. Either way the core's constructor judges the numbers.
pub fn counts(ruby: &Ruby, value: Value) -> Result<Counts, Error> {
    const MESSAGE: &str = "expected a Counts (good, total)";
    let own = classes(ruby)?.counts;
    let (good, total): (Number, Number) = if is(ruby, value, own) {
        let held = exact(ruby, value, own, MESSAGE)?;
        (field(held, 0)?, field(held, 1)?)
    } else if value.is_kind_of(ruby.get_inner(own))
        || !(value.respond_to("good", false)? && value.respond_to("total", false)?)
    {
        return Err(type_error(ruby, MESSAGE));
    } else {
        (value.funcall("good", ())?, value.funcall("total", ())?)
    };
    Counts::new(good.0, total.0).ok_or_else(|| range_error(ruby, "good exceeds total"))
}

pub fn pairs(ruby: &Ruby, value: Value) -> Result<Pairs, Error> {
    let held = exact(
        ruby,
        value,
        classes(ruby)?.pairs,
        "expected a CanaryMachines::Pairs",
    )?;
    let (canary_only, baseline_only, concordant): (Number, Number, Number) =
        (field(held, 0)?, field(held, 1)?, field(held, 2)?);
    Pairs::new(canary_only.0, baseline_only.0, concordant.0)
        .ok_or_else(|| range_error(ruby, "pair counts must sum within 64 bits"))
}

pub fn model(ruby: &Ruby, value: Value) -> Result<Model, Error> {
    const MESSAGE: &str = "expected a CanaryMachines::Model::Reference or Model::Paired";
    let classes = classes(ruby)?;
    let reference = is(ruby, value, classes.reference);
    let class = if reference {
        classes.reference
    } else {
        classes.paired
    };
    let held = exact(ruby, value, class, MESSAGE)?;
    let (acceptable, unacceptable): (Value, Value) = (field(held, 0)?, field(held, 1)?);
    if reference {
        return Ok(Model::Reference {
            acceptable: rate(ruby, acceptable)?,
            unacceptable: rate(ruby, unacceptable)?,
        });
    }
    Ok(Model::Paired {
        acceptable: odds(ruby, acceptable)?,
        unacceptable: odds(ruby, unacceptable)?,
    })
}

fn model_value(ruby: &Ruby, model: Model) -> Result<Value, Error> {
    match model {
        Model::Reference {
            acceptable,
            unacceptable,
        } => build(
            ruby,
            classes(ruby)?.reference,
            (
                rate_value(ruby, acceptable)?,
                rate_value(ruby, unacceptable)?,
            ),
        ),
        Model::Paired {
            acceptable,
            unacceptable,
        } => build(
            ruby,
            classes(ruby)?.paired,
            (
                odds_value(ruby, acceptable)?,
                odds_value(ruby, unacceptable)?,
            ),
        ),
    }
}

pub fn plan(ruby: &Ruby, value: Value) -> Result<Plan, Error> {
    let held = exact(
        ruby,
        value,
        classes(ruby)?.plan,
        "expected a CanaryMachines::Plan",
    )?;
    let limit: Number = field(held, 3)?;
    Plan::new(
        model(ruby, field(held, 0)?)?,
        rate(ruby, field(held, 1)?)?,
        rate(ruby, field(held, 2)?)?,
        limit.0,
        symbols::parse::<OnLimit>(ruby, field(held, 4)?)?,
    )
    .map_err(|invalid| {
        range_error(
            ruby,
            match invalid {
                Invalid::Hypotheses => "acceptable must be strictly below unacceptable",
                Invalid::ErrorLevels => "alpha + beta must be below 1",
                Invalid::Limit => "limit must be positive",
                Invalid::Indistinguishable => "hypotheses are too close to tell apart",
            },
        )
    })
}

pub fn plan_value(ruby: &Ruby, plan: &Plan) -> Result<Value, Error> {
    build(
        ruby,
        classes(ruby)?.plan,
        (
            model_value(ruby, plan.model())?,
            rate_value(ruby, plan.alpha())?,
            rate_value(ruby, plan.beta())?,
            plan.limit(),
            symbols::symbol(ruby, plan.on_limit()),
        ),
    )
}

pub fn interval_value(ruby: &Ruby, interval: Interval) -> Result<Value, Error> {
    build(
        ruby,
        classes(ruby)?.interval,
        (
            ruby.integer_from_i128(interval.lo),
            ruby.integer_from_i128(interval.hi),
        ),
    )
}

pub fn bounds_value(ruby: &Ruby, bounds: Bounds) -> Result<Value, Error> {
    let classes = classes(ruby)?;
    let fraction = |fraction: Option<Fraction>| {
        fraction
            .map(|fraction| build(ruby, classes.fraction, (fraction.num(), fraction.den())))
            .transpose()
    };
    build(
        ruby,
        classes.bounds,
        (
            fraction(bounds.false_rollback())?,
            fraction(bounds.false_promote())?,
        ),
    )
}

pub fn look_value(ruby: &Ruby, look: &Look) -> Result<Value, Error> {
    build(
        ruby,
        classes(ruby)?.look,
        (
            symbols::symbol(ruby, look.outcome()),
            look.hits(),
            look.misses(),
            look.samples(),
            look.llr_micro(),
        ),
    )
}

pub fn status_value(ruby: &Ruby, status: &Status) -> Result<Value, Error> {
    build(
        ruby,
        classes(ruby)?.status,
        (
            look_value(ruby, &status.look())?,
            status.decided_at().map(|at| nanos(ruby, at)),
        ),
    )
}

/// `Guardrail.from`: the core's `From<State>` and `From<Standing>`, read
/// from a `budget_machines` rule state or account standing by duck typing.
pub fn guardrail_from(ruby: &Ruby, value: Value) -> Result<Symbol, Error> {
    const MESSAGE: &str = "expected a budget state (:no_data, :quiet, :firing) or standing";
    let verdict = if let Some(symbol) = Symbol::from_value(value) {
        match symbol.name()?.as_ref() {
            // `State::NoData` and `Standing::NoData` are the same symbol
            // and the same verdict.
            "no_data" => Guardrail::from(State::NoData),
            "quiet" => Guardrail::from(State::Quiet),
            "firing" => Guardrail::from(State::Firing),
            _ => return Err(range_error(ruby, MESSAGE)),
        }
    } else if value.respond_to("remaining", false)? {
        let remaining: Number = value.funcall("remaining", ())?;
        Guardrail::from(Standing::Within {
            remaining: remaining.0,
        })
    } else if value.respond_to("overspent", false)? {
        let overspent: Number = value.funcall("overspent", ())?;
        Guardrail::from(Standing::Exhausted {
            overspent: overspent.0,
        })
    } else {
        return Err(type_error(ruby, MESSAGE));
    };
    Ok(symbols::symbol(ruby, verdict))
}

/// `Rate.failure_of`: read from anything with an objective's `good` and
/// `per` readers. `nil` for an objective that tolerates every event.
pub fn failure_of(ruby: &Ruby, objective: Value) -> Result<Option<Value>, Error> {
    if !(objective.respond_to("good", false)? && objective.respond_to("per", false)?) {
        return Err(type_error(ruby, "expected an Objective (good, per)"));
    }
    let (good, per): (Small, Small) = (
        objective.funcall("good", ())?,
        objective.funcall("per", ())?,
    );
    let objective = Objective::new(good.0, per.0)
        .ok_or_else(|| range_error(ruby, "objective needs good < per"))?;
    Rate::failure_of(objective)
        .map(|rate| rate_value(ruby, rate))
        .transpose()
}

/// `CanaryMachines.micro_nats`.
pub fn micro(value: Value) -> Result<i64, Error> {
    Ok(micro_nats(values::wide(value)?))
}
