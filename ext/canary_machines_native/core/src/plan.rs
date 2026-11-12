//! Hypotheses, error levels and limits of one sequential test.

use budget_machines::Objective;

use crate::ln::{Interval, ln_ratio, micro};

/// A probability strictly between zero and one, as an exact ratio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rate {
    num: u32,
    den: u32,
}

impl Rate {
    /// `None` unless `0 < num < den`: a rate of exactly zero or one has no
    /// finite log-likelihood.
    #[must_use]
    pub const fn new(num: u32, den: u32) -> Option<Self> {
        if num == 0 || num >= den {
            return None;
        }
        Some(Self { num, den })
    }

    /// The failure rate an [`Objective`] tolerates: `tolerated / per`. `None`
    /// for an objective that tolerates every event.
    #[must_use]
    pub const fn failure_of(objective: Objective) -> Option<Self> {
        Self::new(objective.tolerated(), objective.per())
    }

    /// Numerator.
    #[must_use]
    pub const fn num(self) -> u32 {
        self.num
    }

    /// Denominator.
    #[must_use]
    pub const fn den(self) -> u32 {
        self.den
    }
}

/// A positive odds ratio `num / den` of canary failure against baseline failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OddsRatio {
    num: u32,
    den: u32,
}

impl OddsRatio {
    /// `None` when either part is zero.
    #[must_use]
    pub const fn new(num: u32, den: u32) -> Option<Self> {
        if num == 0 || den == 0 {
            return None;
        }
        Some(Self { num, den })
    }

    /// Numerator.
    #[must_use]
    pub const fn num(self) -> u32 {
        self.num
    }

    /// Denominator.
    #[must_use]
    pub const fn den(self) -> u32 {
        self.den
    }
}

/// The sampling model a test assumes, with its two simple hypotheses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    /// Canary trials are independent Bernoulli draws with one failure
    /// probability `p`, judged against rates the caller fixed in advance.
    /// The baseline is a constant, not a sample: its uncertainty is the
    /// caller's to have settled before the test began.
    Reference {
        /// `H0`: `p` is at most this.
        acceptable: Rate,
        /// `H1`: `p` is at least this.
        unacceptable: Rate,
    },
    /// Baseline and canary are sampled concurrently and matched into pairs by
    /// a rule that does not look at outcomes. Pairs where both arms agree are
    /// discarded; given that a pair disagrees, the canary is the failing arm
    /// with probability `psi / (1 + psi)`, where `psi` is the odds ratio. The
    /// unknown baseline rate cancels out of that conditional probability.
    Paired {
        /// `H0`: the odds ratio is at most this.
        acceptable: OddsRatio,
        /// `H1`: the odds ratio is at least this.
        unacceptable: OddsRatio,
    },
}

impl Model {
    /// Probability of a hit under `H0` and under `H1`, as `(num, den)` pairs.
    const fn hit_rates(self) -> [(u64, u64); 2] {
        match self {
            Self::Reference {
                acceptable,
                unacceptable,
            } => [
                (acceptable.num as u64, acceptable.den as u64),
                (unacceptable.num as u64, unacceptable.den as u64),
            ],
            Self::Paired {
                acceptable,
                unacceptable,
            } => [
                (
                    acceptable.num as u64,
                    acceptable.num as u64 + acceptable.den as u64,
                ),
                (
                    unacceptable.num as u64,
                    unacceptable.num as u64 + unacceptable.den as u64,
                ),
            ],
        }
    }
}

/// What a test that reaches its sample limit undecided stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnLimit {
    /// Neither promote nor roll back on this evidence. Keeps both error bounds.
    Hold,
    /// Count it as a regression. Keeps the false-promote bound only.
    Rollback,
    /// Count it as cleared. Keeps the false-rollback bound only.
    Promote,
}

/// Why a plan cannot be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Invalid {
    /// The acceptable hypothesis is not strictly below the unacceptable one.
    Hypotheses,
    /// `alpha + beta` is not below one, so the thresholds would cross.
    ErrorLevels,
    /// The sample limit is zero.
    Limit,
    /// The hypotheses are closer than the fixed-point logarithm can resolve.
    Indistinguishable,
}

/// An exact non-negative fraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fraction {
    /// Numerator.
    pub num: u64,
    /// Denominator; never zero.
    pub den: u64,
}

/// Error probabilities a plan guarantees for the statistical test alone,
/// under its sampling model. `None` means the limit policy forces that
/// decision at truncation, so this crate claims nothing about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    /// Bound on rollback evidence when `H0` holds: `alpha / (1 - beta)`.
    pub false_rollback: Option<Fraction>,
    /// Bound on promote evidence when `H1` holds: `beta / (1 - alpha)`.
    pub false_promote: Option<Fraction>,
}

/// A fully specified test: model, error levels, sample limit, limit policy,
/// and the fixed-point weights and thresholds derived from them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan {
    model: Model,
    alpha: Rate,
    beta: Rate,
    limit: u64,
    on_limit: OnLimit,
    pub(crate) hit: Interval,
    pub(crate) miss: Interval,
    pub(crate) upper: Interval,
    pub(crate) lower: Interval,
}

impl Plan {
    /// Wald's test of `model` with nominal levels `alpha` (rollback under
    /// `H0`) and `beta` (promote under `H1`), stopped no later than the first
    /// look at or after `limit` samples.
    ///
    /// # Errors
    /// [`Invalid`] names the first requirement the arguments miss.
    pub fn new(
        model: Model,
        alpha: Rate,
        beta: Rate,
        limit: u64,
        on_limit: OnLimit,
    ) -> Result<Self, Invalid> {
        let [(null_num, null_den), (alt_num, alt_den)] = model.hit_rates();
        if u128::from(null_num) * u128::from(alt_den) >= u128::from(alt_num) * u128::from(null_den)
        {
            return Err(Invalid::Hypotheses);
        }
        let (alpha_num, alpha_den) = (u64::from(alpha.num), u64::from(alpha.den));
        let (beta_num, beta_den) = (u64::from(beta.num), u64::from(beta.den));
        if u128::from(alpha_num) * u128::from(beta_den)
            + u128::from(beta_num) * u128::from(alpha_den)
            >= u128::from(alpha_den) * u128::from(beta_den)
        {
            return Err(Invalid::ErrorLevels);
        }
        if limit == 0 {
            return Err(Invalid::Limit);
        }
        // ln(p1 / p0) and ln((1 - p1) / (1 - p0)).
        let hit = ln_ratio([alt_num, null_den], [alt_den, null_num]);
        let miss = ln_ratio(
            [alt_den - alt_num, null_den],
            [alt_den, null_den - null_num],
        );
        if hit.lo <= 0 || miss.hi >= 0 {
            return Err(Invalid::Indistinguishable);
        }
        Ok(Self {
            model,
            alpha,
            beta,
            limit,
            on_limit,
            hit,
            miss,
            // ln((1 - beta) / alpha) and ln(beta / (1 - alpha)).
            upper: ln_ratio([beta_den - beta_num, alpha_den], [beta_den, alpha_num]),
            lower: ln_ratio([beta_num, alpha_den], [beta_den, alpha_den - alpha_num]),
        })
    }

    /// Sampling model and hypotheses.
    #[must_use]
    pub const fn model(&self) -> Model {
        self.model
    }

    /// Nominal probability of rollback evidence under `H0`.
    #[must_use]
    pub const fn alpha(&self) -> Rate {
        self.alpha
    }

    /// Nominal probability of promote evidence under `H1`.
    #[must_use]
    pub const fn beta(&self) -> Rate {
        self.beta
    }

    /// Samples at or after which an undecided test is truncated.
    #[must_use]
    pub const fn limit(&self) -> u64 {
        self.limit
    }

    /// What truncation stands for.
    #[must_use]
    pub const fn on_limit(&self) -> OnLimit {
        self.on_limit
    }

    /// Log-likelihood ratio the evidence must reach to count as a regression,
    /// in millionths of a nat: `ln((1 - beta) / alpha)`.
    #[must_use]
    pub fn upper_micro(&self) -> i64 {
        micro(self.upper.hi)
    }

    /// Log-likelihood ratio the evidence must fall to to count as cleared, in
    /// millionths of a nat: `ln(beta / (1 - alpha))`.
    #[must_use]
    pub fn lower_micro(&self) -> i64 {
        micro(self.lower.lo)
    }

    /// The error bounds this plan guarantees; see [`Bounds`].
    #[must_use]
    pub fn bounds(&self) -> Bounds {
        let (alpha_num, alpha_den) = (u64::from(self.alpha.num), u64::from(self.alpha.den));
        let (beta_num, beta_den) = (u64::from(self.beta.num), u64::from(self.beta.den));
        let false_rollback = Fraction {
            num: alpha_num * beta_den,
            den: alpha_den * (beta_den - beta_num),
        };
        let false_promote = Fraction {
            num: beta_num * alpha_den,
            den: beta_den * (alpha_den - alpha_num),
        };
        Bounds {
            false_rollback: (self.on_limit != OnLimit::Rollback).then_some(false_rollback),
            false_promote: (self.on_limit != OnLimit::Promote).then_some(false_promote),
        }
    }
}
