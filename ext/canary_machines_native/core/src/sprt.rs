//! One sequential probability ratio test over a Bernoulli stream.

use budget_machines::Counts;

use crate::{Model, Plan, Refusal, ln::micro_nats};

/// Outcomes of matched baseline/canary pairs, supplied by the caller.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Pairs {
    canary_only: u64,
    baseline_only: u64,
    concordant: u64,
}

impl Pairs {
    /// `None` when the three counts do not sum within 64 bits.
    #[must_use]
    pub const fn new(canary_only: u64, baseline_only: u64, concordant: u64) -> Option<Self> {
        let Some(discordant) = canary_only.checked_add(baseline_only) else {
            return None;
        };
        if discordant.checked_add(concordant).is_none() {
            return None;
        }
        Some(Self {
            canary_only,
            baseline_only,
            concordant,
        })
    }

    /// Pairs where only the canary failed.
    #[must_use]
    pub const fn canary_only(self) -> u64 {
        self.canary_only
    }

    /// Pairs where only the baseline failed.
    #[must_use]
    pub const fn baseline_only(self) -> u64 {
        self.baseline_only
    }

    /// Pairs where both arms succeeded or both failed. They carry no evidence
    /// about the odds ratio but do count towards the sample limit.
    #[must_use]
    pub const fn concordant(self) -> u64 {
        self.concordant
    }

    /// All pairs.
    #[must_use]
    pub const fn total(self) -> u64 {
        self.canary_only + self.baseline_only + self.concordant
    }
}

/// One batch of new observations since the previous look.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Observation {
    /// Canary trials for a [`Model::Reference`] test.
    Trials(Counts),
    /// Matched pairs for a [`Model::Paired`] test.
    Pairs(Pairs),
}

/// Where the evidence stands.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Neither threshold has been reached and the limit has not been hit.
    #[default]
    Undecided,
    /// The lower threshold was reached: evidence for `H0`, no regression.
    Cleared,
    /// The upper threshold was reached: evidence for `H1`, a regression.
    Regressed,
    /// The sample limit was reached with neither threshold crossed.
    Truncated,
}

/// A test's evidence after a look.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Look {
    /// Where the evidence stands.
    pub outcome: Outcome,
    /// Observations favouring `H1`: canary failures, or canary-only failures.
    pub hits: u64,
    /// Observations favouring `H0`: canary successes, or baseline-only failures.
    pub misses: u64,
    /// Everything counted against the limit, concordant pairs included.
    pub samples: u64,
    /// Log-likelihood ratio in millionths of a nat, rounded down, saturating.
    pub llr_micro: i64,
}

/// A sequential probability ratio test: accumulates evidence look by look and
/// latches the first decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sprt {
    plan: Plan,
    hits: u64,
    misses: u64,
    samples: u64,
    outcome: Outcome,
}

impl Sprt {
    /// A test with no evidence.
    #[must_use]
    pub const fn new(plan: Plan) -> Self {
        Self {
            plan,
            hits: 0,
            misses: 0,
            samples: 0,
            outcome: Outcome::Undecided,
        }
    }

    /// The plan being run.
    #[must_use]
    pub const fn plan(&self) -> &Plan {
        &self.plan
    }

    /// Where the evidence stands.
    #[must_use]
    pub const fn outcome(&self) -> Outcome {
        self.outcome
    }

    /// The evidence as of the last accepted observation.
    #[must_use]
    pub fn look(&self) -> Look {
        let (lo, hi) = self.bracket(self.hits, self.misses);
        Look {
            outcome: self.outcome,
            hits: self.hits,
            misses: self.misses,
            samples: self.samples,
            llr_micro: micro_nats(i128::midpoint(lo, hi)),
        }
    }

    /// Add a batch and look once, at its end.
    ///
    /// A threshold counts as reached only when the whole fixed-point interval
    /// around the log-likelihood ratio is beyond it, so rounding can delay a
    /// decision but never invent one.
    ///
    /// # Errors
    /// [`Refusal::Concluded`] once an outcome has latched,
    /// [`Refusal::WrongModel`] when the observation is not the kind the plan's
    /// model takes, and [`Refusal::Overflow`] when the sample count would not
    /// fit in 64 bits.
    pub fn observe(&mut self, observation: Observation) -> Result<Look, Refusal> {
        if self.outcome != Outcome::Undecided {
            return Err(Refusal::Concluded);
        }
        let (hits, misses, batch) = match (self.plan.model(), observation) {
            (Model::Reference { .. }, Observation::Trials(counts)) => {
                (counts.bad(), counts.good(), counts.total())
            }
            (Model::Paired { .. }, Observation::Pairs(pairs)) => {
                (pairs.canary_only(), pairs.baseline_only(), pairs.total())
            }
            (Model::Reference { .. }, Observation::Pairs(_))
            | (Model::Paired { .. }, Observation::Trials(_)) => return Err(Refusal::WrongModel),
        };
        let samples = self.samples.checked_add(batch).ok_or(Refusal::Overflow)?;
        // hits and misses never exceed samples, so these cannot wrap.
        let (hits, misses) = (self.hits + hits, self.misses + misses);
        let (lo, hi) = self.bracket(hits, misses);
        self.outcome = if lo >= self.plan.upper().hi {
            Outcome::Regressed
        } else if hi <= self.plan.lower().lo {
            Outcome::Cleared
        } else if samples >= self.plan.limit() {
            Outcome::Truncated
        } else {
            Outcome::Undecided
        };
        (self.hits, self.misses, self.samples) = (hits, misses, samples);
        Ok(self.look())
    }

    /// Drop all evidence and the latched outcome; the plan stays.
    pub const fn reset(&mut self) {
        *self = Self::new(self.plan);
    }

    /// Interval containing the exact log-likelihood ratio. Weights are below
    /// `2^55` and counts below `2^64`, so nothing here can overflow.
    fn bracket(&self, hits: u64, misses: u64) -> (i128, i128) {
        let (hits, misses) = (i128::from(hits), i128::from(misses));
        let (hit, miss) = (self.plan.hit(), self.plan.miss());
        (
            hits * hit.lo + misses * miss.lo,
            hits * hit.hi + misses * miss.hi,
        )
    }
}
