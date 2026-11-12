#![no_std]
#![doc = include_str!("../README.md")]

mod canary;
mod ln;
mod plan;
mod sprt;

pub use budget_machines::Counts;
pub use canary::{Action, Canary, Guardrail, Reason, Recommendation, Status};
pub use ln::FRACTION_BITS;
pub use plan::{Bounds, Fraction, Invalid, Model, OddsRatio, OnLimit, Plan, Rate};
pub use sprt::{Look, Observation, Outcome, Pairs, Sprt};

/// A refused call. The receiver is left exactly as it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Refused {
    /// The generation is not the canary's current one, or does not move forward.
    StaleGeneration,
    /// The observation instant is earlier than the previous one.
    TimeRegression,
    /// Every test slot is taken.
    Full,
    /// No test is registered at that index.
    UnknownTest,
    /// The observation is not the kind the test's sampling model takes.
    WrongModel,
    /// The test already latched an outcome and takes no more evidence.
    Concluded,
    /// The sample count would not fit in 64 bits.
    Overflow,
}

#[cfg(test)]
mod tests;
