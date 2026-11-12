extern crate std;

use budget_machines::Objective;

use super::*;
use crate::ln::{ln_ratio, micro};

mod canary;
mod model;
mod random;

use model::Rng;

fn rate(num: u32, den: u32) -> Rate {
    Rate::new(num, den).unwrap_or_else(|| panic!("valid rate"))
}

fn odds(num: u32, den: u32) -> OddsRatio {
    OddsRatio::new(num, den).unwrap_or_else(|| panic!("valid odds ratio"))
}

fn reference(null: (u32, u32), alt: (u32, u32)) -> Model {
    Model::Reference {
        acceptable: rate(null.0, null.1),
        unacceptable: rate(alt.0, alt.1),
    }
}

fn paired(null: (u32, u32), alt: (u32, u32)) -> Model {
    Model::Paired {
        acceptable: odds(null.0, null.1),
        unacceptable: odds(alt.0, alt.1),
    }
}

/// A plan at alpha = beta = 5%.
fn plan(model: Model, limit: u64, on_limit: OnLimit) -> Plan {
    Plan::new(model, rate(5, 100), rate(5, 100), limit, on_limit)
        .unwrap_or_else(|invalid| panic!("valid plan: {invalid:?}"))
}

fn trials(bad: u64, total: u64) -> Observation {
    Observation::Trials(Counts::new(total - bad, total).unwrap_or_else(|| panic!("valid counts")))
}

fn pairs(canary_only: u64, baseline_only: u64, concordant: u64) -> Observation {
    Observation::Pairs(
        Pairs::new(canary_only, baseline_only, concordant).unwrap_or_else(|| panic!("valid pairs")),
    )
}

/// One in a hundred against one in ten.
fn tenfold() -> Model {
    reference((1, 100), (1, 10))
}

#[allow(clippy::cast_precision_loss)]
fn real(value: i128) -> f64 {
    value as f64 / (1u64 << FRACTION_BITS) as f64
}

#[test]
fn log_likelihoods_are_the_shared_kernel_at_48_fraction_bits() {
    let mut rng = Rng(0x5EED);
    for _ in 0..2_000 {
        let (a, b, c, d) = (rng.next() | 1, rng.below(1000) + 1, rng.next() | 1, 7);
        let interval = ln_ratio([a, b], [c, d]);
        assert_eq!(
            interval,
            sliderule_machines::ln_ratio::<FRACTION_BITS>(&[a, b], &[c, d])
        );
        assert!(interval.lo < interval.hi && interval.hi - interval.lo <= 2);
    }
    // ln 2 = sum 1 / (k 2^k), in 120 fraction bits with every term rounded down.
    let sum: u128 = (1..=120u32)
        .map(|k| (1u128 << (120 - k)) / u128::from(k))
        .sum();
    let interval = ln_ratio([2, 1], [1, 1]);
    let exact = i128::try_from(sum >> (120 - FRACTION_BITS)).unwrap_or(0);
    assert!(interval.lo <= exact && exact < interval.hi);
    assert!((real(interval.lo) - core::f64::consts::LN_2).abs() < 1e-12);
    assert_eq!(ln_ratio([1, 1], [1, 1]), ln_ratio([7, 9], [9, 7]));
    assert!(ln_ratio([u64::MAX, u64::MAX], [1, 1]).hi < 89 << FRACTION_BITS);
    assert_eq!(micro(1 << FRACTION_BITS), 1_000_000);
    assert_eq!(micro(-(1 << FRACTION_BITS)), -1_000_000);
    assert_eq!(micro(i128::MAX >> 6), i64::MAX);
    assert_eq!(micro(i128::MIN >> 6), i64::MIN);
}

#[test]
fn constructors_reject_impossible_inputs() {
    assert_eq!(Rate::new(0, 10), None);
    assert_eq!(Rate::new(10, 10), None);
    assert_eq!(Rate::new(1, 0), None);
    assert_eq!(OddsRatio::new(0, 1), None);
    assert_eq!(OddsRatio::new(1, 0), None);
    assert_eq!(Pairs::new(u64::MAX, 1, 0), None);
    assert_eq!(Pairs::new(u64::MAX, 0, 1), None);
    assert!(Pairs::new(u64::MAX, 0, 0).is_some());

    let slo = Objective::new(999, 1000).unwrap_or_else(|| panic!("valid objective"));
    assert_eq!(Rate::failure_of(slo), Some(rate(1, 1000)));
    let anything = Objective::new(0, 5).unwrap_or_else(|| panic!("valid objective"));
    assert_eq!(Rate::failure_of(anything), None);

    let level = rate(5, 100);
    let build = |model, alpha, beta, limit| Plan::new(model, alpha, beta, limit, OnLimit::Hold);
    assert_eq!(
        build(reference((1, 10), (1, 10)), level, level, 9),
        Err(Invalid::Hypotheses)
    );
    assert_eq!(
        build(reference((2, 10), (1, 10)), level, level, 9),
        Err(Invalid::Hypotheses)
    );
    assert_eq!(
        build(paired((2, 1), (2, 1)), level, level, 9),
        Err(Invalid::Hypotheses)
    );
    assert_eq!(
        build(paired((3, 1), (2, 1)), level, level, 9),
        Err(Invalid::Hypotheses)
    );
    assert_eq!(
        build(tenfold(), rate(1, 2), rate(1, 2), 9),
        Err(Invalid::ErrorLevels)
    );
    assert_eq!(
        build(tenfold(), rate(2, 3), rate(1, 2), 9),
        Err(Invalid::ErrorLevels)
    );
    assert!(build(tenfold(), rate(1, 2), rate(499, 1000), 9).is_ok());
    assert_eq!(build(tenfold(), level, level, 0), Err(Invalid::Limit));
    let hair = reference((1, u32::MAX), (1, u32::MAX - 1));
    assert_eq!(
        build(hair, level, level, 9),
        Err(Invalid::Indistinguishable)
    );
    let wide = reference((1, u32::MAX), (u32::MAX - 1, u32::MAX));
    assert!(build(wide, level, level, u64::MAX).is_ok());
    assert!(build(paired((1, u32::MAX), (u32::MAX, 1)), level, level, 9).is_ok());
}

#[test]
fn plan_reports_thresholds_and_bounds() {
    let hold = plan(tenfold(), 1000, OnLimit::Hold);
    // ln 19 = 2.944438979...
    assert_eq!(hold.upper_micro(), 2_944_438);
    assert_eq!(hold.lower_micro(), -2_944_439);
    assert_eq!(
        (hold.limit(), hold.on_limit(), hold.model()),
        (1000, OnLimit::Hold, tenfold())
    );
    assert_eq!((hold.alpha(), hold.beta()), (rate(5, 100), rate(5, 100)));
    let wald = Some(Fraction {
        num: 500,
        den: 9500,
    });
    assert_eq!(
        hold.bounds(),
        Bounds {
            false_rollback: wald,
            false_promote: wald
        }
    );
    let rollback = plan(tenfold(), 1000, OnLimit::Rollback).bounds();
    assert_eq!(
        (rollback.false_rollback, rollback.false_promote),
        (None, wald)
    );
    let promote = plan(tenfold(), 1000, OnLimit::Promote).bounds();
    assert_eq!(
        (promote.false_rollback, promote.false_promote),
        (wald, None)
    );

    let lopsided = Plan::new(tenfold(), rate(1, 100), rate(1, 5), 9, OnLimit::Hold)
        .unwrap_or_else(|invalid| panic!("valid plan: {invalid:?}"))
        .bounds();
    assert_eq!(lopsided.false_rollback, Some(Fraction { num: 5, den: 400 }));
    assert_eq!(
        lopsided.false_promote,
        Some(Fraction { num: 100, den: 495 })
    );
}

#[test]
fn reference_test_crosses_each_threshold_at_the_textbook_sample() {
    // A failure weighs ln 10 = 2.302585; a success ln(0.9 / 0.99) = -0.095310.
    let mut sprt = Sprt::new(plan(tenfold(), 1000, OnLimit::Hold));
    assert_eq!(sprt.look().llr_micro, 0);
    let first = sprt.observe(trials(1, 1));
    assert_eq!(
        first,
        Ok(Look {
            outcome: Outcome::Undecided,
            hits: 1,
            misses: 0,
            samples: 1,
            llr_micro: 2_302_585
        })
    );
    let second = sprt
        .observe(trials(1, 1))
        .unwrap_or_else(|_| panic!("accepted"));
    assert_eq!(
        (second.outcome, second.llr_micro),
        (Outcome::Regressed, 4_605_170)
    );
    assert_eq!(sprt.observe(trials(0, 1)), Err(Refused::Concluded));
    assert_eq!(sprt.look(), second);

    // 2.944439 / 0.095310 = 30.9: thirty clean trials are not enough.
    sprt.reset();
    assert_eq!(sprt.look().samples, 0);
    let thirty = sprt
        .observe(trials(0, 30))
        .unwrap_or_else(|_| panic!("accepted"));
    assert_eq!(
        (thirty.outcome, thirty.llr_micro),
        (Outcome::Undecided, -2_859_306)
    );
    assert_eq!(
        sprt.observe(trials(0, 0)).map(|look| look.outcome),
        Ok(Outcome::Undecided)
    );
    assert_eq!(
        sprt.observe(trials(0, 1)).map(|look| look.outcome),
        Ok(Outcome::Cleared)
    );
    assert_eq!(sprt.outcome(), Outcome::Cleared);
    assert_eq!(sprt.plan().limit(), 1000);
}

#[test]
fn paired_test_ignores_concordant_pairs_as_evidence() {
    // Odds ratio 1 against 2: a discordant pair is the canary's fault with
    // probability 1/2 against 2/3. ln(4/3) = 0.287682, ln(2/3) = -0.405465.
    let mut sprt = Sprt::new(plan(paired((1, 1), (2, 1)), 10_000, OnLimit::Hold));
    let look = sprt
        .observe(pairs(1, 1, 500))
        .unwrap_or_else(|_| panic!("accepted"));
    assert_eq!((look.hits, look.misses, look.samples), (1, 1, 502));
    assert_eq!(look.llr_micro, -117_784);
    // 2.944439 / 0.287682 = 10.2 more canary-only failures than that cancels.
    assert_eq!(
        sprt.observe(pairs(10, 0, 0)).map(|look| look.outcome),
        Ok(Outcome::Undecided)
    );
    assert_eq!(
        sprt.observe(pairs(1, 0, 0)).map(|look| look.outcome),
        Ok(Outcome::Regressed)
    );

    let mut sprt = Sprt::new(plan(paired((1, 1), (2, 1)), 10_000, OnLimit::Hold));
    assert_eq!(
        sprt.observe(pairs(0, 7, 0)).map(|look| look.outcome),
        Ok(Outcome::Undecided)
    );
    assert_eq!(
        sprt.observe(pairs(0, 1, 0)).map(|look| look.outcome),
        Ok(Outcome::Cleared)
    );

    let before = sprt;
    assert_eq!(
        Sprt::new(*before.plan()).observe(trials(0, 1)),
        Err(Refused::WrongModel)
    );
    let mut single = Sprt::new(plan(tenfold(), 10, OnLimit::Hold));
    assert_eq!(single.observe(pairs(1, 0, 0)), Err(Refused::WrongModel));
    assert_eq!(single.look().samples, 0);
}

#[test]
fn truncation_at_the_limit_and_u64_extremes() {
    // A batch may carry the count past the limit; the look happens at its end.
    let mut sprt = Sprt::new(plan(paired((1, 1), (2, 1)), 100, OnLimit::Hold));
    assert_eq!(
        sprt.observe(pairs(1, 1, 97)).map(|look| look.outcome),
        Ok(Outcome::Undecided)
    );
    assert_eq!(
        sprt.observe(pairs(0, 0, 50)).map(|look| look.outcome),
        Ok(Outcome::Truncated)
    );
    assert_eq!(sprt.observe(pairs(0, 9, 0)), Err(Refused::Concluded));
    // A threshold reached on the limit's own look still counts.
    let mut sprt = Sprt::new(plan(tenfold(), 2, OnLimit::Hold));
    assert_eq!(
        sprt.observe(trials(2, 2)).map(|look| look.outcome),
        Ok(Outcome::Regressed)
    );

    let mut sprt = Sprt::new(plan(paired((1, 1), (2, 1)), u64::MAX, OnLimit::Hold));
    let quiet = sprt
        .observe(pairs(0, 0, u64::MAX - 1))
        .unwrap_or_else(|_| panic!("accepted"));
    assert_eq!((quiet.outcome, quiet.llr_micro), (Outcome::Undecided, 0));
    assert_eq!(sprt.observe(pairs(1, 1, 0)), Err(Refused::Overflow));
    assert_eq!(sprt.look(), quiet);
    assert_eq!(
        sprt.observe(pairs(0, 0, 1)).map(|look| look.outcome),
        Ok(Outcome::Truncated)
    );

    let mut sprt = Sprt::new(plan(tenfold(), u64::MAX, OnLimit::Hold));
    let flood = sprt
        .observe(trials(0, u64::MAX))
        .unwrap_or_else(|_| panic!("accepted"));
    assert_eq!(
        (flood.outcome, flood.llr_micro),
        (Outcome::Cleared, i64::MIN)
    );
    let mut sprt = Sprt::new(plan(tenfold(), u64::MAX, OnLimit::Hold));
    let fire = sprt
        .observe(trials(u64::MAX, u64::MAX))
        .unwrap_or_else(|_| panic!("accepted"));
    assert_eq!(
        (fire.outcome, fire.llr_micro),
        (Outcome::Regressed, i64::MAX)
    );
}
