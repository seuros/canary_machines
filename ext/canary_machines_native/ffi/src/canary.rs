//! `CanaryMachines::Canary`.
use std::cell::RefCell;

use canary_machines::{Action, Observation, Reason};
use magnus::{Error, Integer, Ruby, Value, prelude::*, typed_data::Obj};

use crate::{CAPACITY, Nanos, Number, convert, data, index, nanos, refusal};

#[magnus::wrap(class = "CanaryMachines::Canary", free_immediately, size)]
pub struct Canary {
    core: RefCell<canary_machines::Canary<CAPACITY>>,
}

impl Canary {
    pub const fn new(generation: Number) -> Self {
        Self {
            core: RefCell::new(canary_machines::Canary::new(generation.0)),
        }
    }

    pub fn add(ruby: &Ruby, this: Obj<Self>, plan: Value) -> Result<Value, Error> {
        this.check_frozen()?;
        let plan = convert::plan(ruby, plan)?;
        let outcome = this.core.borrow_mut().add(plan);
        Ok(match outcome {
            Ok(index) => ruby.integer_from_u64(index as u64).as_value(),
            Err(refused) => refusal(ruby, refused).as_value(),
        })
    }

    fn observe(
        ruby: &Ruby,
        this: &Self,
        (generation, at, test): (Number, Nanos, Number),
        observation: Observation,
    ) -> Result<Value, Error> {
        let outcome = this
            .core
            .borrow_mut()
            .observe(generation.0, at.0, index(test), observation);
        match outcome {
            Ok(seen) => convert::status_value(ruby, &seen),
            Err(refused) => Ok(refusal(ruby, refused).as_value()),
        }
    }

    pub fn observe_trials(
        ruby: &Ruby,
        this: Obj<Self>,
        generation: Number,
        at: Nanos,
        test: Number,
        counts: Value,
    ) -> Result<Value, Error> {
        this.check_frozen()?;
        let observation = Observation::Trials(convert::counts(ruby, counts)?);
        Self::observe(ruby, &this, (generation, at, test), observation)
    }

    pub fn observe_pairs(
        ruby: &Ruby,
        this: Obj<Self>,
        generation: Number,
        at: Nanos,
        test: Number,
        pairs: Value,
    ) -> Result<Value, Error> {
        this.check_frozen()?;
        let observation = Observation::Pairs(convert::pairs(ruby, pairs)?);
        Self::observe(ruby, &this, (generation, at, test), observation)
    }

    pub fn recommend(ruby: &Ruby, this: &Self, guardrail: Value) -> Result<Value, Error> {
        let guardrail = convert::guardrail(ruby, guardrail)?;
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
        data(
            ruby,
            "Recommendation",
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

    pub fn plan(ruby: &Ruby, this: &Self, test: Number) -> Result<Option<Value>, Error> {
        let plan = this.core.borrow().plan(index(test));
        plan.map(|plan| convert::plan_value(ruby, &plan))
            .transpose()
    }

    pub fn status(ruby: &Ruby, this: &Self, test: Number) -> Result<Option<Value>, Error> {
        let seen = this.core.borrow().status(index(test));
        seen.map(|seen| convert::status_value(ruby, &seen))
            .transpose()
    }

    pub fn generation(&self) -> u64 {
        self.core.borrow().generation()
    }

    pub fn last(ruby: &Ruby, this: &Self) -> Option<Integer> {
        let last = this.core.borrow().last()?;
        Some(nanos(ruby, last))
    }

    pub fn size(&self) -> usize {
        self.core.borrow().len()
    }

    pub fn is_empty(&self) -> bool {
        self.core.borrow().is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.core.borrow().capacity()
    }
}
