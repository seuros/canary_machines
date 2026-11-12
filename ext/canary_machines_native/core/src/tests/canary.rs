//! The bounded set of tests: refusals, restarts and recommendations.

use budget_machines::{Standing, State};

use super::{paired, pairs, plan, tenfold, trials};
use crate::{Action, Canary, Guardrail, OnLimit, Outcome, Reason, Recommendation, Refused};

#[test]
fn canary_refusals_change_nothing() {
    let mut canary = Canary::<2>::new(7);
    assert!(canary.is_empty());
    assert_eq!(
        (canary.capacity(), canary.generation(), canary.last()),
        (2, 7, None)
    );
    assert_eq!(canary.add(plan(tenfold(), 100, OnLimit::Hold)), Ok(0));
    assert_eq!(
        canary.add(plan(paired((1, 1), (2, 1)), 100, OnLimit::Hold)),
        Ok(1)
    );
    assert_eq!(
        canary.add(plan(tenfold(), 100, OnLimit::Hold)),
        Err(Refused::Full)
    );
    assert_eq!(canary.len(), 2);
    assert_eq!(
        canary.plan(1).map(|plan| plan.model()),
        Some(paired((1, 1), (2, 1)))
    );
    assert_eq!((canary.plan(2), canary.status(2)), (None, None));

    let seen = canary
        .observe(7, 50, 0, trials(1, 5))
        .unwrap_or_else(|_| panic!("accepted"));
    assert_eq!((seen.look.samples, seen.decided_at), (5, None));
    let before = canary;
    assert_eq!(
        canary.observe(6, 60, 0, trials(1, 5)),
        Err(Refused::StaleGeneration)
    );
    assert_eq!(
        canary.observe(8, 60, 0, trials(1, 5)),
        Err(Refused::StaleGeneration)
    );
    assert_eq!(
        canary.observe(7, 49, 0, trials(1, 5)),
        Err(Refused::TimeRegression)
    );
    assert_eq!(
        canary.observe(7, 60, 2, trials(1, 5)),
        Err(Refused::UnknownTest)
    );
    assert_eq!(
        canary.observe(7, 60, 1, trials(1, 5)),
        Err(Refused::WrongModel)
    );
    assert_eq!(canary.restart(7), Err(Refused::StaleGeneration));
    assert_eq!(canary.restart(6), Err(Refused::StaleGeneration));
    assert_eq!(canary, before);

    // The same instant is not a regression.
    let done = canary
        .observe(7, 50, 0, trials(1, 1))
        .unwrap_or_else(|_| panic!("accepted"));
    assert_eq!(
        (done.look.outcome, done.decided_at),
        (Outcome::Regressed, Some(50))
    );
    let before = canary;
    assert_eq!(
        canary.observe(7, 70, 0, trials(0, 1)),
        Err(Refused::Concluded)
    );
    assert_eq!(canary, before);
    assert_eq!(
        canary
            .observe(7, u64::MAX, 1, pairs(0, 0, 1))
            .map(|s| s.decided_at),
        Ok(None)
    );
    assert_eq!(canary.last(), Some(u64::MAX));

    assert_eq!(canary.restart(u64::MAX), Ok(()));
    assert_eq!(canary.restart(u64::MAX), Err(Refused::StaleGeneration));
    let fresh = canary.status(0).unwrap_or_else(|| panic!("registered"));
    assert_eq!(
        (fresh.look.samples, fresh.look.outcome, fresh.decided_at),
        (0, Outcome::Undecided, None)
    );
    // Time did not rewind with the generation.
    assert_eq!(
        canary.observe(u64::MAX, 5, 0, trials(0, 1)),
        Err(Refused::TimeRegression)
    );
    assert!(canary.observe(u64::MAX, u64::MAX, 0, trials(0, 1)).is_ok());
    assert_eq!(
        Canary::<0>::new(0).add(plan(tenfold(), 1, OnLimit::Hold)),
        Err(Refused::Full)
    );
}

fn advice(action: Action, reason: Reason, test: Option<usize>) -> Recommendation {
    Recommendation {
        action,
        reason,
        test,
    }
}

#[test]
fn recommendation_follows_evidence_limits_and_guardrail() {
    let mut canary = Canary::<4>::new(1);
    assert_eq!(
        canary.recommend(Guardrail::Clear),
        advice(Action::Hold, Reason::NoTests, None)
    );
    assert_eq!(
        canary.recommend(Guardrail::Breached),
        advice(Action::Rollback, Reason::Guardrail, None)
    );
    let errors = canary.add(plan(tenfold(), 1000, OnLimit::Hold));
    let latency = canary.add(plan(paired((1, 1), (2, 1)), 10, OnLimit::Promote));
    assert_eq!((errors, latency), (Ok(0), Ok(1)));
    let sampling = advice(Action::Continue, Reason::Sampling, None);
    assert_eq!(canary.recommend(Guardrail::Clear), sampling);
    assert_eq!(canary.recommend(Guardrail::NoData), sampling);

    assert!(canary.observe(1, 0, 0, trials(0, 31)).is_ok());
    assert_eq!(canary.recommend(Guardrail::Clear), sampling);
    assert!(canary.observe(1, 1, 1, pairs(0, 0, 10)).is_ok());
    let promote = advice(Action::Promote, Reason::Limit, Some(1));
    assert_eq!(canary.recommend(Guardrail::Clear), promote);
    assert_eq!(
        canary.recommend(Guardrail::NoData),
        advice(Action::Hold, Reason::GuardrailNoData, None)
    );
    assert_eq!(
        canary.recommend(Guardrail::Breached),
        advice(Action::Rollback, Reason::Guardrail, None)
    );
    let before = canary;
    assert_eq!(canary.recommend(Guardrail::Clear), promote);
    assert_eq!(canary, before);

    assert_eq!(canary.restart(2), Ok(()));
    assert!(canary.observe(2, 2, 0, trials(0, 31)).is_ok());
    assert!(canary.observe(2, 2, 1, pairs(0, 8, 0)).is_ok());
    assert_eq!(
        canary.recommend(Guardrail::Clear),
        advice(Action::Promote, Reason::Evidence, None)
    );

    assert_eq!(canary.restart(3), Ok(()));
    assert_eq!(canary.add(plan(tenfold(), 5, OnLimit::Hold)), Ok(2));
    assert_eq!(canary.add(plan(tenfold(), 5, OnLimit::Rollback)), Ok(3));
    assert!(canary.observe(3, 3, 0, trials(0, 31)).is_ok());
    assert!(canary.observe(3, 3, 1, pairs(0, 8, 0)).is_ok());
    assert!(canary.observe(3, 3, 2, trials(0, 5)).is_ok());
    assert_eq!(canary.recommend(Guardrail::Clear), sampling);
    assert!(canary.observe(3, 4, 3, trials(0, 4)).is_ok());
    assert_eq!(canary.recommend(Guardrail::Clear), sampling);
    let held = canary;
    assert!(canary.observe(3, 5, 3, trials(0, 1)).is_ok());
    assert_eq!(
        canary.recommend(Guardrail::NoData),
        advice(Action::Rollback, Reason::Limit, Some(3))
    );
    let mut canary = held;
    assert!(canary.observe(3, 5, 3, trials(0, 30)).is_ok());
    assert_eq!(
        canary.status(3).map(|status| status.look.outcome),
        Some(Outcome::Cleared)
    );
    assert_eq!(
        canary.recommend(Guardrail::NoData),
        advice(Action::Hold, Reason::Limit, Some(2))
    );
    let mut canary = held;
    assert!(canary.observe(3, 5, 3, trials(2, 2)).is_ok());
    assert_eq!(
        canary.recommend(Guardrail::Clear),
        advice(Action::Rollback, Reason::Evidence, Some(3))
    );
}

#[test]
fn budget_verdicts_convert_to_guardrails() {
    assert_eq!(Guardrail::from(State::Firing), Guardrail::Breached);
    assert_eq!(Guardrail::from(State::Quiet), Guardrail::Clear);
    assert_eq!(Guardrail::from(State::NoData), Guardrail::NoData);
    assert_eq!(
        Guardrail::from(Standing::Exhausted { overspent: 1 }),
        Guardrail::Breached
    );
    assert_eq!(
        Guardrail::from(Standing::Within { remaining: 0 }),
        Guardrail::Clear
    );
    assert_eq!(Guardrail::from(Standing::NoData), Guardrail::NoData);
}
