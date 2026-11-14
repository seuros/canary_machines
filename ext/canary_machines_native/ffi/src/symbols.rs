//! The core's unit-variant enums as Ruby symbols, both ways.
use canary_machines::{Action, Guardrail, OnLimit, Outcome, Reason, Refusal};
use magnus::{Error, Ruby, Symbol, Value};

/// An enum whose variants travel as their `snake_case` symbol.
pub trait Named: Copy + 'static {
    /// Every variant; a parsed symbol names one of these.
    const ALL: &'static [Self];
    /// What the value is, for the messages of the errors it raises.
    const WHAT: &'static str;
    /// The variant's symbol name. An exhaustive match, so a new variant
    /// does not compile until it has one.
    fn name(self) -> &'static str;
}

/// `value` as its symbol.
pub fn symbol<T: Named>(ruby: &Ruby, value: T) -> Symbol {
    ruby.to_symbol(value.name())
}

/// The variant a symbol names: `TypeError` for anything but a Symbol,
/// `RangeError` for a symbol naming no variant.
pub fn parse<T: Named>(ruby: &Ruby, value: Value) -> Result<T, Error> {
    let symbol = Symbol::from_value(value).ok_or_else(|| {
        Error::new(
            ruby.exception_type_error(),
            format!("{} must be a Symbol", T::WHAT),
        )
    })?;
    let name = symbol.name()?;
    T::ALL
        .iter()
        .copied()
        .find(|variant| variant.name() == name)
        .ok_or_else(|| {
            let names: Vec<String> = T::ALL.iter().map(|v| format!(":{}", v.name())).collect();
            Error::new(
                ruby.exception_range_error(),
                format!("{} must be one of {}", T::WHAT, names.join(", ")),
            )
        })
}

impl Named for Refusal {
    const ALL: &'static [Self] = &[
        Self::StaleGeneration,
        Self::TimeRegression,
        Self::Full,
        Self::UnknownTest,
        Self::WrongModel,
        Self::Concluded,
        Self::Overflow,
    ];
    const WHAT: &'static str = "a refusal";

    fn name(self) -> &'static str {
        match self {
            Self::StaleGeneration => "stale_generation",
            Self::TimeRegression => "time_regression",
            Self::Full => "full",
            Self::UnknownTest => "unknown_test",
            Self::WrongModel => "wrong_model",
            Self::Concluded => "concluded",
            Self::Overflow => "overflow",
        }
    }
}

impl Named for OnLimit {
    const ALL: &'static [Self] = &[Self::Hold, Self::Rollback, Self::Promote];
    const WHAT: &'static str = "on_limit";

    fn name(self) -> &'static str {
        match self {
            Self::Hold => "hold",
            Self::Rollback => "rollback",
            Self::Promote => "promote",
        }
    }
}

impl Named for Outcome {
    const ALL: &'static [Self] = &[
        Self::Undecided,
        Self::Cleared,
        Self::Regressed,
        Self::Truncated,
    ];
    const WHAT: &'static str = "an outcome";

    fn name(self) -> &'static str {
        match self {
            Self::Undecided => "undecided",
            Self::Cleared => "cleared",
            Self::Regressed => "regressed",
            Self::Truncated => "truncated",
        }
    }
}

impl Named for Guardrail {
    const ALL: &'static [Self] = &[Self::Clear, Self::NoData, Self::Breached];
    const WHAT: &'static str = "a guardrail";

    fn name(self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::NoData => "no_data",
            Self::Breached => "breached",
        }
    }
}

impl Named for Action {
    const ALL: &'static [Self] = &[Self::Continue, Self::Promote, Self::Rollback, Self::Hold];
    const WHAT: &'static str = "an action";

    fn name(self) -> &'static str {
        match self {
            Self::Continue => "continue",
            Self::Promote => "promote",
            Self::Rollback => "rollback",
            Self::Hold => "hold",
        }
    }
}

impl Named for Reason {
    const ALL: &'static [Self] = &[
        Self::Evidence,
        Self::Limit,
        Self::Guardrail,
        Self::GuardrailNoData,
        Self::Sampling,
        Self::NoTests,
    ];
    const WHAT: &'static str = "a reason";

    fn name(self) -> &'static str {
        match self {
            Self::Evidence => "evidence",
            Self::Limit => "limit",
            Self::Guardrail => "guardrail",
            Self::GuardrailNoData => "guardrail_no_data",
            Self::Sampling => "sampling",
            Self::NoTests => "no_tests",
        }
    }
}
