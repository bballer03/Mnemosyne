pub mod budget;
pub mod collapse;
pub mod render;
pub mod types;

pub use budget::apply_budget;
pub use collapse::{
    collapse, collapse_class_hierarchy, collapse_dominator, collapse_gc_root_path, CollapseOptions,
};
pub use render::{
    render, render_folded_stacks, render_json, render_svg, FlameFormat, FlameGraphEnvelope,
};
pub use types::{sanitize_frame_name, FlameRoot, FoldedStack, FoldedStacks, FrameName};
