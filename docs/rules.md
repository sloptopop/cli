# Rules

Every rule `sloptopop check` can report, what it means, why it matters,
which controls it maps to, and what to do.

- **Severity.** Each finding is an `error` or a `warning`. Under the
  `strict` profile, which `check` uses, every finding fails the
  repository, warnings included.
- **Where rules run.** Every rule runs on the sloptopop server
  (`sloptopop-engine`, hosted or self-hosted), on the masked snapshot that
  `sloptopop local` builds and you confirm before upload. Nothing is
  detected on your machine; without a server there is no check.
- **Determinism.** Every rule is a pure function of the snapshot (and the
  analyzer reports `sloptopop collect` wrote to `.sloptopop/reports/`).
  The same content always gives the same findings, and every Report names
  the Detector, rule and analyzer versions and digests.
- **What is read.** The snapshot holds the files of the commit `HEAD`
  points to. Build output, dependency directories, real environment
  files, files over 1 MiB, files that are not UTF-8, and symbolic links
  are left out (see [safety.md](safety.md)), and every secret value the
  secret scanner reports is masked.
- **Secrets are never printed.** A message never contains a secret value.

## How controls are shown

Each rule lists up to three kinds of control:

- **Report control:** the control of the `strict` profile the finding
  counts against. It is the `control` field of the finding in the
  [Report](report-format.md), and the bracket in `check` output
  (`[STD-1]`). The `strict` profile is the
  [quality standard](quality-standard.md), STD-1 to STD-11.
- **Plan trace:** for rules with an automatic transformation, the
  controls the plan's steps trace to. They appear in the transformation
  report (see [transformations.md](transformations.md)).
- **Standards:** the identifiers in public frameworks that a finding (or
  its fix) gives evidence for. A mapping means the rule produces relevant
  evidence. It does not mean that passing the rule satisfies the control.
  These identifiers are not yet in the Report itself.

Abbreviations: SCF = Secure Controls Framework 2026.3; ASVS = OWASP
Application Security Verification Standard 5.0; ISO 27001 = ISO/IEC
27001:2022 Annex A; CWE = Common Weakness Enumeration. Attribution notices
are at the [end of this page](#attribution).

"Automatic fix" names what sloptopop can do about a finding:
`sloptopop fix` creates a missing file (a step the server returns and
`sloptopop local` commits); a **transformation** is applied by
`sloptopop transform` (see [transformations.md](transformations.md)).

## Contents

- [Secrets and transport security](#secrets-and-transport-security)
- [Error handling and types](#error-handling-and-types)
- [Database queries in code](#database-queries-in-code)
- [Database schema](#database-schema)
- [Build and delivery](#build-and-delivery)
- [Dependencies and imports](#dependencies-and-imports)
- [Analyzer reports](#analyzer-reports)
- [Architecture](#architecture)
- [Layout](#layout)

## Secrets and transport security

### `secrets.committed` (error)

**What it means.** Secret material is in a tracked file. The Detector
recognises private-key headers, tokens with a fixed provider prefix and
exact length (AWS, GitHub, GitLab, Slack, Stripe live keys, Google API,
npm), and committed `.env` files with values that the root `.gitignore`
does not ignore. Secret scanners such as gitleaks, run by
`sloptopop collect`, report here too.

**Why it matters.** Anyone with read access to the repository, a fork or
a clone has the credential. Removing the line later does not revoke it.

**Controls.** Plan trace: ASVS V13.3.1, V13.3.2; SCF IAC-14.1, IAC-14.3,
IAC-29. Standards: SCF CRY-10, IAC-14.2, IAC-14.3; ASVS V13.3.1; ISO
27001 A.5.17, A.8.24; CWE-798.

**What to do.**

1. Treat the credential as compromised: rotate it at the provider and
   revoke the old one.
2. Load it from the environment or a secret manager instead of the source.
3. Remove it from the repository and its history (for example with
   git filter-repo), and from forks.
4. For `.env` files: untrack the file, add `.env*` to `.gitignore`, and
   keep a `.env.example` with keys but no values.

**Automatic fix.** `fix` creates a `.gitignore` when an environment file is
the problem and there is none. Transformation: a secret literal in
JavaScript or TypeScript becomes an environment read
([secrets → env reads](transformations.md#secrets--environment-reads)).
Coming: [secrets in env files](transformations.md#secrets-in-env-files-coming).
Rotation always stays a manual task.

### `secrets.in-history` (error)

**What it means.** A secret scanner found a secret in a commit of the
repository's history. It comes from a history scan, such as the
`gitleaks-history` entry of `collect`.

**Why it matters.** The secret is still readable in every clone, even if
the current files no longer contain it.

**Controls.** Standards: SCF IAC-14.1, IAC-14.2, IAC-14.3; ASVS V13.3.1;
ISO 27001 A.5.17, A.8.24.

**What to do.** Rotate the credential. Deleting the line revoked nothing.
Then purge it from history if your policy requires it.

**Automatic fix.** None. Rotation is work outside the repository.

### `tls.verification-disabled` (error)

**What it means.** TLS certificate verification is turned off:
`rejectUnauthorized: false`, `verify=False`, `InsecureSkipVerify: true`,
`NODE_TLS_REJECT_UNAUTHORIZED=0`, `danger_accept_invalid_certs`,
`danger_accept_invalid_hostnames`, or `CURLOPT_SSL_VERIFYPEER, false`.
Comment lines, tests and fixtures are skipped.

**Why it matters.** Anyone on the network path can read and change the
traffic. The code still appears to use TLS, which hides the problem.

**Controls.** Standards: SCF CRY-05, CRY-06; ASVS V12.3.2, V12.3.4; ISO
27001 A.5.14, A.8.24.

**What to do.** Fix the certificate chain, or trust the specific internal
certificate authority. Never turn verification off, not even temporarily.

**Automatic fix.** None.

### `config.hardcoded-host` (warning)

**What it means.** Source code contains a private network address
(RFC 1918 IPv4, such as `10.0.0.5` or `192.168.1.20`) or a
`://localhost` or `://127.0.0.1` URL. Tests, fixtures and comment lines are
skipped, and so are CIDR ranges.

**Why it matters.** The code works on one machine or network and breaks,
or reaches the wrong system, everywhere else.

**Controls.** Standards: SCF CFG-02, TDA-15.1; ASVS V15.2.3; ISO 27001
A.8.31, A.8.9.

**What to do.** Read hosts and URLs from configuration (environment or a
config file) validated at startup.

**Automatic fix.** Coming:
[hard-coded hosts](transformations.md#hard-coded-hosts--configurable-coming).

### `arch.privileged-key-client-side` (error / warning)

**What it means.** A privileged key can reach browser code:

- **error:** an environment variable with a prefix the bundler inlines into
  client code (`NEXT_PUBLIC_`, `VITE_`, `REACT_APP_`, `EXPO_PUBLIC_`) names
  a service-role or secret key;
- **warning:** a file that client components import (`'use client'` and
  what it imports) uses a service-role credential and does not import
  `server-only`.

**Why it matters.** Every client bundle built with the key contains it.
A service-role key bypasses row-level security.

**Controls.** None mapped yet.

**What to do.** Drop the public prefix and read the key only in server-side
code. Move service-role code into a module that imports `server-only`.
Rotate the key: every bundle built so far contains it.

**Automatic fix.** None.

## Error handling and types

### `errors.swallowed` (warning)

**What it means.** An error handler discards the error: an empty or
comment-only `catch` block, an empty `.catch(() => {})` callback
(JavaScript, TypeScript, Java, C#, Kotlin, C++, PHP), or a Python bare
`except` (or `except Exception` / `BaseException`) whose only statement
is `pass` or `...`.

**Why it matters.** A failure looks like success. Nobody is told, and the
code carries on with missing or wrong data.

**Controls.** Plan trace: CWE-390; ASVS V16.5.3; SCF MON-03. Standards:
SCF SEA-15, TDA-12.1, TDA-06; ASVS V16.3.4, V16.5.3; ISO 27001 A.8.28;
CWE-390, CWE-391.

**What to do.** Handle the error, propagate it, or log it with context
through a server-side logger. Keep data out of the log message.

**Automatic fix.** Transformation for JavaScript and TypeScript:
[swallowed errors](transformations.md#swallowed-errors--observed-errors).

### `types.disabled` (error)

**What it means.** Type checking is switched off: `tsconfig.json` with
`"strict": false`, or with no `strict` and no `extends`; a
`// @ts-nocheck` comment; a mypy section without `strict = true`.

**Why it matters.** Type errors that the compiler would catch reach
production instead.

**Controls.** Report control: STD-3. Standards: SCF TDA-17.1; ASVS
V15.3.5; ISO 27001 A.8.28.

**What to do.** Turn strict checking on (`"strict": true`, mypy
`strict = true`), remove `@ts-nocheck`, and fix the errors that appear.

**Automatic fix.** None yet.

## Database queries in code

These rules read the queries JavaScript and TypeScript code builds with
supabase-js style query builders (`.from('<table>')` chains), outside test
code.

### `db.result-unchecked` (error for writes, warning for reads)

**What it means.** A query's error is never read. The result is dropped,
destructured without `error`, or destructured with an `error` that the
function never uses. Writes (`insert`, `upsert`, `update`, `delete`) are
errors; reads (`select`) are warnings.

**Why it matters.** A failed write passes silently, and the code carries
on as if the row was written. A failed read looks like "no rows".

**Controls.** Plan trace: CWE-252; ASVS V16.5.3; SCF TDA-12.1.
Standards: SCF SEA-15, TDA-12.1; ISO 27001 A.8.28; NIS2 Implementing
Regulation 2024/2690 Annex 6.2.2(b).

**What to do.** Read `error` from every query result and handle it (return
an error response, roll back, retry or stop), or call `.throwOnError()` so
failures throw.

**Automatic fix.** Transformation for writes:
[unchecked writes](transformations.md#unchecked-writes--failures-that-throw).
Coming for reads:
[unchecked reads](transformations.md#unchecked-reads-coming).

### `db.transition-unguarded` (warning)

**What it means.** An update sets a state column (`status`, `state`,
`phase`, `stage`, `*_status`, …) to a fixed value without filtering on
its current value.

**Why it matters.** Two requests at the same time can both make the
transition: an order approved twice, an invoice issued twice.

**Controls.** Standards: ASVS V15.4.2; CWE-367.

**What to do.** Make the transition a compare-and-set: filter on the
expected current state (`.eq('status', 'pending')`), check how many rows
changed, and treat zero as a conflict.

**Automatic fix.** None yet.

### `db.version-unchecked` (warning)

**What it means.** A whole-record update (a variable or spread payload) of
a table with a `version`, `lock_version` or `row_version` column does not
filter on that column.

**Why it matters.** A stale read overwrites newer changes: a lost update.

**Controls.** None mapped yet.

**What to do.** Filter the update on the version the client read,
increment it in the same update, and answer zero changed rows with a
conflict (HTTP 409).

**Automatic fix.** None yet.

### `db.multi-write-no-tx` (warning)

**What it means.** One top-level function or class method writes two or
more tables through separate calls, without a transaction. Writes to
history and log tables are not counted.

**Why it matters.** If a later write fails, the earlier ones stay. The
data is left half-changed.

**Controls.** Standards: ASVS V2.3.3.

**What to do.** Put the writes in one transaction in code, through your
database client (Prisma `$transaction`, Drizzle `db.transaction`,
node-postgres `BEGIN … COMMIT`, knex `transaction`). supabase-js has no
client-side transactions: use a server-side database client for that
operation, or a database function called once with `.rpc()` if you
choose that design. If one step calls an external system, record it
through an outbox instead.

**Automatic fix.** None yet.

## Database schema

These rules read the SQL files in the repository (migrations and schema
files, outside tests, fixtures and down migrations), in path order, so the
schema they see is the state after the last migration. Prisma and Drizzle
schemas are not read yet.

### `db.rls-missing` (error)

**What it means.** Row-level security is enabled on some tables, but not
on this table of the default schema.

**Why it matters.** Rows of this table can be read or changed by roles
that the other tables protect against, such as the public API roles.

**Controls.** Standards: CWE-285; OWASP API Security Top 10 API1:2023.

**What to do.** Enable row-level security on the table in a migration
(`ALTER TABLE … ENABLE ROW LEVEL SECURITY`) and add policies for the roles
that need access.

**Automatic fix.** None yet.

### `db.sensitive-auth-data-stored` (error)

**What it means.** A column is named as sensitive authentication data: a
card verification code (`cvv`, `cvc2`), card track data (`track1`,
`magstripe`) or PIN data (`pin_block`). Booleans and check outcomes
(`cvc_check`) are left out.

**Why it matters.** This data must not be kept after authorisation, not
even encrypted.

**Controls.** Standards: SCF DCH-15.

**What to do.** Stop every writer of the column. Then overwrite and drop
it in a reviewed migration, without archiving or copying it. If the column
only records the outcome of a check, rename it so (`…_check`).

**Automatic fix.** None.

### `db.sensitive-column-plaintext` (warning)

**What it means.** A card-number, national-identifier (`ssn`,
`passport_number`) or health column shows no sign of protection: no
protecting name (`encrypted_…`, `…_hash`, `…_token`, `…_last4`), no
encryption or hash call on it, no `sloptopop:protected=` column comment, no
Supabase Vault reference.

**Why it matters.** A database leak exposes the values directly.
Encryption of the storage layer cannot be seen from the repository.

**Controls.** Standards: SCF CRY-07.

**What to do.** Encrypt the value, keep only a hash or a token of it, or
keep a truncated form. If it is protected in a way the repository cannot
show, declare it with a column comment:
`COMMENT ON COLUMN <table>.<column> IS 'sloptopop:protected=…'`.

**Automatic fix.** None.

### `db.audit-trail-missing` (warning)

**What it means.** A table of contracts, invoices, payments, refunds,
payouts, ledgers, receipts or consents, or a table with a sensitive
column, has no audit trigger and no audit-extension call. Nothing is
reported when `pgaudit.log` logs writes.

**Why it matters.** Nobody can tell later who changed or deleted a
business record, or when.

**Controls.** Standards: SCF MON-09, MON-09.1.

**What to do.** Record every change of the table in an append-only audit
table, with the operation, the old and new row, and the acting user.

**Automatic fix.** None yet.

### `db.audit-table-mutable` (warning)

**What it means.** An audit or history table can be changed: a `GRANT`
(or, in a Supabase project, the default privileges of `anon` and
`authenticated`) lets a role update, delete or truncate it, or JavaScript
or TypeScript code updates or deletes from it outside a purge or retention
job.

**Why it matters.** An audit trail that can be edited proves nothing.

**Controls.** Standards: SCF MON-12.

**What to do.** Revoke `UPDATE`, `DELETE` and `TRUNCATE` on the table from
every application role. Grant `SELECT` only to the roles that read it.
Move deliberate clean-up into a retention job.

**Automatic fix.** None.

### `db.cascade-deletes-history` (warning)

**What it means.** A history, audit, log, activity, journal or trail table
references another table with `ON DELETE CASCADE`.

**Why it matters.** Deleting the row also deletes the records about it.

**Controls.** Standards: ISO 27001 A.8.15.

**What to do.** Reference the row with `ON DELETE SET NULL` or `RESTRICT`
(keeping the key value in a plain column), or soft-delete the row instead.

**Automatic fix.** None.

### `db.cascade-reach` (warning)

**What it means.** Deleting one row cascades into at least 10 other
tables. It is reported at the table and at every JavaScript or TypeScript
delete from it.

**Why it matters.** One delete silently removes a large part of the data.

**Controls.** None mapped yet.

**What to do.** Make deletes of business records explicit: use
`ON DELETE RESTRICT` for records with their own lifetime, soft-delete or
archive the root row, and delete dependent rows in code, inside one
transaction, after checking they may go.

**Automatic fix.** None.

### `db.fk-missing` (warning)

**What it means.** A column `<x>_id` sits beside a table `<x>` (or `<x>s`,
`<x>es`, `<x>ies`) but has no foreign key.

**Why it matters.** Rows can point at records that do not exist.

**Controls.** None mapped yet.

**What to do.** Clean up rows whose key matches nothing, then add the
foreign key with the `ON DELETE` action the business rule needs. If the
column deliberately points at another system, rename it so (for example
`erp_invoice_ref`).

**Automatic fix.** None yet.

### `db.duplicate-table` (warning)

**What it means.** Two tables share most of their columns: at least 6
columns making up 60 % of the smaller one, or at least 10 columns and a
word of their names. A history or audit copy is not a duplicate.

**Why it matters.** The same entity lives in two places, and the copies
drift apart.

**Controls.** None mapped yet.

**What to do.** Keep one table per entity. Merge the copies, move every
reader and writer to the one table, then drop the other. If one is a cache
or staging table of an external system, name it so.

**Automatic fix.** None yet.

### `db.repeating-column-group` (warning)

**What it means.** At least three columns are numbered without a gap
(`room1_area` … `room3_area`): a list held as columns.

**Why it matters.** The fourth item does not fit, and queries over the
list are awkward and error-prone.

**Controls.** None mapped yet.

**What to do.** Move the columns into a child table with one row per item,
migrate the data, and read the list from there.

**Automatic fix.** None yet.

## Build and delivery

These rules read the files at the repository root and directly in
`.github/workflows/`, `.githooks/` and `scripts/`. Ecosystems are detected
from the manifest at the root: `Cargo.toml`, `package.json`,
`pyproject.toml`, `go.mod`, `*.csproj`, `build.gradle(.kts)`,
`composer.json`, `Package.swift`, `CMakeLists.txt` and `*.psd1`.

### `toolchain.unpinned` (error)

**What it means.** An ecosystem's toolchain is not pinned to an exact
`X.Y.Z` version (for example `rust-toolchain.toml`, `.python-version`,
`.node-version`, `global.json`, the `go`/`toolchain` line of `go.mod`).

**Why it matters.** Different machines build with different compilers, so
a build can pass on one and fail, or behave differently, on another.

**Controls.** Report control: STD-1. Standards: SCF TDA-05.3, TDA-20; ISO
27001 A.8.25, A.8.9.

**What to do.** Pin the exact version in a file in the repository, and
make the gate fail on any other version.

**Automatic fix.** `fix` creates `rust-toolchain.toml` for Rust when no pin
file exists.

### `lockfile.missing` (error)

**What it means.** An ecosystem has no lockfile at the root (Go only when
`go.mod` has requirements; Swift only when `Package.swift` declares
packages).

**Why it matters.** Every install can resolve different dependency
versions, including a compromised one.

**Controls.** Report control: STD-1. Standards: SCF AST-14, TDA-21.4;
ASVS V15.1.2, V15.2.4; ISO 27001 A.5.21.

**What to do.** Generate the lockfile with the package manager, commit it,
and install from it in CI (`npm ci`, `cargo build --locked`,
`uv sync --locked`).

**Automatic fix.** None.

### `licence.missing` (error)

**What it means.** No file at the root starts with `LICENSE`, `LICENCE`,
`COPYING` or `UNLICENSE`.

**Why it matters.** Without a licence, others have no right to use the
code, and you cannot show under which terms it is released.

**Controls.** Report control: STD-8. Standards: SCF AST-13; ISO 27001
A.5.32.

**What to do.** Add a `LICENSE` file. Choosing a licence is a legal
decision, so sloptopop does not generate one.

**Automatic fix.** None, on purpose.

### `hook.missing` (error)

**What it means.** There is no `.githooks/pre-push`, or it does not run the
gate (`scripts/gate.sh`).

**Why it matters.** Broken code is pushed before anyone runs the checks.

**Controls.** Report control: STD-10. Standards: SCF CHG-07, TDA-17; ISO
27001 A.8.28, A.8.29.

**What to do.** Add `.githooks/pre-push` running `scripts/gate.sh`, and
enable it with `git config core.hooksPath .githooks`.

**Automatic fix.** `fix` creates `.githooks/pre-push` when it is missing.

### `ci.missing` (error)

**What it means.** No GitHub workflow or `.gitlab-ci.yml` runs the gate.

**Why it matters.** Nothing stops a change that breaks the checks from
being merged.

**Controls.** Report control: STD-11. Standards: SCF CHG-07, TDA-05.3,
TDA-17; ISO 27001 A.8.29, A.8.32.

**What to do.** Run `scripts/gate.sh` in CI on every push and pull request.

**Automatic fix.** `fix` creates `.github/workflows/gate.yml` when the
repository has no CI file at all.

### `ci.nonblocking-checks` (error)

**What it means.** A CI step that runs a test, lint or security tool
cannot fail the build: `continue-on-error: true`, `allow_failure: true`,
`|| true` or `|| exit 0`.

**Why it matters.** The check runs, fails, and nobody notices.

**Controls.** Report control: STD-11. Standards: SCF CHG-07, TDA-17; ISO
27001 A.8.29, A.8.32.

**What to do.** Remove `continue-on-error`, `allow_failure` or `|| true`,
so a failing check fails the pipeline.

**Automatic fix.** None yet.

### `tests.no-coverage-floor` (error)

**What it means.** Tests run without an enforced coverage threshold (no
marker such as `fail-under` or `coverageThreshold` in the files these
rules read).

**Why it matters.** Coverage can fall without anyone deciding it should.

**Controls.** Report control: STD-5. Standards: SCF TDA-17; ISO 27001
A.8.29.

**What to do.** Set a threshold the test run enforces: Jest
`coverageThreshold`, Vitest `coverage.thresholds`, `c8 --check-coverage`,
coverage.py `fail_under`, or your ecosystem's equivalent.

**Automatic fix.** None.

## Dependencies and imports

### `deps.unpinned` (error)

**What it means.** A dependency is declared without an exact version or
commit, in a manifest anywhere in the tree: `Cargo.toml`, `package.json`,
`requirements*.txt`, `pyproject.toml`, `composer.json`,
`build.gradle(.kts)`, `*.csproj`, `Package.swift` or `CMakeLists.txt`.

**Why it matters.** A new release, or a hijacked one, can enter the build
without a change in the repository.

**Controls.** None mapped yet.

**What to do.** Pin an exact version or commit, and commit the lockfile.

**Automatic fix.** None yet.

### `deps.circular` (error)

**What it means.** Modules import one another in a cycle. Covered: Rust,
JavaScript and TypeScript, Python, Java and Kotlin, C and C++, Go, C# and
PHP. Each cycle is reported once, at one of its files.

**Why it matters.** Modules in a cycle cannot be understood, tested or
changed one at a time, and import order can break at runtime.

**Controls.** None mapped yet.

**What to do.** Move what both sides need into a lower module that both
can import, so dependencies point one way.

**Automatic fix.** None yet.

### `deps.layering` (warning)

**What it means.** A module imports the module that contains it: a Rust
`super::` import, a JavaScript import of an `index.*` barrel in its own or
a parent directory, a Python import of its own package's `__init__.py`.

**Why it matters.** The dependency points upward, which invites cycles.

**Controls.** None mapped yet.

**What to do.** Move the shared item into a lower module that both the
parent and this module import.

**Automatic fix.** None yet.

## Analyzer reports

These rules read the reports in `.sloptopop/reports/` that
[`sloptopop collect`](collect.md) (or you) put there: `*.sarif` and
`*.sarif.json` as SARIF 2.1.0, `*.lcov`, `lcov.info` and `*.lcov.info` as
LCOV. sloptopop never runs a tool during `check`. Results about files the
check does not read (ignored, generated, deleted, or outside the
repository) are kept as metadata in the Report, never reported as
findings.

### `analyzer.finding` (error / warning)

**What it means.** An analyzer reported a problem at SARIF level `error`
or `warning`, and no accepted suppression covers it. The finding's
`origin` names the tool and the tool's own rule id. Notes stay metadata.

**Why it matters.** It is the analyzer's finding. Its documentation says
why it matters.

**Controls.** Standards: SCF TDA-17.1.

**What to do.** Fix what the analyzer reports, following its rule
documentation.

**Automatic fix.** None yet.

### `analyzer.unreadable-report` (error)

**What it means.** A file in `.sloptopop/reports/` has a report name but
cannot be used: it is larger than 32 MiB, not UTF-8, or not a valid
report.

**Why it matters.** A report that cannot be read is no evidence. sloptopop
fails closed rather than ignore it.

**Controls.** Standards: SCF CPL-30.

**What to do.** Regenerate the report with the analyzer's documented
command (SARIF 2.1.0 or LCOV), or remove it.

**Automatic fix.** None.

### `analyzer.reports-unignored` (error)

**What it means.** A report in `.sloptopop/reports/` is not ignored by git
(by the root, `.sloptopop/` or `.sloptopop/reports/` ignore file).

**Why it matters.** Reports can hold secret locations and machine paths,
and could be committed.

**Controls.** Standards: SCF IAC-14.3.

**What to do.** Add `.sloptopop/reports/` to the root `.gitignore`.
`sloptopop collect` does this for you. If reports were committed, remove
them from history and rotate any secret they held.

**Automatic fix.** `sloptopop collect` writes `.sloptopop/reports/.gitignore`
when nothing ignores the directory.

### `coverage.uncovered` (error)

**What it means.** A coverage report shows lines of a file that no test
executes, merged over every coverage report.

**Why it matters.** Untested lines can break without any test failing.

**Controls.** Report control: STD-5. Standards: SCF CHG-07, TDA-14; ISO
27001 A.8.29.

**What to do.** Write tests that execute these lines and check their
behaviour, or remove the code if nothing can reach it.

**Automatic fix.** None.

## Architecture

Checks of the import graph between components (a directory, Go package or
C# namespace), the deployable units (Compose services, Dockerfiles,
Kubernetes workloads, executables declared by build manifests) and
governance files. Each rests on a published rule or threshold. A role
inferred from names only ever gives a warning. None of these rules has an
automatic fix yet, and no control is mapped yet except where shown.

| Rule | Severity | What it means | What to do |
|---|---|---|---|
| `arch.module-internals-access` | error / warning | An import past a module's public interface: a subpath a workspace package's `exports` does not export (error); another Spring Modulith module's internal package (error); another Python package's `_private` name or a name outside its `__all__` (warning); a component's `internal`, `impl` or `private` directory, or past its facade (warning). | Import through the public interface, or make the item public there. |
| `arch.package-cycle` | warning | Components depend on one another in a cycle (unless one file-level cycle that `deps.circular` reports explains it). | Move what they share into a lower component, or invert one dependency through an interface. |
| `arch.layer-violation` | warning | A dependency breaks the rules of a recognised architecture style (layered, hexagonal, clean or onion, MVC, MVVM, Feature-Sliced Design). | Make the dependency point the way the style allows, or declare the intended structure in an architecture rule file. |
| `arch.db-access-outside-persistence` | warning | A database connection or query API is imported outside a named persistence layer. Standards: CWE-1057. | Move the access into the persistence layer and call that instead. |
| `arch.unstable-dependency` | warning | At least 30 % of a component's dependencies are less stable than itself (Stable Dependencies Principle). | Depend on stabler components, or invert the dependency through an abstraction the stable side owns. |
| `arch.hub-dependency` | warning | A component has many dependents and many dependencies at once. | Split it: separate what its dependents need from what it needs. |
| `arch.god-component` | warning | A component has more than 27,000 lines of code, or more than 30 one-type files in Java, Kotlin, C# or PHP. | Split it along the concerns it mixes. |
| `arch.duplicate-across-units` | warning | The same source file (more than 20 lines) is copied into two deployable units or build modules. | Keep one copy in a shared module both depend on, or let one unit own it behind an API. |
| `arch.shared-schema-owner` | warning | Two or more units migrate the same database. | Give the schema one owner; the others reach the data through its API. |
| `arch.healthcheck-missing` | warning | A long-running Compose service or Kubernetes container has no health check or probe. | Declare a health check (Compose `healthcheck:`, Dockerfile `HEALTHCHECK`, or readiness and liveness probes). |
| `arch.rules-missing` | warning | A structured repository (several build modules or units, or a recognised style) has no executable architecture rules (ArchUnit, dependency-cruiser, import-linter, go-arch-lint, deptrac, …). | Add architecture rules and run them in CI. |
| `arch.rules-not-in-ci` | warning | Architecture rules are configured, but no blocking CI step runs them. | Run them in a blocking CI step. |
| `arch.adr-missing` | warning | A structured repository records no architecture decisions. | Record each significant decision as an ADR (for example MADR in `docs/decisions/`). |
| `arch.threat-model-missing` | warning | A network-facing unit has no threat model (a named file, `.tm7`, OWASP Threat Dragon, pytm). | Model the threats to each network-facing unit and keep the model with the code. |

`arch.privileged-key-client-side` is described under
[Secrets and transport security](#archprivileged-key-client-side-error--warning).

## Layout

Files that build, test or forge tools silently skip, misread or never find
because of where they sit or how they are named. Each rule rests on the
tool's own documentation: an error where the tool depends on the layout,
a warning where the documentation recommends it.

- Findings inside fixture directories (`testdata`, `fixtures`,
  `__fixtures__`, `test-fixtures`, `snapshots`) are skipped.
- Rules for one ecosystem run only when its manifest is at the root, or in
  the directory concerned.
- No control is mapped to these rules yet, and none has an automatic fix
  yet.
- Why they matter, in every case: the file does nothing, or does the wrong
  thing, and nothing tells you.

| Rule | Severity | What it means | What to do |
|---|---|---|---|
| `layout.gh-workflow-location` | error | A GitHub Actions workflow sits in a subdirectory of `.github/workflows/` or in `.github/workflow/`, so it never runs. | Move it directly into `.github/workflows/`. |
| `layout.dependabot-location` | error | `dependabot.yml` is outside `.github/`, so it is never read. | Move it to `.github/dependabot.yml`. |
| `layout.codeowners-location` | error | `CODEOWNERS` is outside the root, `.github/`, `docs/` and `.gitlab/`, so it is never read. | Move it to `.github/CODEOWNERS`, the root or `docs/`. |
| `layout.cmake-lists-case` | error | A CMake list file is not named exactly `CMakeLists.txt`. | Rename it to exactly `CMakeLists.txt`. |
| `layout.cmake-in-source-build` | warning | CMake build output (`CMakeCache.txt`, `CMakeFiles/`, `cmake_install.cmake`) sits next to a `CMakeLists.txt`. | Build out of source (`cmake -S . -B build`) and delete or ignore the generated files. |
| `layout.cmake-user-presets` | warning | `CMakeUserPresets.json` is not ignored. | Add it to `.gitignore`. |
| `layout.editorconfig-case` | warning | The EditorConfig file is not named `.editorconfig` in lower case. | Rename it to `.editorconfig`. |
| `layout.build-output-unignored` | warning | A root build-output or dependency directory (`target`, `build`, `bin`, `obj`, `node_modules`, `__pycache__`, `dist`, `.build`, `vendor`, …) is not in the root `.gitignore`. | Add it to the root `.gitignore`. |
| `layout.kotlin-java-in-kotlin-dir` | error | A `.java` file sits under `src/<set>/kotlin/`, where Gradle never compiles it. | Move it to `src/<set>/java/`. |
| `layout.jvm-sources-outside-src` | warning | A Java or Kotlin file sits outside every `src/` directory of its build. | Move it under `src/main/…` or `src/test/…`, or configure the source set. |
| `layout.gradle-wrapper-incomplete` | error / warning | `gradlew` without `gradle/wrapper/gradle-wrapper.properties` (error), or a Gradle build without `gradlew` (warning). | Run `gradle wrapper` and commit `gradlew`, `gradlew.bat` and `gradle/wrapper/`. |
| `layout.gradle-settings-missing` | warning / error | A root Gradle build without a settings file (warning), or a build file no settings file covers (error). | Add `settings.gradle.kts` at the root, including every subproject. |
| `layout.gradle-root-sources` | warning | The root project keeps sources in `src/main/` although the build has subprojects. | Move the root project's sources into a subproject. |
| `layout.maven-test-naming` | warning | A JUnit or TestNG class is named so that neither Surefire nor Failsafe runs it. | Rename the class to end in `Test` (unit) or `IT` (integration). |
| `layout.dotnet-nested-project` | error | A .NET project sits inside another project's directory, so its files compile twice. | Move the inner project into a sibling folder. |
| `layout.swift-sources-outside-targets` | error | A `.swift` file belongs to no Swift package target. | Move it into `Sources/<Target>/`, `Tests/<Target>/` or `Plugins/<Target>/`. |
| `layout.python-flat-multi-package` | error | A setuptools flat layout has several top-level packages, so it does not build. | Move the packages under `src/`, or list them in `[tool.setuptools] packages`. |
| `layout.python-package-name` | error / warning | A package directory name is not a valid identifier (error) or not lower case (warning). | Rename the directory. |
| `layout.python-src-is-package` | warning | `src/__init__.py` makes `src` itself the package. | Remove `src/__init__.py`; keep the package in `src/<package>/`. |
| `layout.python-test-naming` | warning | A test file is named neither `test_*.py` nor `*_test.py`, so pytest does not collect it. | Rename it. |
| `layout.go-ignored-source` | warning | A `.go` file name starts with `_` or `.`, so the go tool ignores it. | Rename it, or delete it if it is parked code. |
| `layout.go-testdata-name` | warning | A fixture directory with `.go` files is not named `testdata`, so it is compiled. | Rename it to `testdata`. |
| `layout.go-package-dir-case` | warning | A Go package directory has upper-case letters. | Rename it in lower case. |
| `layout.rust-orphan-target-dir` | warning | A directory under `tests/`, `examples/`, `benches/` or `src/bin/` holds `.rs` files that Cargo never compiles. | Add a `main.rs`, or declare it with `mod <name>;` from a target. |
| `layout.rust-test-helper-as-crate` | warning | A shared test helper `tests/<name>.rs` is also built as an empty test crate. | Move it to `tests/<name>/mod.rs`. |
| `layout.rust-target-names` | warning | A Cargo target is named in neither kebab-case nor `snake_case`. | Rename it in kebab-case. |
| `layout.php-psr4-filename` | error | A PHP file is not named after the class it declares, as PSR-4 requires. | Rename the file to `<ClassName>.php`, case included. |
| `layout.ps-module-dir-name` | error | A PowerShell module directory is not named after its manifest. | Rename the directory to the manifest's base name. |
| `layout.pester-test-naming` | warning | A Pester test file is not named `*.Tests.ps1`. | Rename it to `<Name>.Tests.ps1`. |

## Attribution

- **SCF:** control identifiers from the Secure Controls Framework 2026.3,
  © SCF Council, licensed under CC BY-ND 4.0. Only identifiers are
  referenced.
- **OWASP ASVS:** requirement identifiers from the OWASP Application
  Security Verification Standard 5.0.0, © OWASP Foundation, licensed under
  CC BY-SA 4.0. Only identifiers are referenced.
- **ISO/IEC 27001:2022:** Annex A control numbers only. © ISO/IEC. No ISO
  text is reproduced, and ISO does not endorse these mappings.
- **NIS2:** Commission Implementing Regulation (EU) 2024/2690, from
  EUR-Lex, © European Union. Only the Official Journal texts are
  authentic.
- **CWE:** identifiers from the Common Weakness Enumeration, © The MITRE
  Corporation.
- **PCI DSS** and **OWASP API Security Top 10** are named for reference
  only.

The descriptions on this page are sloptopop's own.
