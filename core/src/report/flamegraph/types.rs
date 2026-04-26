use serde::{Deserialize, Serialize};
use std::borrow::Cow;

pub type FrameName = String;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum FlameRoot {
    Dominator,
    ClassHierarchy,
    GcRootPath,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FoldedStack {
    pub frames: Vec<String>,
    pub weight: u64,
}

impl FoldedStack {
    pub fn to_folded_line(&self) -> String {
        let frames = self
            .frames
            .iter()
            .map(|frame| sanitize_frame_name(frame).into_owned())
            .collect::<Vec<_>>();
        format!("{} {}", frames.join(";"), self.weight)
    }

    pub fn parse_folded_line(line: &str) -> Option<Self> {
        let (frames, weight) = line.rsplit_once(' ')?;
        let weight = weight.parse::<u64>().ok()?;
        let mut parsed_frames = Vec::new();
        let mut current = String::new();
        let mut escaped = false;

        for ch in frames.chars() {
            if escaped {
                current.push(ch);
                escaped = false;
                continue;
            }

            match ch {
                '\\' => escaped = true,
                ';' => {
                    parsed_frames.push(std::mem::take(&mut current));
                }
                _ => current.push(ch),
            }
        }

        if escaped {
            current.push('\\');
        }
        parsed_frames.push(current);

        Some(Self {
            frames: parsed_frames,
            weight,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FoldedStacks {
    pub strategy: FlameRoot,
    pub stacks: Vec<FoldedStack>,
    pub total_weight: u64,
    pub truncated_to_other: u64,
    pub frame_count: usize,
}

impl FoldedStacks {
    pub fn new(strategy: FlameRoot, stacks: Vec<FoldedStack>) -> Self {
        Self {
            strategy,
            total_weight: stacks.iter().map(|stack| stack.weight).sum(),
            frame_count: stacks.iter().map(|stack| stack.frames.len()).sum(),
            stacks,
            truncated_to_other: 0,
        }
    }
}

pub fn sanitize_frame_name(name: &str) -> Cow<'_, str> {
    let needs_sanitization =
        name.contains(';') || name.chars().any(|ch| ch.is_control()) || name.chars().count() > 256;

    if !needs_sanitization {
        return Cow::Borrowed(name);
    }

    let mut sanitized = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_control() {
            continue;
        }
        if ch == ';' {
            sanitized.push('\\');
            sanitized.push(';');
        } else {
            sanitized.push(ch);
        }
    }

    if sanitized.chars().count() > 256 {
        let mut truncated = sanitized.chars().take(255).collect::<String>();
        truncated.push('…');
        sanitized = truncated;
    }

    Cow::Owned(sanitized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folded_stack_serialize_roundtrip() {
        let stack = FoldedStack {
            frames: vec![
                "<gc-root>".into(),
                "com.example.Root".into(),
                "com.example.Leaf".into(),
            ],
            weight: 42,
        };

        let line = stack.to_folded_line();
        let reparsed = FoldedStack::parse_folded_line(&line).expect("stack should parse");

        assert_eq!(reparsed, stack);
    }

    #[test]
    fn folded_stack_sanitizes_semicolons_in_frame_names() {
        let sanitized = sanitize_frame_name("com.example;Root").into_owned();
        assert_eq!(sanitized, "com.example\\;Root");
    }

    #[test]
    fn folded_stack_truncates_frame_name_at_256_chars() {
        let original = format!("frame{}", "x".repeat(400));
        let sanitized = sanitize_frame_name(&original).into_owned();

        assert_eq!(sanitized.chars().count(), 256);
        assert!(sanitized.ends_with('…'));
    }
}
