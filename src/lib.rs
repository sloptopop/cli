//! The sloptopop command-line tool and JSON-RPC server.
//!
//! Every language library talks to this binary: it starts
//! `sloptopop serve --stdio` as a child process and exchanges JSON-RPC 2.0
//! messages with LSP-style `Content-Length` framing. See `PROTOCOL.md`.
//!
//! `sloptopop check` and `sloptopop fix` start the separately installed
//! `sloptopop-engine` binary as a child process and exchange the Report
//! JSON with it (see `docs/report-format.md`); this crate never links it.
//!
//! # Module layers
//!
//! A module imports only from modules in strictly lower layers; modules in
//! the same layer know nothing of each other. Bottom to top:
//!
//! 0. `args`, `diagnostic`, `engine_locate`, `framing`, `handshake`,
//!    `json_fields`, `jsonrpc` (no crate imports)
//! 1. `engine_run`, `render_fix`, `render_report`, `schema`, `session`
//! 2. `command_io`, `server`
//! 3. `check_command`, `fix_command`
//! 4. `cli`
//! 5. this file: declares the modules and re-exports the public API
//!    (`engine`, `protocol` and `render` gather items from the layers
//!    below; `cli`, `framing`, `schema` and `server` are public as they are).

mod args;
mod check_command;
pub mod cli;
mod command_io;
mod diagnostic;
mod engine_locate;
mod engine_run;
mod fix_command;
pub mod framing;
mod handshake;
mod json_fields;
mod jsonrpc;
mod render_fix;
mod render_report;
pub mod schema;
pub mod server;
mod session;

/// Running `sloptopop-engine`, the separately installed engine binary.
///
/// `check` and `fix` never link the engine: they start it as a child
/// process and read the JSON document it prints (see
/// `docs/report-format.md`). The engine is found through the
/// `SLOPTOPOP_ENGINE` environment variable, then next to the `sloptopop`
/// executable, then on `PATH`.
pub mod engine {
    pub use crate::engine_locate::{ENGINE_ENV, ENGINE_NAME, Locator};
    pub use crate::engine_run::{ENGINE_ERROR, NOT_FOUND};
}

/// JSON-RPC 2.0 message types and the sloptopop method set.
///
/// These Rust types are the source of truth for the protocol: the JSON
/// Schema that language clients are generated from is derived from them
/// (see [`crate::schema`]).
pub mod protocol {
    pub use crate::handshake::{
        InitializeParams, InitializeResult, PROTOCOL_VERSION, SERVER_NAME, SERVER_VERSION,
    };
    pub use crate::jsonrpc::{ErrorObject, Id, Message, error_code, failure, success};
}

/// Readable text for the JSON documents `sloptopop-engine` prints.
///
/// Only the public Report format is read; unknown fields are ignored and
/// missing ones render as empty, so newer engines stay readable.
pub mod render {
    pub use crate::render_fix::fix;
    pub use crate::render_report::report;
}

#[cfg(test)]
mod tests;
