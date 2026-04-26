pub mod budget;
pub mod collapse;
pub mod types;

pub use budget::apply_budget;
pub use collapse::{collapse_dominator, CollapseOptions};
pub use types::{sanitize_frame_name, FlameRoot, FoldedStack, FoldedStacks, FrameName};
