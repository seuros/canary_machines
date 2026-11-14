extern crate std;

use budget_machines::Objective;

use super::*;

mod canary;
mod model;
mod random;
mod seam;
mod values;

fn rate(num: u32, den: u32) -> Rate {
    Rate::new(num, den).expect("valid rate")
}

fn odds(num: u32, den: u32) -> OddsRatio {
    OddsRatio::new(num, den).expect("valid odds ratio")
}

fn fraction(num: u64, den: u64) -> Fraction {
    Fraction::new(num, den).expect("valid fraction")
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
    Plan::new(model, rate(5, 100), rate(5, 100), limit, on_limit).expect("valid plan")
}

fn trials(bad: u64, total: u64) -> Observation {
    Observation::Trials(Counts::new(total - bad, total).expect("valid counts"))
}

fn pairs(canary_only: u64, baseline_only: u64, concordant: u64) -> Observation {
    Observation::Pairs(Pairs::new(canary_only, baseline_only, concordant).expect("valid pairs"))
}

/// One in a hundred against one in ten.
fn tenfold() -> Model {
    reference((1, 100), (1, 10))
}

/// Odds ratio one against two.
fn doubled() -> Model {
    paired((1, 1), (2, 1))
}

/// `sprt` observes `observation`; answers the outcome.
fn outcome_after(sprt: &mut Sprt, observation: Observation) -> Result<Outcome, Refusal> {
    sprt.observe(observation).map(|look| look.outcome())
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

    let slo = Objective::new(999, 1000).expect("valid objective");
    assert_eq!(Rate::failure_of(slo), Some(rate(1, 1000)));
    let anything = Objective::new(0, 5).expect("valid objective");
    assert_eq!(Rate::failure_of(anything), None);

    let level = (5, 100);
    let cases = [
        (
            reference((1, 10), (1, 10)),
            level,
            level,
            9,
            Err(Invalid::Hypotheses),
        ),
        (
            reference((2, 10), (1, 10)),
            level,
            level,
            9,
            Err(Invalid::Hypotheses),
        ),
        (
            paired((2, 1), (2, 1)),
            level,
            level,
            9,
            Err(Invalid::Hypotheses),
        ),
        (
            paired((3, 1), (2, 1)),
            level,
            level,
            9,
            Err(Invalid::Hypotheses),
        ),
        (tenfold(), (1, 2), (1, 2), 9, Err(Invalid::ErrorLevels)),
        (tenfold(), (2, 3), (1, 2), 9, Err(Invalid::ErrorLevels)),
        (tenfold(), (1, 2), (499, 1000), 9, Ok(())),
        (tenfold(), level, level, 0, Err(Invalid::Limit)),
        (
            reference((1, u32::MAX), (1, u32::MAX - 1)),
            level,
            level,
            9,
            Err(Invalid::Indistinguishable),
        ),
        (
            reference((1, u32::MAX), (u32::MAX - 1, u32::MAX)),
            level,
            level,
            u64::MAX,
            Ok(()),
        ),
        (
            paired((1, u32::MAX), (u32::MAX, 1)),
            level,
            level,
            9,
            Ok(()),
        ),
    ];
    for (model, alpha, beta, limit, expected) in cases {
        let built = Plan::new(
            model,
            rate(alpha.0, alpha.1),
            rate(beta.0, beta.1),
            limit,
            OnLimit::Hold,
        );
        assert_eq!(built.map(|_| ()), expected, "{model:?} {alpha:?} {beta:?}");
    }
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

    let wald = Some(fraction(500, 9500));
    let lopsided =
        Plan::new(tenfold(), rate(1, 100), rate(1, 5), 9, OnLimit::Hold).expect("valid plan");
    let edge =
        Plan::new(tenfold(), rate(49, 100), rate(50, 100), 9, OnLimit::Hold).expect("valid plan");
    let cases = [
        (hold, wald, wald),
        (plan(tenfold(), 1000, OnLimit::Rollback), None, wald),
        (plan(tenfold(), 1000, OnLimit::Promote), wald, None),
        (lopsided, Some(fraction(5, 400)), Some(fraction(100, 495))),
        (edge, Some(fraction(4900, 5000)), Some(fraction(5000, 5100))),
    ];
    for (test, false_rollback, false_promote) in cases {
        let bounds = test.bounds();
        assert_eq!(
            (bounds.false_rollback(), bounds.false_promote()),
            (false_rollback, false_promote)
        );
        // Every bound a plan gives is accepted back.
        assert_eq!(Bounds::new(false_rollback, false_promote), Some(bounds));
    }
}

#[test]
fn reference_test_crosses_each_threshold_at_the_textbook_sample() {
    // A failure weighs ln 10 = 2.302585; a success ln(0.9 / 0.99) = -0.095310.
    let mut sprt = Sprt::new(plan(tenfold(), 1000, OnLimit::Hold));
    assert_eq!(sprt.look().llr_micro(), 0);
    let first = sprt.observe(trials(1, 1));
    assert_eq!(
        first,
        Ok(Look::new(Outcome::Undecided, 1, 0, 1, 2_302_585).expect("valid look"))
    );
    let second = sprt.observe(trials(1, 1)).expect("accepted");
    assert_eq!(
        (second.outcome(), second.llr_micro()),
        (Outcome::Regressed, 4_605_170)
    );
    let before = sprt;
    assert_eq!(sprt.observe(trials(0, 1)), Err(Refusal::Concluded));
    assert_eq!(sprt, before);

    // 2.944439 / 0.095310 = 30.9: thirty clean trials are not enough.
    sprt.reset();
    assert_eq!(sprt, Sprt::new(*sprt.plan()));
    let thirty = sprt.observe(trials(0, 30)).expect("accepted");
    assert_eq!(
        (thirty.outcome(), thirty.llr_micro()),
        (Outcome::Undecided, -2_859_306)
    );
    assert_eq!(
        outcome_after(&mut sprt, trials(0, 0)),
        Ok(Outcome::Undecided)
    );
    assert_eq!(outcome_after(&mut sprt, trials(0, 1)), Ok(Outcome::Cleared));
    assert_eq!(sprt.outcome(), Outcome::Cleared);
}

#[test]
fn paired_test_ignores_concordant_pairs_as_evidence() {
    // Odds ratio 1 against 2: a discordant pair is the canary's fault with
    // probability 1/2 against 2/3. ln(4/3) = 0.287682, ln(2/3) = -0.405465.
    let fresh = Sprt::new(plan(doubled(), 10_000, OnLimit::Hold));
    let mut sprt = fresh;
    let look = sprt.observe(pairs(1, 1, 500)).expect("accepted");
    assert_eq!((look.hits(), look.misses(), look.samples()), (1, 1, 502));
    assert_eq!(look.llr_micro(), -117_784);
    // 2.944439 / 0.287682 = 10.2 more canary-only failures than that cancels.
    assert_eq!(
        outcome_after(&mut sprt, pairs(10, 0, 0)),
        Ok(Outcome::Undecided)
    );
    assert_eq!(
        outcome_after(&mut sprt, pairs(1, 0, 0)),
        Ok(Outcome::Regressed)
    );

    let mut sprt = fresh;
    assert_eq!(
        outcome_after(&mut sprt, pairs(0, 7, 0)),
        Ok(Outcome::Undecided)
    );
    assert_eq!(
        outcome_after(&mut sprt, pairs(0, 1, 0)),
        Ok(Outcome::Cleared)
    );

    for (mut test, wrong) in [
        (fresh, trials(0, 1)),
        (
            Sprt::new(plan(tenfold(), 10, OnLimit::Hold)),
            pairs(1, 0, 0),
        ),
    ] {
        let before = test;
        assert_eq!(test.observe(wrong), Err(Refusal::WrongModel));
        assert_eq!(test, before);
    }
}

#[test]
fn truncation_at_the_limit_and_u64_extremes() {
    // A batch may carry the count past the limit; the look happens at its end.
    let mut sprt = Sprt::new(plan(doubled(), 100, OnLimit::Hold));
    assert_eq!(
        outcome_after(&mut sprt, pairs(1, 1, 97)),
        Ok(Outcome::Undecided)
    );
    assert_eq!(
        outcome_after(&mut sprt, pairs(0, 0, 50)),
        Ok(Outcome::Truncated)
    );
    assert_eq!(sprt.observe(pairs(0, 9, 0)), Err(Refusal::Concluded));
    // A threshold reached on the limit's own look still counts.
    let mut sprt = Sprt::new(plan(tenfold(), 2, OnLimit::Hold));
    assert_eq!(
        outcome_after(&mut sprt, trials(2, 2)),
        Ok(Outcome::Regressed)
    );

    let mut sprt = Sprt::new(plan(doubled(), u64::MAX, OnLimit::Hold));
    let quiet = sprt.observe(pairs(0, 0, u64::MAX - 1)).expect("accepted");
    assert_eq!(
        (quiet.outcome(), quiet.llr_micro()),
        (Outcome::Undecided, 0)
    );
    let before = sprt;
    assert_eq!(sprt.observe(pairs(1, 1, 0)), Err(Refusal::Overflow));
    assert_eq!(sprt, before);
    assert_eq!(
        outcome_after(&mut sprt, pairs(0, 0, 1)),
        Ok(Outcome::Truncated)
    );

    for (bad, extreme) in [
        (0, (Outcome::Cleared, i64::MIN)),
        (u64::MAX, (Outcome::Regressed, i64::MAX)),
    ] {
        let mut sprt = Sprt::new(plan(tenfold(), u64::MAX, OnLimit::Hold));
        let look = sprt.observe(trials(bad, u64::MAX)).expect("accepted");
        assert_eq!((look.outcome(), look.llr_micro()), extreme);
    }
}
