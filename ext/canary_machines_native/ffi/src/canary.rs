//! `CanaryMachines::Canary`.
use std::cell::RefCell;

use canary_machines::Observation;
use core::time::Duration;

use magnus::{Error, Integer, Ruby, Value, prelude::*, typed_data::Obj};
use matryona_machines::ruby::{Symbols, duration, integer, keep, nanoseconds};

use crate::classes::classes;
use crate::symbols::{Actions, Guardrails, Reasons};
use crate::{CAPACITY, answer, build, convert, index};

#[magnus::wrap(class = "CanaryMachines::Canary", free_immediately, size)]
pub struct Canary {
    core: RefCell<canary_machines::Canary<CAPACITY>>,
}

impl Canary {
    pub fn new(generation: Value) -> Result<Self, Error> {
        Ok(Self {
            core: RefCell::new(canary_machines::Canary::new(integer(
                generation,
                "generation",
            )?)),
        })
    }

    pub fn add(ruby: &Ruby, this: Obj<Self>, plan: Value) -> Result<Value, Error> {
        this.check_frozen()?;
        let plan = convert::plan(ruby, plan)?;
        let outcome = this.core.borrow_mut().add(plan);
        answer(ruby, outcome, |index| {
            Ok(ruby.integer_from_u64(index as u64).as_value())
        })
    }

    /// Clone-then-keep: the answer is a `Status` built through `new`.
    fn observe(
        ruby: &Ruby,
        this: Obj<Self>,
        (generation, at, test): (u64, Duration, usize),
        observation: Observation,
    ) -> Result<Value, Error> {
        keep(
            ruby,
            this,
            &this.core,
            |core| core.observe(generation, at, test, observation),
            |outcome| answer(ruby, outcome, |seen| convert::status_value(ruby, &seen)),
        )
    }

    pub fn observe_trials(
        ruby: &Ruby,
        this: Obj<Self>,
        generation: Value,
        at: Value,
        test: Value,
        counts: Value,
    ) -> Result<Value, Error> {
        this.check_frozen()?;
        let coordinates = Self::coordinates(generation, at, test)?;
        let observation = Observation::Trials(convert::counts(ruby, counts)?);
        Self::observe(ruby, this, coordinates, observation)
    }

    pub fn observe_pairs(
        ruby: &Ruby,
        this: Obj<Self>,
        generation: Value,
        at: Value,
        test: Value,
        pairs: Value,
    ) -> Result<Value, Error> {
        this.check_frozen()?;
        let coordinates = Self::coordinates(generation, at, test)?;
        let observation = Observation::Pairs(convert::pairs(ruby, pairs)?);
        Self::observe(ruby, this, coordinates, observation)
    }

    /// A look's generation, instant and test index, converted in that order.
    fn coordinates(
        generation: Value,
        at: Value,
        test: Value,
    ) -> Result<(u64, Duration, usize), Error> {
        let generation = integer(generation, "generation")?;
        let at = duration(at, "at")?;
        Ok((generation, at, index(integer(test, "test")?)))
    }

    pub fn recommend(ruby: &Ruby, this: &Self, guardrail: Value) -> Result<Value, Error> {
        let guardrail = Guardrails::parse(ruby, guardrail)?;
        let advice = this.core.borrow().recommend(guardrail);
        build(
            ruby,
            classes(ruby)?.recommendation,
            (
                Actions::symbol(ruby, advice.action()),
                Reasons::symbol(ruby, advice.reason()),
                advice.test(),
            ),
        )
    }

    pub fn restart(ruby: &Ruby, this: Obj<Self>, generation: Value) -> Result<Value, Error> {
        this.check_frozen()?;
        let generation = integer(generation, "generation")?;
        let outcome = this.core.borrow_mut().restart(generation);
        answer(ruby, outcome, |()| Ok(ruby.to_symbol("ok").as_value()))
    }

    pub fn plan(ruby: &Ruby, this: &Self, test: Value) -> Result<Option<Value>, Error> {
        let test = index(integer(test, "test")?);
        let plan = this.core.borrow().plan(test);
        plan.map(|plan| convert::plan_value(ruby, &plan))
            .transpose()
    }

    pub fn status(ruby: &Ruby, this: &Self, test: Value) -> Result<Option<Value>, Error> {
        let test = index(integer(test, "test")?);
        let seen = this.core.borrow().status(test);
        seen.map(|seen| convert::status_value(ruby, &seen))
            .transpose()
    }

    pub fn generation(&self) -> u64 {
        self.core.borrow().generation()
    }

    pub fn last(ruby: &Ruby, this: &Self) -> Option<Integer> {
        let last = this.core.borrow().last()?;
        Some(nanoseconds(ruby, last))
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
