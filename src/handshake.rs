//! The `initialize` handshake: who this binary is, which protocol version
//! it speaks, and the params/result of the request that negotiates it.

use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

/// Version of the sloptopop protocol spoken by this binary. Bumped on any
/// breaking change to a method, type or behaviour.
pub const PROTOCOL_VERSION: u32 = 1;

/// Name the server reports in [`InitializeResult::server_name`].
pub const SERVER_NAME: &str = "sloptopop";

/// Version of this binary, reported in [`InitializeResult::server_version`].
pub const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Params of the `initialize` request, the first request of every session.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InitializeParams {
    /// Name of the client library, e.g. `"sloptopop-python"`.
    pub client_name: String,
    /// Version of the client library.
    pub client_version: String,
    /// Protocol version the client speaks; must equal [`PROTOCOL_VERSION`].
    pub protocol_version: u32,
}

/// Result of the `initialize` request.
#[derive(Debug, Clone, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    /// Always [`SERVER_NAME`].
    pub server_name: String,
    /// Version of the binary.
    pub server_version: String,
    /// Protocol version the server speaks.
    pub protocol_version: u32,
}

impl InitializeResult {
    /// The result this binary returns.
    #[must_use]
    pub fn current() -> Self {
        Self {
            server_name: SERVER_NAME.into(),
            server_version: SERVER_VERSION.into(),
            protocol_version: PROTOCOL_VERSION,
        }
    }

    /// The result as a JSON value.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "serverName": self.server_name,
            "serverVersion": self.server_version,
            "protocolVersion": self.protocol_version,
        })
    }
}

#[cfg(test)]
mod tests;
