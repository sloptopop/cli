# Report format (schema version 2)

The public contract between the sloptopop server (`sloptopop-engine`,
where the Detector runs), the open-source `sloptopop` CLI and the language
libraries. Schema version 2 (SPLIT, 2026-10-21) adds the versions and
digests that make every Report reproducible, and lists every tool behind a
finding.

`sloptopop check --format json` prints one JSON document:

```json
{
  "schemaVersion": 2,
  "tool": { "name": "sloptopop", "version": "0.1.0" },
  "engine": { "version": "0.1.0", "detector": "0.1.0", "ruleSet": "blake3:…" },
  "snapshot": "blake3:…",
  "rules": [ { "id": "analyzer.finding", "version": 1 }, { "id": "errors.swallowed", "version": 1 } ],
  "analyzers": [
    { "tool": "eslint", "version": "9.39.5", "component": "sha256:…", "source": "client", "report": "blake3:…" },
    { "tool": "ruff", "version": "0.17.1", "component": "sha256:…", "source": "server", "report": "blake3:…" }
  ],
  "root": ".",
  "ecosystems": ["rust", "python"],
  "findings": [
    {
      "rule": "errors.swallowed",
      "severity": "warning",
      "path": "src/app.ts",
      "line": 12,
      "message": "Error handler discards the error",
      "remediation": "Handle the error, propagate it, or log it with context; never discard it silently.",
      "origins": [
        { "tool": "sloptopop", "rule": "errors.swallowed", "source": "native", "version": null, "component": null },
        { "tool": "eslint", "rule": "no-empty", "source": "client", "version": "9.39.5", "component": "sha256:…" }
      ]
    },
    {
      "rule": "toolchain.unpinned",
      "severity": "error",
      "path": "rust-toolchain.toml",
      "message": "Rust toolchain is not pinned to an exact version",
      "control": "STD-1",
      "fix": { "kind": "createFile", "path": "rust-toolchain.toml", "template": "rust-toolchain" },
      "remediation": "Pin the exact toolchain version."
    }
  ],
  "controls": [ { "id": "STD-1", "title": "Pinned toolchain", "status": "fail" } ],
  "passed": false,
  "analysis": {
    "inputs": [
      { "path": ".sloptopop/reports/eslint.sarif", "format": "sarif", "outcome": "read",
        "tools": [ { "name": "eslint", "version": "9.39.5" } ] },
      { "path": ".sloptopop/reports/server.ruff.sarif", "format": "sarif", "outcome": "read",
        "tools": [ { "name": "ruff", "version": "0.17.1" } ] }
    ],
    "observations": { "error": 0, "warning": 1, "note": 1, "none": 0, "suppressed": 1 },
    "coverage": { "files": 1, "covered": 1, "total": 2 },
    "disagreements": [ { "tool": "ruff", "clientOnly": 0, "serverOnly": 1 } ]
  }
}
```

- **Who detects.** The server runs every sloptopop rule on the uploaded
  snapshot. Open-source analyzer reports are advisory input: a result the
  server's alias table maps is a finding of that sloptopop rule (its
  `rule` stays the stable sloptopop id, the tool and its rule go into
  `origins`); any other result is `analyzer.finding`. A tool the server can
  run from source alone is re-run there from the same pinned image
  (`source: "server"`), and its result wins over the uploaded one
  (`analysis.disagreements` says how they differed).
- **One problem, one finding.** Results of one rule about the same file on
  overlapping lines are one finding with every origin. A sloptopop rule
  listed among the origins (`tool: "sloptopop"`, `source: "native"`) made
  it too; a finding without `origins` comes from a sloptopop rule alone.
- **Versions and digests.** `engine.ruleSet` digests every rule's version
  and the alias table; a rule's `version` goes up whenever its semantics
  change. `snapshot` digests the uploaded files; `analyzers[].component`
  is the pinned image's digest and `report` the digest of the report used.
  The same snapshot, reports and versions give a byte-identical Report.

The schema is versioned; adding fields is non-breaking, removing or
renaming them bumps `schemaVersion`. Version 2 replaced `findings[].origin`
with `findings[].origins` and added `engine`, `snapshot`, `rules`,
`analyzers` and `analysis.disagreements`.

## Fields

| Field | Meaning |
|---|---|
| `schemaVersion` | Always `2` for this format. |
| `tool` | Name and version of the producing tool. |
| `engine` | The server: `version`, the Detector's `detector` version, and `ruleSet` (the digest of every rule's version and the alias table). |
| `snapshot` | The digest of the uploaded snapshot (`null` for a directory the server checked in place). |
| `rules[]` | Every rule with its `version`, sorted by `id`. |
| `analyzers[]` | Every tool of every analyzer report read, sorted: `tool`, `version`, `component` (the pinned image's digest, or `null`), `source` (`client`: uploaded; `server`: re-run by the server) and `report` (the digest of the report's bytes). |
| `root` | Always `.`; every `path` is relative to the checked directory and uses `/`. No absolute paths appear. |
| `ecosystems` | Ecosystems found, sorted. |
| `findings[]` | Sorted by `rule`, then `path`, then `line`. |
| `findings[].rule` | Rule identifier, such as `toolchain.unpinned`. |
| `findings[].severity` | `error`, `warning` or `info`. |
| `findings[].path` | The file the finding is about. |
| `findings[].line` | 1-based line; omitted when the finding is about a whole file. |
| `findings[].message` | What is wrong. Never contains a secret value. |
| `findings[].control` | The control the finding counts against; omitted when none applies. |
| `findings[].fix` | A mechanical fix, when one exists: `{ "kind": "createFile", "path", "template" }`. Applied by `sloptopop fix --apply`, which never overwrites a file. |
| `findings[].remediation` | How to fix it, as text. Present on every finding. |
| `findings[].origins[]` | Every tool behind the finding, sorted by source (`native`, `server`, `client`): `tool` (lower case; `sloptopop` for a sloptopop rule), `rule` (its own rule identifier), `source`, `version` and `component` (as in `analyzers`, or `null`). Omitted for a finding a sloptopop rule alone made. |
| `findings[].analyzerFix` | Planned (WP-21, from Report schema version 2): the analyzer's own safe fix for this finding, as the server verified its bytes: `{ "tool", "version", "component", "source", "rule", "applicability", "toolApplicability", "description", "path", "file", "edits": [{ "start", "end", "digest", "text" }], "report" }`, offsets in bytes. `sloptopop check` shows it; only `sloptopop transform` applies it, after the server verifies the change. Omitted when there is none. |
| `controls[]` | Every control of the profile, in order: `id`, `title`, `status` (`pass`, `fail`, or `notChecked` when the engine has no check for it). |
| `passed` | Whether the repository meets the profile. `sloptopop check` exits 0 when `true`, 1 when `false`. |
| `analysis` | What the analyzer reports in `.sloptopop/reports/` held. Always present; its lists are empty and its counts zero when there are no reports. |
| `analysis.inputs[]` | Every file in `.sloptopop/reports/`, sorted by `path`. |
| `analysis.inputs[].path` | The report file, relative to the root. |
| `analysis.inputs[].format` | `sarif` (SARIF 2.1.0) or `lcov`, chosen by the file name; omitted when the outcome is `unrecognised`. |
| `analysis.inputs[].outcome` | `read`; `rejected` (the name picks a format, but the file cannot be used as evidence: too large, not UTF-8 or not valid); or `unrecognised` (no reader exists for the name, so it was not read). |
| `analysis.inputs[].reason` | Why the report was rejected; only when `outcome` is `rejected`. |
| `analysis.inputs[].tools[]` | The tools a read report names as its producers, in report order: `name` (lower case) and `version` (omitted when the report gives none). Empty for LCOV, which names no tool, and for reports that were not read. |
| `analysis.observations` | Counts of every result the reports hold, findings or not: `error`, `warning`, `note` and `none` count the unsuppressed results at that level; `suppressed` counts the results a suppression covers, at any level. |
| `analysis.coverage` | Line coverage of files inside the root, merged over every coverage report: `files` with coverage data, `covered` instrumented lines executed at least once, `total` instrumented lines. |
| `analysis.disagreements[]` | Every tool the server re-ran that an uploaded report also covers: `tool`, `clientOnly` and `serverOnly` (results only one side holds). The server's results are the ones used. |

The same snapshot, analyzer reports and versions always produce a
byte-identical document: no timestamps, no absolute paths, a fixed order.

## Fix result

`sloptopop fix` (like `transform`) delivers the server's steps with
`sloptopop-local apply`, one step per missing file, and prints the run's
`SUMMARY.md`; the run's `report.json` is the transformation report below.
There is no separate fix result document any more: the fixes come back
from the server as deliveries.

## Transformation report (preview)

`sloptopop transform` and `sloptopop fix` write their own report to
`.sloptopop/transform/<run-id>/report.json` (schema version 2: the run,
the plan, the base, the branch and its head, whether every automated step
was delivered, each step's kind, class, title, status, commit, files and
reason, and the server's re-detection), with `SUMMARY.md` and `PR.md`
beside it. See [commands.md](commands.md#transform-preview).
