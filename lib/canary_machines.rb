# frozen_string_literal: true

require_relative 'canary_machines/version'

module CanaryMachines
  # One test's evidence. `outcome` is :undecided, :cleared, :regressed or
  # :truncated; `llr_micro` is the log-likelihood ratio in millionths of a nat;
  # `decided_at` is nil while the test is undecided.
  Status = Data.define(:outcome, :hits, :misses, :samples, :llr_micro, :decided_at) do
    def decided? = outcome != :undecided
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

  # Error probabilities a test guarantees, as Rationals. nil where the test's
  # limit policy forces that decision at truncation and nothing is claimed.
  Bounds = Data.define(:false_rollback, :false_promote)
end

# Native is required: no algorithm fallback.
require 'canary_machines_native/canary_machines_native'
