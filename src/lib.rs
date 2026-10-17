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
//! `sloptopop transform` (preview) also starts the engine, twice, and
//! between the two posts the request the engine prepared to the sloptopop
//! backend with `curl`, after showing what it sends and asking: the only
//! network call sloptopop makes (see `docs/commands.md`).
//!
//! `sloptopop collect` runs the installed analyzers from a checked-in
//! catalogue with fixed flags, each as a separate process, and writes their
//! reports to `.sloptopop/reports/` for the engine to read (see
//! `docs/collect.md`).
//!
//! # Module layers
//!
//! A module imports only from modules in strictly lower layers; modules in
//! the same layer know nothing of each other. Bottom to top:
//!
//! 0. `args`, `collect_detect`, `collect_gitignore`, `collect_lookup`,
//!    `collect_managed`, `collect_pack`, `diagnostic`, `engine_locate`,
//!    `framing`, `handshake`, `json_fields`, `jsonrpc`, `backend_post`
//!    (no crate imports)
//! 1. `collect_catalogue`, `engine_run`, `render_fix`, `render_manifest`,
//!    `render_report`, `schema`, `session`
//! 2. `collect_plan`, `command_io`, `server`
//! 3. `check_command`, `collect_run`, `fix_command`, `transform_command`
//! 4. `collect_record`
//! 5. `collect_command`
//! 6. `cli`
//! 7. this file: declares the modules and re-exports the public API
//!    (`engine`, `protocol` and `render` gather items from the layers
//!    below; `cli`, `framing`, `schema` and `server` are public as they are).
//!
//! Standards: SCF TDA-08 (Technical Documentation Artifacts: the module
//! layers above). Each module states its own controls; every item
//! inherits its module's.

mod args;
mod backend_post;
mod check_command;
pub mod cli;
mod collect_catalogue;
mod collect_command;
mod collect_detect;
mod collect_gitignore;
mod collect_lookup;
mod collect_managed;
mod collect_pack;
mod collect_plan;
mod collect_record;
mod collect_run;
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
mod render_manifest;
mod render_report;
pub mod schema;
pub mod server;
mod session;
mod transform_command;

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
