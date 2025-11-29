use crate::{
    analysis::AnalyzeRequest,
    errors::{CoreError, CoreResult},
};
use serde::{Deserialize, Serialize};
use tracing::info;

/// Placeholder MCP server harness. The real implementation will publish
/// commands like `parse_heap` and `detect_leaks` over stdio.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpServerOptions {
    pub host: String,
    pub port: u16,
}

pub async fn serve(options: McpServerOptions, request: AnalyzeRequest) -> CoreResult<()> {
    info!(host = %options.host, port = options.port, "starting MCP server (stub)");
    let _ = request;
    Err(CoreError::NotImplemented(
        "MCP server runtime is not wired up yet".into(),
    ))
}
