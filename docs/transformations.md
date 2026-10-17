# Transformations

sloptopop detects problems first, then transforms the code to fix them.
This page lists every automatic transformation: what it changes, when it
applies, when it leaves the work to you, and what is checked before
anything is applied.

> `sloptopop transform`, the command that applies these, is in **preview**
> (see [commands.md](commands.md#transform-preview)). The transformations
> marked **coming** are designed and decided, but not yet available.

| Transformation | Rule | Languages | Status |
|---|---|---|---|
| [Secrets → environment reads](#secrets--environment-reads) | `secrets.committed` | JavaScript, TypeScript | available |
| [Unchecked writes → failures that throw](#unchecked-writes--failures-that-throw) | `db.result-unchecked` (writes) | JavaScript, TypeScript (supabase-js) | available |
| [Swallowed errors → observed errors](#swallowed-errors--observed-errors) | `errors.swallowed` | JavaScript, TypeScript | available |
| [Secrets in env files](#secrets-in-env-files-coming) | `secrets.committed` (`.env*` files) | any | coming |
| [Module context inference](#module-context-inference-coming) | `errors.swallowed`, `db.result-unchecked` | JavaScript, TypeScript | coming |
| [Hard-coded hosts → configurable](#hard-coded-hosts--configurable-coming) | `config.hardcoded-host` | JavaScript, TypeScript | coming |
| [Unchecked reads](#unchecked-reads-coming) | `db.result-unchecked` (reads) | JavaScript, TypeScript (supabase-js) | coming |

`sloptopop fix` also creates a few missing files from templates. Those are
listed in [commands.md](commands.md#fix).

## How a transformation runs

1. **Detect.** The Detector reports the finding, with its file and line.
2. **Describe.** The backend builds a description of the code: its
   modules, symbols, queries, error handlers and constant values. It never
   judges; it only records facts, with the exact byte range of each one.
3. **Plan.** The planner places the finding on exactly one site in that
   description. Then it chooses a fixed method, or a manual task with the
   reasons. The planner is rule-based: the same input always gives the
   same plan.
4. **Apply.** Each edit replaces an exact byte range. Before writing, it
   checks that the bytes there are still the ones the plan was made from
   (compare-and-swap on a digest). It writes nothing outside its range.
5. **Verify.** The Guider checks the step independently. Only a step that
   passes every check is kept. See [What the Guider checks](#what-the-guider-checks).
   The backend applies and verifies every step on the uploaded snapshot,
   with the checks that need the description of the whole code. Your
   machine then applies the step again and keeps it only if every file it
   changes has exactly the bytes the backend verified.
6. **Deliver.** Each kept step is one commit on a new branch, on top of
   the previous step (one branch, stacked commits). The first step that is
   not kept stops the delivery: the steps after it build on it.

### Applied, refused or manual

Every finding a transformation covers ends in one of three states:

| State | Meaning |
|---|---|
| **Applied** | The edit was made on the branch and the Guider verified it. |
| **Manual** | The plan did not automate it, on purpose. The report gives the reasons (the codes below) and a checklist for you. |
| **Refused** | An automated step failed a Guider check, so nothing of it was applied. The report names the check that failed. The backend plans again without that problem (at most three rounds), so one refusal does not hold up the others. |
| **Pending** | A declared change waits for a person's confirmation; the steps after it wait too. |

"Refuse, never wrongly apply": when in doubt, sloptopop leaves the code
alone and tells you why.

### Equivalent and declared changes

- An **equivalent** change keeps the program's behaviour. Example: an
  environment read whose default is the old literal.
- A **declared** change alters behaviour on purpose, and says exactly how.
  Example: a failed write now throws instead of passing silently.

A declared change carries a statement for a person to confirm, such as
"a failed write to orders now aborts the operation instead of
continuing". It is not committed until a person listed in
`.sloptopop/context.json` confirms it with `sloptopop transform --apply
--confirm <id> --by <name>` (see
[commands.md](commands.md#declared-behaviour-changes-need-a-person)). The
transformation report and the pull-request description list each
statement and who confirmed it.

### Where the code runs

Several transformations turn silence into a thrown error. That is safe
only where a thrown error reaches something that handles it. The planner
decides where each module runs:

| Context | Examples | Thrown error goes to |
|---|---|---|
| server | Next.js `app/**/route.(ts\|js)` handlers, `pages/api/**`, modules with `'use server'` (server actions), modules that import `server-only` | the framework: a 500 response or an error boundary |
| script | `scripts/**`, files with a `#!` line, files whose only entry is a top-level `main()` call | the runtime: the process exits non-zero |
| client | modules with `'use client'`, React component files (`.tsx`/`.jsx` exporting a component) | nothing: the browser. Never transformed |
| unknown | anything else | not known. Left manual (`handler-unknown`) |

[Module context inference](#module-context-inference-coming) will decide
more modules (shared library code, command-line tools).

## Secrets → environment reads

**Rule:** `secrets.committed`, from the Detector or a secret scanner such
as gitleaks.

**What it does.** A secret string in a top-level constant of a JavaScript
or TypeScript module is replaced by a required environment read. The
variable name goes into `.env.example` with no value. Rotating the leaked
credential is always a manual task: the old value is still in git history.

### Before and after

Without a config module (a script, for example):

```js
// before
const API_TOKEN = 'ghp_…';

// after
const API_TOKEN = (process.env.GITHUB_TOKEN ?? (() => { throw new Error("GITHUB_TOKEN is not set"); })());
```

An existing environment read with a secret fallback loses the fallback.
The variable it already reads is kept:

```ts
// before
const SUPABASE_KEY = process.env.SUPABASE_SERVICE_ROLE_KEY || 'eyJ…';

// after
const SUPABASE_KEY = (process.env.SUPABASE_SERVICE_ROLE_KEY ?? (() => { throw new Error("SUPABASE_SERVICE_ROLE_KEY is not set"); })());
```

`.env.example` gains the key with no value:

```
SUPABASE_SERVICE_ROLE_KEY=
```

With one config object in the project (`config.ts` exporting `config`),
the key is added there instead, and the program refuses to start without
it:

```ts
// config.ts, step A
export const config = {
  stripeUrl: process.env.STRIPE_URL,
  STRIPE_SECRET_KEY: process.env.STRIPE_SECRET_KEY,
};

// billing.ts, step A
import { config } from './config';
const stripe = new Stripe(config.STRIPE_SECRET_KEY);

// config.ts, step B: a separate step, after step A is deployed
if (!config.STRIPE_SECRET_KEY) throw new Error('STRIPE_SECRET_KEY is not set');
```

### The variable name

- An existing environment read keeps its variable (`SUPABASE_SERVICE_ROLE_KEY`
  above).
- A known token format uses the provider's usual name: `AWS_SECRET_ACCESS_KEY`,
  `GITHUB_TOKEN`, `GITLAB_TOKEN`, `SLACK_TOKEN`, `STRIPE_SECRET_KEY`.
- Otherwise the constant's name in upper snake case (`apiToken` →
  `API_TOKEN`).
- A name already in use gets `_2`, `_3`.

### When it applies

- The secret is a string literal (or a template literal without
  substitutions) initialising a top-level `const`, `let` or `var`, or the
  fallback in `process.env.X || '…'` or `process.env.X ?? '…'`.
- Or it is a literal passed straight to a known client constructor
  (`new Stripe('sk_live_…')`), and the project has one config object.
- The finding's line matches exactly one such value.
- The file is JavaScript or TypeScript.

The change is **declared**: the program now needs the variable. Its
statement for the operator: "`<KEY>` is set to the current value in every
environment that runs this service".

### When it is manual

Every secret plan ends with a manual task: **retire the exposed
credential**. The checklist:

- Purge the old value from git history and forks.
- Revoke the old key.
- Rotate the key at the provider and set the new value in every
  environment.

The whole fix is manual when:

| Reason | What it means |
|---|---|
| `no-value-site` | No constant value sits at the finding's line (for example, the secret is inside a function). |
| `no-config-module` | The fix needs the project's config object, and there is not exactly one. |
| `several-value-sites` | Several values sit at that line, so the target is ambiguous. |
| `not-single-use` | The literal fills no slot, or several (for example in a destructuring). |
| `no-env-key` | No valid environment variable name can be derived. |
| `opaque` | Part of the code around the site could not be described, so the change cannot be checked. |
| `config-import-cycle` | The config module imports the secret's module, so the config may not be loaded first. |
| `external-system` | The work happens outside the repository (rotation, revocation). |

### What the Guider checks

- The bytes at the edit's range are the plan's (digest), and they are the
  literal value the description recorded at that site.
- The file is JavaScript or TypeScript, the read is required, and a
  fallback site keeps the variable the code already reads (checks
  `GP022`, `GP023`, `GV401`).
- The change to the whole program's description is exactly the declared
  one: the literal value is gone, an environment value is present, and
  nothing else changed (`GV501`).
- No literal value is left at that site (`GV502`).

## Unchecked writes → failures that throw

**Rule:** `db.result-unchecked` at error level (a write: `insert`,
`upsert`, `update`, `delete`).

**What it does.** A supabase-js write whose error is never read gets
`.throwOnError()`, or an explicit check, so a failed write aborts the
operation instead of continuing as if the row was written.

### Before and after

The result is dropped, or destructured without `error`: the modifier goes
at the end of the builder chain, after every filter.

```ts
// before
await db.from('orders').insert(order);
const { data } = await db.from('orders').update(changes).eq('id', id);

// after
await db.from('orders').insert(order).throwOnError();
const { data } = await db.from('orders').update(changes).eq('id', id).throwOnError();
```

`error` is destructured but never read: a check goes on the next line,
indented like the statement.

```ts
// before
const { data, error } = await db.from('orders').delete().eq('id', id);

// after
const { data, error } = await db.from('orders').delete().eq('id', id);
if (error) throw error;
```

### When it applies

- The write runs in a **server** or **script** context (see
  [Where the code runs](#where-the-code-runs)).
- The finding's line falls inside exactly one write query.
- The result is dropped or destructured, not passed on.

The change is **declared**. Its statement for the approver: "a failed
write to `<table>` now aborts the operation instead of continuing".

### When it is manual

| Reason | What it means |
|---|---|
| `unhandled-in-client` | The write runs in a client module or a component file. A throw there reaches no handler. |
| `handler-unknown` | It is not known what handles an error thrown in this module. |
| `result-chained` | The result is chained with `.then`, `.catch` or `.finally`. |
| `result-passed-on` | The result is returned or assigned, so the caller decides what to do with it. |
| `result-in-expression` | The query is part of a larger expression, such as `Promise.all([…])`. |
| `error-read` | The error is already read somewhere. |
| `in-callback` | The query sits in a callback nested in its function. |
| `no-query-site` | No write query with the byte ranges an edit needs is at the finding's line. |
| `several-query-sites` | Several write queries are at that line. |

The manual checklist:

- Make sure whoever calls the function turns a thrown error into a
  failure response.
- Read the write's `error` and stop on failure: return an error response,
  roll back or rethrow.

### What the Guider checks

- The bytes at the edit's range are the plan's, the query is a write, and
  the edit sits at the end of its chain (modifier) or right after its
  statement (check) (`GP024`, `GV401`).
- The module is not a client module (`GV403`).
- After the edit, the query observes and reads its error, so the
  Detector's condition no longer holds (`GV502`).
- Nothing else in the program's description changed (`GV501`). The only
  allowance: the body of the function that owns the query, which any edit
  inside it changes.

## Swallowed errors → observed errors

**Rule:** `errors.swallowed`, in JavaScript and TypeScript.

**What it does.** An empty `catch` block is made to report the error:
logged through the project's own logger and rethrown, or only rethrown
when there is no logger. A fire-and-forget `.catch(() => {})` is made to
log the error, never to rethrow it, because a rethrow there would become
an unhandled rejection.

### Before and after

A server module where the project has a logger:

```ts
// before
try {
  await syncInvoices();
} catch (e) {}

// after
import { logger } from './lib/logger';
…
try {
  await syncInvoices();
} catch (e) {
  logger.error("runSync: syncInvoices failed", e);
  throw e;
}
```

No logger reachable: rethrow only.

```ts
} catch (e) { throw e; }
```

A `catch` without a binding gets one:

```ts
// before
} catch {
}

// after
} catch (error) {
  throw error;
}
```

Fire-and-forget, with a logger:

```ts
// before
sendReceipt(order).catch(() => {});

// after
sendReceipt(order).catch((error) => logger.warn("placeOrder: sendReceipt failed", error));
```

Details:

- The log message is fixed text with no data: `"<function or file
  name>: <operation> failed"`. The operation is the one awaited call in
  the `try`, or `operation` when there is not exactly one. No user data or
  secret goes into the log.
- The logger is the module the server code imports most often under the
  name `logger`, and only if that module is itself server-side. A server
  logger is never imported into client code. The import is added once per
  file.
- A new binding is named `error`. It is named `caughtError` when the same
  function has a query whose `error` is unread, so the fix does not hide
  that query's own finding.

### When it applies

- The module is **server** or **script** (see
  [Where the code runs](#where-the-code-runs)).
- The handler is truly empty. A handler that holds only a comment is
  treated as a deliberate choice and goes to a person instead (see
  `commented-intent` below).

The change is **declared**: a rethrow changes behaviour, and a log line
adds output. Its statement for the approver: "errors in `<file>` are now
reported and propagated instead of ignored" (or "logged").

### When it is manual

| Reason | What it means |
|---|---|
| `client-catch` | The handler is in a client module or a component file, where neither a rethrow nor a server logger belongs. |
| `commented-intent` | The handler holds only a comment. A comment marks the swallow as deliberate (a framework's documented idiom, a probe with a fallback, best-effort cleanup), so a person confirms the reason still holds. |
| `in-handler` | The `catch` sits inside another handler's body, where a rethrow would replace the error being handled. |
| `handler-unknown` | It is not known where the module runs. |
| `no-logger-for-fire-and-forget` | A fire-and-forget `.catch` in a project with no reachable logger. Rethrowing would be an unhandled rejection. |
| `handler-has-statements` | The handler already does something. |
| `catch-in-finally` | The `catch` sits in a `finally` block. |
| `loop-continues` | The `catch` sits in a loop that it lets continue on purpose. |
| `binding-pattern` | The `catch` destructures the error. |
| `generated-file` | The file is generated. |
| `logger-name-taken` | The name `logger` already means something else in the file. |
| `no-handler-site` | No handler with the byte ranges an edit needs starts at the finding's line. |
| `several-handler-sites` | Several handlers start at that line. |

The manual checklist:

- Handle the error, report it through a server-side logger, or propagate
  it.
- Keep data out of the log message.

### What the Guider checks

- The bytes at each range are the plan's; the handler is empty (a
  comment-only handler is refused) and of the kind the edit expects; an added import is not
  bound yet (`GP025`, `GP026`, `GV401`).
- The log message is the fixed form with no data (`GP025`).
- The file is not client code: no `'use client'`, and not a component file
  outside server code (`GV404`).
- After the edit, the handler rethrows and/or logs, and the module binds
  the logger (`GV502`).
- Nothing else in the program's description changed, apart from the body
  of the function that owns the handler (`GV501`).

## Secrets in env files (coming)

**Rule:** `secrets.committed` on a tracked `.env*` file.

**What it will do.**

- **Example files** (`.env.example`, `.env.sample`, `.env.template`,
  `*.env.example`, `*.env.sample`, `*.env.template`, `.env.*.example`):
  blank each value, keeping the key, the comments and the order. This is a
  **declared** change to documentation only; applications do not load
  example files.

  ```
  # before                                # after
  STRIPE_SECRET_KEY=sk_live_…             STRIPE_SECRET_KEY=
  ```

- **Real env files** (any other tracked `.env` or `.env.<name>`): remove
  the file from git's index and add a `.gitignore` line when it is not
  already ignored. The file's contents are never read, moved or deleted:
  your local copy stays. An example file with the keys and blank values is
  created, or missing keys are appended to it.
- In both cases, a manual task to rotate the credentials, because the
  values stay in git history.

**Manual:** multi-line values and interpolated values (`${…}`).

**What the Guider will check:** the same principles as for every
transformation: exact byte ranges, the declared change only, and the
finding gone afterwards.

## Module context inference (coming)

**Rules:** `errors.swallowed` and `db.result-unchecked`, wherever their fix
depends on where the code runs.

**What it will do.** Decide the context of more modules, so fewer
findings stay manual as `handler-unknown`. The first rule that decides
wins:

1. **The module's own evidence.** `'use client'` → client. The existing
   server rules → server. Next.js App Router `page`, `layout`,
   `template`, `default`, `not-found` and `loading` files without
   `'use client'` are server components → server. Importing `next/headers`,
   a `node:` module or a Node-only built-in (`fs`, `child_process`, `net`,
   `http`, …), or a server framework or driver (`express`, `fastify`,
   `pg`, `@prisma/client`, …) → server.
2. **The package.** A package with a `bin` field and no UI framework
   dependency is a command-line tool → server.
3. **The import graph.** A module imported by any client module → client.
   A module whose importers are all server → server.
4. Anything else stays unknown.

The report records which rule decided each step. The Guider recomputes the
context on its own and refuses a step whose context is not server.

The same update adds one manual reason for swallowed errors:

| Reason | What it means |
|---|---|
| `in-handler` | The `catch` sits inside another handler's body. A rethrow there would replace the error being handled and skip the rest of the outer handler. |

## Hard-coded hosts → configurable (coming)

**Rule:** `config.hardcoded-host`.

**What it will do.** Make a hard-coded host configurable, keeping the
literal as the default. The change is **equivalent** while the variable is
unset, so no confirmation is needed.

```ts
// before
const API_URL = 'http://localhost:3000';
export const services = { billing: 'http://10.0.0.12:8080' };

// after
const API_URL = (process.env.API_URL ?? 'http://localhost:3000');
export const services = { billing: (process.env.SERVICES_BILLING ?? 'http://10.0.0.12:8080') };
```

`.env.example` gains `API_URL=http://localhost:3000` as documentation:
the default value, never a secret.

**Manual:** an inline host in a call or template
(`fetch('http://localhost:3000/x')`): reason `inline-host`.

**Detector change with it:** a line that already reads
`process.env.X || '<host>'` or `?? '<host>'` will no longer be a finding,
because it is configurable today.

**What the Guider will check:** the value becomes an environment read
whose default equals the old literal byte for byte, and the program's
description with the variable unset equals the description before.

## Unchecked reads (coming)

**Rule:** `db.result-unchecked` at warning level (a `select`).

**What it will do.** The same edits as for
[unchecked writes](#unchecked-writes--failures-that-throw), so a failed
read no longer looks like "no rows":

```ts
// before
const { data } = await db.from('orders').select('*').eq('id', id);

// after
const { data } = await db.from('orders').select('*').eq('id', id).throwOnError();
```

When `error` is destructured and never read, `if (error) throw error;` goes
on the next line. The statement for the approver: "a failed read of
`<table>` now aborts the operation instead of looking like no rows".

**When it is manual:** every reason of unchecked writes, plus:

| Reason | What it means |
|---|---|
| `single-row-read` | The read uses `.single()`. PostgREST reports zero rows as an error there, so a throw would turn "not found" into a failure. `.maybeSingle()` is fine and is transformed. |

**What the Guider will check:** as for writes, and it refuses a
`.single()` read.

## What the Guider checks

Every automated step, whatever the transformation, passes these checks
before it is kept. The Guider is a separate, deterministic verifier. It
never proposes changes; it only accepts or refuses them.

| Check | What it means |
|---|---|
| Plan lint (`GP…`) | The step uses only operations from a closed set, with well-formed parameters, in the right kind of file. |
| Preconditions (`GV401`) | The bytes at each edit's range are exactly the bytes the plan was made from, and they are the site the plan names. |
| Context lints (`GV403`, `GV404`) | No throw and no server logger in client code. |
| Declared change (`GV501`) | The description of the whole program after the edit equals the description before plus the declared change, and nothing else. |
| Re-detection (`GV502`) | The condition the finding reported no longer holds at that site. |
| Final re-check | After all steps, the Detector runs again on the result. |

A step that fails any check is refused and not applied. Steps that build
on a refused step are not applied either. See [safety.md](safety.md).
