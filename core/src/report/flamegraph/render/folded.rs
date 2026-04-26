use std::io::Write;

use crate::{errors::CoreResult, report::flamegraph::FoldedStacks};

pub fn render_folded_stacks(stacks: &FoldedStacks, w: &mut impl Write) -> CoreResult<()> {
    for stack in &stacks.stacks {
        w.write_all(stack.to_folded_line().as_bytes())?;
        w.write_all(b"\n")?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::flamegraph::{FlameRoot, FoldedStack, FoldedStacks};

    fn sample_stacks() -> FoldedStacks {
        FoldedStacks {
            strategy: FlameRoot::Dominator,
            total_weight: 24,
            truncated_to_other: 0,
            frame_count: 8,
            stacks: vec![
                FoldedStack {
                    frames: vec![
                        String::from("<gc-root:sticky_class>"),
                        String::from("com.example.Root"),
                        String::from("com.example.Left"),
                    ],
                    weight: 16,
                },
                FoldedStack {
                    frames: vec![
                        String::from("<gc-root:sticky_class>"),
                        String::from("com.example.Root"),
                        String::from("<other:2 classes>"),
                    ],
                    weight: 8,
                },
            ],
        }
    }

    #[test]
    fn render_folded_emits_one_line_per_stack() {
        let mut out = Vec::new();
        render_folded_stacks(&sample_stacks(), &mut out).expect("render should succeed");

        let text = String::from_utf8(out).expect("folded output should be utf-8");
        assert_eq!(text.lines().count(), 2);
    }

    #[test]
    fn render_folded_preserves_frame_order_within_stack() {
        let mut out = Vec::new();
        render_folded_stacks(&sample_stacks(), &mut out).expect("render should succeed");

        let text = String::from_utf8(out).expect("folded output should be utf-8");
        let first_line = text.lines().next().expect("first line should exist");
        assert!(first_line.starts_with("<gc-root:sticky_class>;com.example.Root;com.example.Left "));
    }

    #[test]
    fn render_folded_emits_weight_with_space_separator() {
        let mut out = Vec::new();
        render_folded_stacks(&sample_stacks(), &mut out).expect("render should succeed");

        let text = String::from_utf8(out).expect("folded output should be utf-8");
        let first_line = text.lines().next().expect("first line should exist");
        assert!(first_line.ends_with(" 16"));
        assert_eq!(first_line.matches(' ').count(), 1);
    }

    #[test]
    fn render_folded_handles_other_bucket_as_terminal_frame() {
        let mut out = Vec::new();
        render_folded_stacks(&sample_stacks(), &mut out).expect("render should succeed");

        let text = String::from_utf8(out).expect("folded output should be utf-8");
        let last_line = text.lines().last().expect("last line should exist");
        assert!(last_line.contains(";<other:2 classes> 8"));
    }
}
