pub(crate) mod cloud;
/// noxa-mcp library wrapper.
///
/// This exposes the MCP server so it can be embedded by the `noxa` CLI via
/// `noxa mcp` without duplicating the transport/bootstrap code.
///
/// Callers must initialize tracing before calling `run()`. Stdout must remain
/// untouched after `run()` begins because it carries the MCP wire protocol.
pub(crate) mod config;
pub(crate) mod error;
pub(crate) mod research;
pub(crate) mod serialization;
pub(crate) mod server;
pub(crate) mod tools;
pub(crate) mod validation;

use rmcp::ServiceExt;
use rmcp::transport::stdio;

pub use error::NoxaMcpError;

/// Start the MCP server over stdio and block until the client disconnects.
pub async fn run() -> Result<(), NoxaMcpError> {
    let service = server::NoxaMcp::new()
        .await?
        .serve(stdio())
        .await
        .map_err(|e| NoxaMcpError::message(format!("failed to start MCP stdio transport: {e}")))?;
    service
        .waiting()
        .await
        .map_err(|e| NoxaMcpError::message(format!("MCP service exited with an error: {e}")))?;
    Ok(())
}
