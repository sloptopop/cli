# Safety

sloptopop changes code. This page explains what keeps those changes safe,
and what you can rely on.

The short version:

- **Refuse, never wrongly apply.** When a change cannot be shown to be
  right, sloptopop does not make it. It tells you why and leaves the work
  to you.
- **Deterministic.** The same input always gives the same findings, the
  same plan and the same edits.
- **Nothing is applied without verification.** An independent verifier,
  the Guider, checks every automated step before it is kept.
- **You stay in control.** Changes land on a branch as reviewable commits.
  sloptopop never merges, pushes or deploys.

## Determinism

- Every Detector rule is a pure function of the files it reads. The same
  repository content, analyzer reports included, gives a byte-identical
  Report: findings in a fixed order, no timestamps, no absolute paths.
- The planner is rule-based. Each finding is placed on exactly one site in
  the code and mapped to a fixed method from a closed set. AI is used only
  in the backend, and anything it proposes passes the same Guider checks.
- Each edit has fixed text. For example, an unchecked write always gets
  exactly `.throwOnError()` at the end of its query chain.
- The analyzers that `sloptopop collect` runs come from pinned container
  images and get fixed flags, so the same repository gives the same
  reports on every machine.

So you can run sloptopop twice and compare, review a plan before applying
it, and reproduce any result later.

## The Guider

The Guider is a separate verifier. Every other part of sloptopop proposes
something: the Detector a finding, the planner a plan, the Implementor an
edit. The Guider only decides whether each proposal is accepted. It never
proposes anything itself.

Its checks are deterministic. Each check gives one of three answers:

| Answer | What happens |
|---|---|
| valid | The step may be kept. |
| invalid | The step is refused, with the check that failed. |
| unknown | The step is not applied. "Unknown" never passes. |

For each automated step it checks, among other things:

- **Preconditions:** the bytes at each edit's range are exactly the ones
  the plan was made from.
- **Context:** no thrown error and no server logger is added to code that
  runs in the browser.
- **The declared change, and nothing else:** sloptopop describes the whole
  program before and after the edit, and the difference must equal the
  change the plan declared.
- **Re-detection:** the problem the finding reported is gone at that site.

After all steps, the Detector runs again on the result. The checks for
each transformation are listed in
[transformations.md](transformations.md#what-the-guider-checks).

## Refusals

A refusal is a normal, safe outcome. It means a step did not pass a check,
so it was not applied. The rest of the run goes on, except for steps that
build on the refused one.

The transformation report lists each refusal with the check that failed.
What you can do:

- treat the finding as a manual task and fix it by hand, using the rule's
  advice in [rules.md](rules.md);
- if you think the refusal is wrong, report it with the check code and a
  synthetic example (never send real secrets or confidential code).

**Manual tasks** are different from refusals. The plan chose not to
automate a finding on purpose, and says why with a reason code (for
example `unhandled-in-client`). Each manual task comes with a checklist.
The reason codes are explained in
[transformations.md](transformations.md).

## Your repository

`sloptopop check` and `sloptopop collect` never change your source files.
`collect` writes only under `.sloptopop/`, plus a `.gitignore` line there
for the reports. The analyzers it runs see your repository read-only, in
containers without network, capabilities or privileges, and can write
only to `.sloptopop/reports/` (see [collect.md](collect.md#how-a-tool-runs)). `sloptopop fix` only creates missing files, and never
overwrites or edits an existing one. Without `--apply` it writes nothing.

`sloptopop transform` (preview) is built to these rules:

- **Dry run by default.** Without `--apply` it shows the plan and changes
  nothing.
- **Your checkout is never touched.** It works on a new branch, made with
  git's plumbing in a private index: your files, your index and `HEAD`
  stay as they are.
- **A dirty tree is refused.** It does not start while you have
  uncommitted or untracked changes, and stops if `HEAD` moved between
  the upload and the delivery.
- **Stacked, reviewable commits.** Each step is one commit, on top of the
  previous one, so you can review, test and revert step by step. A
  two-step change (for example, read a new configuration key, then
  require it at startup) is two commits, so you can deploy the first
  before the second.
- **It never merges, pushes, deploys or runs migrations,** and it never
  runs your project's own tools.
- **Facts about production are never guessed.** A step that depends on
  one (for example "this variable is set in every environment") states it
  for a person to confirm.

## Secrets

- A finding's message never contains a secret value.
- `sloptopop collect` runs gitleaks with `--redact`, and makes sure git
  ignores `.sloptopop/reports/`.
- A transformation never writes a secret value. Its replacement text is
  fixed (an environment read), and `.env.example` gets the key with no
  value.
- Rotating a leaked credential is always a manual task, because the old
  value stays in git history until it is revoked.

## What leaves your machine

| Command | What it sends to sloptopop |
|---|---|
| `check`, `fix` | One request to the server you name (hosted by default): the text files of the commit `HEAD` points to with every secret value masked, the analyzer reports in `.sloptopop/reports/`, and `.sloptopop/context.json` if you have one. The Report (`check`) or the fixes (`fix`) come back. |
| `collect` | Nothing about your code. It fetches the analyzer images and the daily vulnerability snapshots from sloptopop's component library (or your mirror), each announced before it is fetched; no analyzer has network. For Go and .NET projects, a prepare step downloads your project's own dependencies from your module or package registries. See [collect.md](collect.md#how-a-tool-runs). |
| `transform` (preview) | The same request: the server detects, plans and verifies; the Report, the plan and the verified changes come back. |

What `transform` uploads, exactly:

- the files of the commit, read from git: UTF-8 text files only, at most
  1 MiB each;
- never: binary files, symbolic links, submodules, `node_modules`,
  `dist`, `build`, `vendor` and other dependency or build directories,
  `.sloptopop/`, and real environment files (`.env`, `.env.local`, …;
  `.env.example` is uploaded);
- secret values masked: every span the gitleaks report in
  `.sloptopop/reports/` names (in any file), and every value-like token
  on its line, is replaced by `x`s of the same length before upload;
  without a usable, fresh gitleaks report nothing is uploaded.
  A documented public prefix (`sk_live_`, `ghp_`, `AKIA`, …) is kept, so
  the provider is still known. The real value is put back on your
  machine in the commits;
- never: your git history, other branches, your confirmations
  (`attestations.json`) or who made them.

Before anything is sent, `transform` prints this manifest (files, bytes,
what is left out and why, how many values are masked, the destination)
and asks. `.sloptopop/transform/<run>/request.json` keeps exactly what was
sent, so you can check it.

The request goes over `https` with the system's `curl` (`http` only if
you name an `http://` backend, for one inside your own network). The
`sloptopop` CLI is the only program that makes the call, and it is open
source; `sloptopop-local` has no network code. Neither sends usage data
or checks for updates.

`check`, `fix` and `transform` all need the server: since decision
D-0183 detection, planning and verification run only there, and nothing
proprietary runs on your machine. Only `collect` (the open-source
analyzers) works without an upload. Before anything is sent, every secret
value gitleaks reports is masked (`sloptopop-local` refuses to upload
without a usable gitleaks report), and the server scans what it receives
with the same pinned gitleaks and refuses a request that still holds a
secret in the clear, naming only the file and line.

The backend keeps nothing of a request after it: the snapshot exists only
in a private scratch directory while the request runs. Its logs hold
counts, codes and durations, never source, paths or findings.

**Still to be decided** before `transform` is released (owner decisions
D-0172 to D-0181):

- where the hosted backend runs (provider and region) and the
  data-processing terms;
- the licence terms of the self-hosted server and the hosted service.

This page will be updated when these are decided. The `sloptopop` CLI is
open source (MIT OR Apache-2.0), so you can read its code.

## Analyzer fixes

Analyzers such as Ruff, ESLint and Biome know how to fix some of what they
report. sloptopop treats their fixes as claims, not proofs:

- **Kept, never changed, never applied locally.** `collect` keeps each fix
  exactly as the tool gave it, translated into byte offsets of the file
  as committed ([collect.md](collect.md#fixes)). It never runs a tool's fix
  mode and never edits your files.
- **Only the tool's own safe level.** A fix the tool itself applies only
  with an opt-in (`unsafe`), or never applies (`suggestion`), is never
  turned into a change. A safe fix whose safety your configuration can
  change (Ruff `extend-safe-fixes`, a Biome rule's `fix` option) is
  dropped.
- **Only reviewed rules.** A fix becomes a change only for a tool, version
  and rule in the reviewed fix table, at the level the table records. The
  owner defaults `useImportType`, `noTsIgnore` (Biome) and `I001` (Ruff)
  are in the table and can be switched off.
- **Not on `info` findings** (decision D-0182), unless the owner switches
  that on.
- **oxlint's fixes are not used**: none of its report formats carries
  them; diff-only engines are a later package (WP-22).
- **Verified on the server** (planned, WP-21 phase 2; until it is built
  no analyzer fix is applied at all). The server re-runs the same pinned tool on
  your snapshot, checks the digests of the bytes each fix replaces, and
  verifies each step: nothing else in the file changes, no token outside
  the edit changes its kind, the targeted finding is gone, the tool reports
  no new parse error and no new finding outside the edit. Your project's
  own type check and tests run on your machine, as evidence. A fix that
  fails any check is refused, never applied.
- **What is not proven.** A safe fix can still change behaviour your tests
  do not cover; the change is labelled as the tool's, not as equivalent.
- **Licences.** A fix is the tool's output about your code; whether fixes
  from copyleft rules (eslint-plugin-sonarjs is LGPL-3.0-only) need a notice
  is an open question for the owner.
