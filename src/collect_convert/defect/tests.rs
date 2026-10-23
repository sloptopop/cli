//! Unit tests for [`super`].

use super::*;

#[test]
fn class_ids_are_distinct_and_well_formed() {
    let all = [
        Defect::NullDereference,
        Defect::UncheckedResult,
        Defect::ResourceLeak,
        Defect::LockNotReleased,
        Defect::UseAfterFree,
        Defect::OutOfBounds,
        Defect::OffByOne,
        Defect::Overflow,
        Defect::DivisionByZero,
        Defect::Uninitialised,
        Defect::DataRace,
        Defect::Deadlock,
        Defect::Injection,
        Defect::UnobservedAsync,
        Defect::LoopCapture,
        Defect::WrongOperator,
        Defect::SwallowedError,
        Defect::LanguageTrap,
        Defect::ForcedCrash,
    ];
    let ids: Vec<&str> = all.iter().map(|class| class.id()).collect();
    assert_eq!(
        ids,
        [
            "DC01", "DC02", "DC03", "DC04", "DC05", "DC06", "DC07", "DC08", "DC09", "DC10", "DC11",
            "DC12", "DC13", "DC14", "DC15", "DC16", "DC17", "DC19", "DC20"
        ],
        "ids are stable and in class order"
    );
}

#[test]
fn only_rules_that_are_on_are_reported() {
    let rules = [
        Rule {
            id: "a",
            class: Defect::NullDereference,
            reason: "finds a",
            off: None,
        },
        Rule {
            id: "b",
            class: Defect::Overflow,
            reason: "finds b",
            off: Some("below the precision budget"),
        },
    ];
    assert_eq!(
        Rule::reported(&rules, "a").map(|rule| rule.class),
        Some(Defect::NullDereference)
    );
    assert_eq!(Rule::reported(&rules, "b"), None, "off");
    assert_eq!(Rule::reported(&rules, "c"), None, "not listed");
}
