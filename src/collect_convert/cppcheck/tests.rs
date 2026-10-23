//! Unit tests for [`super`]: the reader on what Cppcheck 2.17.1 prints, and
//! on everything it must refuse.

use super::*;
use crate::collect_convert::Defect;

const RULES: &[Rule] = &[
    Rule {
        id: "nullPointerRedundantCheck",
        class: Defect::NullDereference,
        reason: "a dereference after a check",
        off: None,
    },
    Rule {
        id: "zerodiv",
        class: Defect::DivisionByZero,
        reason: "a division by zero",
        off: None,
    },
    Rule {
        id: "signConversion",
        class: Defect::Overflow,
        reason: "a sign conversion",
        off: Some("below the precision budget"),
    },
];

fn document(errors: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<results version=\"2\">\n    <cppcheck version=\"2.17.1\"/>\n    <errors>\n{errors}    </errors>\n</results>\n"
    )
}

#[test]
fn errors_of_reported_rules_at_their_first_location() {
    let xml = document(
        r#"        <error id="nullPointerRedundantCheck" severity="warning" msg="Either the condition &apos;p&apos; is redundant or there is possible null pointer dereference: p." verbose="v" cwe="476" file0="src/null.cpp">
            <location file="src/null.cpp" line="1" column="42" info="Null pointer dereference"/>
            <location file="src/other.cpp" line="9" column="23" info="Assuming that condition &apos;p&apos; is not redundant"/>
            <symbol>p</symbol>
        </error>
        <!-- a comment -->
        <error id="zerodiv" severity="error" msg="Division by zero &amp; &lt;more&gt; &quot;q&quot; &#65;&#x42;" file0="/src/lib/a.c">
            <location file="/src/lib/a.c" line="7" column="5"></location>
        </error>
        <error id="signConversion" severity="warning" msg="off by default">
            <location file="b.c" line="2" column="1"/>
        </error>
        <error id="syntaxError" severity="error" msg="not a listed rule">
            <location file="b.c" line="3" column="1"/>
        </error>
"#,
    );
    let found: Vec<Finding> = findings(&xml, RULES).unwrap().into_iter().collect();
    assert_eq!(found.len(), 2, "{found:?}");
    let zerodiv = &found[0];
    assert_eq!(
        (zerodiv.uri.as_str(), zerodiv.line, zerodiv.column),
        ("lib/a.c", 7, 5)
    );
    assert_eq!(zerodiv.level, "error");
    assert_eq!(zerodiv.message, "Division by zero & <more> \"q\" AB");
    let null = &found[1];
    assert_eq!(null.rule, "nullPointerRedundantCheck");
    assert_eq!(
        (null.uri.as_str(), null.line, null.column),
        ("src/null.cpp", 1, 42),
        "the first location"
    );
    assert_eq!(null.level, "warning");
    assert_eq!(
        null.message,
        "Either the condition 'p' is redundant or there is possible null pointer dereference: p."
    );
}

#[test]
fn severities_and_errors_without_a_location() {
    let rules: Vec<Rule> = ["a", "b", "c", "d", "e"]
        .into_iter()
        .map(|id| Rule {
            id,
            class: Defect::OutOfBounds,
            reason: "a listed id",
            off: None,
        })
        .collect();
    let xml = document(
        r#"<error id="a" severity="portability" msg="m"/>
<error id="b" severity="performance" msg="m"/>
<error id="c" severity="style" msg="m"/>
<error id="d" severity="error" msg="m"><location file="x.c" line="no" column=""/></error>
<error id="e" severity="information" msg="m">
</error>
"#,
    );
    let found: Vec<Finding> = findings(&xml, &rules).unwrap().into_iter().collect();
    let level = |rule: &str| {
        found
            .iter()
            .find(|finding| finding.rule == rule)
            .map(|finding| (finding.level, finding.uri.as_str(), finding.line))
    };
    assert_eq!(level("a"), Some(("warning", "", 0)));
    assert_eq!(level("b"), Some(("warning", "", 0)));
    assert_eq!(level("c"), Some(("note", "", 0)));
    assert_eq!(
        level("d"),
        Some(("error", "x.c", 0)),
        "an unreadable line is 0"
    );
    assert_eq!(level("e"), Some(("note", "", 0)));
    let location_outside = document(r#"<location file="x.c" line="1" column="1"/>"#);
    assert!(findings(&location_outside, &rules).unwrap().is_empty());
}

#[test]
fn the_wrappers_empty_report_has_no_findings() {
    let empty = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<results version=\"2\">\n    <errors>\n    </errors>\n</results>\n";
    assert!(findings(empty, RULES).unwrap().is_empty());
}

#[test]
fn anything_else_is_an_error_never_an_empty_report() {
    for (xml, error) in [
        ("", "Cppcheck printed no XML report"),
        (
            "cppcheck: error: no files",
            "Cppcheck printed no XML report",
        ),
        ("</results>", "Cppcheck printed no XML report"),
        ("<results>", "Cppcheck XML: version \"\", not 2"),
        (
            "<results version=\"1\"/>",
            "Cppcheck XML: version \"1\", not 2",
        ),
        (
            "<results version=\"2\"",
            "Cppcheck XML: an unterminated tag",
        ),
        ("<?xml version=\"1.0\"", "Cppcheck XML: an unterminated tag"),
        ("<!-- open", "Cppcheck XML: an unterminated tag"),
        ("< >", "Cppcheck XML: a tag without a name: < >"),
        (
            "<results version>",
            "Cppcheck XML: a malformed attribute in <results>",
        ),
        (
            "<results version='2'>",
            "Cppcheck XML: a malformed attribute in <results>",
        ),
        (
            "<results version=\"2>",
            "Cppcheck XML: an unterminated attribute in <results>",
        ),
        (
            "<results version=\"2\" a=\"&amp\">",
            "Cppcheck XML: an unterminated reference in \"&amp\"",
        ),
        (
            "<results version=\"2\" a=\"&nbsp;\">",
            "Cppcheck XML: unknown reference &nbsp;",
        ),
        (
            "<results version=\"2\" a=\"&#xD800;\">",
            "Cppcheck XML: unknown reference &#xD800;",
        ),
        (
            "<results version=\"2\" a=\"&#x;\">",
            "Cppcheck XML: unknown reference &#x;",
        ),
        (
            "<results version=\"2\" a=\"&#y;\">",
            "Cppcheck XML: unknown reference &#y;",
        ),
    ] {
        assert_eq!(findings(xml, RULES).unwrap_err(), error, "{xml}");
    }
}

#[test]
fn tags_attributes_and_references() {
    let tags = tags("<a x=\"1\"  y=\"&lt;2&gt;\" ><b/></a>text<?pi?>").unwrap();
    assert_eq!(
        tags,
        [
            Tag {
                name: "a",
                attributes: vec![("x", "1".to_owned()), ("y", "<2>".to_owned())],
                kind: Kind::Open,
            },
            Tag {
                name: "b",
                attributes: Vec::new(),
                kind: Kind::Empty,
            },
            Tag {
                name: "a",
                attributes: Vec::new(),
                kind: Kind::Close,
            },
        ]
    );
    assert_eq!(tags[0].attribute("y"), "<2>");
    assert_eq!(tags[0].attribute("z"), "", "absent");
    assert_eq!(unescape("&#10;&#x9;").unwrap(), "\n\t");
}
