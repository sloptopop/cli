# Conformance suite

Language-neutral test cases for `sloptopop serve --stdio`. The Rust tests run
every case against the real binary; each language client runs them through its
own framing code so all clients agree on the wire format.

Each `cases/*.json` file:

- `input`: messages to send, in order, each framed as
  `Content-Length: N\r\n\r\n<body>`. An object `{"$raw": "..."}` is sent as
  that exact body instead of serialised JSON. After the last message stdin is
  closed.
- `output`: the replies expected on stdout, in order and nothing else. Compare
  as JSON values. `"$SERVER_VERSION"` matches the binary's version. An error
  object with `messagePrefix` instead of `message` matches any message that
  starts with that prefix.
- `exitCode`: the expected process exit code.
