# sloptopop rule pack

Rules for [Opengrep](https://github.com/opengrep/opengrep) and
[Semgrep Community Edition](https://github.com/semgrep/semgrep), which
share one rule format. `sloptopop collect` passes this pack to both
when no `--rules <dir>` is given (see [docs/collect.md](../docs/collect.md)):
the files are built into the `sloptopop` binary and written to
`.sloptopop/reports/.rules/` before the scan.

Each rule finds a problem that sloptopop can then fix or turn into a task,
so the ids are stable and the matched spans are exact.

## Licence

Every rule and test file here was written for sloptopop. Like the rest of
this repository, the pack is licensed under either of
[Apache-2.0](../LICENSE-APACHE) or [MIT](../LICENSE-MIT), at your option.

None of it comes from the Semgrep registry
(`semgrep/semgrep-rules`, `--config auto`, `p/...`): those rules are under
the Semgrep Rules License, which is not an open-source licence, and
sloptopop neither uses nor ships them. Contributions must be original work
too; do not copy or adapt registry rules.

## Rules

| Id | Languages | Finds | CWE | Autofix |
|---|---|---|---|---|
| `sloptopop.errors.empty-catch` | JS, TS | `try { … } catch (e) { }` and `catch { }` with an empty or comment-only body | CWE-390 | no |
| `sloptopop.errors.empty-promise-catch` | JS, TS | `.catch(() => {})`, `.catch(() => undefined)`, `.catch(function () {})`; the span is the callback | CWE-390 | no |
| `sloptopop.errors.python-except-pass` | Python | `except: pass`, `except Exception: pass`, `except BaseException: pass` | CWE-390 | no |
| `sloptopop.errors.go-empty-err-check` | Go | an empty `if err != nil { }`, also with an init statement | CWE-390 | no |
| `sloptopop.supabase.unchecked-write` | JS, TS | a supabase-js `insert`, `update`, `upsert` or `delete` whose result is dropped (`await …;`) or destructured without `error` | CWE-252 | appends `.throwOnError()` |
| `sloptopop.config.js-local-url` | JS, TS | a `localhost`, loopback or private-network URL literal outside tests and config files, unless it is the default of an environment variable (`process.env.X ?? …`, `process.env.X?.trim() || …`, the last branch of `process.env.X ? … : …`) or opens no connection (compared, logged, a `new Request(…)` key) | CWE-1051 | no |
| `sloptopop.config.python-local-url` | Python | the same, unless it is the default of `os.environ.get(…, …)`, `os.getenv(…) or …` or a field of a pydantic `BaseSettings` class | CWE-1051 | no |
| `sloptopop.config.go-local-url` | Go | the same, outside `_test.go` files | CWE-1051 | no |
| `sloptopop.tls.js-reject-unauthorized` | JS, TS | `rejectUnauthorized: false`, `NODE_TLS_REJECT_UNAUTHORIZED = "0"` | CWE-295 | no |
| `sloptopop.tls.python-verify-false` | Python | `verify=False`, `ssl._create_unverified_context()` | CWE-295 | no |
| `sloptopop.tls.go-insecure-skip-verify` | Go | `InsecureSkipVerify: true` | CWE-295 | no |
| `sloptopop.sql.js-built-query` | JS, TS | SQL built with `+` or a template literal and passed to `query`, `execute`, `raw`, `$queryRawUnsafe` and the like (tagged templates are fine) | CWE-89 | no |
| `sloptopop.sql.python-built-query` | Python | SQL built with `+`, `%`, `.format()` or an f-string and passed to `execute` and the like | CWE-89 | no |
| `sloptopop.sql.go-built-query` | Go | SQL built with `+` or `fmt.Sprintf` and passed to `Query`, `Exec` and the like | CWE-89 | no |
| `sloptopop.eval.js-dynamic-code` | JS, TS | `eval`, `new Function` or `Function` of anything but a string literal | CWE-95 | no |

A rule has an autofix only where the rewrite is exact and needs no
judgement. `sloptopop.supabase.unchecked-write` inserts exactly
`.throwOnError()` after the call that ends the builder chain; every other
byte of the statement stays as it was. It is written as a `fix-regex` on
the whole statement, because a `fix` template re-indents a chain that spans
several lines. Use the JSON output (`--json`) for the fix: `extra.fix`
replaces the bytes from `start.offset` to `end.offset`. Columns in both the
JSON and the SARIF output are 1-based byte columns.

## Layout and conventions

- `rules/<area>/<name>.yaml` holds one rule with the id
  `sloptopop.<area>.<name>`, a short `message`, `metadata.cwe` and
  `metadata.confidence`.
- Next to it, `<name>.<ext>` holds the test cases in the engines' own test
  format: a `ruleid: <id>` comment on the line before each expected finding
  and `ok: <id>` before each case that must not match. A rule with an
  autofix also has `<name>.fixed.<ext>`, the test file after the fix.
- Rules use only the open-source engine features shared by Opengrep and
  Semgrep (no Pro or cross-file analysis, no registry references).
- The unit tests of this crate check that the pack built into the binary is
  exactly `rules/*/*.yaml` and that every rule follows these conventions.

## Testing

```
semgrep --test --metrics=off --disable-version-check rules/
opengrep scan --test rules/
```

Both must report every test passing, including the autofix test.
