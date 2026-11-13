//! The `sliderule-machines` seam: what a plan exposes is the shared kernel's
//! own interval, and every micro-unit reading is `micro_nats` of it.

use sliderule_machines::ln_ratio;

use super::{paired, plan, reference, tenfold, trials};
use crate::{FRACTION_BITS, Interval, OnLimit, Plan, Refusal, Sprt, micro_nats};

fn kernel(nums: [u64; 2], dens: [u64; 2]) -> Interval {
    ln_ratio::<FRACTION_BITS>(&nums, &dens)
}

#[test]
fn plan_intervals_are_direct_ln_ratio_calls() {
    // alpha = beta = 5/100, p0 = 1/100, p1 = 1/10.
    let plan = plan(tenfold(), 1000, OnLimit::Hold);
    assert_eq!(plan.upper(), kernel([95, 100], [100, 5]));
    assert_eq!(plan.lower(), kernel([5, 100], [100, 95]));
    assert_eq!(plan.hit(), kernel([1, 100], [10, 1]));
    assert_eq!(plan.miss(), kernel([9, 100], [10, 99]));

    // Odds ratios 1/1 and 2/1 are hit probabilities 1/2 and 2/3.
    let plan = plan_of(paired((1, 1), (2, 1)));
    assert_eq!(plan.hit(), kernel([2, 2], [3, 1]));
    assert_eq!(plan.miss(), kernel([1, 2], [3, 1]));
    assert_eq!(plan.upper(), kernel([95, 100], [100, 5]));
}

fn plan_of(model: crate::Model) -> Plan {
    plan(model, 1000, OnLimit::Hold)
}

#[test]
fn micro_accessors_are_the_public_conversion_of_the_intervals() {
    let models = [
        tenfold(),
        reference((1, 1000), (5, 1000)),
        paired((1, 1), (2, 1)),
        paired((3, 2), (7, 2)),
    ];
    for model in models {
        let plan = plan_of(model);
        assert_eq!(plan.upper_micro(), micro_nats(plan.upper().hi));
        assert_eq!(plan.lower_micro(), micro_nats(plan.lower().lo));
        assert!(plan.lower().hi < 0 && 0 < plan.upper().lo);
        assert!(plan.miss().hi < 0 && 0 < plan.hit().lo);
    }
    let plan = plan_of(tenfold());
    assert_eq!(
        plan.upper_micro(),
        micro_nats(kernel([95, 100], [100, 5]).hi)
    );
    assert_eq!(
        plan.lower_micro(),
        micro_nats(kernel([5, 100], [100, 95]).lo)
    );
}

#[test]
fn a_look_is_the_conversion_of_the_weighted_intervals() {
    let plan = plan_of(tenfold());
    let mut sprt = Sprt::new(plan);
    let look = sprt
        .observe(trials(1, 12))
        .unwrap_or_else(|refusal| panic!("accepted: {refusal:?}"));
    let lo = plan.hit().lo + 11 * plan.miss().lo;
    let hi = plan.hit().hi + 11 * plan.miss().hi;
    assert_eq!(look.llr_micro, micro_nats(i128::midpoint(lo, hi)));
}

#[test]
#[allow(deprecated)]
fn the_old_refusal_name_is_the_same_type() {
    let old: crate::Refused = Refusal::Full;
    assert_eq!(old, Refusal::Full);
}

#[test]
fn micro_nats_is_an_exact_floor_on_both_sides_of_zero() {
    let one = 1_i128 << FRACTION_BITS;
    // One micro-nat is 2^48 / 10^6 = 281_474_976.71 units: the first value at
    // or above it is 281_474_977, which a pre-shifted product would call zero.
    assert_eq!(micro_nats(281_474_976), 0);
    assert_eq!(micro_nats(281_474_977), 1);
    assert_eq!(micro_nats(-1), -1);
    assert_eq!(micro_nats(-281_474_976), -1);
    assert_eq!(micro_nats(-281_474_977), -2);
    assert_eq!(micro_nats(0), 0);
    assert_eq!(micro_nats(one), 1_000_000);
    assert_eq!(micro_nats(-one), -1_000_000);
    assert_eq!(micro_nats(one - 1), 999_999);
    assert_eq!(micro_nats(i128::MAX >> 5), i64::MAX);
    assert_eq!(micro_nats(-(i128::MAX >> 5)), i64::MIN);
}
