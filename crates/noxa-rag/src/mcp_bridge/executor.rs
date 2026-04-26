use async_trait::async_trait;
use serde_json::Value;
use tokio::process::Command;

use crate::RagError;

use super::{McpSource, McporterExecutor};

#[derive(Debug, Clone)]
pub struct ProcessMcporterExecutor {
    executable: String,
}

impl ProcessMcporterExecutor {
    pub fn new(executable: impl Into<String>) -> Self {
        Self {
            executable: executable.into(),
        }
    }
}

#[async_trait]
impl McporterExecutor for ProcessMcporterExecutor {
    async fn call(
        &self,
        server: &str,
        service: McpSource,
        action: &str,
        params: Value,
    ) -> Result<Value, RagError> {
        let selector = format!("{}.{}", server, service.as_str());
        let args = serde_json::json!({
            "action": action,
            "params": params,
        });
        let output = Command::new(&self.executable)
            .arg("call")
            .arg(&selector)
            .arg("--args")
            .arg(args.to_string())
            .arg("--output")
            .arg("json")
            .output()
            .await
            .map_err(|e| RagError::Generic(format!("failed to execute mcporter: {e}")))?;

        if !output.status.success() {
            return Err(RagError::Generic(format!(
                "mcporter call {} {} failed: {}",
                selector,
                action,
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }

        serde_json::from_slice(&output.stdout)
            .map_err(|e| RagError::Generic(format!("mcporter returned invalid JSON: {e}")))
    }
}
