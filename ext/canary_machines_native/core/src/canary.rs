//! A bounded set of tests for one rollout, and the recommendation they add up to.

use core::time::Duration;

use budget_machines::{Standing, State};

use crate::{Observation, OnLimit, Outcome, Plan, Refusal, Sprt, sprt::Look};

/// An external guardrail's verdict, passed in by the caller at each decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Guardrail {
    /// The guardrail is measured and healthy, or the caller has none.
    Clear,
    /// The guardrail has no data. Promotion is withheld; nothing is forced.
    NoData,
    /// The guardrail is breached. Rollback, whatever the evidence says.
    Breached,
}

impl From<State> for Guardrail {
    /// A firing burn-rate alert is a breach.
    fn from(state: State) -> Self {
        match state {
            State::NoData => Self::NoData,
            State::Quiet => Self::Clear,
            State::Firing => Self::Breached,
        }
    }
}

impl From<Standing> for Guardrail {
    /// An exhausted error budget is a breach.
    fn from(standing: Standing) -> Self {
        match standing {
            Standing::NoData => Self::NoData,
            Standing::Within { .. } => Self::Clear,
            Standing::Exhausted { .. } => Self::Breached,
        }
    }
}

/// What the caller is advised to do. Nothing here is ever executed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Keep sampling.
    Continue,
    /// Every test is satisfied.
    Promote,
    /// A test, a limit policy or the guardrail calls for rolling back.
    Rollback,
    /// Sampling is over, or pointless, without grounds to promote or roll back.
    Hold,
}

/// Why an [`Action`] was recommended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// Thresholds were crossed: a regression, or every test cleared.
    Evidence,
    /// A test hit its sample limit and its [`OnLimit`] policy decided.
    Limit,
    /// The guardrail is breached.
    Guardrail,
    /// The evidence would promote, but the guardrail has no data.
    GuardrailNoData,
    /// At least one test is still undecided.
    Sampling,
    /// No test is registered, so there is no evidence of anything.
    NoTests,
}

/// Advice for the rollout as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recommendation {
    action: Action,
    reason: Reason,
    test: Option<usize>,
}

impl Recommendation {
    /// A recommendation rebuilt from its fields, or `None` for a combination
    /// [`Canary::recommend`] never gives. A deciding test is named exactly
    /// for a regression, for a limit policy and for nothing else:
    /// `Rollback` for `Guardrail` (no test), `Evidence` or `Limit` (a test);
    /// `Continue` for `Sampling`; `Hold` for `NoTests` or `GuardrailNoData`
    /// (no test) or `Limit` (a test); `Promote` for `Evidence` (no test) or
    /// `Limit` (a test).
    #[must_use]
    pub const fn new(action: Action, reason: Reason, test: Option<usize>) -> Option<Self> {
        let named = test.is_some();
        let (rollback, carry_on, hold, promote) = match action {
            Action::Rollback => (true, false, false, false),
            Action::Continue => (false, true, false, false),
            Action::Hold => (false, false, true, false),
            Action::Promote => (false, false, false, true),
        };
        let valid = match reason {
            Reason::Evidence => (rollback && named) || (promote && !named),
            Reason::Limit => named && !carry_on,
            Reason::Guardrail => rollback && !named,
            Reason::Sampling => carry_on && !named,
            Reason::NoTests | Reason::GuardrailNoData => hold && !named,
        };
        if !valid {
            return None;
        }
        Some(Self {
            action,
            reason,
            test,
        })
    }

    /// What to do.
    #[must_use]
    pub const fn action(&self) -> Action {
        self.action
    }

    /// Why.
    #[must_use]
    pub const fn reason(&self) -> Reason {
        self.reason
    }

    /// The test that decided, when one did.
    #[must_use]
    pub const fn test(&self) -> Option<usize> {
        self.test
    }
}

/// One test's evidence and the instant it was decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    look: Look,
    decided_at: Option<Duration>,
}

impl Status {
    /// A status rebuilt from its fields, or `None` unless `decided_at` is
    /// present exactly when the look has latched an outcome.
    #[must_use]
    pub const fn new(look: Look, decided_at: Option<Duration>) -> Option<Self> {
        if look.is_decided() != decided_at.is_some() {
            return None;
        }
        Some(Self { look, decided_at })
    }

    /// Evidence as of the last accepted observation.
    #[must_use]
    pub const fn look(&self) -> Look {
        self.look
    }

    /// Instant the outcome latched; `None` while undecided.
    #[must_use]
    pub const fn decided_at(&self) -> Option<Duration> {
        self.decided_at
    }

    /// Whether the outcome has latched; see [`Look::is_decided`].
    #[must_use]
    pub const fn is_decided(&self) -> bool {
        self.look.is_decided()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Slot {
    sprt: Sprt,
    decided_at: Option<Duration>,
}

impl Slot {
    fn status(&self) -> Status {
        Status {
            look: self.sprt.look(),
            decided_at: self.decided_at,
        }
    }
}

/// Up to `M` sequential tests for one rollout generation.
///
/// The generation is an opaque number naming the rollout attempt; instants
/// are monotonic time since an epoch the caller picks. The canary has no
/// clock and takes no action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Canary<const M: usize> {
    generation: u64,
    last: Option<Duration>,
    len: usize,
    slots: [Option<Slot>; M],
}

impl<const M: usize> Canary<M> {
    /// An empty canary for `generation`.
    #[must_use]
    pub const fn new(generation: u64) -> Self {
        Self {
            generation,
            last: None,
            len: 0,
            slots: [None; M],
        }
    }

    /// Current rollout generation.
    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    /// Instant of the last accepted observation.
    #[must_use]
    pub const fn last(&self) -> Option<Duration> {
        self.last
    }

    /// Registered tests.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Whether no test is registered.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Tests the canary can hold.
    #[must_use]
    pub const fn capacity(&self) -> usize {
        M
    }

    /// Register a test and return its index.
    ///
    /// # Errors
    /// [`Refusal::Full`] when all `M` slots are taken.
    pub fn add(&mut self, plan: Plan) -> Result<usize, Refusal> {
        let index = self.len;
        let slot = self.slots.get_mut(index).ok_or(Refusal::Full)?;
        *slot = Some(Slot {
            sprt: Sprt::new(plan),
            decided_at: None,
        });
        self.len += 1;
        Ok(index)
    }

    /// The plan of test `index`.
    #[must_use]
    pub fn plan(&self, index: usize) -> Option<Plan> {
        self.slot(index).map(|slot| *slot.sprt.plan())
    }

    /// The evidence of test `index`.
    #[must_use]
    pub fn status(&self, index: usize) -> Option<Status> {
        self.slot(index).map(Slot::status)
    }

    /// Feed test `index` a batch observed at `at` and look once.
    ///
    /// # Errors
    /// [`Refusal::StaleGeneration`] when `generation` is not the current one,
    /// [`Refusal::TimeRegression`] when `at` is earlier than the last accepted
    /// observation, [`Refusal::UnknownTest`] for an unregistered index, and
    /// whatever [`Sprt::observe`] refuses. A refusal changes nothing.
    pub fn observe(
        &mut self,
        generation: u64,
        at: Duration,
        index: usize,
        observation: Observation,
    ) -> Result<Status, Refusal> {
        if generation != self.generation {
            return Err(Refusal::StaleGeneration);
        }
        if self.last.is_some_and(|last| at < last) {
            return Err(Refusal::TimeRegression);
        }
        let slot = self
            .slots
            .get_mut(index)
            .and_then(Option::as_mut)
            .ok_or(Refusal::UnknownTest)?;
        let look = slot.sprt.observe(observation)?;
        if look.is_decided() {
            slot.decided_at = Some(at);
        }
        self.last = Some(at);
        Ok(slot.status())
    }

    /// Start a new rollout attempt: every test keeps its plan and loses its
    /// evidence. Time keeps moving forward across generations.
    ///
    /// # Errors
    /// [`Refusal::StaleGeneration`] unless `generation` is greater than the
    /// current one.
    pub fn restart(&mut self, generation: u64) -> Result<(), Refusal> {
        if generation <= self.generation {
            return Err(Refusal::StaleGeneration);
        }
        self.generation = generation;
        for slot in self.slots.iter_mut().flatten() {
            slot.sprt.reset();
            slot.decided_at = None;
        }
        Ok(())
    }

    /// The advice the evidence and the guardrail add up to. Pure: asking does
    /// not change the canary, and nothing is ever acted on.
    ///
    /// In order: a breached guardrail, a regressed test, or a truncated test
    /// whose policy is [`OnLimit::Rollback`] recommends rollback. Otherwise an
    /// undecided test recommends continuing. Otherwise a truncated test whose
    /// policy is [`OnLimit::Hold`], a guardrail without data, or an empty
    /// canary recommends holding. Only what is left is a promotion.
    #[must_use]
    pub fn recommend(&self, guardrail: Guardrail) -> Recommendation {
        let advise = |action, reason, test| Recommendation {
            action,
            reason,
            test,
        };
        if guardrail == Guardrail::Breached {
            return advise(Action::Rollback, Reason::Guardrail, None);
        }
        let find = |outcome, policy: Option<OnLimit>| {
            self.slots.iter().flatten().position(|slot| {
                slot.sprt.outcome() == outcome
                    && policy.is_none_or(|policy| slot.sprt.plan().on_limit() == policy)
            })
        };
        if let Some(test) = find(Outcome::Regressed, None) {
            return advise(Action::Rollback, Reason::Evidence, Some(test));
        }
        if let Some(test) = find(Outcome::Truncated, Some(OnLimit::Rollback)) {
            return advise(Action::Rollback, Reason::Limit, Some(test));
        }
        if self.is_empty() {
            return advise(Action::Hold, Reason::NoTests, None);
        }
        if find(Outcome::Undecided, None).is_some() {
            return advise(Action::Continue, Reason::Sampling, None);
        }
        if let Some(test) = find(Outcome::Truncated, Some(OnLimit::Hold)) {
            return advise(Action::Hold, Reason::Limit, Some(test));
        }
        if guardrail == Guardrail::NoData {
            return advise(Action::Hold, Reason::GuardrailNoData, None);
        }
        find(Outcome::Truncated, Some(OnLimit::Promote)).map_or_else(
            || advise(Action::Promote, Reason::Evidence, None),
            |test| advise(Action::Promote, Reason::Limit, Some(test)),
        )
    }

    fn slot(&self, index: usize) -> Option<&Slot> {
        self.slots.get(index).and_then(Option::as_ref)
    }
}
