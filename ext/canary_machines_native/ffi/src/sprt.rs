//! `CanaryMachines::Sprt`: one sequential test on its own.
use std::cell::RefCell;

use canary_machines::Observation;
use magnus::{Error, Ruby, Symbol, Value, prelude::*, typed_data::Obj};

use crate::{convert, refusal};

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
        ruby.to_symbol(convert::outcome(this.core.borrow().outcome()))
    }

    pub fn look(ruby: &Ruby, this: &Self) -> Result<Value, Error> {
        let look = this.core.borrow().look();
        convert::look_value(ruby, &look)
    }

    fn observe(ruby: &Ruby, this: &Self, observation: Observation) -> Result<Value, Error> {
        let outcome = this.core.borrow_mut().observe(observation);
        match outcome {
            Ok(look) => convert::look_value(ruby, &look),
            Err(refused) => Ok(refusal(ruby, refused).as_value()),
        }
    }

    pub fn observe_trials(ruby: &Ruby, this: Obj<Self>, counts: Value) -> Result<Value, Error> {
        this.check_frozen()?;
        let observation = Observation::Trials(convert::counts(ruby, counts)?);
        Self::observe(ruby, &this, observation)
    }

    pub fn observe_pairs(ruby: &Ruby, this: Obj<Self>, pairs: Value) -> Result<Value, Error> {
        this.check_frozen()?;
        let observation = Observation::Pairs(convert::pairs(ruby, pairs)?);
        Self::observe(ruby, &this, observation)
    }

    pub fn reset(this: Obj<Self>) -> Result<(), Error> {
        this.check_frozen()?;
        this.core.borrow_mut().reset();
        Ok(())
    }
}
