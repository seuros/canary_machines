# frozen_string_literal: true

require_relative 'canary_machines/version'

# Sequential canary tests. Every value mirrors the Rust core's type of the
# same name, field for field; each checks itself natively when it is built.
module CanaryMachines
  # A probability strictly between zero and one, as an exact ratio:
  # `0 < num < den`, both in 0..2**32-1. `Rate.failure_of(objective)` is native.
  Rate = Data.define(:num, :den) do
    def initialize(num:, den:)
      super
      __validate
    end
  end

  # A positive odds ratio `num / den` of canary failure against baseline
  # failure, both parts in 1..2**32-1.
  OddsRatio = Data.define(:num, :den) do
    def initialize(num:, den:)
      super
      __validate
    end
  end

  # Exact good and total event counts, `good <= total`. The core re-exports
  # this type from budget-machines; a BudgetMachines::Counts is accepted
  # wherever this one is.
  Counts = Data.define(:good, :total) do
    def initialize(good:, total:)
      super
      __validate
    end
  end

  # Outcomes of matched baseline/canary pairs. The three counts sum within 64 bits.
  Pairs = Data.define(:canary_only, :baseline_only, :concordant) do
    def initialize(canary_only:, baseline_only:, concordant:)
      super
      __validate
    end

    def total = canary_only + baseline_only + concordant
  end

  # The sampling model a test assumes, with its two simple hypotheses.
  module Model
    # Canary trials judged against two Rates fixed in advance.
    Reference = Data.define(:acceptable, :unacceptable) do
      def initialize(acceptable:, unacceptable:)
        super
        __validate
      end
    end

    # Matched pairs judged against two OddsRatios.
    Paired = Data.define(:acceptable, :unacceptable) do
      def initialize(acceptable:, unacceptable:)
        super
        __validate
      end
    end
  end

  # An enclosing interval around a logarithm, in units of 2**-FRACTION_BITS nat.
  Interval = Data.define(:lo, :hi)

  # An exact non-negative fraction; `den` is never zero.
  Fraction = Data.define(:num, :den)

  # Error probabilities a plan guarantees, as Fractions. nil where the limit
  # policy forces that decision at truncation and nothing is claimed.
  Bounds = Data.define(:false_rollback, :false_promote)

  # A fully specified test: the arguments of the core's `Plan::new`, in order.
  # `on_limit` is :hold, :rollback or :promote. `upper`, `lower`, `hit`,
  # `miss`, `upper_micro`, `lower_micro` and `bounds` are native.
  Plan = Data.define(:model, :alpha, :beta, :limit, :on_limit) do
    def initialize(model:, alpha:, beta:, limit:, on_limit:)
      super
      __validate
    end
  end

  # A test's evidence after a look. `outcome` is :undecided, :cleared,
  # :regressed or :truncated; `llr_micro` is the log-likelihood ratio in
  # millionths of a nat.
  Look = Data.define(:outcome, :hits, :misses, :samples, :llr_micro) do
    def decided? = outcome != :undecided
  end

  # One test's evidence and the instant it was decided. `decided_at` is the
  # instant the outcome latched, in Integer nanoseconds as passed to
  # `observe_trials`/`observe_pairs`; nil while the test is undecided.
  Status = Data.define(:look, :decided_at) do
    def decided? = look.decided?
  end

  # Advice for the rollout. `action` is :continue, :promote, :rollback or :hold;
  # `reason` is :evidence, :limit, :guardrail, :guardrail_no_data, :sampling or
  # :no_tests; `test` is the index of the deciding test, when one decided.
  Recommendation = Data.define(:action, :reason, :test) do
    def promote? = action == :promote
    def rollback? = action == :rollback
    def continue? = action == :continue
    def hold? = action == :hold
  end
end

# Native is required: no algorithm fallback. It adds the checks and the
# derived readings to the values above, so they must exist first.
require 'canary_machines_native/canary_machines_native'
