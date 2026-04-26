use std::io::Write;

use serde::{Deserialize, Serialize};

use crate::{
    errors::CoreResult,
    report::flamegraph::{FlameRoot, FoldedStack, FoldedStacks},
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FlameGraphEnvelope {
    pub tool: String,
    pub subcommand: String,
    pub version: String,
    pub strategy: FlameRoot,
    pub total_weight: u64,
    pub truncated_to_other: u64,
    pub frame_count: usize,
    pub stacks: Vec<FoldedStack>,
}

impl From<&FoldedStacks> for FlameGraphEnvelope {
    fn from(stacks: &FoldedStacks) -> Self {
        Self {
            tool: String::from("mnemosyne"),
            subcommand: String::from("flamegraph"),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            strategy: stacks.strategy,
            total_weight: stacks.total_weight,
            truncated_to_other: stacks.truncated_to_other,
            frame_count: stacks.frame_count,
            stacks: stacks.stacks.clone(),
        }
    }
}

pub fn render_json(stacks: &FoldedStacks, w: &mut impl Write) -> CoreResult<()> {
    serde_json::to_writer_pretty(w, &FlameGraphEnvelope::from(stacks))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_stacks() -> FoldedStacks {
        FoldedStacks {
            strategy: FlameRoot::ClassHierarchy,
            total_weight: 36,
            truncated_to_other: 4,
            frame_count: 6,
            stacks: vec![
                FoldedStack {
                    frames: vec![
                        String::from("java.lang.Object"),
                        String::from("com.example.Root"),
                        String::from("com.example.Leaf"),
                    ],
                    weight: 20,
                },
                FoldedStack {
                    frames: vec![
                        String::from("java.lang.Object"),
                        String::from("com.example.Other"),
                    ],
                    weight: 16,
                },
            ],
        }
    }

    #[test]
    fn render_json_envelope_well_formed_pretty_print() {
        let mut out = Vec::new();
        render_json(&sample_stacks(), &mut out).expect("json render should succeed");

        let text = String::from_utf8(out).expect("json output should be utf-8");
        assert!(text.contains("\n  \"tool\":"));
        let parsed: serde_json::Value = serde_json::from_str(&text).expect("json should parse");
        assert_eq!(parsed["tool"], "mnemosyne");
        assert_eq!(parsed["subcommand"], "flamegraph");
    }

    #[test]
    fn render_json_envelope_includes_strategy_kebab_case() {
        let mut out = Vec::new();
        render_json(&sample_stacks(), &mut out).expect("json render should succeed");

        let parsed: serde_json::Value =
            serde_json::from_slice(&out).expect("json output should parse");
        assert_eq!(parsed["strategy"], "class-hierarchy");
    }

    #[test]
    fn render_json_envelope_total_weight_matches_input() {
        let mut out = Vec::new();
        let stacks = sample_stacks();
        render_json(&stacks, &mut out).expect("json render should succeed");

        let parsed: serde_json::Value =
            serde_json::from_slice(&out).expect("json output should parse");
        assert_eq!(parsed["total_weight"], stacks.total_weight);
        assert_eq!(parsed["truncated_to_other"], stacks.truncated_to_other);
        assert_eq!(parsed["frame_count"], stacks.frame_count);
    }

    #[test]
    fn render_json_envelope_round_trip_via_serde() {
        let mut out = Vec::new();
        render_json(&sample_stacks(), &mut out).expect("json render should succeed");

        let reparsed: FlameGraphEnvelope =
            serde_json::from_slice(&out).expect("json envelope should round trip");
        assert_eq!(reparsed.strategy, FlameRoot::ClassHierarchy);
        assert_eq!(reparsed.stacks.len(), 2);
    }
}
