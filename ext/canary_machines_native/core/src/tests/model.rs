//! Independent naive models: floating-point likelihoods and plain vectors.

extern crate std;

use std::vec::Vec;

use crate::{Action, Guardrail, OnLimit, Outcome, Reason, Recommendation, Refusal};

/// Distance from a threshold below which floating point cannot be trusted to
/// agree with the exact answer; such looks are not compared.
const MARGIN: f64 = 1e-6;

/// Deterministic xorshift64* generator.
pub struct Rng(pub u64);

impl Rng {
    pub fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    pub fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }

    /// True with probability `num / den`.
    pub fn chance(&mut self, num: u64, den: u64) -> bool {
        self.below(den) < num
    }
}

/// Wald's test straight from the textbook, in `f64`.
pub struct Naive {
    pub paired: bool,
    pub hit: f64,
    pub miss: f64,
    pub upper: f64,
    pub lower: f64,
    pub limit: u64,
    pub on_limit: OnLimit,
    pub hits: u64,
    pub misses: u64,
    pub samples: u64,
    pub outcome: Outcome,
    pub decided_at: Option<u64>,
    /// A look landed too close to a threshold to compare.
    pub doubtful: bool,
}

impl Naive {
    /// `null` and `alt` are the hit probabilities under each hypothesis.
    pub fn new(
        paired: bool,
        null: f64,
        alt: f64,
        alpha: f64,
        beta: f64,
        limit: u64,
        on_limit: OnLimit,
    ) -> Self {
        Self {
            paired,
            hit: (alt / null).ln(),
            miss: ((1.0 - alt) / (1.0 - null)).ln(),
            upper: ((1.0 - beta) / alpha).ln(),
            lower: (beta / (1.0 - alpha)).ln(),
            limit,
            on_limit,
            hits: 0,
            misses: 0,
            samples: 0,
            outcome: Outcome::Undecided,
            decided_at: None,
            doubtful: false,
        }
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn llr(&self) -> f64 {
        self.hits as f64 * self.hit + self.misses as f64 * self.miss
    }

    pub fn observe(
        &mut self,
        paired: bool,
        hits: u64,
        misses: u64,
        other: u64,
    ) -> Result<(), Refusal> {
        if self.outcome != Outcome::Undecided {
            return Err(Refusal::Concluded);
        }
        if paired != self.paired {
            return Err(Refusal::WrongModel);
        }
        self.hits += hits;
        self.misses += misses;
        self.samples += hits + misses + other;
        let llr = self.llr();
        if (llr - self.upper).abs() < MARGIN || (llr - self.lower).abs() < MARGIN {
            self.doubtful = true;
        }
        self.outcome = if llr >= self.upper {
            Outcome::Regressed
        } else if llr <= self.lower {
            Outcome::Cleared
        } else if self.samples >= self.limit {
            Outcome::Truncated
        } else {
            Outcome::Undecided
        };
        Ok(())
    }
}

/// A rollout as a vector of naive tests.
pub struct Fleet {
    pub generation: u64,
    pub last: Option<u64>,
    pub tests: Vec<Naive>,
}

impl Fleet {
    pub fn observe(
        &mut self,
        generation: u64,
        at: u64,
        index: usize,
        paired: bool,
        counts: (u64, u64, u64),
    ) -> Result<(), Refusal> {
        if generation != self.generation {
            return Err(Refusal::StaleGeneration);
        }
        if self.last.is_some_and(|last| at < last) {
            return Err(Refusal::TimeRegression);
        }
        let test = self.tests.get_mut(index).ok_or(Refusal::UnknownTest)?;
        test.observe(paired, counts.0, counts.1, counts.2)?;
        if test.outcome != Outcome::Undecided {
            test.decided_at = Some(at);
        }
        self.last = Some(at);
        Ok(())
    }

    pub fn restart(&mut self, generation: u64) -> Result<(), Refusal> {
        if generation <= self.generation {
            return Err(Refusal::StaleGeneration);
        }
        self.generation = generation;
        for test in &mut self.tests {
            (test.hits, test.misses, test.samples) = (0, 0, 0);
            test.outcome = Outcome::Undecided;
            test.decided_at = None;
        }
        Ok(())
    }

    /// Each test votes; the votes are tallied in priority order.
    pub fn recommend(&self, guardrail: Guardrail) -> Recommendation {
        let vote = |test: &Naive| match (test.outcome, test.on_limit) {
            (Outcome::Regressed, _) => (0, Action::Rollback, Reason::Evidence),
            (Outcome::Truncated, OnLimit::Rollback) => (1, Action::Rollback, Reason::Limit),
            (Outcome::Undecided, _) => (2, Action::Continue, Reason::Sampling),
            (Outcome::Truncated, OnLimit::Hold) => (3, Action::Hold, Reason::Limit),
            (Outcome::Truncated, OnLimit::Promote) => (5, Action::Promote, Reason::Limit),
            (Outcome::Cleared, _) => (6, Action::Promote, Reason::Evidence),
        };
        let mut best = (7, Action::Hold, Reason::NoTests, None);
        for (index, test) in self.tests.iter().enumerate() {
            let (rank, action, reason) = vote(test);
            if rank < best.0 {
                let named = !matches!(reason, Reason::Sampling) && rank != 6;
                best = (rank, action, reason, named.then_some(index));
            }
        }
        let (rank, mut action, mut reason, mut test) = best;
        if guardrail == Guardrail::Breached {
            (action, reason, test) = (Action::Rollback, Reason::Guardrail, None);
        } else if guardrail == Guardrail::NoData && (rank == 5 || rank == 6) {
            (action, reason, test) = (Action::Hold, Reason::GuardrailNoData, None);
        }
        Recommendation::new(action, reason, test).expect("the model advises what recommend can")
    }
}
