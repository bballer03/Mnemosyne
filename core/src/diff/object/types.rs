use crate::hprof::ObjectId;
use serde::{Deserialize, Serialize};

use super::fingerprint::ObjectFingerprint;

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectDelta {
    pub class_name: String,
    pub fingerprint: ObjectFingerprint,
    pub example_object_id: ObjectId,
    pub before_count: u64,
    pub after_count: u64,
    pub before_retained_bytes: u64,
    pub after_retained_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ObjectDiffReport {
    pub strategy: IdentityStrategy,
    pub retained_bucket_bits: u8,
    pub min_retained_bytes: u64,
    pub added: Vec<ObjectDelta>,
    pub removed: Vec<ObjectDelta>,
    pub retained_changed: Vec<ObjectDelta>,
}

impl ObjectDiffReport {
    pub fn new(
        strategy: IdentityStrategy,
        retained_bucket_bits: u8,
        min_retained_bytes: u64,
    ) -> Self {
        Self {
            strategy,
            retained_bucket_bits,
            min_retained_bytes,
            added: Vec::new(),
            removed: Vec::new(),
            retained_changed: Vec::new(),
        }
    }
}
