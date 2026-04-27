pub mod dominator_chain;
pub mod engine;
pub mod field_signature;
pub mod fingerprint;
pub mod match_quality;
pub mod reference_chain;
pub mod types;

pub use fingerprint::ObjectFingerprint;
pub use types::{
    DiffMode, IdentityStrategy, MatchQuality, ObjectDelta, ObjectDeltaKind, ObjectDiffReport,
    ObjectDiffTotals, Risk,
};
