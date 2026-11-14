//! Ruby `Data` values to core types and back.
use canary_machines::{
    Bounds, Counts, Fraction, Guardrail, Interval, Invalid, Look, Model, Objective, OddsRatio,
    Pairs, Plan, Rate, Standing, State, Status, micro_nats,
};
use magnus::{Error, Ruby, Symbol, Value, prelude::*};
use matryona_machines::ruby::{Symbols, exact_fields, instance_of, integer, nanoseconds};

use crate::classes::classes;
use crate::symbols::{Guardrails, OnLimits, Outcomes};
use crate::{build, range_error, type_error};

pub fn rate(ruby: &Ruby, value: Value) -> Result<Rate, Error> {
    let class = classes(ruby)?.rate.get(ruby);
    let [num, den] = exact_fields::<2>(ruby, value, class, "expected a CanaryMachines::Rate")?;
    let (num, den) = (integer(num, "num")?, integer(den, "den")?);
    Rate::new(num, den).ok_or_else(|| range_error(ruby, "a rate needs 0 < num < den"))
}

fn rate_value(ruby: &Ruby, rate: Rate) -> Result<Value, Error> {
    build(ruby, classes(ruby)?.rate, (rate.num(), rate.den()))
}

pub fn odds(ruby: &Ruby, value: Value) -> Result<OddsRatio, Error> {
    let message = "expected a CanaryMachines::OddsRatio";
    let [num, den] = exact_fields::<2>(ruby, value, classes(ruby)?.odds.get(ruby), message)?;
    let (num, den) = (integer(num, "num")?, integer(den, "den")?);
    OddsRatio::new(num, den)
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
    let own = classes(ruby)?.counts.get(ruby);
    let (good, total): (u64, u64) = if instance_of(value, own) {
        let [good, total] = exact_fields::<2>(ruby, value, own, MESSAGE)?;
        (integer(good, "good")?, integer(total, "total")?)
    } else if value.is_kind_of(own)
        || !(value.respond_to("good", false)? && value.respond_to("total", false)?)
    {
        return Err(type_error(ruby, MESSAGE));
    } else {
        let good = integer(value.funcall("good", ())?, "good")?;
        (good, integer(value.funcall("total", ())?, "total")?)
    };
    Counts::new(good, total).ok_or_else(|| range_error(ruby, "good exceeds total"))
}

pub fn pairs(ruby: &Ruby, value: Value) -> Result<Pairs, Error> {
    let class = classes(ruby)?.pairs.get(ruby);
    let [canary_only, baseline_only, concordant] =
        exact_fields::<3>(ruby, value, class, "expected a CanaryMachines::Pairs")?;
    let canary_only = integer(canary_only, "canary_only")?;
    let baseline_only = integer(baseline_only, "baseline_only")?;
    Pairs::new(
        canary_only,
        baseline_only,
        integer(concordant, "concordant")?,
    )
    .ok_or_else(|| range_error(ruby, "pair counts must sum within 64 bits"))
}

pub fn model(ruby: &Ruby, value: Value) -> Result<Model, Error> {
    const MESSAGE: &str = "expected a CanaryMachines::Model::Reference or Model::Paired";
    let classes = classes(ruby)?;
    let reference = classes.reference.get(ruby);
    if instance_of(value, reference) {
        let [acceptable, unacceptable] = exact_fields::<2>(ruby, value, reference, MESSAGE)?;
        return Ok(Model::Reference {
            acceptable: rate(ruby, acceptable)?,
            unacceptable: rate(ruby, unacceptable)?,
        });
    }
    let paired = classes.paired.get(ruby);
    let [acceptable, unacceptable] = exact_fields::<2>(ruby, value, paired, MESSAGE)?;
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
    let class = classes(ruby)?.plan.get(ruby);
    let [model_field, alpha, beta, limit, on_limit] =
        exact_fields::<5>(ruby, value, class, "expected a CanaryMachines::Plan")?;
    let limit = integer(limit, "limit")?;
    Plan::new(
        model(ruby, model_field)?,
        rate(ruby, alpha)?,
        rate(ruby, beta)?,
        limit,
        OnLimits::parse(ruby, on_limit)?,
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
            OnLimits::symbol(ruby, plan.on_limit()),
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
            Outcomes::symbol(ruby, look.outcome()),
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
            status.decided_at().map(|at| nanoseconds(ruby, at)),
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
        let remaining = integer(value.funcall("remaining", ())?, "remaining")?;
        Guardrail::from(Standing::Within { remaining })
    } else if value.respond_to("overspent", false)? {
        let overspent = integer(value.funcall("overspent", ())?, "overspent")?;
        Guardrail::from(Standing::Exhausted { overspent })
    } else {
        return Err(type_error(ruby, MESSAGE));
    };
    Ok(Guardrails::symbol(ruby, verdict))
}

/// `Rate.failure_of`: read from anything with an objective's `good` and
/// `per` readers. `nil` for an objective that tolerates every event.
pub fn failure_of(ruby: &Ruby, objective: Value) -> Result<Option<Value>, Error> {
    if !(objective.respond_to("good", false)? && objective.respond_to("per", false)?) {
        return Err(type_error(ruby, "expected an Objective (good, per)"));
    }
    let good = integer(objective.funcall("good", ())?, "good")?;
    let per = integer(objective.funcall("per", ())?, "per")?;
    let objective =
        Objective::new(good, per).ok_or_else(|| range_error(ruby, "objective needs good < per"))?;
    Rate::failure_of(objective)
        .map(|rate| rate_value(ruby, rate))
        .transpose()
}

/// `CanaryMachines.micro_nats`.
pub fn micro(value: Value) -> Result<i64, Error> {
    Ok(micro_nats(integer(value, "value")?))
}
