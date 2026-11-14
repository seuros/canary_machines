//! Fixed-point log-likelihoods on top of the shared integer logarithm.
//!
//! Every value is a fixed-point number, so two machines never disagree about
//! a log-likelihood. The logarithm itself is `sliderule-machines`' `ln_ratio`:
//! computed with 58 fraction bits and handed out as an enclosing interval
//! with [`FRACTION_BITS`] fraction bits.

pub use sliderule_machines::Interval;

/// Fraction bits of every log-likelihood weight and threshold.
pub const FRACTION_BITS: u32 = 48;

/// Interval around `ln(a * b / (c * d))` for `nums = [a, b]`, `dens = [c, d]`,
/// in units of `2^-FRACTION_BITS`.
///
/// The interval is one or two units wide and always contains the exact
/// logarithm: the working value is within `2^-54` of it, and the conversion
/// rounds outwards.
pub const fn ln_ratio(nums: [u64; 2], dens: [u64; 2]) -> Interval {
    sliderule_machines::ln_ratio::<FRACTION_BITS>(&nums, &dens)
}

/// Micro-nats in one nat: the unit of every `*_micro` reading.
const MICRO_PER_NAT: i128 = 1_000_000;

/// Millionths of a nat for a value in units of `2^-FRACTION_BITS` nat, rounded
/// down (towards negative infinity) and saturating at the limits of `i64`.
///
/// This is the one conversion behind every `*_micro` reading of this crate:
/// [`Plan::upper_micro`](crate::Plan::upper_micro) is `micro_nats` of the `hi`
/// end of [`Plan::upper`](crate::Plan::upper), and so on. Apply it to either end
/// of a `sliderule_machines::ln_ratio::<FRACTION_BITS>` interval to compare
/// that logarithm with them.
#[must_use]
pub fn micro_nats(value: i128) -> i64 {
    // Exact floor: scale the whole nats and the fraction apart, so neither
    // product overflows (|value| < 2^122) and no low bit is dropped first.
    let whole = value >> FRACTION_BITS;
    let fraction = value & ((1 << FRACTION_BITS) - 1);
    let scaled = whole * MICRO_PER_NAT + ((fraction * MICRO_PER_NAT) >> FRACTION_BITS);
    i64::try_from(scaled).unwrap_or(if scaled < 0 { i64::MIN } else { i64::MAX })
}
