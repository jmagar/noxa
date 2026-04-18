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
#[cfg(test)]
pub(crate) mod test_support;
pub(crate) mod tools;
pub(crate) mod validation;

use std::path::PathBuf;

use rmcp::ServiceExt;
use rmcp::transport::stdio;

pub use error::NoxaMcpError;

pub fn load_env() -> Result<Option<PathBuf>, NoxaMcpError> {
    let home_dir = dirs::home_dir()
        .ok_or_else(|| NoxaMcpError::message("failed to determine home directory for noxa-mcp"))?;
    let exe_path = std::env::current_exe().ok();
    let cwd = std::env::current_dir().ok();
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from);

    let env_path = config::find_env_file(
        &home_dir,
        exe_path.as_deref(),
        cwd.as_deref(),
        repo_root.as_deref(),
    );

    if let Some(path) = env_path {
        dotenvy::from_path(&path).map_err(|error| {
            NoxaMcpError::message(format!("failed to load env file {}: {error}", path.display()))
        })?;
        Ok(Some(path))
    } else {
        Ok(None)
    }
}

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
