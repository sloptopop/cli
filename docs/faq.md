# FAQ

## What is sloptopop?

A tool that finds problems in a codebase and then fixes them
automatically, where a fix can be verified. It is aimed at the problems
that AI-assisted ("vibe-coded") projects often have: committed secrets,
ignored database errors, swallowed exceptions, missing tests and CI.

## What is the difference between `check`, `fix` and `transform`?

- `check` reports problems. It changes nothing.
- `fix` creates missing files from templates (a pre-push hook, a CI
  workflow, `rust-toolchain.toml`, `.gitignore`). It never edits an
  existing file.
- `transform` (preview) edits code to fix problems, verifies each edit,
  and puts the result on a branch for you to review.

## Which languages are supported?

`check` recognises these ecosystems from the manifest at the repository
root: Rust (`Cargo.toml`), Node.js (`package.json`), Python
(`pyproject.toml`), Go (`go.mod`), .NET (`*.csproj`), JVM (Gradle
`build.gradle`, `build.gradle.kts`), PHP (`composer.json`), Swift
(`Package.swift`), C and C++ (`CMakeLists.txt`) and PowerShell (`*.psd1`).
The toolchain, lockfile and coverage rules run per ecosystem. Other rules
read files directly, whatever the ecosystem: secrets in any tracked file,
TLS and hosts in source files, swallowed errors in JavaScript, TypeScript, Python,
Java, C#, Kotlin, C++ and PHP. See [rules.md](rules.md).

The automatic transformations work on JavaScript and TypeScript today
(Next.js, Node scripts, supabase-js). See
[transformations.md](transformations.md).

## Does it need the network?

`check` and `fix` work offline and send nothing to sloptopop. `collect`
fetches the analyzer images and the daily vulnerability snapshots from
sloptopop's component library (or your mirror), announcing each before it
is fetched; it sends nothing about your code, and with everything cached
`collect --offline` needs no network. `transform` needs the sloptopop
backend. See
[What leaves your machine](safety.md#what-leaves-your-machine).

## Does it collect telemetry?

The current `sloptopop` and `sloptopop-engine` contain no network code:
no usage data, no install id, no update check. The policy for future
versions is to be confirmed.

## Is my code uploaded?

Not by `check`, `fix` or `collect`. `transform` sends the Report, the
text files of your last commit (secret values masked, environment files
left out) and `.sloptopop/context.json` to the backend, because
describing and planning the code happens there. It shows exactly what it
will send and asks first, and the backend keeps nothing after the
request. Where the hosted backend runs is still to be decided. See
[safety.md](safety.md#what-leaves-your-machine).

## Does it use AI to write code?

No. The planner is rule-based, and every edit has fixed text. AI is used
only in the backend, and anything it proposes passes the same
deterministic Guider checks as everything else. See
[safety.md](safety.md#determinism).

## Can a transformation break my code?

It is designed not to. Each edit checks that the bytes it replaces are the
ones the plan was made from. The Guider then checks that the program
changed exactly as declared and in no other way, and that the problem is
gone. A step that fails any check is refused. Some changes alter behaviour
on purpose (for example, a failed write now throws). Those are marked as
**declared** and state the change for a reviewer. Always review the
branch and run your tests before you merge.

## Why was a finding left manual?

Because sloptopop could not be sure the automatic fix is right there. The
report gives the reason code and a checklist. The codes are explained in
[transformations.md](transformations.md). A common one is
`handler-unknown`: sloptopop could not tell whether a thrown error would
be handled where that code runs.

## What is the difference between "refused" and "manual"?

"Manual" is the plan's choice: it did not try to automate the finding.
"Refused" means an automated step was tried and failed a Guider check, so
nothing of it was applied. See [safety.md](safety.md#refusals).

## Why does `check` fail on a warning?

`check` uses the `strict` profile, where every finding fails the
repository. Warnings are still shown apart from errors, so you can decide
what to fix first.

## What does `notChecked` mean in the controls list?

No rule gives evidence about that control yet, so sloptopop does not
claim it passes or fails.

## What are STD-1 to STD-11?

The controls of the `strict` profile: sloptopop's
[quality standard](quality-standard.md) (pinned toolchain, zero warnings,
100 % coverage, a CI gate, and so on). A finding's `control` field names
the one it counts against.

## How do I add my own rules?

Put Opengrep or Semgrep rules in `.sloptopop/rules/`. `sloptopop collect`
passes them to Opengrep (and to Semgrep with `--only semgrep`), together
with sloptopop's own rule pack. Their results appear as `analyzer.finding`. See
[collect.md](collect.md).

## Why does `collect` run analyzers in containers instead of using mine?

Because the version of a tool decides what it finds. With the tools on
everyone's `PATH`, two machines (or your machine and the server) would
report different findings for the same code. Each image holds exactly the
version sloptopop tested, verified by its digest, and runs without network
and without access to anything but your repository (read-only) and the
reports directory. `--host-tools` still runs your own tools where no
container runtime can run, unsupported. See [analyzers.md](analyzers.md).

## Can I use sloptopop in CI?

Yes. Run `sloptopop check` and use its exit code: 0 passed, 1 not
passed, 3 the engine is missing or failed. `sloptopop check --format json`
gives the machine-readable [Report](report-format.md). Run
`sloptopop collect` first if you want analyzer results in it.

## Can I call it from my own language?

Yes. The language libraries (Python, Node, Go, JVM, .NET, C++, PHP,
PowerShell, shell, Rust, Swift) start `sloptopop serve --stdio` and speak
JSON-RPC 2.0 to it. See [PROTOCOL.md](../PROTOCOL.md).

## What is open source?

The `sloptopop` CLI, the protocol and the language libraries, under MIT OR
Apache-2.0. The engine is a proprietary binary that the CLI starts as a
separate process; the CLI never links it. The engine's licence terms are
to be confirmed.

## How much does it cost?

Pricing is to be confirmed.

## Does sloptopop push or merge anything?

No. It never merges, pushes or deploys. You review the branch and merge it
yourself.
