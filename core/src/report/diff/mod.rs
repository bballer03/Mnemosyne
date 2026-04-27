mod json;
mod text;
mod toon;

use crate::{
    diff::{IdentityStrategy, Risk},
    errors::CoreResult,
    hprof::HeapDiff,
};

pub use json::render_json;
pub use text::render_text;
pub use toon::render_toon;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Text,
    Json,
    Toon,
}

pub fn render(diff: &HeapDiff, format: Format) -> CoreResult<String> {
    match format {
        Format::Text => Ok(render_text(diff)),
        Format::Json => Ok(render_json(diff)),
        Format::Toon => Ok(render_toon(diff)),
    }
}

pub(crate) fn identity_strategy_label(strategy: IdentityStrategy) -> &'static str {
    match strategy {
        IdentityStrategy::ClassRetained => "class+retained",
        IdentityStrategy::ClassDominator => "class+dominator",
        IdentityStrategy::FullFingerprint => "full-fingerprint",
    }
}

pub(crate) fn risk_label(risk: Risk) -> &'static str {
    match risk {
        Risk::Low => "Low",
        Risk::Medium => "Medium",
        Risk::High => "High",
    }
}

pub(crate) fn bucket_label(bucket_bits: u8) -> String {
    match 1_u64.checked_shl(u32::from(bucket_bits)) {
        Some(bytes) => binary_unit_label(bytes),
        None => format!("2^{bucket_bits}B"),
    }
}

pub(crate) fn binary_unit_label(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * 1024;
    const GIB: u64 = 1024 * 1024 * 1024;
    const TIB: u64 = 1024 * 1024 * 1024 * 1024;

    if bytes >= TIB && bytes % TIB == 0 {
        format!("{}TB", bytes / TIB)
    } else if bytes >= GIB && bytes % GIB == 0 {
        format!("{}GB", bytes / GIB)
    } else if bytes >= MIB && bytes % MIB == 0 {
        format!("{}MB", bytes / MIB)
    } else if bytes >= KIB && bytes % KIB == 0 {
        format!("{}KB", bytes / KIB)
    } else {
        format!("{bytes}B")
    }
}

pub(crate) fn dominator_chain_label(chain: &[String]) -> String {
    if chain.is_empty() {
        return String::from("[]");
    }

    let joined = chain.join(",");
    let truncated = chain.len() >= 4
        && chain
            .last()
            .is_some_and(|label| label != "GC Root" && label != "<unknown>");
    if truncated {
        format!("[{joined},...]")
    } else {
        format!("[{joined}]")
    }
}
