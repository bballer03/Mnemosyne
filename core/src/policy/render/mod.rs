use std::path::Path;

use crate::{Policy, PolicyResult, Severity};

mod github_actions;
mod json;
mod junit;
mod text;

pub use github_actions::render_github_actions_report;
pub use json::render_json_envelope;
pub use junit::render_junit_report;
pub use text::render_text_report;

pub struct PolicyRenderContext<'a> {
    pub heap_path: &'a Path,
    pub policy_path: &'a Path,
    pub policy: &'a Policy,
    pub result: &'a PolicyResult,
    pub fail_on: Severity,
}
