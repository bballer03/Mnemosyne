//! Object inspector report renderers (M8 Slice 8.C).
//!
//! Mirrors `core::report::diff`'s layout (`mod.rs` + one file per format)
//! — the established placement precedent for small, single-report-type
//! renderer families in this crate.

mod json;
mod text;
mod toon;

use crate::{analysis::ObjectInspection, errors::CoreResult};

pub use json::render_json;
pub use text::render_text;
pub use toon::render_toon;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Text,
    Json,
    Toon,
}

pub fn render(inspection: &ObjectInspection, format: Format) -> CoreResult<String> {
    match format {
        Format::Text => Ok(render_text(inspection)),
        Format::Json => Ok(render_json(inspection)),
        Format::Toon => Ok(render_toon(inspection)),
    }
}
