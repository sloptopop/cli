# Slop To Pop CLI

The `sloptopop` command-line tool and JSON-RPC server.

```
sloptopop collect [root]   # run the installed analyzers into .sloptopop/reports/
sloptopop check [root]     # check the repository, reading those reports
sloptopop fix [root]       # show (or --apply) mechanical fixes
```

`sloptopop collect` runs the open-source analyzers you already have
installed (gitleaks, OSV-Scanner, Ruff, golangci-lint and more) with fixed,
offline flags, each as a separate process; it never installs or bundles a
tool. See [docs/collect.md](docs/collect.md) for every tool, licence and
flag, and [docs/](docs/README.md) for the rest of the documentation.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
