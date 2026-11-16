# frozen_string_literal: true

require 'minitest/autorun'
require 'open3'
require 'rbconfig'

# The classes of the values the extension wraps (Sprt, Canary) are fixed when
# it loads. Nothing a caller does later, redefining a constant or
# `Object.const_get`, reaches a value being built. Each case runs in a fresh
# process: the class is fixed once per process, at load.
class ClassLookupTest < Minitest::Test
  LIB = File.expand_path('../lib', __dir__)

  PRELUDE = <<~RUBY
    require 'canary_machines'
    C = CanaryMachines
    level = C::Rate.new(5, 100)
    reference = C::Model::Reference.new(C::Rate.new(1, 100), C::Rate.new(1, 10))
    plan = C::Plan.new(reference, level, level, 1000, :hold)
  RUBY

  # Runs `body` after PRELUDE in a new Ruby; answers its last line of output.
  def isolated(body)
    out, err, status = Open3.capture3(RbConfig.ruby, '-I', LIB, '-e', PRELUDE + body)
    assert status.success?, "exited #{status.exitstatus}: #{err}"
    out.lines.last&.chomp
  end

  # Bug: a Sprt constant redefined before the first Sprt made it an instance
  # of the new class, without any of the gem's methods.
  def test_a_redefined_sprt_constant_does_not_change_the_sprt_class
    assert_equal 'true undecided 1', isolated(<<~RUBY)
      original = C::Sprt
      C.send(:remove_const, :Sprt)
      C.const_set(:Sprt, Class.new)
      sprt = original.new(plan)
      puts [sprt.instance_of?(original), sprt.outcome, sprt.observe_trials(C::Counts.new(1, 1)).samples].join(' ')
    RUBY
  end

  # Bug: the same for a Canary.
  def test_a_redefined_canary_constant_does_not_change_the_canary_class
    assert_equal 'true 0 7', isolated(<<~RUBY)
      original = C::Canary
      C.send(:remove_const, :Canary)
      C.const_set(:Canary, Class.new)
      canary = original.new(7)
      puts [canary.instance_of?(original), canary.add(plan), canary.generation].join(' ')
    RUBY
  end

  # Bug: building the first value ran `Object.const_get`, Ruby code in the
  # middle of a native call. No Ruby runs while one is built now.
  def test_building_a_value_runs_no_const_get
    assert_equal '0 Sprt Canary', isolated(<<~RUBY)
      hits = 0
      Object.define_singleton_method(:const_get) do |*args|
        hits += 1
        super(*args)
      end
      sprt = C::Sprt.new(plan)
      canary = C::Canary.new(1)
      puts [hits, sprt.class.name.split('::').last, canary.class.name.split('::').last].join(' ')
    RUBY
  end

  # Bug: a redefined `Object.const_get` that raised hit an `unwrap` in the
  # extension: an uncatchable fatal, and the process exited.
  def test_a_raising_const_get_does_not_kill_the_process
    assert_equal 'undecided 0', isolated(<<~RUBY)
      Object.define_singleton_method(:const_get) { |*| raise IOError, 'lookup ran while building' }
      sprt = C::Sprt.new(plan)
      canary = C::Canary.new(1)
      puts [sprt.outcome, canary.add(plan)].join(' ')
    RUBY
  end
end
