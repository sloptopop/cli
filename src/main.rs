//! The `sloptopop` binary; all logic lives in the library.
//!
//! Standards: as `cli`: SCF TDA-12, TDA-12.1.

use std::io;
use std::process::ExitCode;

fn main() -> ExitCode {
    ExitCode::from(sloptopop_cli::cli::run(
        &std::env::args_os().skip(1).collect::<Vec<_>>(),
        &mut io::stdin().lock(),
        &mut io::stdout().lock(),
        &mut io::stderr().lock(),
    ))
}
