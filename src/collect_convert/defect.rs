//! The defect classes `collect` tags analyzer rules with, so a report says
//! what kind of wrong behaviour each rule finds, whatever the tool: the
//! taxonomy in `docs/collect.md`, keyed by CWE. A rule belongs to exactly
//! one class; class ids are stable and never reused.
//!
//! Standards: SCF TDA-17.1 (Static Code Analysis: every defect rule named with its
//! CWE class), MON-09 (deterministic records: the class is part of the
//! report).

/// A defect class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Defect {
    /// DC01: null or absent-value dereference (CWE-476, CWE-690).
    NullDereference,
    /// DC02: missing error or result check (CWE-252, CWE-253).
    UncheckedResult,
    /// DC03: resource leak: memory, file, handle (CWE-401, CWE-772, CWE-775).
    ResourceLeak,
    /// DC04: a lock not released on some path, such as an error or
    /// exception path (CWE-667, CWE-832).
    LockNotReleased,
    /// DC05: use after free, double free, dangling reference (CWE-416,
    /// CWE-415, CWE-825, CWE-562).
    UseAfterFree,
    /// DC06: out-of-bounds access (CWE-125, CWE-787, CWE-129).
    OutOfBounds,
    /// DC07: off-by-one, wrong bound comparison (CWE-193).
    OffByOne,
    /// DC08: integer overflow, wrap, truncation, sign error (CWE-190,
    /// CWE-191, CWE-197, CWE-681).
    Overflow,
    /// DC09: division or remainder by zero (CWE-369).
    DivisionByZero,
    /// DC10: read of an uninitialised value (CWE-457, CWE-908).
    Uninitialised,
    /// DC11: data race, missing synchronisation, check-then-act (CWE-362,
    /// CWE-366, CWE-367).
    DataRace,
    /// DC12: wrong lock order, deadlock, blocking while holding a lock
    /// (CWE-833).
    Deadlock,
    /// DC13: injection: SQL, OS command, path traversal, format string
    /// (CWE-89, CWE-78, CWE-88, CWE-22, CWE-134).
    Injection,
    /// DC14: an asynchronous result nobody observes: a floating promise, an
    /// unawaited coroutine or task, a discarded future (CWE-252, CWE-754).
    UnobservedAsync,
    /// DC15: a closure that captures a loop variable late (CWE-665 is the
    /// closest).
    LoopCapture,
    /// DC16: wrong operator, condition or call (CWE-480, CWE-481, CWE-570,
    /// CWE-571, CWE-563).
    WrongOperator,
    /// DC17: swallowed error, an empty handler (CWE-390).
    SwallowedError,
    /// DC18: unsafe shell expansion: word splitting and globbing, an
    /// empty variable in `rm -rf "$d/"`, an unchecked `cd` (CWE-78, CWE-88,
    /// CWE-73).
    ShellExpansion,
    /// DC19: a language trap: a mutable default argument, `==` coercion,
    /// `finally` overriding `return` (CWE-1188, CWE-597).
    LanguageTrap,
    /// DC20: a forced unwrap, cast or `try!` that crashes when it fails
    /// (CWE-617, CWE-248).
    ForcedCrash,
}

impl Defect {
    /// The class id, e.g. `DC01`.
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::NullDereference => "DC01",
            Self::UncheckedResult => "DC02",
            Self::ResourceLeak => "DC03",
            Self::LockNotReleased => "DC04",
            Self::UseAfterFree => "DC05",
            Self::OutOfBounds => "DC06",
            Self::OffByOne => "DC07",
            Self::Overflow => "DC08",
            Self::DivisionByZero => "DC09",
            Self::Uninitialised => "DC10",
            Self::DataRace => "DC11",
            Self::Deadlock => "DC12",
            Self::Injection => "DC13",
            Self::UnobservedAsync => "DC14",
            Self::LoopCapture => "DC15",
            Self::WrongOperator => "DC16",
            Self::SwallowedError => "DC17",
            Self::ShellExpansion => "DC18",
            Self::LanguageTrap => "DC19",
            Self::ForcedCrash => "DC20",
        }
    }
}

/// One rule of an analyzer that `collect` reports, with its class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rule {
    /// The tool's own id for it.
    pub(crate) id: &'static str,
    /// The defect class it finds.
    pub(crate) class: Defect,
    /// Why it is in the list: what it finds.
    pub(crate) reason: &'static str,
    /// If its findings are not reported by default, why (the precision
    /// study); `None` when they are.
    pub(crate) off: Option<&'static str>,
}

impl Rule {
    /// The rule `id` in `rules` if its findings are reported.
    pub(crate) fn reported<'r>(rules: &'r [Self], id: &str) -> Option<&'r Self> {
        rules
            .iter()
            .find(|rule| rule.id == id)
            .filter(|rule| rule.off.is_none())
    }
}

#[cfg(test)]
mod tests;
