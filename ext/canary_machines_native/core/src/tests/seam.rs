//! The `sliderule-machines` seam: what a plan exposes is the shared kernel's
//! own interval, and every micro-unit reading is `micro_nats` of it.

use sliderule_machines::ln_ratio;

use super::{doubled, model::Rng, paired, plan, reference, tenfold, trials};
use crate::{FRACTION_BITS, Interval, Model, OnLimit, Plan, Sprt, micro_nats};

fn kernel(nums: [u64; 2], dens: [u64; 2]) -> Interval {
    ln_ratio::<FRACTION_BITS>(&nums, &dens)
}

fn plan_of(model: Model) -> Plan {
    plan(model, 1000, OnLimit::Hold)
}

#[allow(clippy::cast_precision_loss)]
fn real(value: i128) -> f64 {
    value as f64 / (1u64 << FRACTION_BITS) as f64
}

#[test]
fn the_crates_logarithm_is_the_shared_kernel_at_48_fraction_bits() {
    let mut rng = Rng(0x5EED);
    for _ in 0..2_000 {
        let (a, b, c, d) = (rng.next() | 1, rng.below(1000) + 1, rng.next() | 1, 7);
        let interval = crate::ln::ln_ratio([a, b], [c, d]);
        assert_eq!(interval, kernel([a, b], [c, d]));
        assert!(interval.lo < interval.hi && interval.hi - interval.lo <= 2);
    }
    // ln 2 = sum 1 / (k 2^k), in 120 fraction bits with every term rounded down.
    let sum: u128 = (1..=120u32)
        .map(|k| (1u128 << (120 - k)) / u128::from(k))
        .sum();
    let interval = kernel([2, 1], [1, 1]);
    let exact = i128::try_from(sum >> (120 - FRACTION_BITS)).expect("fits");
    assert!(interval.lo <= exact && exact < interval.hi);
    assert!((real(interval.lo) - core::f64::consts::LN_2).abs() < 1e-12);
    assert!(kernel([u64::MAX, u64::MAX], [1, 1]).hi < 89 << FRACTION_BITS);
}

#[test]
fn plan_and_look_readings_are_direct_ln_ratio_calls() {
    // alpha = beta = 5/100, p0 = 1/100, p1 = 1/10.
    let plan = plan_of(tenfold());
    assert_eq!(plan.upper(), kernel([95, 100], [100, 5]));
    assert_eq!(plan.lower(), kernel([5, 100], [100, 95]));
    assert_eq!(plan.hit(), kernel([1, 100], [10, 1]));
    assert_eq!(plan.miss(), kernel([9, 100], [10, 99]));

    // Odds ratios 1/1 and 2/1 are hit probabilities 1/2 and 2/3.
    let paired_plan = plan_of(doubled());
    assert_eq!(paired_plan.hit(), kernel([2, 2], [3, 1]));
    assert_eq!(paired_plan.miss(), kernel([1, 2], [3, 1]));
    assert_eq!(paired_plan.upper(), kernel([95, 100], [100, 5]));

    let models = [
        tenfold(),
        reference((1, 1000), (5, 1000)),
        doubled(),
        paired((3, 2), (7, 2)),
    ];
    for model in models {
        let plan = plan_of(model);
        assert_eq!(plan.upper_micro(), micro_nats(plan.upper().hi));
        assert_eq!(plan.lower_micro(), micro_nats(plan.lower().lo));
        assert!(plan.lower().hi < 0 && 0 < plan.upper().lo);
        assert!(plan.miss().hi < 0 && 0 < plan.hit().lo);
    }

    // A look is the conversion of the weighted intervals' midpoint.
    let look = Sprt::new(plan).observe(trials(1, 12)).expect("accepted");
    let lo = plan.hit().lo + 11 * plan.miss().lo;
    let hi = plan.hit().hi + 11 * plan.miss().hi;
    assert_eq!(look.llr_micro(), micro_nats(i128::midpoint(lo, hi)));
}

#[test]
fn micro_nats_is_an_exact_floor_on_both_sides_of_zero() {
    let one = 1_i128 << FRACTION_BITS;
    // One micro-nat is 2^48 / 10^6 = 281_474_976.71 units: the first value at
    // or above it is 281_474_977, which a pre-shifted product would call zero.
    let cases = [
        (281_474_976, 0),
        (281_474_977, 1),
        (-1, -1),
        (-281_474_976, -1),
        (-281_474_977, -2),
        (0, 0),
        (one, 1_000_000),
        (-one, -1_000_000),
        (one - 1, 999_999),
        (i128::MAX >> 5, i64::MAX),
        (-(i128::MAX >> 5), i64::MIN),
    ];
    for (value, micro) in cases {
        assert_eq!(micro_nats(value), micro, "{value}");
    }
}
