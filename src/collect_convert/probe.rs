//! Test support for the LX-CAR-1 converters: the synthetic probe files
//! (`tests/fixtures/fix-probe/`) and the carried fixes applied once.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

/// A path below `tests/fixtures/fix-probe`.
pub(super) fn probe(dir: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/fix-probe")
        .join(dir)
}

/// The SARIF document `sarif`.
pub(super) fn document(sarif: &str) -> Value {
    serde_json::from_str(sarif).unwrap()
}

/// The results of the first run.
pub(super) fn results(sarif: &Value) -> Vec<Value> {
    sarif["runs"][0]["results"].as_array().unwrap().clone()
}

/// The dropped fixes: (rule, path, reason).
pub(super) fn dropped(sarif: &Value) -> Vec<(String, String, String)> {
    sarif["runs"][0]["properties"]["sloptopop/droppedFixes"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|d| {
            (
                d["rule"].as_str().unwrap().to_owned(),
                d["path"].as_str().unwrap().to_owned(),
                d["reason"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

/// Every carried fix of `level`, applied once per file to the files below
/// `root`, in byte order.
pub(super) fn applied(sarif: &Value, root: &Path, level: &str) -> BTreeMap<String, Vec<u8>> {
    let mut edits: BTreeMap<String, Vec<(usize, usize, String)>> = BTreeMap::new();
    for result in results(sarif) {
        for fix in result["fixes"].as_array().into_iter().flatten() {
            if fix["properties"]["sloptopop/applicability"] != level {
                continue;
            }
            let change = &fix["artifactChanges"][0];
            let path = change["artifactLocation"]["uri"]
                .as_str()
                .unwrap()
                .to_owned();
            for replacement in change["replacements"].as_array().unwrap() {
                let region = &replacement["deletedRegion"];
                let start = usize::try_from(region["byteOffset"].as_u64().unwrap()).unwrap();
                let length = usize::try_from(region["byteLength"].as_u64().unwrap()).unwrap();
                let text = replacement["insertedContent"]["text"]
                    .as_str()
                    .unwrap_or_default()
                    .to_owned();
                edits
                    .entry(path.clone())
                    .or_default()
                    .push((start, start + length, text));
            }
        }
    }
    edits
        .into_iter()
        .map(|(path, mut list)| {
            list.sort();
            let bytes = fs::read(root.join(&path)).unwrap();
            let mut out = Vec::new();
            let mut last = 0;
            for (start, end, text) in list {
                assert!(start >= last, "{path}: overlapping edits");
                out.extend_from_slice(&bytes[last..start]);
                out.extend_from_slice(text.as_bytes());
                last = end;
            }
            out.extend_from_slice(&bytes[last..]);
            (path, out)
        })
        .collect()
}
