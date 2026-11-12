# frozen_string_literal: true
require 'minitest/autorun'
require 'canary_machines'

class CanaryTest < Minitest::Test
  LEVELS = { alpha: 5/100r, beta: 5/100r }.freeze

  def canary(generation = 1) = CanaryMachines::Canary.new(generation)

  # One failure in a hundred against one in ten.
  def errors(canary, limit: 1000, **options)
    canary.add_reference(1/100r, 1/10r, **LEVELS, limit:, **options)
  end

  # Odds ratio one against two.
  def matched(canary, limit: 1000, **options)
    canary.add_paired(1r, 2r, **LEVELS, limit:, **options)
  end

  def test_starts_empty
    c = canary(7)
    assert_equal [7, nil, 0, 8], [c.generation, c.last, c.size, c.capacity]
    assert_equal 8, CanaryMachines::CAPACITY
    assert_equal CanaryMachines::Recommendation.new(action: :hold, reason: :no_tests, test: nil), c.recommend
    assert_nil c.status(0)
    assert_nil c.bounds(0)
    assert_nil c.thresholds(0)
  end

  def test_reference_test_reaches_each_threshold
    c = canary
    assert_equal 0, errors(c)
    assert_equal [-2_944_439, 2_944_438], c.thresholds(0)

    first = c.observe_trials(1, 10, 0, good: 0, total: 1)
    assert_equal CanaryMachines::Status.new(
      outcome: :undecided, hits: 1, misses: 0, samples: 1, llr_micro: 2_302_585, decided_at: nil
    ), first
    refute_predicate first, :decided?
    assert_predicate first, :frozen?
    assert_predicate c.recommend, :continue?
    assert_equal :sampling, c.recommend.reason

    second = c.observe_trials(1, 11, 0, good: 0, total: 1)
    assert_equal [:regressed, 11], [second.outcome, second.decided_at]
    assert_predicate second, :decided?
    assert_equal second, c.status(0)
    assert_equal CanaryMachines::Recommendation.new(action: :rollback, reason: :evidence, test: 0), c.recommend
    assert_predicate c.recommend, :rollback?
    assert_equal :concluded, c.observe_trials(1, 12, 0, good: 1, total: 1)
    assert_equal 11, c.last

    assert_equal :ok, c.restart(2)
    assert_equal :undecided, c.observe_trials(2, 12, 0, good: 30, total: 30).outcome
    assert_equal :cleared, c.observe_trials(2, 13, 0, good: 1, total: 1).outcome
    assert_equal CanaryMachines::Recommendation.new(action: :promote, reason: :evidence, test: nil), c.recommend
    assert_predicate c.recommend, :promote?
  end

  def test_paired_test_discards_concordant_pairs
    c = canary
    assert_equal 0, matched(c)
    seen = c.observe_pairs(1, 0, 0, canary_only: 1, baseline_only: 1, concordant: 500)
    assert_equal [1, 1, 502, -117_784], [seen.hits, seen.misses, seen.samples, seen.llr_micro]
    assert_equal :undecided, c.observe_pairs(1, 1, 0, canary_only: 10, baseline_only: 0, concordant: 0).outcome
    assert_equal :regressed, c.observe_pairs(1, 2, 0, canary_only: 1, baseline_only: 0, concordant: 0).outcome
  end

  def test_refusals_are_symbols
    c = canary(5)
    errors(c)
    matched(c)
    assert_equal :stale_generation, c.observe_trials(4, 0, 0, good: 1, total: 1)
    assert_equal :unknown_test, c.observe_trials(5, 0, 2, good: 1, total: 1)
    assert_equal :unknown_test, c.observe_trials(5, 0, 2**64 - 1, good: 1, total: 1)
    assert_equal :wrong_model, c.observe_trials(5, 0, 1, good: 1, total: 1)
    assert_equal :wrong_model, c.observe_pairs(5, 0, 0, canary_only: 0, baseline_only: 0, concordant: 1)
    assert_nil c.last
    assert_kind_of CanaryMachines::Status, c.observe_trials(5, 9, 0, good: 1, total: 1)
    assert_equal :time_regression, c.observe_trials(5, 8, 0, good: 1, total: 1)
    assert_equal 1, c.status(0).samples
    assert_equal :stale_generation, c.restart(5)
    assert_equal :stale_generation, c.restart(4)
    assert_equal 5, c.generation

    huge = 2**64 - 1
    wide = canary
    matched(wide, limit: huge)
    assert_equal :undecided, wide.observe_pairs(1, 0, 0, canary_only: 0, baseline_only: 0, concordant: huge - 1).outcome
    assert_equal :overflow, wide.observe_pairs(1, 0, 0, canary_only: 1, baseline_only: 1, concordant: 0)
    assert_equal :truncated, wide.observe_pairs(1, huge, 0, canary_only: 0, baseline_only: 0, concordant: 1).outcome
  end

  def test_capacity_is_fixed
    c = canary
    assert_equal (0...8).to_a, Array.new(8) { errors(c) }
    assert_equal :full, errors(c)
    assert_equal :full, matched(c)
    assert_equal 8, c.size
  end

  def test_limit_policies_and_bounds
    wald = 1/19r
    { hold: [:hold, wald, wald], rollback: [:rollback, nil, wald], promote: [:promote, wald, nil] }
      .each do |policy, (action, false_rollback, false_promote)|
      c = canary
      errors(c, limit: 5, on_limit: policy)
      assert_equal CanaryMachines::Bounds.new(false_rollback:, false_promote:), c.bounds(0)
      assert_equal :truncated, c.observe_trials(1, 0, 0, good: 5, total: 5).outcome
      assert_equal CanaryMachines::Recommendation.new(action:, reason: :limit, test: 0), c.recommend
    end
    c = canary
    errors(c, limit: 5)
    assert_predicate c.recommend, :continue?
    c.observe_trials(1, 0, 0, good: 5, total: 5)
    assert_predicate c.recommend, :hold?
    assert_equal CanaryMachines::Bounds.new(false_rollback: wald, false_promote: wald), c.bounds(0)
  end

  def test_guardrail_overrides_and_withholds
    c = canary
    errors(c)
    assert_equal %i[rollback guardrail], c.recommend(:breached).to_h.values_at(:action, :reason)
    assert_predicate c.recommend(:no_data), :continue?
    c.observe_trials(1, 0, 0, good: 31, total: 31)
    assert_predicate c.recommend(:clear), :promote?
    assert_equal %i[hold guardrail_no_data], c.recommend(:no_data).to_h.values_at(:action, :reason)
    assert_predicate c.recommend(:breached), :rollback?
  end

  def test_bad_input_raises
    c = canary
    assert_raises(TypeError) { CanaryMachines::Canary.new(1.0) }
    assert_raises(TypeError) { CanaryMachines::Canary.new(nil) }
    assert_raises(RangeError) { CanaryMachines::Canary.new(-1) }
    assert_raises(RangeError) { CanaryMachines::Canary.new(2**64) }
    assert_raises(TypeError) { CanaryMachines::Canary.allocate }
    assert_raises(TypeError) { c.add_reference(0.01, 1/10r, **LEVELS, limit: 10) }
    assert_raises(TypeError) { c.add_paired(1, 2r, **LEVELS, limit: 10) }
    assert_raises(TypeError) { errors(c, on_limit: 'hold') }
    assert_raises(ArgumentError) { errors(c, on_limit: :panic) }
    assert_raises(ArgumentError) { c.add_reference(1/100r, 1/10r, limit: 10) }
    assert_raises(RangeError) { c.add_reference(0r, 1/10r, **LEVELS, limit: 10) }
    assert_raises(RangeError) { c.add_reference(1/100r, 1r, **LEVELS, limit: 10) }
    assert_raises(RangeError) { c.add_reference(-1/100r, 1/10r, **LEVELS, limit: 10) }
    assert_raises(RangeError) { c.add_reference(Rational(1, 2**32), 1/10r, **LEVELS, limit: 10) }
    assert_raises(RangeError) { c.add_reference(1/10r, 1/10r, **LEVELS, limit: 10) }
    assert_raises(RangeError) { c.add_paired(2r, 1r, **LEVELS, limit: 10) }
    assert_raises(RangeError) { c.add_paired(0r, 1r, **LEVELS, limit: 10) }
    assert_raises(RangeError) { c.add_reference(1/100r, 1/10r, alpha: 1/2r, beta: 1/2r, limit: 10) }
    assert_raises(RangeError) { errors(c, limit: 0) }
    assert_raises(RangeError) do
      c.add_reference(Rational(1, 2**32 - 1), Rational(1, 2**32 - 2), **LEVELS, limit: 10)
    end
    assert_equal 0, c.size

    errors(c)
    assert_raises(RangeError) { c.observe_trials(1, 0, 0, good: 2, total: 1) }
    assert_raises(TypeError) { c.observe_trials(1, 0, 0, good: 1.0, total: 1) }
    assert_raises(TypeError) { c.observe_trials(1, '0', 0, good: 1, total: 1) }
    assert_raises(ArgumentError) { c.observe_trials(1, 0, 0, good: 1) }
    matched(c)
    assert_raises(RangeError) { c.observe_pairs(1, 0, 1, canary_only: 2**64 - 1, baseline_only: 1, concordant: 0) }
    assert_raises(RangeError) { c.observe_pairs(1, 0, 1, canary_only: -1, baseline_only: 1, concordant: 0) }
    assert_raises(TypeError) { c.recommend('clear') }
    assert_raises(ArgumentError) { c.recommend(:fine) }
    assert_raises(TypeError) { c.status(nil) }
    assert_raises(TypeError) { c.bounds(0.0) }
    assert_raises(TypeError) { c.thresholds('0') }
    assert_raises(TypeError) { c.restart(nil) }
    assert_equal 0, c.status(0).samples
  end

  def test_frozen_canary_refuses_mutation
    c = canary
    errors(c)
    c.freeze
    assert_raises(FrozenError) { errors(c) }
    assert_raises(FrozenError) { matched(c) }
    assert_raises(FrozenError) { c.observe_trials(1, 0, 0, good: 1, total: 1) }
    assert_raises(FrozenError) { c.observe_pairs(1, 0, 0, canary_only: 0, baseline_only: 0, concordant: 1) }
    assert_raises(FrozenError) { c.restart(2) }
    assert_predicate c.recommend, :continue?
    assert_equal :undecided, c.status(0).outcome
  end
end
