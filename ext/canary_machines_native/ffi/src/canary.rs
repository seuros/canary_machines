//! `CanaryMachines::Canary`.
use std::cell::RefCell;

use canary_machines::{
    Action, Counts, Fraction, Guardrail, Invalid, Model, Observation, OnLimit, Outcome, Pairs,
    Plan, Reason, Status,
};
use magnus::{
    Error, RHash, Ruby, Value,
    prelude::*,
    scan_args::{get_kwargs, scan_args},
    typed_data::Obj,
};

use crate::{CAPACITY, Number, data_class, odds, rate, refusal, symbol_name};

#[magnus::wrap(class = "CanaryMachines::Canary", free_immediately, size)]
pub struct Canary {
    core: RefCell<canary_machines::Canary<CAPACITY>>,
}

fn range_error(ruby: &Ruby, message: &'static str) -> Error {
    Error::new(ruby.exception_range_error(), message)
}

/// A test index. One that does not fit in `usize` names no test.
fn index(test: Number) -> usize {
    usize::try_from(test.0).unwrap_or(usize::MAX)
}

fn status(ruby: &Ruby, status: &Status) -> Result<Value, Error> {
    let outcome = match status.look.outcome {
        Outcome::Undecided => "undecided",
        Outcome::Cleared => "cleared",
        Outcome::Regressed => "regressed",
        Outcome::Truncated => "truncated",
    };
    data_class(ruby, "Status")?.funcall(
        "new",
        (
            ruby.to_symbol(outcome),
            status.look.hits,
            status.look.misses,
            status.look.samples,
            status.look.llr_micro,
            status.decided_at,
        ),
    )
}

fn rational(ruby: &Ruby, fraction: Option<Fraction>) -> Result<Option<Value>, Error> {
    fraction
        .map(|fraction| {
            ruby.module_kernel()
                .funcall("Rational", (fraction.num, fraction.den))
        })
        .transpose()
}

/// Keyword arguments shared by both kinds of test.
fn plan(ruby: &Ruby, model: Model, keywords: RHash) -> Result<Plan, Error> {
    let keywords = get_kwargs::<_, (Value, Value, Number), (Option<Value>,), ()>(
        keywords,
        &["alpha", "beta", "limit"],
        &["on_limit"],
    )?;
    let (alpha, beta, limit) = keywords.required;
    let on_limit = match keywords.optional.0 {
        None => OnLimit::Hold,
        Some(value) => match symbol_name(ruby, value, "on_limit must be a Symbol")?.as_str() {
            "hold" => OnLimit::Hold,
            "rollback" => OnLimit::Rollback,
            "promote" => OnLimit::Promote,
            _ => {
                return Err(Error::new(
                    ruby.exception_arg_error(),
                    "on_limit must be :hold, :rollback or :promote",
                ));
            }
        },
    };
    Plan::new(
        model,
        rate(ruby, alpha)?,
        rate(ruby, beta)?,
        limit.0,
        on_limit,
    )
    .map_err(|invalid| {
        range_error(
            ruby,
            match invalid {
                Invalid::Hypotheses => "acceptable must be strictly below unacceptable",
                Invalid::ErrorLevels => "alpha + beta must be below 1",
                Invalid::Limit => "limit must be positive",
                _ => "hypotheses are too close to tell apart",
            },
        )
    })
}

impl Canary {
    pub const fn new(generation: Number) -> Self {
        Self {
            core: RefCell::new(canary_machines::Canary::new(generation.0)),
        }
    }

    fn add(ruby: &Ruby, this: Obj<Self>, plan: Plan) -> Result<Value, Error> {
        this.check_frozen()?;
        let outcome = this.core.borrow_mut().add(plan);
        Ok(match outcome {
            Ok(index) => ruby.integer_from_u64(index as u64).as_value(),
            Err(refused) => refusal(ruby, refused).as_value(),
        })
    }

    pub fn add_reference(ruby: &Ruby, this: Obj<Self>, args: &[Value]) -> Result<Value, Error> {
        let args = scan_args::<(Value, Value), (), (), (), RHash, ()>(args)?;
        let (acceptable, unacceptable) = args.required;
        let model = Model::Reference {
            acceptable: rate(ruby, acceptable)?,
            unacceptable: rate(ruby, unacceptable)?,
        };
        Self::add(ruby, this, plan(ruby, model, args.keywords)?)
    }

    pub fn add_paired(ruby: &Ruby, this: Obj<Self>, args: &[Value]) -> Result<Value, Error> {
        let args = scan_args::<(Value, Value), (), (), (), RHash, ()>(args)?;
        let (acceptable, unacceptable) = args.required;
        let model = Model::Paired {
            acceptable: odds(ruby, acceptable)?,
            unacceptable: odds(ruby, unacceptable)?,
        };
        Self::add(ruby, this, plan(ruby, model, args.keywords)?)
    }

    fn observe(
        ruby: &Ruby,
        this: Obj<Self>,
        (generation, at, test): (Number, Number, Number),
        observation: Observation,
    ) -> Result<Value, Error> {
        this.check_frozen()?;
        let outcome = this
            .core
            .borrow_mut()
            .observe(generation.0, at.0, index(test), observation);
        match outcome {
            Ok(seen) => status(ruby, &seen),
            Err(refused) => Ok(refusal(ruby, refused).as_value()),
        }
    }

    pub fn observe_trials(ruby: &Ruby, this: Obj<Self>, args: &[Value]) -> Result<Value, Error> {
        let args = scan_args::<(Number, Number, Number), (), (), (), RHash, ()>(args)?;
        let (good, total) =
            get_kwargs::<_, (Number, Number), (), ()>(args.keywords, &["good", "total"], &[])?
                .required;
        let counts =
            Counts::new(good.0, total.0).ok_or_else(|| range_error(ruby, "good exceeds total"))?;
        Self::observe(ruby, this, args.required, Observation::Trials(counts))
    }

    pub fn observe_pairs(ruby: &Ruby, this: Obj<Self>, args: &[Value]) -> Result<Value, Error> {
        let args = scan_args::<(Number, Number, Number), (), (), (), RHash, ()>(args)?;
        let (canary_only, baseline_only, concordant) =
            get_kwargs::<_, (Number, Number, Number), (), ()>(
                args.keywords,
                &["canary_only", "baseline_only", "concordant"],
                &[],
            )?
            .required;
        let pairs = Pairs::new(canary_only.0, baseline_only.0, concordant.0)
            .ok_or_else(|| range_error(ruby, "pair counts must sum within 64 bits"))?;
        Self::observe(ruby, this, args.required, Observation::Pairs(pairs))
    }

    pub fn recommend(ruby: &Ruby, this: &Self, args: &[Value]) -> Result<Value, Error> {
        let args = scan_args::<(), (Option<Value>,), (), (), (), ()>(args)?;
        let guardrail = match args.optional.0 {
            None => Guardrail::Clear,
            Some(value) => match symbol_name(ruby, value, "guardrail must be a Symbol")?.as_str() {
                "clear" => Guardrail::Clear,
                "no_data" => Guardrail::NoData,
                "breached" => Guardrail::Breached,
                _ => {
                    return Err(Error::new(
                        ruby.exception_arg_error(),
                        "guardrail must be :clear, :no_data or :breached",
                    ));
                }
            },
        };
        let advice = this.core.borrow().recommend(guardrail);
        let action = match advice.action {
            Action::Continue => "continue",
            Action::Promote => "promote",
            Action::Rollback => "rollback",
            Action::Hold => "hold",
        };
        let reason = match advice.reason {
            Reason::Evidence => "evidence",
            Reason::Limit => "limit",
            Reason::Guardrail => "guardrail",
            Reason::GuardrailNoData => "guardrail_no_data",
            Reason::Sampling => "sampling",
            Reason::NoTests => "no_tests",
        };
        data_class(ruby, "Recommendation")?.funcall(
            "new",
            (ruby.to_symbol(action), ruby.to_symbol(reason), advice.test),
        )
    }

    pub fn restart(ruby: &Ruby, this: Obj<Self>, generation: Number) -> Result<Value, Error> {
        this.check_frozen()?;
        let outcome = this.core.borrow_mut().restart(generation.0);
        Ok(match outcome {
            Ok(()) => ruby.to_symbol("ok").as_value(),
            Err(refused) => refusal(ruby, refused).as_value(),
        })
    }

    pub fn status(ruby: &Ruby, this: &Self, test: Number) -> Result<Option<Value>, Error> {
        let seen = this.core.borrow().status(index(test));
        seen.map(|seen| status(ruby, &seen)).transpose()
    }

    pub fn bounds(ruby: &Ruby, this: &Self, test: Number) -> Result<Option<Value>, Error> {
        let Some(plan) = this.core.borrow().plan(index(test)) else {
            return Ok(None);
        };
        let bounds = plan.bounds();
        data_class(ruby, "Bounds")?
            .funcall(
                "new",
                (
                    rational(ruby, bounds.false_rollback)?,
                    rational(ruby, bounds.false_promote)?,
                ),
            )
            .map(Some)
    }

    pub fn thresholds(&self, test: Number) -> Option<(i64, i64)> {
        let plan = self.core.borrow().plan(index(test))?;
        Some((plan.lower_micro(), plan.upper_micro()))
    }

    pub fn generation(&self) -> u64 {
        self.core.borrow().generation()
    }

    pub fn last(&self) -> Option<u64> {
        self.core.borrow().last()
    }

    pub fn size(&self) -> usize {
        self.core.borrow().len()
    }

    pub const fn capacity() -> usize {
        CAPACITY
    }
}
