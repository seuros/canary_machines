# frozen_string_literal: true

require_relative 'canary_machines/version'

# Sequential canary tests. Every value mirrors the Rust core's type of the
# same name, field for field; each checks itself natively when it is built.
module CanaryMachines
  # Every value checks itself natively once Data has set its fields; the
  # check is the core's constructor (`__validate`, private, native).
  module Validated
    def initialize(**)
      super
      __validate
    end
  end
  private_constant :Validated

  # A probability strictly between zero and one, as an exact ratio:
  # `0 < num < den`, both in 0..2**32-1. `Rate.failure_of(objective)` is native.
  Rate = Data.define(:num, :den) do
    include Validated
  end

  # A positive odds ratio `num / den` of canary failure against baseline
  # failure, both parts in 1..2**32-1.
  OddsRatio = Data.define(:num, :den) do
    include Validated
  end

  # Exact good and total event counts, `good <= total`. The core re-exports
  # this type from budget-machines; a BudgetMachines::Counts is accepted
  # wherever this one is. `bad` and `checked_add` are native; `checked_add`
  # answers this gem's Counts whatever counts it was given.
  Counts = Data.define(:good, :total) do
    include Validated
  end

  # Outcomes of matched baseline/canary pairs. The three counts sum within 64 bits.
  Pairs = Data.define(:canary_only, :baseline_only, :concordant) do
    include Validated

    def total = canary_only + baseline_only + concordant
  end

  # The sampling model a test assumes, with its two simple hypotheses.
  module Model
    # Canary trials judged against two Rates fixed in advance.
    Reference = Data.define(:acceptable, :unacceptable) do
      include Validated
    end

    # Matched pairs judged against two OddsRatios.
    Paired = Data.define(:acceptable, :unacceptable) do
      include Validated
    end
  end

  # An enclosing interval around a logarithm, in units of 2**-FRACTION_BITS
  # nat: Integers within 128 bits with `lo <= hi`.
  Interval = Data.define(:lo, :hi) do
    include Validated
  end

  # An exact non-negative fraction; `den` is never zero.
  Fraction = Data.define(:num, :den) do
    include Validated
  end

  # Error probabilities a plan guarantees, as Fractions strictly between zero
  # and one. nil where the limit policy forces that decision at truncation
  # and nothing is claimed; never both.
  Bounds = Data.define(:false_rollback, :false_promote) do
    include Validated
  end

  # A fully specified test: the arguments of the core's `Plan::new`, in order.
  # `on_limit` is :hold, :rollback or :promote. `upper`, `lower`, `hit`,
  # `miss`, `upper_micro`, `lower_micro` and `bounds` are native.
  Plan = Data.define(:model, :alpha, :beta, :limit, :on_limit) do
    include Validated
  end

  # A test's evidence after a look. `outcome` is :undecided, :cleared,
  # :regressed or :truncated; `llr_micro` is the log-likelihood ratio in
  # millionths of a nat. `Look::new` refuses what no test produces (see the
  # README). `decided?` is the core's `Look::is_decided`.
  Look = Data.define(:outcome, :hits, :misses, :samples, :llr_micro) do
    include Validated
  end

  # One test's evidence and the instant it was decided. `decided_at` is the
  # instant the outcome latched, in Integer nanoseconds as passed to
  # `observe_trials`/`observe_pairs`; nil exactly while the test is
  # undecided. `decided?` is the core's `Status::is_decided`.
  Status = Data.define(:look, :decided_at) do
    include Validated
  end

  # Advice for the rollout. `action` is :continue, :promote, :rollback or :hold;
  # `reason` is :evidence, :limit, :guardrail, :guardrail_no_data, :sampling or
  # :no_tests; `test` is the index of the deciding test, when one decided.
  # `Recommendation::new` refuses a combination `recommend` never gives.
  Recommendation = Data.define(:action, :reason, :test) do
    include Validated
  end
end

# Native is required: no algorithm fallback. It adds the checks and the
# derived readings to the values above, so they must exist first.
require 'canary_machines_native/canary_machines_native'

module CanaryMachines
  # The values are final. `initialize` is where each validates, and a
  # subclass could override it, or `allocate` build an instance that never
  # ran it: subclassing raises TypeError and `allocate` is gone. `new`,
  # keywords and `with` all still run the check.
  [Rate, OddsRatio, Counts, Pairs, Model::Reference, Model::Paired, Interval, Fraction, Bounds, Plan, Look, Status,
   Recommendation].each do |value|
    value.singleton_class.undef_method(:allocate)
    value.define_singleton_method(:inherited) { |_| raise TypeError, "#{value} cannot be subclassed" }
  end
end
