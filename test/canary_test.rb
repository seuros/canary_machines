# frozen_string_literal: true
require 'minitest/autorun'
require 'canary_machines'

class CanaryTest < Minitest::Test
  include CanaryMachines

  LEVEL = Rate.new(5, 100)
  # One failure in a hundred against one in ten.
  REFERENCE = Model::Reference.new(Rate.new(1, 100), Rate.new(1, 10))
  # Odds ratio one against two.
  PAIRED = Model::Paired.new(OddsRatio.new(1, 1), OddsRatio.new(2, 1))

  def canary(generation = 1) = Canary.new(generation)
  def reference(limit = 1000, on_limit = :hold) = Plan.new(REFERENCE, LEVEL, LEVEL, limit, on_limit)
  def paired(limit = 1000, on_limit = :hold) = Plan.new(PAIRED, LEVEL, LEVEL, limit, on_limit)
  def trials(good, total) = Counts.new(good, total)
  def pairs(canary_only, baseline_only, concordant) = Pairs.new(canary_only, baseline_only, concordant)

  def test_starts_empty
    c = canary(7)
    assert_equal [7, nil, 0, true, 8], [c.generation, c.last, c.size, c.empty?, c.capacity]
    assert_equal 8, CanaryMachines::CAPACITY
    assert_equal Recommendation.new(action: :hold, reason: :no_tests, test: nil), c.recommend(:clear)
    assert_nil c.status(0)
    assert_nil c.plan(0)
    assert_raises(ArgumentError) { c.recommend }
    assert_equal 0, c.add(reference)
    refute_predicate c, :empty?
    assert_equal [reference, nil, nil], [c.plan(0), c.plan(1), c.plan(2**64 - 1)]
  end

  def test_values_mirror_the_core
    assert_equal %i[num den], Rate.members
    assert_equal %i[num den], OddsRatio.members
    assert_equal %i[good total], Counts.members
    assert_equal %i[canary_only baseline_only concordant], Pairs.members
    assert_equal %i[acceptable unacceptable], Model::Reference.members
    assert_equal %i[acceptable unacceptable], Model::Paired.members
    assert_equal %i[model alpha beta limit on_limit], Plan.members
    assert_equal %i[lo hi], Interval.members
    assert_equal %i[num den], Fraction.members
    assert_equal %i[false_rollback false_promote], Bounds.members
    assert_equal %i[look decided_at], Status.members
    assert_equal %i[outcome hits misses samples llr_micro], Look.members
    assert_equal %i[action reason test], Recommendation.members
    assert_equal [1, 2, 500, 503], pairs(1, 2, 500).then { [it.canary_only, it.baseline_only, it.concordant, it.total] }
    plan = reference(50, :rollback)
    assert_equal [REFERENCE, LEVEL, LEVEL, 50, :rollback],
                 [plan.model, plan.alpha, plan.beta, plan.limit, plan.on_limit]
    assert_predicate plan, :frozen?
    assert_raises(NoMethodError) { plan.__validate }
  end

  def test_rate_from_an_objective
    objective = Struct.new(:good, :per)
    assert_equal Rate.new(1, 1000), Rate.failure_of(objective.new(999, 1000))
    assert_nil Rate.failure_of(objective.new(0, 10))
    assert_raises(RangeError) { Rate.failure_of(objective.new(10, 10)) }
    assert_raises(TypeError) { Rate.failure_of(objective.new(0.5, 10)) }
    assert_raises(TypeError) { Rate.failure_of(999) }
  end

  def test_plan_readings
    plan = reference
    assert_equal [-2_944_439, 2_944_438], [plan.lower_micro, plan.upper_micro]
    assert_equal 48, CanaryMachines::FRACTION_BITS
    upper, lower, hit, miss = plan.upper, plan.lower, plan.hit, plan.miss
    [upper, lower, hit, miss].each do |interval|
      assert_instance_of Interval, interval
      assert_operator interval.lo, :<=, interval.hi
    end
    assert_equal plan.upper_micro, CanaryMachines.micro_nats(upper.hi)
    assert_equal plan.lower_micro, CanaryMachines.micro_nats(lower.lo)
    assert_operator hit.lo, :>, 0
    assert_operator miss.hi, :<, 0
    assert_operator hit.hi, :<, upper.lo
    assert_equal [1_000_000, -1, 2**63 - 1, -2**63],
                 [1 << 48, -1, 2**126, -2**126].map { CanaryMachines.micro_nats(it) }
    assert_raises(TypeError) { CanaryMachines.micro_nats(1.0) }
    assert_raises(RangeError) { CanaryMachines.micro_nats(2**127) }
  end

  def test_guardrail_from_a_budget_state_or_standing
    assert_equal %i[no_data clear breached], %i[no_data quiet firing].map { Guardrail.from(it) }
    assert_equal :clear, Guardrail.from(Struct.new(:remaining).new(5))
    assert_equal :breached, Guardrail.from(Struct.new(:overspent).new(1))
    assert_raises(ArgumentError) { Guardrail.from(:clear) }
    assert_raises(TypeError) { Guardrail.from('firing') }
    assert_raises(TypeError) { Guardrail.from(Struct.new(:remaining).new(0.5)) }
    assert_predicate canary.tap { it.add(reference) }.recommend(Guardrail.from(:firing)), :rollback?
  end

  def test_instants_are_integer_nanoseconds_in_and_out
    c = canary
    c.add(reference)
    now = Process.clock_gettime(Process::CLOCK_MONOTONIC, :nanosecond)
    assert_nil c.observe_trials(1, now, 0, trials(0, 1)).decided_at
    assert_equal now, c.last
    assert_equal :time_regression, c.observe_trials(1, now - 1, 0, trials(0, 1))
    done = c.observe_trials(1, now + 60_000_000_000, 0, trials(0, 1))
    assert_equal [:regressed, now + 60_000_000_000], [done.look.outcome, done.decided_at]
    assert_equal [now + 60_000_000_000, now + 60_000_000_000], [c.last, c.status(0).decided_at]
    assert_kind_of Integer, done.decided_at
  end

  def test_reference_test_reaches_each_threshold
    c = canary
    assert_equal 0, c.add(reference)

    first = c.observe_trials(1, 10, 0, trials(0, 1))
    look = Look.new(outcome: :undecided, hits: 1, misses: 0, samples: 1, llr_micro: 2_302_585)
    assert_equal Status.new(look:, decided_at: nil), first
    refute_predicate first.look, :decided?
    refute_predicate first, :decided?
    assert_predicate first, :frozen?
    assert_predicate c.recommend(:clear), :continue?
    assert_equal :sampling, c.recommend(:clear).reason

    second = c.observe_trials(1, 11, 0, trials(0, 1))
    assert_equal [:regressed, 11], [second.look.outcome, second.decided_at]
    assert_predicate second, :decided?
    assert_equal second, c.status(0)
    assert_equal Recommendation.new(action: :rollback, reason: :evidence, test: 0), c.recommend(:clear)
    assert_predicate c.recommend(:clear), :rollback?
    assert_equal :concluded, c.observe_trials(1, 12, 0, trials(1, 1))
    assert_equal 11, c.last

    assert_equal :ok, c.restart(2)
    assert_equal :undecided, c.observe_trials(2, 12, 0, trials(30, 30)).look.outcome
    assert_equal :cleared, c.observe_trials(2, 13, 0, trials(1, 1)).look.outcome
    assert_equal Recommendation.new(action: :promote, reason: :evidence, test: nil), c.recommend(:clear)
    assert_predicate c.recommend(:clear), :promote?
  end

  def test_paired_test_discards_concordant_pairs
    c = canary
    assert_equal 0, c.add(paired)
    seen = c.observe_pairs(1, 0, 0, pairs(1, 1, 500))
    assert_equal [1, 1, 502, -117_784], seen.look.to_h.values_at(:hits, :misses, :samples, :llr_micro)
    assert_equal :undecided, c.observe_pairs(1, 1, 0, pairs(10, 0, 0)).look.outcome
    assert_equal :regressed, c.observe_pairs(1, 2, 0, pairs(1, 0, 0)).look.outcome
  end

  def test_counts_are_read_by_duck_typing
    c = canary
    c.add(reference)
    budget_counts = Struct.new(:good, :total).new(1, 1)
    assert_equal 1, c.observe_trials(1, 0, 0, budget_counts).look.samples
    assert_raises(RangeError) { c.observe_trials(1, 0, 0, Struct.new(:good, :total).new(2, 1)) }
    assert_raises(TypeError) { c.observe_trials(1, 0, 0, [1, 1]) }
    assert_raises(TypeError) { c.observe_trials(1, 0, 0, pairs(0, 0, 1)) }
    assert_raises(TypeError) { c.observe_pairs(1, 0, 0, trials(1, 1)) }
    assert_equal 1, c.status(0).look.samples
  end

  def test_refusals_are_symbols
    c = canary(5)
    c.add(reference)
    c.add(paired)
    assert_equal :stale_generation, c.observe_trials(4, 0, 0, trials(1, 1))
    assert_equal :unknown_test, c.observe_trials(5, 0, 2, trials(1, 1))
    assert_equal :unknown_test, c.observe_trials(5, 0, 2**64 - 1, trials(1, 1))
    assert_equal :wrong_model, c.observe_trials(5, 0, 1, trials(1, 1))
    assert_equal :wrong_model, c.observe_pairs(5, 0, 0, pairs(0, 0, 1))
    assert_nil c.last
    assert_kind_of Status, c.observe_trials(5, 9, 0, trials(1, 1))
    assert_equal :time_regression, c.observe_trials(5, 8, 0, trials(1, 1))
    assert_equal 1, c.status(0).look.samples
    assert_equal :stale_generation, c.restart(5)
    assert_equal :stale_generation, c.restart(4)
    assert_equal 5, c.generation

    huge = 2**64 - 1
    wide = canary
    wide.add(paired(huge))
    assert_equal :undecided, wide.observe_pairs(1, 0, 0, pairs(0, 0, huge - 1)).look.outcome
    assert_equal :overflow, wide.observe_pairs(1, 0, 0, pairs(1, 1, 0))
    assert_equal :truncated, wide.observe_pairs(1, huge, 0, pairs(0, 0, 1)).look.outcome
  end

  def test_capacity_is_fixed
    c = canary
    assert_equal (0...8).to_a, Array.new(8) { c.add(reference) }
    assert_equal :full, c.add(reference)
    assert_equal :full, c.add(paired)
    assert_equal 8, c.size
  end

  def test_limit_policies_and_bounds
    wald = Fraction.new(500, 9500)
    { hold: [:hold, wald, wald], rollback: [:rollback, nil, wald], promote: [:promote, wald, nil] }
      .each do |policy, (action, false_rollback, false_promote)|
      c = canary
      c.add(reference(5, policy))
      assert_equal Bounds.new(false_rollback:, false_promote:), c.plan(0).bounds
      assert_equal :truncated, c.observe_trials(1, 0, 0, trials(5, 5)).look.outcome
      assert_equal Recommendation.new(action:, reason: :limit, test: 0), c.recommend(:clear)
    end
    c = canary
    c.add(reference(5))
    assert_predicate c.recommend(:clear), :continue?
    c.observe_trials(1, 0, 0, trials(5, 5))
    assert_predicate c.recommend(:clear), :hold?
  end

  def test_guardrail_overrides_and_withholds
    c = canary
    c.add(reference)
    assert_equal %i[rollback guardrail], c.recommend(:breached).to_h.values_at(:action, :reason)
    assert_predicate c.recommend(:no_data), :continue?
    c.observe_trials(1, 0, 0, trials(31, 31))
    assert_predicate c.recommend(:clear), :promote?
    assert_equal %i[hold guardrail_no_data], c.recommend(:no_data).to_h.values_at(:action, :reason)
    assert_predicate c.recommend(:breached), :rollback?
  end

  def test_a_single_sprt
    s = Sprt.new(reference(5))
    assert_equal [reference(5), :undecided], [s.plan, s.outcome]
    assert_equal Look.new(outcome: :undecided, hits: 0, misses: 0, samples: 0, llr_micro: 0), s.look
    assert_equal :wrong_model, s.observe_pairs(pairs(1, 0, 0))
    first = s.observe_trials(trials(0, 1))
    assert_equal Look.new(outcome: :undecided, hits: 1, misses: 0, samples: 1, llr_micro: 2_302_585), first
    assert_equal first, s.look
    assert_equal :regressed, s.observe_trials(trials(0, 1)).outcome
    assert_equal :regressed, s.outcome
    assert_equal :concluded, s.observe_trials(trials(1, 1))
    assert_nil s.reset
    assert_equal [:undecided, 0], [s.outcome, s.look.samples]
    assert_equal :truncated, s.observe_trials(trials(5, 5)).outcome

    wide = Sprt.new(paired(2**64 - 1))
    assert_equal :undecided, wide.observe_pairs(pairs(0, 0, 2**64 - 2)).outcome
    assert_equal :overflow, wide.observe_pairs(pairs(1, 1, 0))
    assert_equal 2**64 - 2, wide.look.samples

    assert_raises(TypeError) { Sprt.new(REFERENCE) }
    assert_raises(TypeError) { Sprt.allocate }
    assert_raises(TypeError) { s.observe_trials(nil) }
    assert_raises(TypeError) { s.observe_pairs([1, 0, 0]) }
    s.freeze
    assert_raises(FrozenError) { s.observe_trials(trials(1, 1)) }
    assert_raises(FrozenError) { s.observe_pairs(pairs(0, 0, 1)) }
    assert_raises(FrozenError) { s.reset }
    assert_equal :truncated, s.outcome
  end

  def test_invalid_values_never_exist
    assert_raises(TypeError) { Rate.new(0.01, 10) }
    assert_raises(TypeError) { Rate.new(1, nil) }
    assert_raises(RangeError) { Rate.new(0, 10) }
    assert_raises(RangeError) { Rate.new(10, 10) }
    assert_raises(RangeError) { Rate.new(-1, 10) }
    assert_raises(RangeError) { Rate.new(1, 2**32) }
    assert_raises(ArgumentError) { Rate.new(1) }
    assert_raises(RangeError) { OddsRatio.new(0, 1) }
    assert_raises(RangeError) { OddsRatio.new(1, 0) }
    assert_raises(TypeError) { OddsRatio.new(1r, 1) }
    assert_raises(RangeError) { Counts.new(2, 1) }
    assert_raises(TypeError) { Counts.new(1.0, 1) }
    assert_raises(RangeError) { Counts.new(-1, 1) }
    assert_raises(RangeError) { Pairs.new(2**64 - 1, 1, 0) }
    assert_raises(RangeError) { Pairs.new(-1, 1, 0) }
    assert_raises(TypeError) { Pairs.new(1, 1, nil) }
    assert_raises(TypeError) { Model::Reference.new(1/100r, Rate.new(1, 10)) }
    assert_raises(TypeError) { Model::Reference.new(OddsRatio.new(1, 1), OddsRatio.new(2, 1)) }
    assert_raises(TypeError) { Model::Paired.new(Rate.new(1, 100), Rate.new(1, 10)) }
    assert_raises(TypeError) { Plan.new([1, 2], LEVEL, LEVEL, 10, :hold) }
    assert_raises(TypeError) { Plan.new(REFERENCE, 5/100r, LEVEL, 10, :hold) }
    assert_raises(TypeError) { Plan.new(REFERENCE, LEVEL, LEVEL, 10.0, :hold) }
    assert_raises(TypeError) { Plan.new(REFERENCE, LEVEL, LEVEL, 10, 'hold') }
    assert_raises(ArgumentError) { Plan.new(REFERENCE, LEVEL, LEVEL, 10, :panic) }
    assert_raises(ArgumentError) { Plan.new(REFERENCE, LEVEL, LEVEL, 10) }
    assert_raises(RangeError) { Plan.new(REFERENCE, LEVEL, LEVEL, 0, :hold) }
    assert_raises(RangeError) { Plan.new(REFERENCE, Rate.new(1, 2), Rate.new(1, 2), 10, :hold) }
    same = Model::Reference.new(Rate.new(1, 10), Rate.new(1, 10))
    assert_raises(RangeError) { Plan.new(same, LEVEL, LEVEL, 10, :hold) }
    backwards = Model::Paired.new(OddsRatio.new(2, 1), OddsRatio.new(1, 1))
    assert_raises(RangeError) { Plan.new(backwards, LEVEL, LEVEL, 10, :hold) }
    close = Model::Reference.new(Rate.new(1, 2**32 - 1), Rate.new(1, 2**32 - 2))
    assert_raises(RangeError) { Plan.new(close, LEVEL, LEVEL, 10, :hold) }
  end

  def test_bad_input_raises
    c = canary
    assert_raises(TypeError) { Canary.new(1.0) }
    assert_raises(TypeError) { Canary.new(nil) }
    assert_raises(RangeError) { Canary.new(-1) }
    assert_raises(RangeError) { Canary.new(2**64) }
    assert_raises(TypeError) { Canary.allocate }
    assert_raises(TypeError) { c.add(REFERENCE) }
    assert_raises(TypeError) { c.add(nil) }
    assert_raises(ArgumentError) { c.add(reference, limit: 10) }
    assert_equal 0, c.size

    c.add(reference)
    assert_raises(TypeError) { c.observe_trials(1, '0', 0, trials(1, 1)) }
    assert_raises(TypeError) { c.observe_trials(1, 0.5, 0, trials(1, 1)) }
    assert_raises(TypeError) { c.observe_trials(1, 1r, 0, trials(1, 1)) }
    assert_raises(TypeError) { c.observe_trials(1, nil, 0, trials(1, 1)) }
    assert_raises(RangeError) { c.observe_trials(1, -1, 0, trials(1, 1)) }
    assert_raises(RangeError) { c.observe_trials(1, 2**64, 0, trials(1, 1)) }
    assert_raises(TypeError) { c.observe_pairs(1, 0.5, 0, pairs(0, 0, 1)) }
    assert_raises(RangeError) { c.observe_pairs(1, -1, 0, pairs(0, 0, 1)) }
    assert_raises(ArgumentError) { c.observe_trials(1, 0, 0) }
    assert_raises(TypeError) { c.observe_trials(1, 0, 0, good: 1, total: 1) }
    assert_nil c.last
    assert_raises(TypeError) { c.recommend('clear') }
    assert_raises(ArgumentError) { c.recommend(:fine) }
    assert_raises(TypeError) { c.status(nil) }
    assert_raises(TypeError) { c.plan(0.0) }
    assert_raises(TypeError) { c.restart(nil) }
    assert_equal 0, c.status(0).look.samples
  end

  def test_frozen_canary_refuses_mutation
    c = canary
    c.add(reference)
    c.freeze
    assert_raises(FrozenError) { c.add(reference) }
    assert_raises(FrozenError) { c.observe_trials(1, 0, 0, trials(1, 1)) }
    assert_raises(FrozenError) { c.observe_pairs(1, 0, 0, pairs(0, 0, 1)) }
    assert_raises(FrozenError) { c.restart(2) }
    assert_predicate c.recommend(:clear), :continue?
    assert_equal :undecided, c.status(0).look.outcome
  end
end
