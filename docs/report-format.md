# Report format (schema version 1)

The public contract between the open-source `sloptopop` CLI, the language
libraries and the sloptopop engine.

`sloptopop check --format json` prints one JSON document:

```json
{
  "schemaVersion": 1,
  "tool": { "name": "sloptopop", "version": "0.1.0" },
  "root": ".",
  "ecosystems": ["rust", "python"],
  "findings": [
    {
      "rule": "analyzer.finding",
      "severity": "warning",
      "path": "src/app.py",
      "line": 1,
      "message": "ruff F401: `os` imported but unused",
      "remediation": "Fix what the analyzer reports, following its rule documentation.",
      "origin": { "tool": "ruff", "rule": "F401" }
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
      { "path": ".sloptopop/reports/lcov.info", "format": "lcov", "outcome": "read", "tools": [] },
      { "path": ".sloptopop/reports/notes.txt", "outcome": "unrecognised", "tools": [] },
      {
        "path": ".sloptopop/reports/ruff.sarif",
        "format": "sarif",
        "outcome": "read",
        "tools": [ { "name": "ruff", "version": "0.14.0" } ]
      }
    ],
    "observations": { "error": 0, "warning": 1, "note": 1, "none": 0, "suppressed": 1 },
    "coverage": { "files": 1, "covered": 1, "total": 2 }
  }
}
```

The schema is versioned; adding fields is non-breaking, removing or renaming
them bumps `schemaVersion`. The sloptopop engine produces exactly this
document. `findings[].origin` and `analysis` were added to schema version 1
in this way: the version stays `1`, and readers that do not know them can
ignore them.

## Fields

| Field | Meaning |
|---|---|
| `schemaVersion` | Always `1` for this format. |
| `tool` | Name and version of the producing tool. |
| `root` | Always `.`; every `path` is relative to the checked directory and uses `/`. No absolute paths appear. |
| `ecosystems` | Ecosystems found, sorted. |
| `findings[]` | Sorted by `rule`, then `path`, then `line`. |
| `findings[].rule` | Rule identifier, such as `toolchain.unpinned`. |
| `findings[].severity` | `error` or `warning`. |
| `findings[].path` | The file the finding is about. |
| `findings[].line` | 1-based line; omitted when the finding is about a whole file. |
| `findings[].message` | What is wrong. Never contains a secret value. |
| `findings[].control` | The control the finding counts against; omitted when none applies. |
| `findings[].fix` | A mechanical fix, when one exists: `{ "kind": "createFile", "path", "template" }`. Applied by `sloptopop fix --apply`, which never overwrites a file. |
| `findings[].remediation` | How to fix it, as text. Present on every finding. |
| `findings[].origin` | For a finding taken from an analyzer report: `{ "tool", "rule" }`, the tool's name in lower case and its own rule identifier. Omitted for the engine's own findings. |
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

The same repository content (analyzer reports included) always produces a
byte-identical document: no timestamps, no absolute paths, a fixed order.

## Fix result

`sloptopop-engine fix <path> [--apply]` prints:

```json
{
  "schemaVersion": 1,
  "tool": { "name": "sloptopop", "version": "0.1.0" },
  "root": ".",
  "applied": false,
  "changes": [
    { "path": "rust-toolchain.toml", "template": "rust-toolchain", "status": "planned" }
  ]
}
```

`status` is `planned` (dry run), `created` (with `--apply`) or `exists`
(the file is already there and was left alone). Changes are sorted by
`path`.

