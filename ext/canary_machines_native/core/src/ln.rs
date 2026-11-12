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

/// Millionths of a nat for a `2^-FRACTION_BITS` value, rounded down and
/// saturating at the limits of `i64`.
pub fn micro(value: i128) -> i64 {
    // Split the shift so the product cannot overflow: |value| < 2^122.
    let scaled = ((value >> 20) * 1_000_000) >> (FRACTION_BITS - 20);
    i64::try_from(scaled).unwrap_or(if scaled < 0 { i64::MIN } else { i64::MAX })
}
