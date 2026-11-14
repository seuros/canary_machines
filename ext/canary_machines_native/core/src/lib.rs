#![no_std]
#![doc = include_str!("../README.md")]

mod canary;
mod ln;
mod plan;
mod sprt;

pub use budget_machines::{Counts, Objective, Standing, State};
pub use canary::{Action, Canary, Guardrail, Reason, Recommendation, Status};
pub use ln::{FRACTION_BITS, Interval, micro_nats};
pub use plan::{Bounds, Fraction, Invalid, Model, OddsRatio, OnLimit, Plan, Rate};
pub use sprt::{Look, Observation, Outcome, Pairs, Sprt};

/// A refused call. The receiver is left exactly as it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
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
    ///
    /// Never an honoured repeat, even for the batch that latched it sent
    /// again at the same instant: a test keeps running totals, not batches,
    /// and two distinct batches with equal counts at one instant look exactly
    /// like a replay, so the core cannot prove one.
    Concluded,
    /// The sample count would not fit in 64 bits.
    Overflow,
}

impl core::fmt::Display for Refusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::StaleGeneration => "the generation is not current or does not move forward",
            Self::TimeRegression => "the instant precedes the previous observation",
            Self::Full => "every test slot is taken",
            Self::UnknownTest => "no test is registered at that index",
            Self::WrongModel => "the observation is not the kind the test's model takes",
            Self::Concluded => "the test already latched an outcome",
            Self::Overflow => "the sample count does not fit in 64 bits",
        })
    }
}

impl core::error::Error for Refusal {}

#[cfg(test)]
mod tests;
