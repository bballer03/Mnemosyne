pub mod dominator;

pub use dominator::collapse_dominator;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollapseOptions {
    pub min_fraction: f64,
    pub max_frames: usize,
}

impl Default for CollapseOptions {
    fn default() -> Self {
        Self {
            min_fraction: 0.001,
            max_frames: 5_000,
        }
    }
}
