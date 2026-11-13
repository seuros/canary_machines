//! Randomized runs against the naive models, and simulated error rates.

use super::{
    model::{Fleet, Naive, Rng},
    paired, pairs, plan, rate, reference,
    std::vec::Vec,
    trials,
};
use core::time::Duration;

use crate::{Canary, Guardrail, Observation, OnLimit, Outcome, Plan, Refusal, Sprt};

/// The model counts whole ticks; the canary is handed the same instants as
/// `Duration`s. A tick is not a whole second, so instants carry.
const fn t(ticks: u64) -> Duration {
    Duration::from_millis(ticks * 400)
}

const POLICIES: [OnLimit; 3] = [OnLimit::Hold, OnLimit::Rollback, OnLimit::Promote];
const GUARDRAILS: [Guardrail; 3] = [Guardrail::Clear, Guardrail::NoData, Guardrail::Breached];

fn small(rng: &mut Rng, bound: u64) -> u32 {
    u32::try_from(rng.below(bound)).unwrap_or(0)
}

/// A random plan and the naive test with the same parameters.
fn random_test(rng: &mut Rng) -> (Plan, Naive) {
    let alpha = (small(rng, 20) + 1, 100);
    let beta = (small(rng, 20) + 1, 100);
    let limit = rng.below(400) + 1;
    let on_limit = POLICIES[usize::try_from(rng.below(3)).unwrap_or(0)];
    let is_paired = rng.chance(1, 2);
    let (model, null, alt) = if is_paired {
        let null = (small(rng, 5) + 1, small(rng, 5) + 1);
        let alt = (null.0 * (small(rng, 4) + 2), null.1);
        let hit = |(num, den): (u32, u32)| f64::from(num) / f64::from(num + den);
        (paired(null, alt), hit(null), hit(alt))
    } else {
        let den = 1000;
        let null = small(rng, 300) + 1;
        let alt = null + small(rng, 500) + 1;
        let model = reference((null, den), (alt, den));
        (
            model,
            f64::from(null) / f64::from(den),
            f64::from(alt) / f64::from(den),
        )
    };
    let level = |(num, den): (u32, u32)| f64::from(num) / f64::from(den);
    let naive = Naive::new(
        is_paired,
        null,
        alt,
        level(alpha),
        level(beta),
        limit,
        on_limit,
    );
    let plan = Plan::new(
        model,
        rate(alpha.0, alpha.1),
        rate(beta.0, beta.1),
        limit,
        on_limit,
    )
    .unwrap_or_else(|invalid| panic!("valid plan: {invalid:?}"));
    (plan, naive)
}

#[test]
#[allow(clippy::cast_possible_truncation)]
fn randomized_canary_matches_the_naive_model() {
    let mut rng = Rng(0x00C0_FFEE);
    let mut doubtful = 0;
    let (mut cleared, mut regressed, mut truncated, mut refused) = (0, 0, 0, 0);
    for _ in 0..1500 {
        let mut canary = Canary::<4>::new(10);
        let mut fleet = Fleet {
            generation: 10,
            last: None,
            tests: Vec::new(),
        };
        for _ in 0..rng.below(6) {
            let (plan, naive) = random_test(&mut rng);
            let added = canary.add(plan);
            if fleet.tests.len() < 4 {
                assert_eq!(added, Ok(fleet.tests.len()));
                fleet.tests.push(naive);
            } else {
                assert_eq!(added, Err(Refusal::Full));
            }
        }
        let mut now = 100;
        for _ in 0..60 {
            if rng.chance(1, 25) {
                let generation = fleet.generation + rng.below(3) - 1;
                assert_eq!(canary.restart(generation), fleet.restart(generation));
            }
            let generation = if rng.chance(1, 12) {
                fleet.generation - 1
            } else {
                fleet.generation
            };
            let at = if rng.chance(1, 12) {
                now - 1
            } else {
                now + rng.below(3)
            };
            let index = rng.below(5) as usize;
            // Usually the kind the test takes, sometimes the wrong one.
            let is_paired =
                fleet.tests.get(index).is_some_and(|test| test.paired) != rng.chance(1, 10);
            let (hits, misses, other) = (rng.below(4), rng.below(12), rng.below(20));
            let (observation, counts) = if is_paired {
                (pairs(hits, misses, other), (hits, misses, other))
            } else {
                (trials(hits, hits + misses), (hits, misses, 0))
            };
            let before = canary;
            let expected = fleet.observe(generation, at, index, is_paired, counts);
            let actual = canary.observe(generation, t(at), index, observation);
            if fleet.tests.iter().any(|test| test.doubtful) {
                doubtful += 1;
                break;
            }
            match (expected, actual) {
                (Ok(()), Ok(status)) => {
                    now = now.max(at);
                    let test = &fleet.tests[index];
                    assert_eq!(status.look.outcome, test.outcome);
                    match status.look.outcome {
                        Outcome::Cleared => cleared += 1,
                        Outcome::Regressed => regressed += 1,
                        Outcome::Truncated => truncated += 1,
                        Outcome::Undecided => {}
                    }
                    assert_eq!(status.decided_at, test.decided_at.map(t));
                    assert_eq!(
                        (status.look.hits, status.look.misses, status.look.samples),
                        (test.hits, test.misses, test.samples)
                    );
                    let micro = (test.llr() * 1e6).floor() as i64;
                    assert!((status.look.llr_micro - micro).abs() <= 1);
                    assert_eq!(canary.status(index), Some(status));
                }
                (Err(expected), Err(actual)) => {
                    assert_eq!(expected, actual);
                    refused += 1;
                    assert_eq!(canary, before);
                }
                (expected, actual) => panic!("model {expected:?}, core {actual:?}"),
            }
            assert_eq!(
                (canary.generation(), canary.last()),
                (fleet.generation, fleet.last.map(t))
            );
            for guardrail in GUARDRAILS {
                assert_eq!(canary.recommend(guardrail), fleet.recommend(guardrail));
            }
        }
    }
    // The run has to have exercised every outcome and plenty of refusals.
    assert!(cleared > 500 && regressed > 200 && truncated > 50 && refused > 1000);
    assert!(
        doubtful < 15,
        "{doubtful} runs were too close to a threshold to compare"
    );
}

/// Fraction of `runs` simulated rollouts that end with `outcome`, in thousandths.
fn frequency(
    rng: &mut Rng,
    plan: Plan,
    outcome: Outcome,
    runs: u64,
    mut draw: impl FnMut(&mut Rng) -> Observation,
) -> u64 {
    let mut seen = 0;
    for _ in 0..runs {
        let mut sprt = Sprt::new(plan);
        while sprt.outcome() == Outcome::Undecided {
            assert!(sprt.observe(draw(rng)).is_ok());
        }
        seen += u64::from(sprt.outcome() == outcome);
    }
    seen * 1000 / runs
}

#[test]
fn simulated_error_rates_respect_the_wald_bounds() {
    // alpha / (1 - beta) = beta / (1 - alpha) = 52.6 thousandths; the slack
    // covers three standard errors of a 4000-run estimate.
    let mut rng = Rng(0xAB_CDEF);
    let bound = 53 + 11;
    let plan_of = |model| plan(model, 1_000_000, OnLimit::Hold);

    let fixed = plan_of(reference((5, 100), (15, 100)));
    let trial = |percent: u64| move |rng: &mut Rng| trials(u64::from(rng.chance(percent, 100)), 1);
    assert!(frequency(&mut rng, fixed, Outcome::Regressed, 4000, trial(5)) <= bound);
    assert!(frequency(&mut rng, fixed, Outcome::Cleared, 4000, trial(15)) <= bound);
    // Composite hypotheses: better than acceptable, worse than unacceptable.
    assert!(frequency(&mut rng, fixed, Outcome::Regressed, 4000, trial(2)) <= 10);
    assert!(frequency(&mut rng, fixed, Outcome::Cleared, 4000, trial(30)) <= 10);
    // Looking only every 20 trials keeps the bounds.
    let batch = |percent: u64| {
        move |rng: &mut Rng| {
            trials(
                (0..20).map(|_| u64::from(rng.chance(percent, 100))).sum(),
                20,
            )
        }
    };
    assert!(frequency(&mut rng, fixed, Outcome::Regressed, 4000, batch(5)) <= bound);
    assert!(frequency(&mut rng, fixed, Outcome::Cleared, 4000, batch(15)) <= bound);

    // Odds ratio 1 against 3, whatever the baseline's own failure rate is.
    let matched = plan_of(paired((1, 1), (3, 1)));
    let pair = |baseline: (u64, u64), canary: (u64, u64)| {
        move |rng: &mut Rng| {
            let (b, c) = (
                rng.chance(baseline.0, baseline.1),
                rng.chance(canary.0, canary.1),
            );
            pairs(u64::from(c && !b), u64::from(b && !c), u64::from(b == c))
        }
    };
    for (baseline, same, tripled) in [((1, 5), (1, 5), (3, 7)), ((1, 2), (1, 2), (3, 4))] {
        assert!(
            frequency(
                &mut rng,
                matched,
                Outcome::Regressed,
                4000,
                pair(baseline, same)
            ) <= bound
        );
        assert!(
            frequency(
                &mut rng,
                matched,
                Outcome::Cleared,
                4000,
                pair(baseline, tripled)
            ) <= bound
        );
    }

    // A tight limit truncates instead of deciding, so the bounds still hold.
    let short = plan(reference((5, 100), (15, 100)), 20, OnLimit::Hold);
    assert!(frequency(&mut rng, short, Outcome::Regressed, 4000, trial(5)) <= bound);
    assert!(frequency(&mut rng, short, Outcome::Cleared, 4000, trial(15)) <= bound);
    assert!(frequency(&mut rng, short, Outcome::Truncated, 4000, trial(5)) > 100);
}
