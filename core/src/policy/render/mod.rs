use std::path::Path;

use crate::{Policy, PolicyResult, Severity};

mod json;
mod text;

pub use json::render_json_envelope;
pub use text::render_text_report;

pub struct PolicyRenderContext<'a> {
    pub heap_path: &'a Path,
    pub policy_path: &'a Path,
    pub policy: &'a Policy,
    pub result: &'a PolicyResult,
    pub fail_on: Severity,
}
