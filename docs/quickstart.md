# Quickstart

This walk-through runs sloptopop on a small sample repository: check it,
collect analyzer reports, check again, then transform it. It takes about
ten minutes. You need `sloptopop` and `sloptopop-engine` installed (see
[install.md](install.md)), plus `git`.

## 1. Create the sample repository

The sample is a tiny Next.js shop on Supabase. It has four typical
problems: a committed token, a database write whose error is ignored, a
swallowed error, and no CI.

```sh
mkdir sample-shop && cd sample-shop && git init
mkdir -p app/api/orders lib scripts
```

`package.json`:

```json
{
  "name": "sample-shop",
  "private": true,
  "dependencies": {
    "@supabase/supabase-js": "2.45.0",
    "next": "14.2.5"
  }
}
```

`lib/db.ts`:

```ts
import { createClient } from '@supabase/supabase-js';

export const db = createClient(
  process.env.SUPABASE_URL ?? '',
  process.env.SUPABASE_ANON_KEY ?? '',
);
```

`app/api/orders/route.ts`:

```ts
import { db } from '../../../lib/db';

export async function POST(request: Request) {
  const order = await request.json();
  await db.from('orders').insert(order);
  try {
    await notifyWarehouse(order.id);
  } catch (e) {
    // ignore
  }
  return Response.json({ ok: true });
}

async function notifyWarehouse(id: string) {
  await fetch(`https://warehouse.example.com/orders/${id}`, { method: 'POST' });
}
```

`scripts/upload.js`. Replace `ghp_…` with any string in the GitHub token
format: `ghp_` followed by 36 letters and digits. Do not use a real token.

```js
const API_TOKEN = 'ghp_…';

async function main() {
  await fetch('https://api.example.com/upload', {
    headers: { Authorization: `Token ${API_TOKEN}` },
  });
}

main();
```

Commit it, so that `transform` has a clean tree to start from:

```sh
git add -A && git commit -m "Sample shop"
```

## 2. Check

```sh
sloptopop check
```

```
sloptopop check: FAILED (9 findings; ecosystems: node)

Errors (8)
  .github/workflows/gate.yml  ci.missing  [STD-11]
    No CI workflow
    fix: `sloptopop fix --apply` creates .github/workflows/gate.yml
    how: Run the full gate (`scripts/gate.sh`) in CI on every push and pull request.
  app/api/orders/route.ts:5  db.result-unchecked
    The result of this insert on `orders` is never checked for an error, so a failed write passes silently
    how: Read `error` from every query result and handle it (return an error response, roll back, retry or stop), or call `.throwOnError()` so failures throw.
  .githooks/pre-push  hook.missing  [STD-10]
    No pre-push hook
    fix: `sloptopop fix --apply` creates .githooks/pre-push
    how: Add `.githooks/pre-push` running the full gate (`scripts/gate.sh`) and enable it with `git config core.hooksPath .githooks`.
  LICENSE  licence.missing  [STD-8]
    No licence file at the repository root
    how: Add a LICENSE file stating the licence the code is released under; choosing a licence is a legal decision, so no file is generated.
  package-lock.json  lockfile.missing  [STD-1]
    No Node.js lockfile (expected one of: package-lock.json, npm-shrinkwrap.json, pnpm-lock.yaml, yarn.lock, bun.lock, bun.lockb)
    how: Generate the lockfile with the package manager, commit it, and install from it in CI (for example `cargo build --locked`, `npm ci`, `uv sync --locked`).
  scripts/upload.js:1  secrets.committed
    GitHub token committed
    how: Rotate the credential now and treat it as compromised, then remove it from the repository and its history (for example with git filter-repo) and load it from the environment or a secret manager.
  package.json  tests.no-coverage-floor  [STD-5]
    No enforced coverage threshold for Node.js
    how: Set a threshold the test run enforces: Jest `coverageThreshold`, Vitest `coverage.thresholds`, or `c8 --check-coverage`.
  .node-version  toolchain.unpinned  [STD-1]
    Node.js toolchain is not pinned to an exact version
    how: Pin the exact toolchain version in a file in the repository and make the gate fail on any other version.

Warnings (1)
  app/api/orders/route.ts:8  errors.swallowed
    Error handler discards the error
    how: Handle the error, propagate it, or log it with context; never discard it silently.

Controls
  STD-1   fail        Pinned toolchain
  STD-2   notChecked  Formatting
  STD-3   pass        Zero warnings
  STD-4   notChecked  All tests pass
  STD-5   fail        100% coverage
  STD-6   notChecked  Mutation testing
  STD-7   notChecked  Dependency vulnerabilities
  STD-8   fail        Dependency licences
  STD-9   notChecked  Docs
  STD-10  fail        Pre-push hook
  STD-11  fail        CI
```

How to read it:

- Each finding shows the file and line, the rule, and the control it
  counts against in brackets. [rules.md](rules.md) explains every rule.
- `fix:` means `sloptopop fix --apply` can create the missing file.
- `how:` is what to do by hand.
- `notChecked` means no rule gives evidence about that control yet.
- The exit code is 1, because the repository did not pass.

For the machine-readable Report, run `sloptopop check --format json` (see
[report-format.md](report-format.md)).

## 3. Collect analyzer reports

`sloptopop collect` runs the open-source analyzers that apply, each from
its pinned container image, and saves their reports for the next `check`.
It needs a container runtime (see [install.md](install.md#analyzers-a-container-runtime)).
First see what it would run:

```sh
sloptopop collect --dry-run
```

Each line shows a tool as `run` (with its image, version and download
size, and the exact command line) or `skipped`. Then run it for real:

```sh
sloptopop collect
sloptopop check
```

The first run prints a `fetch` line for each image before it downloads it;
the next runs reuse them. gitleaks' result for `scripts/upload.js` also appears as
`secrets.committed`. Other analyzers' problems appear as
`analyzer.finding`, with the tool and its own rule id. `collect` also makes
sure git ignores `.sloptopop/reports/`, because reports can hold secret
locations and machine paths.

## 4. Create the missing files

```sh
sloptopop fix            # dry run: lists what it would create
sloptopop fix --apply    # creates them
```

```
sloptopop fix: dry run, nothing written
  planned  .githooks/pre-push  (template pre-push)
  planned  .github/workflows/gate.yml  (template gate-workflow)
Run `sloptopop fix --apply` to create the planned files.
```

Both files run `scripts/gate.sh`, your project's own quality gate, which
you write yourself. `fix` never overwrites a file that already exists.

## 5. Transform (preview)

> **Preview.** `transform` works end to end, but output and options can
> still change, and the hosted backend is not open yet: pass `--backend
> <url>` with a backend you run (`backend-engine serve`).

Commit or stash any changes first: `transform` refuses a dirty tree. Name
who may confirm behaviour changes in `.sloptopop/context.json` (keep
`.sloptopop/` in `.gitignore`):

```json
{ "owners": { "approvers": ["you"], "operators": ["you"], "data": [] } }
```

```sh
sloptopop transform --backend <url>     # shows what it uploads, asks, then a dry run
sloptopop transform --backend <url> --apply --confirm att-1,att-2,att-3 --by you
```

The first run lists the declared behaviour changes to confirm (`att-1`
…) under "Waiting for confirmation"; the second confirms them and
applies the verified steps on a branch.

On the sample, the plan fixes three findings automatically and leaves one
manual task. The steps and commits may group these edits differently.

**Read the token from the environment** (`scripts/upload.js`):

```diff
-const API_TOKEN = 'ghp_…';
+const API_TOKEN = (process.env.GITHUB_TOKEN ?? (() => { throw new Error("GITHUB_TOKEN is not set"); })());
```

The key `GITHUB_TOKEN` comes from the token's format. The same step
adds `GITHUB_TOKEN=` (no value) to `.env.example`.

**Make the failed write abort the request**
(`app/api/orders/route.ts`):

```diff
-  await db.from('orders').insert(order);
+  await db.from('orders').insert(order).throwOnError();
```

**Stop discarding the error** (`app/api/orders/route.ts`). The sample
has no project logger, so the error is rethrown:

```diff
   } catch (e) {
     // ignore
+    throw e;
   }
```

**Manual task: retire the exposed token.** sloptopop cannot do this for
you. The summary gives the checklist:

- Purge the old value from git history and forks.
- Revoke the old key.
- Rotate the key at GitHub and set the new value in every environment.

The token step and the two route steps change behaviour on purpose (the
script now stops without `GITHUB_TOKEN`; a failure that was silent is now
loud). Each waits until you confirm its statement with `--confirm`. See [transformations.md](transformations.md) for every
transformation, and [safety.md](safety.md) for how each step is verified.

With `--apply`, you get:

- a new branch with one commit per step, your checkout untouched;
- `.sloptopop/transform/<run-id>/SUMMARY.md`: what changed and why, the
  refusals and the manual tasks;
- `.sloptopop/transform/<run-id>/PR.md`: a pull-request description.

Review the branch, run your tests, set `GITHUB_TOKEN` in every
environment that runs the script, then merge.

## Next

- [commands.md](commands.md): every command and option.
- [rules.md](rules.md): what each finding means and what to do.
- [faq.md](faq.md) and [troubleshooting.md](troubleshooting.md).
