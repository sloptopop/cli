//! Converting the output of analyzers that write no SARIF (`OpenSSF`
//! Scorecard, `PHP_CodeSniffer`, Pylint, dependency-cruiser, clang-tidy,
//! Cppcheck, `ShellCheck`, rustc and Clippy), or
//! whose fixes only another format carries (Ruff, `ESLint`, Biome, Opengrep
//! and Semgrep), into SARIF 2.1.0, which is all the engine reads. The
//! conversion keeps each tool's own rule id and message, maps its severity
//! to a SARIF level, makes paths relative to the repository root, keeps the
//! tool's own fixes in the byte-region carrier ([`fix`]), and is
//! deterministic: results sorted, duplicates dropped, nothing time- or
//! machine-dependent. For the C, C++ and shell analyzers it keeps only the rules
//! the catalogue lists as reported, tags each with its defect class
//! ([`defect`]), and records how the files were compiled: without the
//! project's compile database, every result is marked lower confidence.
//!
//! Standards: SCF TDA-12 (Input Data Validation: unreadable output is an
//! error, never an empty report), MON-09 (deterministic records).

mod cppcheck;
mod defect;
mod fix;
mod native;
mod reviewed;
mod rustc;
mod shellcheck;
mod tidy;
mod yaml;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_json::{Map, Value, json};

pub(crate) use defect::{Defect, Rule};
pub(crate) use fix::Columns;
use fix::{Drop, Edit, Files, Fix, Level, is_carrier, one_per_level};

/// Where the repository is mounted in the container (absolute paths below
/// it become relative).
const ROOT: &str = "/src/";

/// A format `collect` converts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Converter {
    /// `OpenSSF` Scorecard's JSON: a License check that found no licence
    /// file becomes one result about the repository.
    Scorecard,
    /// `PHP_CodeSniffer`'s JSON report.
    Phpcs,
    /// Pylint's `json2` output.
    Pylint,
    /// dependency-cruiser's JSON: each rule violation.
    Depcruise,
    /// clang-tidy's diagnostics (`file:line:col: level: message [check]`)
    /// of the reported checks in the list, with the fixes it exported for
    /// the reviewed ones.
    ClangTidy(&'static [Rule]),
    /// Cppcheck's XML version 2 report: the errors of the reported ids in
    /// the list.
    Cppcheck(&'static [Rule]),
    /// Ruff's JSON (`--output-format json`): findings with their fixes.
    RuffJson,
    /// `ESLint`'s `json-with-metadata`: messages with their autofix and
    /// suggestions.
    EslintJson,
    /// Biome's rdjson: diagnostics with their code suggestions.
    BiomeRdjson,
    /// Opengrep's or Semgrep's JSON: results with their rule's fix.
    SemgrepJson,
    /// `ShellCheck`'s `json1` reports: the comments of the reported rules in
    /// the list, with their fixes.
    ShellcheckJson1(&'static [Rule]),
    /// rustc's and Clippy's JSON diagnostics: findings with their
    /// machine-applicable suggestions.
    RustcJson,
}

/// One SARIF result, ordered for a deterministic report.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Finding {
    uri: String,
    line: u64,
    column: u64,
    rule: String,
    level: &'static str,
    message: String,
    fixes: Vec<Fix>,
}

/// What a converted report says beyond the tool's own output.
#[derive(Debug, Clone, Copy, Default)]
struct Notes<'a> {
    /// The rules, for their defect classes.
    rules: &'a [Rule],
    /// For the C and C++ analyzers: how the files were compiled, as the
    /// image's wrapper stated it on its first line.
    flags: Option<&'a str>,
    /// The errors the tool reported besides its findings (sorted, unique):
    /// for clang-tidy, the files that did not compile and the files it
    /// could not process. They are kept in the report as the invocation's
    /// tool execution notifications, never as results.
    errors: &'a [Finding],
}

impl Notes<'_> {
    /// Whether the results are of lower confidence: the files were not
    /// compiled with the project's own compile database.
    fn lower_confidence(&self) -> bool {
        self.flags
            .is_some_and(|flags| !flags.starts_with("compile database "))
    }
}

/// The C/C++ wrapper's first line, `# sloptopop-<tool>: <how the files
/// were compiled>`, split from the rest of `text`; "not stated" without
/// one.
fn compile_flags(text: &str) -> (&str, &str) {
    let (first, rest) = text.split_once('\n').unwrap_or((text, ""));
    first
        .strip_prefix("# sloptopop-")
        .and_then(|line| line.split_once(": "))
        .map_or(("not stated", text), |(_, flags)| (flags, rest))
}

/// `path` relative to the repository root, with `/` separators.
fn relative(path: &str) -> String {
    let path = path.strip_prefix(ROOT).unwrap_or(path);
    path.strip_prefix("./").unwrap_or(path).to_owned()
}

/// Converts `raw`, the output of `tool` at `version` on the repository at
/// `root`, to a SARIF document.
pub(crate) fn convert(
    converter: Converter,
    raw: &[u8],
    tool: &str,
    version: &str,
    root: &Path,
) -> Result<String, String> {
    let json = || -> Result<Value, String> {
        serde_json::from_slice(raw).map_err(|e| format!("{tool} printed no readable JSON: {e}"))
    };
    let context = native::Context {
        tool,
        version,
        root,
    };
    let mut store = Files::new(root);
    let text = String::from_utf8_lossy(raw);
    let mut notes = Notes::default();
    let mut errors = Vec::new();
    let findings = match converter {
        Converter::ClangTidy(rules) => {
            let (flags, body) = compile_flags(&text);
            notes = Notes {
                rules,
                flags: Some(flags),
                errors: &[],
            };
            let (diagnostics, exported) = tidy::split(body);
            let (found, failed) = clang_tidy(diagnostics, rules);
            errors.extend(failed);
            tidy::attach(found, exported, rules, &context, &mut store)
        }
        Converter::Cppcheck(rules) => {
            let (flags, body) = compile_flags(&text);
            notes = Notes {
                rules,
                flags: Some(flags),
                errors: &[],
            };
            cppcheck::findings(body, rules)?
        }
        Converter::Scorecard => scorecard(&json()?)?,
        Converter::Phpcs => phpcs(&json()?)?,
        Converter::Pylint => pylint(&json()?)?,
        Converter::Depcruise => depcruise(&json()?)?,
        Converter::RuffJson => native::ruff(&json()?, &context, &mut store)?,
        Converter::EslintJson => native::eslint(&json()?, &context, &mut store)?,
        Converter::BiomeRdjson => native::biome(&json()?, &context, &mut store)?,
        Converter::SemgrepJson => native::semgrep(&json()?, &context, &mut store)?,
        Converter::ShellcheckJson1(rules) => {
            notes.rules = rules;
            shellcheck::findings(&text, rules, &context, &mut store)?
        }
        Converter::RustcJson => rustc::findings(&text, &context, &mut store)?,
    };
    let notes = Notes {
        errors: &errors,
        ..notes
    };
    Ok(sarif(tool, version, &findings, &store, notes))
}

/// The SARIF location of `finding`, if it names a file.
fn location(finding: &Finding) -> Option<Value> {
    if finding.uri.is_empty() {
        return None;
    }
    let mut region = Map::new();
    region.insert("startLine".to_owned(), finding.line.max(1).into());
    if finding.column > 0 {
        region.insert("startColumn".to_owned(), finding.column.into());
    }
    Some(json!([{
        "physicalLocation": {
            "artifactLocation": { "uri": finding.uri },
            "region": region,
        }
    }]))
}

/// The tool execution notification of one of the tool's `error`s.
fn notification(error: &Finding) -> Value {
    let mut notification = Map::new();
    notification.insert("level".to_owned(), error.level.into());
    notification.insert("message".to_owned(), json!({ "text": error.message }));
    if !error.rule.is_empty() {
        notification.insert("descriptor".to_owned(), json!({ "id": error.rule }));
    }
    if let Some(locations) = location(error) {
        notification.insert("locations".to_owned(), locations);
    }
    Value::Object(notification)
}

/// The SARIF 2.1.0 document holding `findings` (sorted, unique), with the
/// files their fixes name, the fixes `store` dropped, and the `notes`.
fn sarif(
    tool: &str,
    version: &str,
    findings: &BTreeSet<Finding>,
    store: &Files<'_>,
    notes: Notes<'_>,
) -> String {
    let rules: BTreeSet<&str> = findings.iter().map(|f| f.rule.as_str()).collect();
    let lower = notes.lower_confidence();
    let results: Vec<Value> = findings
        .iter()
        .map(|f| {
            let mut result = Map::new();
            result.insert("ruleId".to_owned(), f.rule.clone().into());
            result.insert("level".to_owned(), f.level.into());
            result.insert("message".to_owned(), json!({ "text": f.message }));
            if let Some(locations) = location(f) {
                result.insert("locations".to_owned(), locations);
            }
            if !f.fixes.is_empty() {
                result.insert("fixes".to_owned(), f.fixes.iter().map(Fix::sarif).collect());
            }
            if lower {
                result.insert("properties".to_owned(), json!({ "confidence": "lower" }));
            }
            Value::Object(result)
        })
        .collect();
    let mut run = Map::new();
    run.insert(
        "tool".to_owned(),
        json!({ "driver": {
            "name": tool,
            "version": version,
            "rules": rules.iter().map(|id| rule(id, notes.rules)).collect::<Vec<_>>(),
        }}),
    );
    if let Some(flags) = notes.flags {
        run.insert("properties".to_owned(), json!({ "compileFlags": flags }));
    }
    if !notes.errors.is_empty() {
        run.insert(
            "invocations".to_owned(),
            json!([{
                "executionSuccessful": true,
                "toolExecutionNotifications":
                    notes.errors.iter().map(notification).collect::<Vec<_>>(),
            }]),
        );
    }
    run.insert("results".to_owned(), Value::Array(results));
    store.record(&mut run);
    let document = json!({
        "$schema": "https://json.schemastore.org/sarif-2.1.0.json",
        "version": "2.1.0",
        "runs": [Value::Object(run)],
    });
    format!("{document:#}\n")
}

/// The SARIF rule `id`, with its defect class if `rules` lists it.
fn rule(id: &str, rules: &[Rule]) -> Value {
    let mut rule = Map::new();
    rule.insert("id".to_owned(), id.into());
    if let Some(listed) = rules.iter().find(|listed| listed.id == id) {
        rule.insert(
            "properties".to_owned(),
            json!({ "defectClass": listed.class.id() }),
        );
    }
    Value::Object(rule)
}

/// The SARIF log `raw` that `tool` (at `version`) wrote, made comparable
/// from run to run and machine to machine, whatever the tool:
/// - paths under the container's `/src` become relative to the repository
///   root (and base URIs naming `/src` are dropped with their uses);
/// - the invocations' start and end times are dropped;
/// - the rules are sorted by id, the results' rule indexes following them;
/// - the rules' help texts are dropped: some tools (OSV-Scanner) print
///   their tables in a random order, and the engine reads only the short
///   description.
///
/// - a result's fixes are kept only in the byte-region carrier: sloptopop's
///   own carriers stay; a fix the tool wrote is converted when the tool's
///   column unit is known (`native`), and dropped otherwise (a SARIF
///   producer's `fixes` use text regions whose unit it often misstates).
///
/// - the results of a tool that writes them in no fixed order
///   (`unordered`) are sorted ([`sort_results`]);
///
/// - the rules `defects` lists get their defect class
///   (`properties.defectClass`), and the results of those that are off
///   are dropped ([`classify`]).
///
/// Everything else is kept. No output at all (OSV-Scanner finding no
/// packages) is a log without results.
pub(crate) fn normalize(
    raw: &[u8],
    tool: &str,
    version: &str,
    native: Option<Columns>,
    defects: &[Rule],
    unordered: bool,
    root: &Path,
) -> Result<String, String> {
    if raw.iter().all(u8::is_ascii_whitespace) {
        return Ok(sarif(
            tool,
            version,
            &BTreeSet::new(),
            &Files::new(root),
            Notes::default(),
        ));
    }
    let mut document: Value =
        serde_json::from_slice(raw).map_err(|e| format!("{tool} wrote no readable SARIF: {e}"))?;
    let runs = document
        .get_mut("runs")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| format!("{tool} SARIF: no runs array"))?;
    let strip = |rules: Option<&mut Value>| {
        for rule in rules.and_then(Value::as_array_mut).into_iter().flatten() {
            if let Some(rule) = rule.as_object_mut() {
                rule.remove("help");
                // The C# compiler's per-rule analyzer timings.
                if let Some(properties) = rule.get_mut("properties").and_then(Value::as_object_mut)
                {
                    properties.remove("executionTimeInSeconds");
                    properties.remove("executionTimeInPercentage");
                }
            }
        }
    };
    for run in runs.iter_mut() {
        sort_rules(run);
        for invocation in run
            .get_mut("invocations")
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            if let Some(invocation) = invocation.as_object_mut() {
                invocation.remove("startTimeUtc");
                invocation.remove("endTimeUtc");
            }
        }
        if let Some(properties) = run.get_mut("properties").and_then(Value::as_object_mut) {
            properties.remove("analyzerExecutionTime");
        }
        strip(run.pointer_mut("/tool/driver/rules"));
        for extension in run
            .pointer_mut("/tool/extensions")
            .and_then(Value::as_array_mut)
            .into_iter()
            .flatten()
        {
            strip(extension.get_mut("rules"));
        }
        let bases = drop_root_bases(run);
        relativize(run, &bases);
        carry_native(run, native, &mut Files::new(root));
        classify(run, defects);
        if unordered {
            sort_results(run);
        }
    }
    Ok(format!("{document:#}\n"))
}

/// Gives the descriptor of every rule `defects` lists (in the driver or an
/// extension) its defect class, `properties.defectClass`, beside any
/// properties the tool wrote, and drops the results of the listed rules
/// that are off. A listed rule that is on and has results but no
/// descriptor (golangci-lint writes none) gets one in the driver, so its
/// class shows. A rule `defects` does not list is left as the tool wrote
/// it.
fn classify(run: &mut Value, defects: &[Rule]) {
    let listed = |id: &str| defects.iter().find(|rule| rule.id == id);
    let described: BTreeSet<String> = run
        .pointer("/tool/driver/rules")
        .into_iter()
        .chain(
            run.pointer("/tool/extensions")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|extension| extension.get("rules")),
        )
        .filter_map(Value::as_array)
        .flatten()
        .map(|descriptor| text(descriptor, "id"))
        .collect();
    let missing: BTreeSet<&str> = run
        .get("results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|result| listed(rule_id(result)))
        .filter(|rule| rule.off.is_none() && !described.contains(rule.id))
        .map(|rule| rule.id)
        .collect();
    if !missing.is_empty() {
        if let Some(driver) = run
            .pointer_mut("/tool/driver")
            .and_then(Value::as_object_mut)
            && let Some(rules) = driver
                .entry("rules")
                .or_insert_with(|| json!([]))
                .as_array_mut()
        {
            rules.extend(missing.into_iter().map(|id| json!({ "id": id })));
        }
        sort_rules(run);
    }
    let tag = |rules: Option<&mut Value>| {
        for descriptor in rules.and_then(Value::as_array_mut).into_iter().flatten() {
            let class = listed(&text(descriptor, "id")).map(|rule| rule.class.id());
            if let (Some(class), Some(object)) = (class, descriptor.as_object_mut()) {
                let properties = object
                    .entry("properties")
                    .or_insert_with(|| Value::Object(Map::new()));
                if let Some(properties) = properties.as_object_mut() {
                    properties.insert("defectClass".to_owned(), class.into());
                }
            }
        }
    };
    tag(run.pointer_mut("/tool/driver/rules"));
    for extension in run
        .pointer_mut("/tool/extensions")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        tag(extension.get_mut("rules"));
    }
    if let Some(results) = run.get_mut("results").and_then(Value::as_array_mut) {
        results.retain(|result| listed(rule_id(result)).is_none_or(|rule| rule.off.is_none()));
    }
}

/// Sorts `run`'s results by their first location (file, line, column),
/// then rule and message; results equal in all of these keep the tool's
/// order. The C# compiler writes its log in the order its concurrent
/// analyzers finish, so the same build gives the same results in another
/// order.
fn sort_results(run: &mut Value) {
    if let Some(results) = run.get_mut("results").and_then(Value::as_array_mut) {
        results.sort_by_cached_key(|result| {
            let location = result
                .pointer("/locations/0/physicalLocation")
                .unwrap_or(&Value::Null);
            (
                location
                    .pointer("/artifactLocation/uri")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                location
                    .pointer("/region/startLine")
                    .and_then(Value::as_u64)
                    .unwrap_or_default(),
                location
                    .pointer("/region/startColumn")
                    .and_then(Value::as_u64)
                    .unwrap_or_default(),
                rule_id(result).to_owned(),
                result
                    .pointer("/message/text")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            )
        });
    }
}

/// The id of the rule `result` reports (`ruleId`, else `rule.id`), or "".
fn rule_id(result: &Value) -> &str {
    result
        .get("ruleId")
        .or_else(|| result.pointer("/rule/id"))
        .and_then(Value::as_str)
        .unwrap_or_default()
}

/// Puts every tool-written fix of `run`'s results into the carrier (or
/// drops it), and records the files and drops in `run`.
fn carry_native(run: &mut Value, native: Option<Columns>, store: &mut Files<'_>) {
    for result in run
        .get_mut("results")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        let Some(result) = result.as_object_mut() else {
            continue;
        };
        let written = match result.get("fixes").and_then(Value::as_array) {
            Some(listed) if !listed.iter().all(is_carrier) => listed.clone(),
            Some(_) | None => continue,
        };
        result.remove("fixes");
        let rule = result
            .get("ruleId")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let carried: Vec<Fix> = written
            .iter()
            .filter_map(|fix| native_fix(fix, &rule, native, store))
            .collect();
        let kept = one_per_level(store, &rule, carried);
        if !kept.is_empty() {
            result.insert("fixes".to_owned(), kept.iter().map(Fix::sarif).collect());
        }
    }
    if let Value::Object(run) = run {
        store.record(run);
    }
}

/// A fix a SARIF producer wrote (text regions), in the carrier, if its
/// column unit is known and it converts.
fn native_fix(
    fix: &Value,
    rule: &str,
    native: Option<Columns>,
    store: &mut Files<'_>,
) -> Option<Fix> {
    let changes = fix.get("artifactChanges").map_or(&[][..], items);
    let path = changes
        .first()
        .and_then(|change| change.pointer("/artifactLocation/uri"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    let drop = match (native, changes.len()) {
        (_, 0) => Some(Drop::Placeholder),
        (_, 2..) => Some(Drop::MultiFile),
        (None, _) => Some(Drop::Unconverted),
        (Some(_), _) if path.is_empty() => Some(Drop::Placeholder),
        (Some(_), _) => None,
    };
    if let Some(drop) = drop {
        store.drop(rule, path, drop);
        return None;
    }
    let columns = native.unwrap_or(Columns::Utf8Bytes);
    let replacements = changes
        .first()
        .and_then(|change| change.get("replacements"))
        .map_or(&[][..], items);
    store.keep(rule, path, |source| {
        let edits = replacements
            .iter()
            .map(|replacement| {
                let region = replacement.get("deletedRegion").unwrap_or(&Value::Null);
                Ok(Edit {
                    start: source.line_column(
                        columns,
                        number(region, "startLine"),
                        number(region, "startColumn"),
                    )?,
                    end: source.line_column(
                        columns,
                        number(region, "endLine"),
                        number(region, "endColumn"),
                    )?,
                    text: replacement
                        .pointer("/insertedContent/text")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                })
            })
            .collect::<Result<Vec<_>, Drop>>()?;
        Ok(Fix {
            level: Level::Suggestion,
            tool_level: "sarif".to_owned(),
            description: fix
                .pointer("/description/text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            source: "sarif",
            path: path.to_owned(),
            edits,
            reviewed: false,
        })
    })
}

/// `uri` relative to the repository root, if it names the container's
/// `/src` or a path below it.
fn below_root(uri: &str) -> Option<&str> {
    ["file:///src/", "/src/"]
        .iter()
        .find_map(|prefix| uri.strip_prefix(prefix))
        .or_else(|| ["file:///src", "/src"].contains(&uri).then_some(""))
}

/// Drops the run's `originalUriBaseIds` that name `/src` (or below it) and
/// returns their names.
fn drop_root_bases(run: &mut Value) -> BTreeSet<String> {
    let Some(bases) = run
        .get_mut("originalUriBaseIds")
        .and_then(Value::as_object_mut)
    else {
        return BTreeSet::new();
    };
    let dropped: BTreeSet<String> = bases
        .iter()
        .filter(|(_, base)| below_root(&text(base, "uri")).is_some())
        .map(|(name, _)| name.clone())
        .collect();
    bases.retain(|name, _| !dropped.contains(name));
    dropped
}

/// Rewrites every `uri` below `/src` as a relative path ("." for the root
/// itself), and drops each `uriBaseId` naming one of the dropped `bases`.
fn relativize(value: &mut Value, bases: &BTreeSet<String>) {
    match value {
        Value::Object(object) => {
            if object
                .get("uriBaseId")
                .and_then(Value::as_str)
                .is_some_and(|base| bases.contains(base))
            {
                object.remove("uriBaseId");
            }
            if let Some(Value::String(uri)) = object.get_mut("uri")
                && let Some(relative) = below_root(uri).map(str::to_owned)
            {
                *uri = if relative.is_empty() {
                    ".".to_owned()
                } else {
                    relative
                };
            }
            for child in object.values_mut() {
                relativize(child, bases);
            }
        }
        Value::Array(items) => {
            for item in items {
                relativize(item, bases);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

/// Sorts the driver's rules by id and points every result's `ruleIndex`
/// (and `rule.index`) at the rule's new place.
fn sort_rules(run: &mut Value) {
    let Some(rules) = run
        .pointer_mut("/tool/driver/rules")
        .and_then(Value::as_array_mut)
    else {
        return;
    };
    let mut order: Vec<(usize, Value)> = rules.drain(..).enumerate().collect();
    order.sort_by_key(|(_, rule)| text(rule, "id"));
    let moved: BTreeMap<u64, u64> = order
        .iter()
        .enumerate()
        .map(|(new, (old, _))| {
            (
                u64::try_from(*old).unwrap_or(u64::MAX),
                u64::try_from(new).unwrap_or(u64::MAX),
            )
        })
        .collect();
    rules.extend(order.into_iter().map(|(_, rule)| rule));
    let remap = |index: &mut Value| {
        let mapped = index.as_u64().and_then(|old| moved.get(&old));
        if let Some(new) = mapped {
            *index = (*new).into();
        }
    };
    for result in run
        .get_mut("results")
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
    {
        if let Some(index) = result.get_mut("ruleIndex") {
            remap(index);
        }
        if let Some(index) = result.pointer_mut("/rule/index") {
            remap(index);
        }
    }
}

fn text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

fn number(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(Value::as_u64).unwrap_or_default()
}

/// The items of `value` if it is an array, else none.
fn items(value: &Value) -> &[Value] {
    value.as_array().map_or(&[][..], Vec::as_slice)
}

fn array<'v>(value: &'v Value, key: &str, what: &str) -> Result<&'v Vec<Value>, String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .ok_or_else(|| format!("{what}: no {key} array"))
}

/// Scorecard: only "license file not detected" (score 0) is a finding.
fn scorecard(document: &Value) -> Result<BTreeSet<Finding>, String> {
    Ok(array(document, "checks", "Scorecard JSON")?
        .iter()
        .filter(|check| {
            text(check, "name") == "License"
                && check.get("score").and_then(Value::as_i64) == Some(0)
        })
        .map(|check| Finding {
            uri: String::new(),
            line: 0,
            column: 0,
            rule: "License".to_owned(),
            level: "warning",
            message: text(check, "reason"),
            fixes: Vec::new(),
        })
        .collect())
}

/// `PHP_CodeSniffer`: every message of every file.
fn phpcs(document: &Value) -> Result<BTreeSet<Finding>, String> {
    let files = document
        .get("files")
        .and_then(Value::as_object)
        .ok_or_else(|| "PHP_CodeSniffer JSON: no files object".to_owned())?;
    let mut findings = BTreeSet::new();
    for (path, file) in files {
        for message in array(file, "messages", "PHP_CodeSniffer JSON")? {
            findings.insert(Finding {
                uri: relative(path),
                line: number(message, "line"),
                column: number(message, "column"),
                rule: text(message, "source"),
                level: if text(message, "type") == "ERROR" {
                    "error"
                } else {
                    "warning"
                },
                message: text(message, "message"),
                fixes: Vec::new(),
            });
        }
    }
    Ok(findings)
}

/// Pylint: every message; refactor, convention and warning messages are
/// warnings, error and fatal ones errors, anything else a note.
fn pylint(document: &Value) -> Result<BTreeSet<Finding>, String> {
    Ok(array(document, "messages", "Pylint JSON")?
        .iter()
        .map(|message| Finding {
            uri: relative(&text(message, "path")),
            line: number(message, "line"),
            column: number(message, "column") + 1,
            rule: text(message, "symbol"),
            level: match text(message, "type").as_str() {
                "error" | "fatal" => "error",
                "warning" | "refactor" | "convention" => "warning",
                _ => "note",
            },
            message: text(message, "message"),
            fixes: Vec::new(),
        })
        .collect())
}

/// dependency-cruiser: every rule violation, at its `from` module.
fn depcruise(document: &Value) -> Result<BTreeSet<Finding>, String> {
    let summary = document
        .get("summary")
        .ok_or_else(|| "dependency-cruiser JSON: no summary".to_owned())?;
    Ok(array(summary, "violations", "dependency-cruiser JSON")?
        .iter()
        .map(|violation| {
            let rule = violation.get("rule").cloned().unwrap_or(Value::Null);
            let (from, to) = (text(violation, "from"), text(violation, "to"));
            let cycle: Vec<String> = violation
                .get("cycle")
                .and_then(Value::as_array)
                .map(|steps| {
                    steps
                        .iter()
                        .map(|step| {
                            step.as_str()
                                .map_or_else(|| text(step, "name"), str::to_owned)
                        })
                        .collect()
                })
                .unwrap_or_default();
            let message = if cycle.is_empty() {
                format!("{from} → {to}")
            } else {
                format!("{from} → {}", cycle.join(" → "))
            };
            Finding {
                uri: relative(&from),
                line: 1,
                column: 0,
                rule: text(&rule, "name"),
                level: match text(&rule, "severity").as_str() {
                    "error" => "error",
                    "warn" => "warning",
                    _ => "note",
                },
                message,
                fixes: Vec::new(),
            }
        })
        .collect())
}

/// clang-tidy: every diagnostic of a reported check in `rules` (compiler
/// diagnostics, `clang-diagnostic-*`, are not findings), and, apart, its
/// errors: the `error` diagnostics of no reported check (a file that does
/// not compile: `clang-diagnostic-error`) and the lines where it says it
/// could not process a file (`Error while processing …`, or an `error:`
/// without a location). Compiler warnings are neither.
fn clang_tidy(output: &str, rules: &[Rule]) -> (BTreeSet<Finding>, BTreeSet<Finding>) {
    let mut found = BTreeSet::new();
    let mut errors = BTreeSet::new();
    for line in output.lines() {
        match diagnostic(line) {
            Some(finding) if Rule::reported(rules, &finding.rule).is_some() => {
                found.insert(finding);
            }
            Some(finding) => {
                if finding.level == "error" {
                    errors.insert(finding);
                }
            }
            None => errors.extend(tool_error(line)),
        }
    }
    (found, errors)
}

/// A line saying clang-tidy could not do its work, with no location:
/// `Error while processing <file>.`, `error: …` (an argument it cannot use,
/// or the wrapper's `error: no such file: …` for a file the compile
/// database lists), or `LLVM ERROR: …` (it stopped).
fn tool_error(line: &str) -> Option<Finding> {
    let message = line.trim_end();
    ["Error while processing ", "error: ", "LLVM ERROR: "]
        .iter()
        .any(|prefix| message.starts_with(prefix))
        .then(|| Finding {
            uri: String::new(),
            line: 0,
            column: 0,
            rule: String::new(),
            level: "error",
            message: message.to_owned(),
            fixes: Vec::new(),
        })
}

fn diagnostic(line: &str) -> Option<Finding> {
    let (rest, checks) = line.strip_suffix(']')?.rsplit_once(" [")?;
    let (location, message) = rest
        .split_once(": warning: ")
        .map(|(l, m)| (l, (m, "warning")))
        .or_else(|| rest.split_once(": error: ").map(|(l, m)| (l, (m, "error"))))?;
    let (file_line, column) = location.rsplit_once(':')?;
    let (path, line_number) = file_line.rsplit_once(':')?;
    let column = column.parse().ok()?;
    let line_number = line_number.parse().ok()?;
    Some(Finding {
        uri: relative(path),
        line: line_number,
        column,
        rule: checks.split(',').next().unwrap_or(checks).to_owned(),
        level: message.1,
        message: message.0.to_owned(),
        fixes: Vec::new(),
    })
}

#[cfg(test)]
mod probe;
#[cfg(test)]
mod tests;
