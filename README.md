# Slop To Pop CLI

The `sloptopop` command-line tool and JSON-RPC server.

```
sloptopop collect [root]     # run the installed analyzers into .sloptopop/reports/
sloptopop check [root]       # check the repository, reading those reports
sloptopop fix [root]         # show (or --apply) mechanical fixes
sloptopop transform [root]   # plan, apply and verify code transformations (preview)
```

New here? Start with the [quickstart](docs/quickstart.md).

`sloptopop transform` (preview) sends one request to the sloptopop backend:
the Report and the text files of your last commit, secret values masked.
It shows what it will send and asks first (`--yes` for CI), then applies
the verified steps on a new branch, one commit per step, and never touches
your checkout. Declared behaviour changes wait until a person confirms
them. See [commands.md](docs/commands.md#transform-preview) and
[safety.md](docs/safety.md#what-leaves-your-machine). No command sends
usage data or checks for updates.

`sloptopop collect` runs the open-source analyzers you already have
installed (gitleaks, OSV-Scanner, Ruff, golangci-lint and more) with fixed,
offline flags, each as a separate process; it never installs or bundles a
tool. Opengrep and Semgrep run with sloptopop's own rule pack
([rules/](rules/README.md), MIT OR Apache-2.0), never with registry rules.
eslint-plugin-sonarjs and SonarAnalyzer.CSharp, pinned to their last
LGPL-3.0 releases, run from a sloptopop-managed directory
(`.sloptopop/tools/`) once you install them there with one command.
See [docs/collect.md](docs/collect.md) for every tool, licence and flag.

## Documentation

- [Install](docs/install.md)
- [Quickstart](docs/quickstart.md)
- [Commands](docs/commands.md)
- [Rules](docs/rules.md)
- [Transformations](docs/transformations.md)
- [Safety](docs/safety.md)
- [Analyzers](docs/analyzers.md) and the [`collect` reference](docs/collect.md)
- [FAQ](docs/faq.md) and [Troubleshooting](docs/troubleshooting.md)
- [Report format](docs/report-format.md), [Architecture](docs/architecture.md),
  [Protocol](PROTOCOL.md) and the [quality standard](docs/quality-standard.md)

The full index is [docs/README.md](docs/README.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
