# sloptopop documentation

sloptopop finds problems in a codebase, then transforms the code to fix
them, where a fix can be verified.

## Using sloptopop

| Document | What it covers |
|---|---|
| [Install](install.md) | Installing `sloptopop` and `sloptopop-engine`, and checking the installation |
| [Quickstart](quickstart.md) | `check`, `collect`, `fix` and `transform` on a small sample repository |
| [Commands](commands.md) | Every command, option, exit code and environment variable |
| [Rules](rules.md) | Every rule `check` reports: what it means, why it matters, its controls and what to do |
| [Transformations](transformations.md) | Every automatic transformation: before and after, when it applies, when it stays manual, and what is verified |
| [Safety](safety.md) | Determinism, the Guider, refusals, reviewable commits, and what leaves your machine |
| [Analyzers](analyzers.md) | The open-source analyzers `collect` runs, by language, and their licences |
| [`sloptopop collect`](collect.md) | Reference: every analyzer `collect` runs, its licence, fixed flags and output, and why |
| [FAQ](faq.md) | Common questions |
| [Troubleshooting](troubleshooting.md) | Error messages and what to do about them |

## Integrating sloptopop

General documentation shared by every sloptopop language library. Each
library's own repository has a `LANGUAGE.md` with only what is specific to
that language.

| Document | What it covers |
|---|---|
| [Architecture](architecture.md) | How the CLI, the engine and the language libraries fit together |
| [Protocol](../PROTOCOL.md) | JSON-RPC 2.0 over stdio between the libraries and the `sloptopop` binary |
| [Protocol schema](../schema/protocol.json) | Machine-readable description of every method and type |
| [Conformance suite](../conformance/README.md) | Wire-level test cases every library must pass |
| [Report format](report-format.md) | The JSON produced by `sloptopop check --format json` |
| [Quality standard](quality-standard.md) | The checks every open-source sloptopop repository passes before release |
