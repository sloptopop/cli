# sloptopop documentation

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
