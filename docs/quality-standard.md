# Quality standard

Every open-source sloptopop repository (the CLI and every language library)
runs one gate, `scripts/gate.sh`, before anything is pushed (pre-push hook)
and in CI. Nothing is skipped; a missing tool fails the gate.

| # | Standard |
|---|---|
| 1 | Exact toolchain version pinned and verified |
| 2 | Formatter check with no diff |
| 3 | Zero warnings from the compiler and the strictest practical linter |
| 4 | All tests pass, including the conformance suite against the real binary |
| 5 | 100% test coverage on every metric the language's coverage tool offers |
| 6 | Mutation testing: every mutant killed |
| 7 | No known vulnerabilities in any dependency |
| 8 | Every dependency, including dev and test tools, under a permissive licence (MIT, Apache-2.0, BSD-2/3-Clause, ISC, 0BSD, Zlib, CC0-1.0, Unicode-3.0, MPL-2.0, PSF-2.0, BlueOak-1.0.0, BSL-1.0) |
| 9 | Public API fully documented |
| 10 | Pre-push hook runs the gate |
| 11 | CI runs the full gate |

Building a release artifact also requires the tests to pass at 100% coverage,
wherever the language's build system allows it; each `LANGUAGE.md` explains
how, or why it can't.
