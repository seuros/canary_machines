# frozen_string_literal: true
require 'minitest/autorun'
require 'canary_machines'

# Every Data value validates through the core, whether the gem or a caller
# built it, and the binding raises nothing after the core has changed.
class ValuesTest < Minitest::Test
  include CanaryMachines

  LEVEL = Rate.new(5, 100)
  REFERENCE = Model::Reference.new(Rate.new(1, 100), Rate.new(1, 10))
  # A sibling's counts value, read by duck typing.
  SiblingCounts = Data.define(:good, :total)

  def plan(on_limit = :hold) = Plan.new(REFERENCE, LEVEL, LEVEL, 1000, on_limit)
  def rebuilt(value) = value.class.new(*value.deconstruct)

  def test_returned_values_are_accepted_back
    c = Canary.new(1)
    c.add(plan)
    [c.observe_trials(1, 10, 0, Counts.new(1, 1)), c.observe_trials(1, 11, 0, Counts.new(0, 1))].each do |status|
      assert_equal status, rebuilt(status)
      assert_equal status.look, rebuilt(status.look)
    end
    %i[clear no_data breached].each { |guardrail| assert_equal c.recommend(guardrail), rebuilt(c.recommend(guardrail)) }
    %i[hold rollback promote].each do |on_limit|
      bounds = plan(on_limit).bounds
      assert_equal bounds, rebuilt(bounds)
      [bounds.false_rollback, bounds.false_promote].compact.each { |bound| assert_equal bound, rebuilt(bound) }
      %i[upper lower hit miss].each { |name| assert_equal plan.public_send(name), rebuilt(plan.public_send(name)) }
    end
  end

  def test_decided_is_the_cores_reader
    undecided = Look.new(:undecided, 0, 0, 0, 0)
    regressed = Look.new(:regressed, 2, 0, 2, 4_605_170)
    refute_predicate undecided, :decided?
    assert_predicate regressed, :decided?
    refute_predicate Status.new(undecided, nil), :decided?
    assert_predicate Status.new(regressed, 7), :decided?
    refute_respond_to Recommendation.new(:hold, :no_tests, nil), :hold?
  end

  def test_hand_built_values_no_core_produces_raise_range_error
    [
      -> { Rate.new(0, 10) },
      -> { Rate.new(10, 10) },
      -> { Rate.new(-1, 10) },
      -> { Rate.new(1, 2**32) },
      -> { OddsRatio.new(0, 1) },
      -> { OddsRatio.new(1, 0) },
      -> { Counts.new(2, 1) },
      -> { Counts.new(-1, 1) },
      -> { Pairs.new(2**64 - 1, 1, 0) },
      -> { Pairs.new(-1, 1, 0) },
      -> { Plan.new(REFERENCE, LEVEL, LEVEL, 10, :panic) },
      -> { Plan.new(REFERENCE, LEVEL, LEVEL, 0, :hold) },
      -> { Plan.new(REFERENCE, Rate.new(1, 2), Rate.new(1, 2), 10, :hold) },
      -> { Plan.new(Model::Reference.new(Rate.new(1, 10), Rate.new(1, 10)), LEVEL, LEVEL, 10, :hold) },
      -> { Plan.new(Model::Paired.new(OddsRatio.new(2, 1), OddsRatio.new(1, 1)), LEVEL, LEVEL, 10, :hold) },
      -> { Plan.new(Model::Reference.new(Rate.new(1, 2**32 - 1), Rate.new(1, 2**32 - 2)), LEVEL, LEVEL, 10, :hold) },
      -> { Interval.new(2, 1) },
      -> { Interval.new(2**127, 2**128) },
      -> { Fraction.new(1, 0) },
      -> { Bounds.new(nil, nil) },
      -> { Bounds.new(Fraction.new(2, 2), nil) },
      -> { Bounds.new(nil, Fraction.new(0, 3)) },
      -> { Look.new(:undecided, 3, 3, 5, 0) }, # more weighed than sampled
      -> { Look.new(:truncated, 0, 0, 0, 0) }, # decided with no samples
      -> { Look.new(:undecided, 0, 0, 4, 7) }, # a ratio without evidence
      -> { Look.new(:regressed, 0, 4, 4, 9) },
      -> { Look.new(:cleared, 0, 4, 4, 0) },
      -> { Look.new(:maybe, 0, 0, 0, 0) },
      -> { Look.new(:undecided, 0, 0, 0, 2**63) },
      -> { Status.new(Look.new(:undecided, 0, 0, 0, 0), 5) },
      -> { Status.new(Look.new(:regressed, 2, 0, 2, 9), nil) },
      -> { Recommendation.new(:promote, :guardrail, nil) },
      -> { Recommendation.new(:rollback, :evidence, nil) },
      -> { Recommendation.new(:hold, :no_tests, 0) },
      -> { Recommendation.new(:continue, :limit, 0) },
      -> { Recommendation.new(:wait, :sampling, nil) },
      -> { Recommendation.new(:hold, :whim, nil) }
    ].each { |build| assert_raises(RangeError) { build.call } }
  end

  def test_hand_built_values_of_the_wrong_type_raise_type_error
    [
      -> { Rate.new(0.01, 10) },
      -> { Rate.new(1, nil) },
      -> { OddsRatio.new(1r, 1) },
      -> { Counts.new(1.0, 1) },
      -> { Pairs.new(1, 1, nil) },
      -> { Model::Reference.new(1/100r, Rate.new(1, 10)) },
      -> { Model::Reference.new(OddsRatio.new(1, 1), OddsRatio.new(2, 1)) },
      -> { Model::Paired.new(Rate.new(1, 100), Rate.new(1, 10)) },
      -> { Plan.new([1, 2], LEVEL, LEVEL, 10, :hold) },
      -> { Plan.new(REFERENCE, 5/100r, LEVEL, 10, :hold) },
      -> { Plan.new(REFERENCE, LEVEL, LEVEL, 10.0, :hold) },
      -> { Plan.new(REFERENCE, LEVEL, LEVEL, 10, 'hold') },
      -> { Interval.new(1.0, 2) },
      -> { Fraction.new(1, '2') },
      -> { Bounds.new([1, 2], nil) },
      -> { Look.new('undecided', 0, 0, 0, 0) },
      -> { Look.new(:undecided, 0, 0, 0, 0.0) },
      -> { Status.new([:undecided, 0, 0, 0, 0], nil) },
      -> { Status.new(Look.new(:regressed, 2, 0, 2, 9), 1.5) },
      -> { Recommendation.new(:hold, :no_tests, 0.0) },
      -> { Recommendation.new('hold', :no_tests, nil) }
    ].each { |build| assert_raises(TypeError) { build.call } }
  end

  # A subclass overriding `initialize`, or `allocate`, would build a value
  # that never ran the check.
  def test_values_are_final
    [Rate, OddsRatio, Counts, Pairs, Model::Reference, Model::Paired, Interval, Fraction, Bounds, Plan, Look, Status,
     Recommendation].each do |value|
      assert_raises(TypeError) { Class.new(value) { def initialize(**) = nil } }
      assert_raises(NoMethodError) { value.allocate }
    end
    assert_raises(RangeError) { Rate.new(1, 2).with(num: 2) }
  end

  def test_counts_mirror_the_core
    window = Counts.new(995, 1000)
    assert_equal 5, window.bad
    assert_equal Counts.new(996, 1002), window.checked_add(Counts.new(1, 2))
    sum = window.checked_add(SiblingCounts.new(1, 2))
    assert_instance_of Counts, sum
    assert_equal Counts.new(996, 1002), sum
    assert_nil Counts.new(0, 2**64 - 1).checked_add(Counts.new(0, 1))
    assert_raises(TypeError) { window.checked_add([1, 2]) }
    assert_raises(RangeError) { window.checked_add(SiblingCounts.new(2, 1)) }
  end

  # Sealing stops accidents, not reflection: a named subclass raises, yet
  # Ruby has already bound its constant, and `Data#initialize` can be bound
  # to skip the validating one. The binding refuses such a value by its
  # exact class before it reaches the core, even where it reads a sibling's
  # counts by duck typing.
  def test_a_leaked_subclass_never_reaches_the_core
    assert_raises(TypeError) { eval('class ForgedCounts < CanaryMachines::Counts; end', binding, __FILE__, __LINE__) }
    assert_raises(TypeError) { eval('class ForgedPlan < CanaryMachines::Plan; end', binding, __FILE__, __LINE__) }
    assert_equal 'constant', defined?(ForgedCounts) # the constant leaks
    [ForgedCounts, ForgedPlan].each do |forgery|
      forgery.define_method(:initialize) { |**kw| Data.instance_method(:initialize).bind_call(self, **kw) }
    end
    c = Canary.new(1)
    c.add(plan)
    assert_raises(TypeError) { c.observe_trials(1, 10, 0, ForgedCounts.new(good: 5, total: 1)) }
    assert_raises(TypeError) { c.observe_trials(1, 10, 0, ForgedCounts.new(good: 1, total: 1)) }
    assert_raises(TypeError) { Counts.new(1, 1).checked_add(ForgedCounts.new(good: 1, total: 1)) }
    forged_plan = ForgedPlan.new(model: REFERENCE, alpha: LEVEL, beta: LEVEL, limit: 0, on_limit: :hold)
    assert_raises(TypeError) { c.add(forged_plan) }
    assert_raises(TypeError) { Sprt.new(forged_plan) }
    assert_equal [1, nil], [c.size, c.last]
  ensure
    %i[ForgedCounts ForgedPlan].each do |name|
      self.class.send(:remove_const, name) if self.class.const_defined?(name, false)
    end
  end

  # `Marshal.load` builds an instance of the real class without `initialize`;
  # the binding rebuilds every value it reads through the core.
  # A singleton class opened on a value does not change what it is.
  def test_an_opened_singleton_class_is_still_the_gems_value
    c = Canary.new(1)
    assert_equal 0, c.add(plan.tap(&:singleton_class))
    assert_instance_of Status, c.observe_trials(1, 1, 0, Counts.new(1, 1).tap(&:singleton_class))
  end

  # Fields are read from the struct itself: a redefined reader never runs.
  def test_redefined_readers_are_ignored
    built = plan
    counts = Counts.new(1, 1)
    status = Canary.new(1).tap { |c| c.add(built) }.status(0)
    with_raising_readers(Rate, Model::Reference, Plan, Counts, Look, Status) do
      c = Canary.new(1)
      assert_equal 0, c.add(built)
      assert_instance_of Status, c.observe_trials(1, 1, 0, counts)
      assert_equal 0, counts.bad
      assert_equal status, Status.new(*status.deconstruct)
      assert_kind_of Integer, built.upper_micro
    end
  end

  # Every reader of `classes` raises while the block runs.
  def with_raising_readers(*classes)
    saved = classes.flat_map { |klass| klass.members.map { |name| [klass, name, klass.instance_method(name)] } }
    verbose = $VERBOSE
    $VERBOSE = nil
    saved.each { |klass, name, _| klass.define_method(name) { raise 'the binding ran a reader' } }
    $VERBOSE = verbose
    yield
  ensure
    $VERBOSE = nil
    saved.each { |klass, name, method| klass.define_method(name, method) }
    $VERBOSE = verbose
  end

  def test_a_marshal_forged_value_never_reaches_the_core
    c = Canary.new(1)
    c.add(plan)
    assert_raises(RangeError) { c.observe_trials(1, 10, 0, forged(Counts, 5, 1)) }
    assert_raises(RangeError) { c.observe_pairs(1, 10, 0, forged(Pairs, 2**63, 2**63, 0)) }
    assert_raises(RangeError) { c.add(forged(Plan, REFERENCE, LEVEL, LEVEL, 0, :hold)) }
    backwards = forged(Model::Reference, Rate.new(1, 10), Rate.new(1, 100))
    assert_raises(RangeError) { c.add(Plan.new(backwards, LEVEL, LEVEL, 9, :hold)) }
    assert_raises(RangeError) { Sprt.new(Plan.new(REFERENCE, forged(Rate, 2, 1), LEVEL, 9, :hold)) }
    assert_raises(RangeError) { Status.new(forged(Look, :regressed, 0, 0, 0, 0), 1) }
    assert_equal [1, nil, 0], [c.size, c.last, c.status(0).look.samples]
  end

  def test_observe_applies_nothing_when_building_its_answer_raises_re_enters_or_freezes
    [
      [IOError, ->(_) { raise IOError }],
      [RuntimeError, ->(c) { c.observe_trials(1, 20, 0, Counts.new(1, 1)) }],
      [FrozenError, lambda(&:freeze)]
    ].each do |error, hook|
      c = Canary.new(1)
      c.add(plan)
      replacing(Status, ->(*) { hook.call(c) }) do
        assert_raises(error) { c.observe_trials(1, 10, 0, Counts.new(0, 1)) }
      end
      expected = error == RuntimeError ? [20, 1, 1] : [nil, 0, 0]
      assert_equal expected, [c.last, c.status(0).look.samples, c.status(0).look.misses]
    end
  end

  def test_sprt_observe_applies_nothing_when_building_its_answer_raises_re_enters_or_freezes
    [
      [IOError, ->(_) { raise IOError }],
      [RuntimeError, ->(sprt) { sprt.observe_trials(Counts.new(1, 1)) }],
      [FrozenError, lambda(&:freeze)]
    ].each do |error, hook|
      sprt = Sprt.new(plan)
      replacing(Look, ->(*) { hook.call(sprt) }) do
        assert_raises(error) { sprt.observe_trials(Counts.new(0, 1)) }
      end
      expected = error == RuntimeError ? [1, 1] : [0, 0]
      assert_equal expected, [sprt.look.samples, sprt.look.misses]
    end
  end

  # No borrow is held while a reader builds its answer, so a re-entrant call
  # from `new` works instead of aborting the process.
  def test_readers_hold_no_borrow_while_building
    c = Canary.new(1)
    c.add(plan)
    sprt = Sprt.new(plan)
    replacing(Plan, ->(*) { c.observe_trials(1, 1, 0, Counts.new(1, 1)) }) { c.plan(0) }
    replacing(Status, ->(*) { c.observe_pairs(1, 2, 0, Pairs.new(0, 0, 1)) }) { c.status(0) }
    replacing(Recommendation, ->(*) { c.restart(2) }) { c.recommend(:clear) }
    replacing(Look, ->(*) { sprt.observe_trials(Counts.new(1, 1)) }) { sprt.look }
    replacing(Plan, ->(*) { sprt.reset }) { sprt.plan }
    assert_equal [2, 0], [c.generation, sprt.look.samples]
  end

  # The classes are resolved once and pinned: after a compaction that moves
  # everything movable, every path that checks or builds a value still finds
  # them (unpinned, the exact-class check would refuse the gem's own values).
  def test_classes_survive_gc_compaction
    if GC.respond_to?(:verify_compaction_references)
      GC.verify_compaction_references(expand_heap: true, toward: :empty)
    elsif GC.respond_to?(:compact)
      GC.compact
    end
    c = Canary.new(1)
    c.add(Plan.new(Model::Paired.new(OddsRatio.new(1, 1), OddsRatio.new(3, 1)), LEVEL, LEVEL, 1000, :hold))
    c.add(plan)
    assert_instance_of Status, c.observe_pairs(1, 1, 0, Pairs.new(1, 0, 0))
    assert_instance_of Status, c.observe_trials(1, 2, 1, Counts.new(1, 1))
    assert_instance_of Plan, c.plan(0)
    assert_instance_of Model::Paired, c.plan(0).model
    assert_instance_of Look, c.status(1).look
    assert_instance_of Recommendation, c.recommend(:clear)
    assert_instance_of Interval, plan.upper
    assert_instance_of Fraction, plan.bounds.false_rollback
    assert_instance_of Rate, Rate.failure_of(SiblingObjective.new(99, 100))
    assert_instance_of Counts, Counts.new(1, 1).checked_add(Counts.new(1, 1))
    sprt = Sprt.new(plan)
    assert_instance_of Look, sprt.observe_trials(Counts.new(1, 1))
    assert_instance_of Model::Reference, sprt.plan.model
  end

  SiblingObjective = Data.define(:good, :per)

  private

  # Runs the block with `klass.new` calling `hook` first, once; the original
  # `new` is restored afterwards.
  def replacing(klass, hook)
    original = klass.method(:new)
    armed = true
    klass.define_singleton_method(:new) do |*args, **kw|
      if armed
        armed = false
        hook.call(*args)
      end
      original.call(*args, **kw)
    end
    yield
  ensure
    klass.singleton_class.send(:remove_method, :new)
    klass.define_singleton_method(:new, original)
  end

  # An instance of `klass` holding `fields`, loaded by Marshal from a look-alike
  # so its `initialize` never ran.
  def forged(klass, *fields)
    stand_in = Data.define(*klass.members)
    Object.const_set(:MarshalStandIn, stand_in)
    bytes = Marshal.dump(stand_in.new(*fields))
    name = klass.name
    Marshal.load(bytes.sub(":#{(5 + 14).chr}MarshalStandIn".b, ":#{(5 + name.size).chr}#{name}".b))
  ensure
    Object.send(:remove_const, :MarshalStandIn)
  end
end
