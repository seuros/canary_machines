//! `CanaryMachines::Sprt`: one sequential test on its own.
use std::cell::RefCell;

use canary_machines::Observation;
use magnus::{Error, Ruby, Symbol, Value, prelude::*, typed_data::Obj};
use matryona_machines::ruby::keep;

use crate::{answer, convert, symbols};

#[magnus::wrap(class = "CanaryMachines::Sprt", free_immediately, size)]
pub struct Sprt {
    core: RefCell<canary_machines::Sprt>,
}

impl Sprt {
    pub fn new(ruby: &Ruby, plan: Value) -> Result<Self, Error> {
        let plan = convert::plan(ruby, plan)?;
        Ok(Self {
            core: RefCell::new(canary_machines::Sprt::new(plan)),
        })
    }

    pub fn plan(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        let plan = *this.core.borrow().plan();
        convert::plan_value(ruby, &plan)
    }

    pub fn outcome(ruby: &Ruby, this: &Self) -> Symbol {
        symbols::symbol(ruby, this.core.borrow().outcome())
    }

    pub fn look(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        let look = this.core.borrow().look();
        convert::look_value(ruby, &look)
    }

    /// Clone-then-keep: the answer is a `Look` built through `new`.
    fn observe(ruby: &Ruby, this: Obj<Self>, observation: Observation) -> Result<Value, Error> {
        keep(
            ruby,
            this,
            &this.core,
            |core| core.observe(observation),
            |outcome| answer(ruby, outcome, |look| convert::look_value(ruby, &look)),
        )
    }

    pub fn observe_trials(ruby: &Ruby, this: Obj<Self>, counts: Value) -> Result<Value, Error> {
        this.check_frozen()?;
        let observation = Observation::Trials(convert::counts(ruby, counts)?);
        Self::observe(ruby, this, observation)
    }

    pub fn observe_pairs(ruby: &Ruby, this: Obj<Self>, pairs: Value) -> Result<Value, Error> {
        this.check_frozen()?;
        let observation = Observation::Pairs(convert::pairs(ruby, pairs)?);
        Self::observe(ruby, this, observation)
    }

    pub fn reset(this: Obj<Self>) -> Result<(), Error> {
        this.check_frozen()?;
        this.core.borrow_mut().reset();
        Ok(())
    }
}
