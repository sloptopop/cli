//! The sloptopop command-line tool and JSON-RPC server.
//!
//! Every language library talks to this binary: it starts
//! `sloptopop serve --stdio` as a child process and exchanges JSON-RPC 2.0
//! messages with LSP-style `Content-Length` framing. See `PROTOCOL.md`.
//!
//! sloptopop detects only on its server, `sloptopop-engine` (hosted, or
//! self-hosted from the same image; decision D-0183). `sloptopop check`,
//! `fix` and `transform` start the open-source `sloptopop-local` binary to
//! prepare the masked snapshot, submit it to the server's job queue with
//! `curl` after showing what is sent and asking, and wait for the answer
//! (the only network calls sloptopop makes), and start `sloptopop-local`
//! again to print the server's Report
//! or apply its verified changes on a new branch (see `docs/commands.md`).
//! `sloptopop local …` runs `sloptopop-local` directly. This crate never
//! links either.
//!
//! `sloptopop collect` runs the analyzers that apply from sloptopop's
//! pinned container images (a checked-in catalogue, every image by digest),
//! with fixed flags, each in its own hardened container, and writes their
//! reports to `.sloptopop/reports/` for the engine to read (see
//! `docs/collect.md`).
//!
//! # Module layers
//!
//! A module imports only from modules in strictly lower layers; modules in
//! the same layer know nothing of each other. Bottom to top:
//!
//! 0. `args`, `collect_convert`, `collect_detect`, `collect_gitignore`,
//!    `collect_lookup`, `collect_pack`, `diagnostic`, `engine_locate`,
//!    `framing`, `handshake`, `history_git`, `history_mask`, `json_fields`,
//!    `jsonrpc`, `backend_jobs` (no crate imports)
//! 1. `collect_catalogue`, `collect_runtime`, `engine_run`,
//!    `history_upload` (the commit range in the upload),
//!    `render_manifest`, `render_report`, `schema`, `session`
//! 2. `collect_container`, `collect_data`, `collect_image`, `collect_plan`,
//!    `command_io`, `server`
//! 3. `collect_run`, `local_command`, `server_command` (the shared exchange
//!    with the server)
//! 4. `check_command`, `collect_record`, `fix_command`, `transform_command`
//! 5. `collect_command`
//! 6. `cli`
//! 7. this file: declares the modules and re-exports the public API
//!    (`carrier`, `engine`, `protocol` and `render` gather items from the
//!    layers below; `cli`, `framing`, `schema` and `server` are public as
//!    they are).
//!
//! Standards: SCF TDA-08 (Technical Documentation Artifacts: the module
//! layers above). Each module states its own controls; every item
//! inherits its module's.

mod args;
mod backend_jobs;
mod check_command;
pub mod cli;
mod collect_catalogue;
mod collect_command;
mod collect_container;
mod collect_convert;
mod collect_data;
mod collect_detect;
mod collect_gitignore;
mod collect_image;
mod collect_lookup;
mod collect_pack;
mod collect_plan;
mod collect_record;
mod collect_run;
mod collect_runtime;
mod command_io;
mod diagnostic;
mod engine_locate;
mod engine_run;
mod fix_command;
pub mod framing;
mod handshake;
mod history_git;
mod history_mask;
mod history_upload;
mod json_fields;
mod jsonrpc;
mod local_command;
mod render_manifest;
mod render_report;
pub mod schema;
pub mod server;
mod server_command;
mod session;
mod transform_command;

/// The SARIF carrier of analyzer fixes, for a server that runs an analyzer
/// itself: the tool's own output converted exactly as `collect` converts
/// it (see `docs/collect.md`, "Fixes").
pub mod carrier {
    pub use crate::collect_convert::{Carrier, carry};
}

/// Running `sloptopop-local`, the open-source half of sloptopop on this
/// machine.
///
/// The commands never link it: they start it as a child process and read
/// the JSON document it prints (see `docs/commands.md`). It is found
/// through the `SLOPTOPOP_LOCAL` environment variable, then next to the
/// `sloptopop` executable, then on `PATH`.
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

/// Readable text for the JSON documents sloptopop prints.
///
/// Only the public Report format is read; unknown fields are ignored and
/// missing ones render as empty, so newer servers stay readable.
pub mod render {
    pub use crate::render_report::report;
}

#[cfg(test)]
mod tests;
