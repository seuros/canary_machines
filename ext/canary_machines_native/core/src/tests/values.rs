//! The validating constructors of the returned values: everything the core
//! produces is accepted back, nothing else is.

use core::{assert_matches, time::Duration};

use super::{fraction, model::Rng, paired, pairs, plan, tenfold, trials};
use crate::{
    Action, Bounds, Canary, Fraction, Guardrail, Look, OnLimit, Outcome, Reason, Recommendation,
    Refusal, Status,
};

const ACTIONS: [Action; 4] = [
    Action::Continue,
    Action::Promote,
    Action::Rollback,
    Action::Hold,
];
const REASONS: [Reason; 6] = [
    Reason::Evidence,
    Reason::Limit,
    Reason::Guardrail,
    Reason::GuardrailNoData,
    Reason::Sampling,
    Reason::NoTests,
];

fn look_back(look: Look) -> Option<Look> {
    Look::new(
        look.outcome(),
        look.hits(),
        look.misses(),
        look.samples(),
        look.llr_micro(),
    )
}

fn accepted_back(canary: &Canary<4>) {
    for index in 0..canary.len() {
        let status = canary.status(index).expect("registered test");
        assert_eq!(look_back(status.look()), Some(status.look()));
        assert_eq!(
            Status::new(status.look(), status.decided_at()),
            Some(status)
        );
        assert_eq!(
            status.is_decided(),
            status.look().outcome() != Outcome::Undecided
        );
    }
    for guardrail in [Guardrail::Clear, Guardrail::NoData, Guardrail::Breached] {
        let advice = canary.recommend(guardrail);
        let back = Recommendation::new(advice.action(), advice.reason(), advice.test());
        assert_eq!(back, Some(advice));
    }
}

#[test]
fn everything_a_canary_produces_is_accepted_back() {
    let mut rng = Rng(0x5EED_CA9A_8F3D_0001);
    for round in 0..300_u64 {
        let mut canary = Canary::<4>::new(0);
        accepted_back(&canary);
        let paired_plan = plan(paired((1, 1), (3, 1)), 60, OnLimit::Promote);
        for added in [plan(tenfold(), 80, OnLimit::Hold), paired_plan] {
            assert!(canary.add(added).is_ok());
        }
        for step in 0..40 {
            let at = Duration::from_secs(round * 100 + step);
            let bad = rng.below(4);
            // Same generation, no earlier instant: only a latched test refuses.
            let (only, other) = (rng.below(3), rng.below(3));
            for (index, batch) in [
                (0, trials(bad, bad + rng.below(12))),
                (1, pairs(only, other, rng.below(5))),
            ] {
                assert_matches!(
                    canary.observe(0, at, index, batch),
                    Ok(_) | Err(Refusal::Concluded)
                );
            }
            accepted_back(&canary);
        }
    }
}

#[test]
fn values_no_test_could_produce_are_refused() {
    assert_eq!(Fraction::new(1, 0), None);
    let half = fraction(1, 2);
    let (whole, zero) = (Some(fraction(2, 2)), Some(fraction(0, 2)));
    assert_eq!(Bounds::new(None, None), None);
    assert_eq!(Bounds::new(whole, Some(half)), None);
    assert_eq!(Bounds::new(Some(half), zero), None);
    assert!(Bounds::new(None, Some(half)).is_some());

    for (outcome, hits, misses, samples, llr_micro) in [
        (Outcome::Undecided, 3, 3, 5, 0),
        (Outcome::Undecided, u64::MAX, 1, u64::MAX, 0),
        (Outcome::Truncated, 0, 0, 0, 0),
        (Outcome::Undecided, 0, 0, 4, 7),
        (Outcome::Regressed, 0, 4, 4, 9),
        (Outcome::Regressed, 4, 0, 4, -1),
        (Outcome::Cleared, 4, 0, 4, -9),
        (Outcome::Cleared, 0, 4, 4, 0),
    ] {
        assert_eq!(Look::new(outcome, hits, misses, samples, llr_micro), None);
    }
    let undecided = Look::new(Outcome::Undecided, 0, 0, 0, 0).expect("a fresh look");
    let regressed = Look::new(Outcome::Regressed, 4, 0, 4, 9).expect("a regression");
    assert_eq!(Status::new(undecided, Some(Duration::ZERO)), None);
    assert_eq!(Status::new(regressed, None), None);

    let mut valid = 0;
    for action in ACTIONS {
        for reason in REASONS {
            for test in [None, Some(0)] {
                valid += usize::from(Recommendation::new(action, reason, test).is_some());
            }
        }
    }
    // The nine combinations `recommend` gives; see `Recommendation::new`.
    assert_eq!(valid, 9);
}
