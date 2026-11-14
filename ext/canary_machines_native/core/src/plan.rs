//! Hypotheses, error levels and limits of one sequential test.

use budget_machines::Objective;

use crate::ln::{Interval, ln_ratio, micro_nats};

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

    /// Numerator and denominator, widened for exact products.
    const fn wide(self) -> (u64, u64) {
        (self.num as u64, self.den as u64)
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

    /// The probability `num / (num + den)` that a discordant pair is the
    /// canary's fault, as a widened `(num, den)`.
    const fn hit_rate(self) -> (u64, u64) {
        (self.num as u64, self.num as u64 + self.den as u64)
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
            } => [acceptable.wide(), unacceptable.wide()],
            Self::Paired {
                acceptable,
                unacceptable,
            } => [acceptable.hit_rate(), unacceptable.hit_rate()],
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
    num: u64,
    den: u64,
}

impl Fraction {
    /// `None` when `den` is zero.
    #[must_use]
    pub const fn new(num: u64, den: u64) -> Option<Self> {
        if den == 0 {
            return None;
        }
        Some(Self { num, den })
    }

    /// Numerator.
    #[must_use]
    pub const fn num(self) -> u64 {
        self.num
    }

    /// Denominator; never zero.
    #[must_use]
    pub const fn den(self) -> u64 {
        self.den
    }

    /// Whether the fraction is strictly between zero and one.
    const fn proper(self) -> bool {
        self.num > 0 && self.num < self.den
    }
}

/// Error probabilities a plan guarantees for the statistical test alone,
/// under its sampling model. `None` means the limit policy forces that
/// decision at truncation, so this crate claims nothing about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    false_rollback: Option<Fraction>,
    false_promote: Option<Fraction>,
}

impl Bounds {
    /// Bounds rebuilt from their parts, or `None` when no plan could guarantee
    /// them: both absent (a limit policy forces one decision, never two), or
    /// one not strictly between zero and one (`alpha + beta < 1` keeps each
    /// bound below one, and positive levels keep it above zero).
    #[must_use]
    pub const fn new(
        false_rollback: Option<Fraction>,
        false_promote: Option<Fraction>,
    ) -> Option<Self> {
        let valid = match (false_rollback, false_promote) {
            (None, None) => false,
            (Some(only), None) | (None, Some(only)) => only.proper(),
            (Some(rollback), Some(promote)) => rollback.proper() && promote.proper(),
        };
        if !valid {
            return None;
        }
        Some(Self {
            false_rollback,
            false_promote,
        })
    }

    /// Bound on rollback evidence when `H0` holds: `alpha / (1 - beta)`.
    #[must_use]
    pub const fn false_rollback(self) -> Option<Fraction> {
        self.false_rollback
    }

    /// Bound on promote evidence when `H1` holds: `beta / (1 - alpha)`.
    #[must_use]
    pub const fn false_promote(self) -> Option<Fraction> {
        self.false_promote
    }
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
    hit: Interval,
    miss: Interval,
    upper: Interval,
    lower: Interval,
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
        let ((alpha_num, alpha_den), (beta_num, beta_den)) = (alpha.wide(), beta.wide());
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

    /// Interval around the upper threshold `ln((1 - beta) / alpha)`, in units
    /// of `2^-FRACTION_BITS` nat. For `alpha = an / ad` and `beta = bn / bd` it
    /// is exactly `sliderule_machines::ln_ratio::<FRACTION_BITS>(&[bd - bn, ad],
    /// &[bd, an])`. A test regresses once its whole log-likelihood interval is
    /// at or above `hi`.
    #[must_use]
    pub const fn upper(&self) -> Interval {
        self.upper
    }

    /// Interval around the lower threshold `ln(beta / (1 - alpha))`, in units
    /// of `2^-FRACTION_BITS` nat: exactly `ln_ratio::<FRACTION_BITS>(&[bn, ad],
    /// &[bd, ad - an])`. A test is cleared once its whole log-likelihood
    /// interval is at or below `lo`.
    #[must_use]
    pub const fn lower(&self) -> Interval {
        self.lower
    }

    /// Interval around the weight of one hit, `ln(p1 / p0)`, in units of
    /// `2^-FRACTION_BITS` nat; always above zero. `p0 = n0 / d0` and
    /// `p1 = n1 / d1` are the hit probabilities under `H0` and `H1`: the two
    /// rates of a [`Model::Reference`], and `num / (num + den)` of each odds
    /// ratio of a [`Model::Paired`]. Exactly `ln_ratio::<FRACTION_BITS>(&[n1,
    /// d0], &[d1, n0])`.
    #[must_use]
    pub const fn hit(&self) -> Interval {
        self.hit
    }

    /// Interval around the weight of one miss, `ln((1 - p1) / (1 - p0))`, in
    /// units of `2^-FRACTION_BITS` nat; always below zero. Exactly
    /// `ln_ratio::<FRACTION_BITS>(&[d1 - n1, d0], &[d1, d0 - n0])` for the
    /// probabilities of [`hit`](Self::hit).
    ///
    /// The log-likelihood ratio after `h` hits and `m` misses lies between
    /// `h * hit().lo + m * miss().lo` and `h * hit().hi + m * miss().hi`.
    #[must_use]
    pub const fn miss(&self) -> Interval {
        self.miss
    }

    /// Log-likelihood ratio the evidence must reach to count as a regression,
    /// in millionths of a nat: [`micro_nats`] of the `hi` end of
    /// [`upper`](Self::upper).
    #[must_use]
    pub fn upper_micro(&self) -> i64 {
        micro_nats(self.upper.hi)
    }

    /// Log-likelihood ratio the evidence must fall to to count as cleared, in
    /// millionths of a nat: [`micro_nats`] of the `lo` end of
    /// [`lower`](Self::lower).
    #[must_use]
    pub fn lower_micro(&self) -> i64 {
        micro_nats(self.lower.lo)
    }

    /// The error bounds this plan guarantees; see [`Bounds`].
    #[must_use]
    pub fn bounds(&self) -> Bounds {
        let ((alpha_num, alpha_den), (beta_num, beta_den)) = (self.alpha.wide(), self.beta.wide());
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
