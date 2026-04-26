use std::io::Write;

use inferno::flamegraph::Options;

use crate::{
    errors::{CoreError, CoreResult},
    report::flamegraph::{render_folded_stacks, FoldedStacks},
};

const DEFAULT_TITLE: &str = "Mnemosyne flame graph";

/// Render flamegraph SVG using `inferno` 0.11.x from folded-stack input.
///
/// `inferno` was chosen for Slice M7-3.C because its license and MSRV fit the
/// workspace, and it provides the standardized interactive SVG output this
/// slice needs. Empty inputs are handled locally so callers get a valid SVG
/// instead of `inferno`'s "no stack counts found" error artifact.
pub fn render_svg(
    stacks: &FoldedStacks,
    title: Option<&str>,
    w: &mut impl Write,
) -> CoreResult<()> {
    if stacks.stacks.is_empty() || stacks.total_weight == 0 {
        return render_empty_svg(title.unwrap_or(DEFAULT_TITLE), w);
    }

    let mut folded = Vec::new();
    render_folded_stacks(stacks, &mut folded)?;

    let mut options = Options::default();
    options.title = title.unwrap_or(DEFAULT_TITLE).to_owned();
    options.count_name = String::from("bytes");
    options.deterministic = true;
    options.pretty_xml = true;

    inferno::flamegraph::from_reader(&mut options, folded.as_slice(), w)
        .map_err(|err| CoreError::Other(err.into()))
}

fn render_empty_svg(title: &str, w: &mut impl Write) -> CoreResult<()> {
    write!(
        w,
        concat!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1200 96\" role=\"img\">",
            "<title>{}</title>",
            "<rect width=\"1200\" height=\"96\" fill=\"#fff8ef\" />",
            "<text x=\"24\" y=\"36\" font-family=\"monospace\" font-size=\"18\" fill=\"#40220f\">{}</text>",
            "<text x=\"24\" y=\"68\" font-family=\"monospace\" font-size=\"12\" fill=\"#6b5645\">No flame graph data available.</text>",
            "</svg>"
        ),
        escape_xml_text(title),
        escape_xml_text(title),
    )?;
    Ok(())
}

fn escape_xml_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::flamegraph::{FlameRoot, FoldedStack, FoldedStacks};

    fn sample_stacks() -> FoldedStacks {
        FoldedStacks {
            strategy: FlameRoot::GcRootPath,
            total_weight: 60,
            truncated_to_other: 0,
            frame_count: 9,
            stacks: vec![
                FoldedStack {
                    frames: vec![
                        String::from("<gc-root:sticky_class>"),
                        String::from("com.example.Root"),
                        String::from("com.example.Leaf"),
                    ],
                    weight: 40,
                },
                FoldedStack {
                    frames: vec![
                        String::from("<gc-root:java_frame>"),
                        String::from("com.example.Root"),
                        String::from("com.example.Other"),
                    ],
                    weight: 20,
                },
            ],
        }
    }

    fn many_stacks(count: usize) -> FoldedStacks {
        let stacks = (0..count)
            .map(|index| FoldedStack {
                frames: vec![
                    String::from("<gc-root:sticky_class>"),
                    format!("com.example.Root{index}"),
                    format!("com.example.Leaf{index}"),
                ],
                weight: u64::try_from(index + 1).expect("weight should fit in u64"),
            })
            .collect::<Vec<_>>();

        FoldedStacks {
            strategy: FlameRoot::Dominator,
            total_weight: stacks.iter().map(|stack| stack.weight).sum(),
            truncated_to_other: 0,
            frame_count: stacks.iter().map(|stack| stack.frames.len()).sum(),
            stacks,
        }
    }

    #[test]
    fn render_svg_produces_valid_xml_starting_with_svg_root() {
        let mut out = Vec::new();
        render_svg(&sample_stacks(), Some("SVG Smoke"), &mut out)
            .expect("svg render should succeed");

        let text = String::from_utf8(out).expect("svg output should be utf-8");
        assert!(text.starts_with("<?xml") || text.starts_with("<svg"));
        assert!(text.contains("<svg"));
    }

    #[test]
    fn render_svg_includes_title_when_provided() {
        let mut out = Vec::new();
        render_svg(&sample_stacks(), Some("Custom Title"), &mut out)
            .expect("svg render should succeed");

        let text = String::from_utf8(out).expect("svg output should be utf-8");
        assert!(text.contains("Custom Title"));
    }

    #[test]
    fn render_svg_byte_size_within_expected_range_for_small_input() {
        let mut out = Vec::new();
        render_svg(&many_stacks(100), Some("Size Bound"), &mut out)
            .expect("svg render should succeed");

        assert!(out.len() < 500_000, "svg output should stay under 500 KB");
    }

    #[test]
    fn render_svg_handles_empty_stacks_without_panic() {
        let mut out = Vec::new();
        let empty = FoldedStacks {
            strategy: FlameRoot::Dominator,
            total_weight: 0,
            truncated_to_other: 0,
            frame_count: 0,
            stacks: Vec::new(),
        };

        render_svg(&empty, Some("Empty"), &mut out).expect("empty svg render should succeed");
        let text = String::from_utf8(out).expect("svg output should be utf-8");
        assert!(text.contains("<svg"));
    }
}
