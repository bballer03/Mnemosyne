use serde_json::json;

use crate::PolicyResult;

pub fn render_json_envelope(
    result: &PolicyResult,
    version: &str,
) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(&json!({
        "tool": "mnemosyne",
        "subcommand": "ci-check",
        "version": version,
        "result": result,
    }))
}
