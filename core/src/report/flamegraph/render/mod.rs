pub mod folded;
pub mod json;
pub mod svg;

pub use folded::render_folded_stacks;
pub use json::{render_json, FlameGraphEnvelope};
pub use svg::render_svg;

use serde::{Deserialize, Serialize};
use std::io::Write;

use crate::{errors::CoreResult, report::flamegraph::FoldedStacks};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FlameFormat {
    Svg,
    FoldedStack,
    Json,
}

pub fn render(
    stacks: &FoldedStacks,
    format: FlameFormat,
    title: Option<&str>,
    w: &mut impl Write,
) -> CoreResult<()> {
    match format {
        FlameFormat::Svg => render_svg(stacks, title, w),
        FlameFormat::FoldedStack => render_folded_stacks(stacks, w),
        FlameFormat::Json => render_json(stacks, w),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::flamegraph::{FlameRoot, FoldedStack, FoldedStacks};

    fn sample_stacks() -> FoldedStacks {
        FoldedStacks {
            strategy: FlameRoot::Dominator,
            total_weight: 42,
            truncated_to_other: 7,
            frame_count: 7,
            stacks: vec![
                FoldedStack {
                    frames: vec![
                        String::from("<gc-root:sticky_class>"),
                        String::from("com.example.Root"),
                        String::from("com.example.Left"),
                    ],
                    weight: 30,
                },
                FoldedStack {
                    frames: vec![
                        String::from("<gc-root:sticky_class>"),
                        String::from("com.example.Root"),
                        String::from("<other:2 classes>"),
                    ],
                    weight: 12,
                },
            ],
        }
    }

    #[test]
    fn render_dispatcher_routes_each_format_correctly() {
        for format in [
            FlameFormat::Svg,
            FlameFormat::FoldedStack,
            FlameFormat::Json,
        ] {
            let mut out = Vec::new();
            render(&sample_stacks(), format, Some("Dispatcher Smoke"), &mut out)
                .expect("dispatcher should render successfully");
            assert!(!out.is_empty(), "{format:?} output should not be empty");
        }
    }
}
