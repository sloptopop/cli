# sloptopop protocol (version 1)

Every language library talks to the `sloptopop` binary the same way language
servers talk to editors: it starts `sloptopop serve --stdio` as a child process
and exchanges JSON-RPC 2.0 messages over the child's stdin/stdout.

Machine-readable description (methods, error codes, JSON Schemas of every
type): [`schema/protocol.json`](schema/protocol.json), also printed by
`sloptopop schema`. Wire-level test cases: [`conformance/`](conformance/).

## Framing

Each message, in both directions, is

```
Content-Length: <N>\r\n
\r\n
<N bytes of UTF-8 JSON>
```

- Header names are case-insensitive. Other headers (e.g. `Content-Type`) are
  allowed and ignored. Every header line ends with `\r\n`; a bare `\n` is an
  error.
- Header lines are at most 1024 bytes; bodies at most 67 108 864 bytes
  (64 MiB). Larger frames are a framing error.
- A framing error leaves the stream unusable: the server writes
  `error: <reason>` to stderr and exits with code 1.
- stderr is for diagnostics only; clients should pass it through or collect
  it for error messages, never parse it.

## Messages

Standard JSON-RPC 2.0. Requests have an `id` (integer or string) and get
exactly one reply with the same `id`. Messages without an `id` (or with
`"id": null`) are notifications and get no reply. Batches (JSON arrays) are
not supported and get an `INVALID_REQUEST` error.

Replies are written in request order.

## Session lifecycle

1. Client sends `initialize`. Any other request first gets
   `SERVER_NOT_INITIALIZED`.
2. Client sends any number of requests.
3. Client sends `shutdown`. Afterwards every request gets `INVALID_REQUEST`.
4. Client sends the `exit` notification. The process exits with code 0 if
   `shutdown` came first, otherwise 1. Closing stdin without `exit` also ends
   the process (code 0 after `shutdown`, otherwise 1).

## Methods

### `initialize` (request)

Params:

```json
{ "clientName": "sloptopop-python", "clientVersion": "0.1.0", "protocolVersion": 1 }
```

Result:

```json
{ "serverName": "sloptopop", "serverVersion": "0.1.0", "protocolVersion": 1 }
```

If `protocolVersion` differs from the server's, the reply is an
`UNSUPPORTED_PROTOCOL_VERSION` error whose `data` is
`{ "supported": [1] }`, and the session stays uninitialized.

### `shutdown` (request)

No params. Result `null`.

### `exit` (notification)

No params, no reply. Ends the process.

## Errors

| Name | Code | When |
|---|---|---|
| `PARSE_ERROR` | -32700 | Body is not valid JSON (`id` is `null`) |
| `INVALID_REQUEST` | -32600 | Not a request object, wrong `jsonrpc`, batch, second `initialize`, or any request after `shutdown` |
| `METHOD_NOT_FOUND` | -32601 | Unknown method |
| `INVALID_PARAMS` | -32602 | Params missing or wrongly shaped |
| `UNSUPPORTED_PROTOCOL_VERSION` | -32001 | `initialize` with another protocol version |
| `SERVER_NOT_INITIALIZED` | -32002 | A request before `initialize` |

## Command line

| Command | Behaviour |
|---|---|
| `sloptopop serve --stdio` | Run the server described above |
| `sloptopop schema` | Print `schema/protocol.json` |
| `sloptopop version`, `--version`, `-V` | Print `sloptopop <version> (protocol <n>)`, exit 0 |
| `sloptopop help`, `--help`, `-h` | Print usage, exit 0 |
| anything else | Print `error: …` and usage to stderr, exit 2 |

## Requirements for language clients

Every client package provides the same two things.

1. **A library** with, in that language's idioms:
   - A client that starts `sloptopop serve --stdio`, sends `initialize`
     (`clientName` = `sloptopop-<language>`, `clientVersion` = the package
     version, `protocolVersion` = 1) and exposes the server's
     `InitializeResult` (`serverName`, `serverVersion`, `protocolVersion`).
   - `close` (or the language's disposal pattern): `shutdown`, then `exit`,
     then wait for the process; idempotent.
   - A generic `request(method, params)` for methods this client doesn't wrap
     yet.
   - Typed errors: an RPC error carrying `code`, `message` and `data`; a
     separate error for transport problems (binary not found, process died,
     framing violation, unexpected reply id).
   - Binary lookup, first match wins: an explicit path passed to the client;
     the `SLOPTOPOP_BIN` environment variable; a binary bundled inside the
     package; `sloptopop` on `PATH`.
2. **A `sloptopop` command** that runs the same binary with the same
   arguments, stdin, stdout, stderr and exit code.

Clients are tested against the real binary, run every case in
`conformance/cases/` through their own framing code, and keep 100% test
coverage.
