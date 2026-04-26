use crate::hprof::ObjectId;
use serde::{Deserialize, Serialize};

use super::fingerprint::ObjectFingerprint;

pub const DEFAULT_OBJECT_DIFF_TOP_N: usize = 50;
pub const DEFAULT_RETAINED_CHANGE_THRESHOLD: u64 = 1_048_576;
pub const DEFAULT_OBJECT_DIFF_MIN_RETAINED_BYTES: u64 = 4 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DiffMode {
    Class,
    Object,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum IdentityStrategy {
    ClassRetained,
    ClassDominator,
    FullFingerprint,
}

impl Default for IdentityStrategy {
    fn default() -> Self {
        // Slice 8-1.B: default flipped from ClassRetained.
        Self::ClassDominator
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ObjectDeltaKind {
    Added,
    Removed,
    RetainedChanged,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Risk {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MatchQuality {
    pub strategy: IdentityStrategy,
    pub collision_rate: f64,
    pub estimated_false_match_risk: Risk,
    pub estimated_false_split_risk: Risk,
    pub notes: Vec<String>,
}

impl MatchQuality {
    pub fn new(strategy: IdentityStrategy) -> Self {
        Self {
            strategy,
            collision_rate: 0.0,
            estimated_false_match_risk: Risk::Low,
            estimated_false_split_risk: Risk::Low,
            notes: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ObjectDiffTotals {
    pub before_object_count: u64,
    pub after_object_count: u64,
    pub fingerprint_collisions_before: u64,
    pub fingerprint_collisions_after: u64,
    pub matched_pairs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectDelta {
    pub class_name: String,
    pub fingerprint: ObjectFingerprint,
    pub example_object_id: ObjectId,
    pub before_count: u64,
    pub after_count: u64,
    pub before_retained_bytes: u64,
    pub after_retained_bytes: u64,
    pub dominator_chain: Vec<String>,
    pub reference_chain: Vec<String>,
    pub kind: ObjectDeltaKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ObjectDiffReport {
    pub strategy: IdentityStrategy,
    pub retained_bucket_bits: u8,
    pub retained_change_threshold: u64,
    pub match_quality: MatchQuality,
    pub added: Vec<ObjectDelta>,
    pub removed: Vec<ObjectDelta>,
    pub retained_changed: Vec<ObjectDelta>,
    pub totals: ObjectDiffTotals,
}

impl ObjectDiffReport {
    pub fn new(
        strategy: IdentityStrategy,
        retained_bucket_bits: u8,
        retained_change_threshold: u64,
    ) -> Self {
        Self {
            strategy,
            retained_bucket_bits,
            retained_change_threshold,
            match_quality: MatchQuality::new(strategy),
            added: Vec::new(),
            removed: Vec::new(),
            retained_changed: Vec::new(),
            totals: ObjectDiffTotals::default(),
        }
    }
}
