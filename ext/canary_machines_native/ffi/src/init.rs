//! Private native entry point.
use crate::{CAPACITY, canary::Canary};
use magnus::{Error, Ruby, function, method, prelude::*};

#[magnus::init]
fn init(ruby: &Ruby) -> Result<(), Error> {
    let module = ruby.define_module("CanaryMachines")?;
    module.const_set("CAPACITY", CAPACITY)?;

    let class = module.define_class("Canary", ruby.class_object())?;
    class.undef_default_alloc_func();
    class.define_singleton_method("new", function!(Canary::new, 1))?;
    class.define_method("add_reference", method!(Canary::add_reference, -1))?;
    class.define_method("add_paired", method!(Canary::add_paired, -1))?;
    class.define_method("observe_trials", method!(Canary::observe_trials, -1))?;
    class.define_method("observe_pairs", method!(Canary::observe_pairs, -1))?;
    class.define_method("recommend", method!(Canary::recommend, -1))?;
    class.define_method("restart", method!(Canary::restart, 1))?;
    class.define_method("status", method!(Canary::status, 1))?;
    class.define_method("bounds", method!(Canary::bounds, 1))?;
    class.define_method("thresholds", method!(Canary::thresholds, 1))?;
    class.define_method("generation", method!(Canary::generation, 0))?;
    class.define_method("last", method!(Canary::last, 0))?;
    class.define_method("size", method!(Canary::size, 0))?;
    class.define_method("capacity", function!(Canary::capacity, 0))?;
    Ok(())
}
