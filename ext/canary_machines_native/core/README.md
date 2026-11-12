# canary-machines

> **AMS Wald**, NCC-4226, *Wald*-class proving scout.

Engine-room flight manual: accumulate a log-likelihood ratio, stop the first
time it leaves the band, and say what that means for the rollout.

Allocation-free `no_std` sequential probability ratio tests over Bernoulli
observations, and a bounded set of them that adds up to a promote, rollback,
continue or hold recommendation. The caller supplies every count, every
instant and the rollout generation. The crate has no clock, no metrics source
and no hand on any deployment: it recommends and nothing else.

## Sampling models

Aggregate baseline and canary counts do not define a test, so a `Plan` names
its `Model`:

- `Model::Reference`: canary trials are independent Bernoulli draws with one
  failure probability `p`. `H0: p <= acceptable`, `H1: p >= unacceptable`, both
  fixed by the caller in advance as exact `Rate`s. The baseline is a constant;
  any uncertainty in it was the caller's to settle. `Rate::failure_of` takes
  the rate straight from a `budget_machines::Objective`.
- `Model::Paired`: baseline and canary are sampled concurrently and matched
  into pairs by a rule that ignores outcomes. Concordant pairs carry no
  evidence. Given a discordant pair, the canary is the failing arm with
  probability `psi / (1 + psi)` for odds ratio `psi`, whatever the baseline
  rate is. `H0: psi <= acceptable`, `H1: psi >= unacceptable`.

Observations are `Observation::Trials(Counts)` or `Observation::Pairs(Pairs)`;
the wrong kind for the model is refused.

## Claimed error bounds

For nominal levels `alpha` and `beta` with `alpha + beta < 1`, under the plan's
sampling model, for any look schedule that depends only on data already
observed, and at every point of the composite hypotheses:

- `P(Outcome::Regressed | H0) <= alpha / (1 - beta)`
- `P(Outcome::Cleared | H1) <= beta / (1 - alpha)`

These are Wald's inequalities, not `alpha` and `beta` themselves. Truncation
at the sample limit is a separate `Outcome::Truncated`, so both bounds hold at
any limit; what truncation costs is the guarantee of a decision. `OnLimit`
says what a truncated test stands for in a recommendation. `Hold` keeps both
bounds. `Rollback` forfeits the first and `Promote` the second, and
`Plan::bounds` reports the forfeited one as `None` instead of a number it
cannot justify. Bounds are per test; across tests the false-rollback bounds
add. A guardrail rollback is outside the statistics.

## Arithmetic

No floating point. `ln` of a ratio of 64-bit integers comes from `ln_ratio` of
the `sliderule-machines` crate, the one integer logarithm the fleet shares: each
factor at 58 fraction bits with a one-sided error, the sum within `2^-54` of
the exact logarithm, rounded outwards to an interval with
`FRACTION_BITS = 48` fraction bits. Weights,
thresholds and the accumulated ratio are all intervals; a threshold is reached
only when the whole interval is beyond it. Rounding can therefore delay a
decision whose exact ratio is within about `samples * 2^-47` of a threshold,
and can never produce one the exact arithmetic would not. Results are
identical on every platform.

## Use

```rust
use canary_machines::{
    Action, Canary, Counts, Guardrail, Model, Observation, OnLimit, Outcome, Plan, Rate,
};

let rate = |num, den| Rate::new(num, den).expect("0 < num < den");
let model = Model::Reference { acceptable: rate(1, 1000), unacceptable: rate(5, 1000) };
let plan = Plan::new(model, rate(5, 100), rate(5, 100), 50_000, OnLimit::Hold).expect("valid");

let mut canary = Canary::<4>::new(1);
let errors = canary.add(plan).expect("room");
assert_eq!(canary.recommend(Guardrail::Clear).action, Action::Continue);

let clean = Observation::Trials(Counts::new(734, 734).expect("good <= total"));
let status = canary.observe(1, 60, errors, clean).expect("current generation");
assert_eq!(status.look.outcome, Outcome::Cleared);
assert_eq!(canary.recommend(Guardrail::Clear).action, Action::Promote);
assert_eq!(canary.recommend(Guardrail::NoData).action, Action::Hold);
assert_eq!(canary.recommend(Guardrail::Breached).action, Action::Rollback);
```

- `Sprt` is one test: `observe` adds a batch and looks once, the first outcome
  latches, `reset` drops the evidence.
- `Canary<M>` holds up to `M` tests for one rollout generation. `observe`
  refuses a stale generation, a backwards instant, an unknown test, the wrong
  observation kind, a concluded test and a 64-bit overflow, and a refusal
  changes nothing. `restart` moves to a greater generation and clears evidence.
- `recommend` is pure. A breached `Guardrail`, a regressed test or a truncated
  `OnLimit::Rollback` test recommends rollback; an undecided test recommends
  continuing; a truncated `OnLimit::Hold` test, a guardrail without data or an
  empty canary recommends holding; only what is left is a promotion.
  `Guardrail` converts from `budget_machines::State` and `Standing`.

This is a policy core with an initial, unstable API. It cannot check that the
sampling model holds, and it does not collect metrics, shift traffic, promote
or roll back.
