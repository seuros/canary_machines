//! The core's unit-variant enums as Ruby symbols, both ways: compiler-checked
//! tables, so a variant added to the core does not build until it is listed.
use canary_machines::{Action, Guardrail, OnLimit, Outcome, Reason, Refusal};

matryona_machines::symbols! {
    /// The core's refusals: the snake case of each variant.
    pub Refusals for Refusal as "a refusal" {
        StaleGeneration => "stale_generation",
        TimeRegression => "time_regression",
        Full => "full",
        UnknownTest => "unknown_test",
        WrongModel => "wrong_model",
        Concluded => "concluded",
        Overflow => "overflow",
    }
}

matryona_machines::symbols! {
    /// What a plan does when its limit is reached undecided.
    pub OnLimits for OnLimit as "on_limit" {
        Hold => "hold",
        Rollback => "rollback",
        Promote => "promote",
    }
}

matryona_machines::symbols! {
    /// Where one sequential test stands.
    pub Outcomes for Outcome as "an outcome" {
        Undecided => "undecided",
        Cleared => "cleared",
        Regressed => "regressed",
        Truncated => "truncated",
    }
}

matryona_machines::symbols! {
    /// The guardrail verdict a recommendation weighs.
    pub Guardrails for Guardrail as "a guardrail" {
        Clear => "clear",
        NoData => "no_data",
        Breached => "breached",
    }
}

matryona_machines::symbols! {
    /// What a recommendation says to do.
    pub Actions for Action as "an action" {
        Continue => "continue",
        Promote => "promote",
        Rollback => "rollback",
        Hold => "hold",
    }
}

matryona_machines::symbols! {
    /// Why a recommendation says it.
    pub Reasons for Reason as "a reason" {
        Evidence => "evidence",
        Limit => "limit",
        Guardrail => "guardrail",
        GuardrailNoData => "guardrail_no_data",
        Sampling => "sampling",
        NoTests => "no_tests",
    }
}
