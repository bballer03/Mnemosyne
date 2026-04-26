use super::types::FoldedStacks;

pub fn apply_budget(
    mut stacks: FoldedStacks,
    min_fraction: f64,
    max_frames: usize,
) -> FoldedStacks {
    let total_weight = stacks.total_weight;
    let threshold = total_weight as f64 * min_fraction.max(0.0);
    let mut truncated_to_other = stacks.truncated_to_other;

    let mut kept = Vec::with_capacity(stacks.stacks.len());
    for stack in stacks.stacks.drain(..) {
        if threshold > 0.0 && (stack.weight as f64) < threshold {
            truncated_to_other += stack.weight;
        } else {
            kept.push(stack);
        }
    }

    while frame_count(&kept) > max_frames {
        let Some((drop_index, _)) = kept.iter().enumerate().min_by(|(_, left), (_, right)| {
            left.weight
                .cmp(&right.weight)
                .then_with(|| left.frames.join(";").cmp(&right.frames.join(";")))
        }) else {
            break;
        };

        truncated_to_other += kept.remove(drop_index).weight;
    }

    stacks.stacks = kept;
    stacks.truncated_to_other = truncated_to_other;
    stacks.frame_count = frame_count(&stacks.stacks);
    stacks.total_weight = total_weight;
    stacks
}

fn frame_count(stacks: &[crate::report::flamegraph::FoldedStack]) -> usize {
    stacks.iter().map(|stack| stack.frames.len()).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::report::flamegraph::{FlameRoot, FoldedStack, FoldedStacks};

    #[test]
    fn budget_min_fraction_folds_small_frames_into_other() {
        let stacks = FoldedStacks::new(
            FlameRoot::Dominator,
            vec![
                FoldedStack {
                    frames: vec!["<gc-root>".into(), "com.example.Big".into()],
                    weight: 1_000,
                },
                FoldedStack {
                    frames: vec!["<gc-root>".into(), "com.example.Small".into()],
                    weight: 1,
                },
            ],
        );

        let budgeted = apply_budget(stacks, 0.01, usize::MAX);

        assert_eq!(budgeted.stacks.len(), 1);
        assert_eq!(budgeted.truncated_to_other, 1);
        assert_eq!(budgeted.total_weight, 1_001);
    }

    #[test]
    fn budget_max_frames_caps_count() {
        let stacks = FoldedStacks::new(
            FlameRoot::Dominator,
            vec![
                FoldedStack {
                    frames: vec!["<gc-root>".into(), "com.example.A".into()],
                    weight: 500,
                },
                FoldedStack {
                    frames: vec!["<gc-root>".into(), "com.example.B".into()],
                    weight: 250,
                },
                FoldedStack {
                    frames: vec!["<gc-root>".into(), "com.example.C".into()],
                    weight: 5,
                },
            ],
        );

        let budgeted = apply_budget(stacks, 0.0, 4);

        assert!(budgeted.frame_count <= 4);
        assert_eq!(budgeted.truncated_to_other, 5);
        assert_eq!(budgeted.total_weight, 755);
    }
}
