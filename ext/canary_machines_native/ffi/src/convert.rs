//! Ruby `Data` values to core types and back.
use canary_machines::{
    Bounds, Counts, Fraction, Guardrail, Interval, Invalid, Look, Model, Objective, OddsRatio,
    OnLimit, Outcome, Pairs, Plan, Rate, Standing, State, Status, micro_nats,
};
use magnus::{Error, Integer, Ruby, Symbol, Value, prelude::*};

use crate::{Number, Small, data, data_class, expect, nanos, range_error, symbol_name, type_error};

pub fn rate(ruby: &Ruby, value: Value) -> Result<Rate, Error> {
    expect(ruby, value, "Rate", "expected a CanaryMachines::Rate")?;
    let (num, den): (Small, Small) = (value.funcall("num", ())?, value.funcall("den", ())?);
    Rate::new(num.0, den.0).ok_or_else(|| range_error(ruby, "a rate needs 0 < num < den"))
}

fn rate_value(ruby: &Ruby, rate: Rate) -> Result<Value, Error> {
    data(ruby, "Rate", (rate.num(), rate.den()))
}

pub fn odds(ruby: &Ruby, value: Value) -> Result<OddsRatio, Error> {
    expect(
        ruby,
        value,
        "OddsRatio",
        "expected a CanaryMachines::OddsRatio",
    )?;
    let (num, den): (Small, Small) = (value.funcall("num", ())?, value.funcall("den", ())?);
    OddsRatio::new(num.0, den.0)
        .ok_or_else(|| range_error(ruby, "an odds ratio needs num > 0 and den > 0"))
}

fn odds_value(ruby: &Ruby, odds: OddsRatio) -> Result<Value, Error> {
    data(ruby, "OddsRatio", (odds.num(), odds.den()))
}

/// Counts are the sibling `budget_machines` type: anything with `good` and
/// `total` readers is read, a `BudgetMachines::Counts` included.
pub fn counts(ruby: &Ruby, value: Value) -> Result<Counts, Error> {
    if !(value.respond_to("good", false)? && value.respond_to("total", false)?) {
        return Err(type_error(ruby, "expected a Counts (good, total)"));
    }
    let (good, total): (Number, Number) = (value.funcall("good", ())?, value.funcall("total", ())?);
    Counts::new(good.0, total.0).ok_or_else(|| range_error(ruby, "good exceeds total"))
}

pub fn pairs(ruby: &Ruby, value: Value) -> Result<Pairs, Error> {
    expect(ruby, value, "Pairs", "expected a CanaryMachines::Pairs")?;
    let (canary_only, baseline_only, concordant): (Number, Number, Number) = (
        value.funcall("canary_only", ())?,
        value.funcall("baseline_only", ())?,
        value.funcall("concordant", ())?,
    );
    Pairs::new(canary_only.0, baseline_only.0, concordant.0)
        .ok_or_else(|| range_error(ruby, "pair counts must sum within 64 bits"))
}

pub fn model(ruby: &Ruby, value: Value) -> Result<Model, Error> {
    let reference = value.is_kind_of(data_class(ruby, "Model::Reference")?);
    if !reference && !value.is_kind_of(data_class(ruby, "Model::Paired")?) {
        return Err(type_error(
            ruby,
            "expected a CanaryMachines::Model::Reference or Model::Paired",
        ));
    }
    let (acceptable, unacceptable): (Value, Value) = (
        value.funcall("acceptable", ())?,
        value.funcall("unacceptable", ())?,
    );
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
        } => data(
            ruby,
            "Model::Reference",
            (
                rate_value(ruby, acceptable)?,
                rate_value(ruby, unacceptable)?,
            ),
        ),
        Model::Paired {
            acceptable,
            unacceptable,
        } => data(
            ruby,
            "Model::Paired",
            (
                odds_value(ruby, acceptable)?,
                odds_value(ruby, unacceptable)?,
            ),
        ),
    }
}

fn on_limit(ruby: &Ruby, value: Value) -> Result<OnLimit, Error> {
    match symbol_name(ruby, value, "on_limit must be a Symbol")?.as_str() {
        "hold" => Ok(OnLimit::Hold),
        "rollback" => Ok(OnLimit::Rollback),
        "promote" => Ok(OnLimit::Promote),
        _ => Err(Error::new(
            ruby.exception_arg_error(),
            "on_limit must be :hold, :rollback or :promote",
        )),
    }
}

pub fn plan(ruby: &Ruby, value: Value) -> Result<Plan, Error> {
    expect(ruby, value, "Plan", "expected a CanaryMachines::Plan")?;
    let limit: Number = value.funcall("limit", ())?;
    Plan::new(
        model(ruby, value.funcall("model", ())?)?,
        rate(ruby, value.funcall("alpha", ())?)?,
        rate(ruby, value.funcall("beta", ())?)?,
        limit.0,
        on_limit(ruby, value.funcall("on_limit", ())?)?,
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
    let on_limit = match plan.on_limit() {
        OnLimit::Hold => "hold",
        OnLimit::Rollback => "rollback",
        OnLimit::Promote => "promote",
    };
    data(
        ruby,
        "Plan",
        (
            model_value(ruby, plan.model())?,
            rate_value(ruby, plan.alpha())?,
            rate_value(ruby, plan.beta())?,
            plan.limit(),
            ruby.to_symbol(on_limit),
        ),
    )
}

pub fn interval_value(ruby: &Ruby, interval: Interval) -> Result<Value, Error> {
    data(
        ruby,
        "Interval",
        (
            ruby.integer_from_i128(interval.lo),
            ruby.integer_from_i128(interval.hi),
        ),
    )
}

pub fn bounds_value(ruby: &Ruby, bounds: Bounds) -> Result<Value, Error> {
    let fraction = |fraction: Option<Fraction>| {
        fraction
            .map(|fraction| data(ruby, "Fraction", (fraction.num, fraction.den)))
            .transpose()
    };
    data(
        ruby,
        "Bounds",
        (
            fraction(bounds.false_rollback)?,
            fraction(bounds.false_promote)?,
        ),
    )
}

pub const fn outcome(outcome: Outcome) -> &'static str {
    match outcome {
        Outcome::Undecided => "undecided",
        Outcome::Cleared => "cleared",
        Outcome::Regressed => "regressed",
        Outcome::Truncated => "truncated",
    }
}

pub fn look_value(ruby: &Ruby, look: &Look) -> Result<Value, Error> {
    data(
        ruby,
        "Look",
        (
            ruby.to_symbol(outcome(look.outcome)),
            look.hits,
            look.misses,
            look.samples,
            look.llr_micro,
        ),
    )
}

pub fn status_value(ruby: &Ruby, status: &Status) -> Result<Value, Error> {
    data(
        ruby,
        "Status",
        (
            look_value(ruby, &status.look)?,
            status.decided_at.map(|at| nanos(ruby, at)),
        ),
    )
}

pub fn guardrail(ruby: &Ruby, value: Value) -> Result<Guardrail, Error> {
    match symbol_name(ruby, value, "guardrail must be a Symbol")?.as_str() {
        "clear" => Ok(Guardrail::Clear),
        "no_data" => Ok(Guardrail::NoData),
        "breached" => Ok(Guardrail::Breached),
        _ => Err(Error::new(
            ruby.exception_arg_error(),
            "guardrail must be :clear, :no_data or :breached",
        )),
    }
}

pub const fn guardrail_name(guardrail: Guardrail) -> &'static str {
    match guardrail {
        Guardrail::Clear => "clear",
        Guardrail::NoData => "no_data",
        Guardrail::Breached => "breached",
    }
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
            _ => return Err(Error::new(ruby.exception_arg_error(), MESSAGE)),
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
    Ok(ruby.to_symbol(guardrail_name(verdict)))
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
pub fn micro(ruby: &Ruby, value: Value) -> Result<i64, Error> {
    let integer =
        Integer::from_value(value).ok_or_else(|| type_error(ruby, "expected an Integer"))?;
    Ok(micro_nats(integer.to_i128()?))
}
