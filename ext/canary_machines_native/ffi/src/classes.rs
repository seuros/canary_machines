//! The `Data` classes, resolved once at load and pinned.
//!
//! A value read back must be an instance of exactly the gem's class: a
//! subclass constant leaks past the seal and `Data#initialize` can be bound
//! directly, so `is_a?` is not enough. `matryona_machines::ruby::exact_struct`
//! is that check; its fields are read straight from the struct (no reader
//! runs) and rebuilt through the core constructor, which `Marshal.load`
//! skipped.
use magnus::{Error, RModule, Ruby, prelude::*};
use matryona_machines::ruby::{Class, Registry};

/// Every `Data` class the binding checks or builds.
pub struct Classes {
    pub rate: Class,
    pub odds: Class,
    pub counts: Class,
    pub pairs: Class,
    pub reference: Class,
    pub paired: Class,
    pub interval: Class,
    pub fraction: Class,
    pub bounds: Class,
    pub plan: Class,
    pub look: Class,
    pub status: Class,
    pub recommendation: Class,
}

static CLASSES: Registry<Classes> = Registry::new("canary_machines");

/// Resolve the classes `canary_machines.rb` defined before requiring the
/// extension.
pub fn resolve(ruby: &Ruby, module: RModule) -> Result<&'static Classes, Error> {
    let model: RModule = module.const_get("Model")?;
    let class = |scope, name| Class::resolve(ruby, scope, name);
    let resolved = Classes {
        rate: class(module, "Rate")?,
        odds: class(module, "OddsRatio")?,
        counts: class(module, "Counts")?,
        pairs: class(module, "Pairs")?,
        reference: class(model, "Reference")?,
        paired: class(model, "Paired")?,
        interval: class(module, "Interval")?,
        fraction: class(module, "Fraction")?,
        bounds: class(module, "Bounds")?,
        plan: class(module, "Plan")?,
        look: class(module, "Look")?,
        status: class(module, "Status")?,
        recommendation: class(module, "Recommendation")?,
    };
    CLASSES.set(ruby, resolved)
}

/// The classes resolved at load.
pub fn classes(ruby: &Ruby) -> Result<&'static Classes, Error> {
    CLASSES.get(ruby)
}
