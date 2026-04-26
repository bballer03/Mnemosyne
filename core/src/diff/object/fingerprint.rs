use crate::{
    errors::{CoreError, CoreResult},
    graph::DominatorTree,
    hprof::{ObjectGraph, ObjectId},
};
use serde::{Deserialize, Serialize};

use super::types::IdentityStrategy;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ObjectFingerprint {
    pub class_id: u32,
    pub retained_bucket: u32,
    pub dominator_signature: u64,
    pub field_signature: u64,
}

impl ObjectFingerprint {
    pub fn build(
        graph: &ObjectGraph,
        dom: &DominatorTree,
        obj_id: ObjectId,
        strategy: IdentityStrategy,
        bucket_bits: u8,
    ) -> CoreResult<Self> {
        match strategy {
            IdentityStrategy::ClassRetained => {
                Ok(Self::build_class_retained(graph, dom, obj_id, bucket_bits))
            }
            _ => Err(CoreError::Unsupported(
                "Slice 8-1.A only supports IdentityStrategy::ClassRetained".into(),
            )),
        }
    }

    pub fn build_class_retained(
        graph: &ObjectGraph,
        dom: &DominatorTree,
        obj_id: ObjectId,
        bucket_bits: u8,
    ) -> Self {
        let class_name = graph
            .get_object(obj_id)
            .and_then(|obj| graph.class_name(obj.class_id))
            .unwrap_or("<unknown>");

        Self {
            class_id: stable_class_name_id(class_name),
            retained_bucket: bucket_for_retained_size(dom.retained_size(obj_id), bucket_bits),
            dominator_signature: 0,
            field_signature: 0,
        }
    }
}

pub fn bucket_for_retained_size(retained_size: u64, bucket_bits: u8) -> u32 {
    let bucket = if bucket_bits >= u64::BITS as u8 {
        0
    } else {
        retained_size >> bucket_bits
    };

    bucket.min(u64::from(u32::MAX)) as u32
}

fn stable_class_name_id(class_name: &str) -> u32 {
    let mut hash = 0x811c9dc5_u32;

    for byte in class_name.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }

    hash
}

#[cfg(test)]
mod tests {
    use super::{bucket_for_retained_size, stable_class_name_id};

    #[test]
    fn stable_class_name_id_ignores_runtime_class_ids() {
        assert_eq!(
            stable_class_name_id("com.example.Cache"),
            stable_class_name_id("com.example.Cache")
        );
        assert_ne!(
            stable_class_name_id("com.example.Cache"),
            stable_class_name_id("com.example.Session")
        );
    }

    #[test]
    fn bucket_for_retained_size_handles_extreme_bucket_bits() {
        assert_eq!(bucket_for_retained_size(17, 0), 17);
        assert_eq!(bucket_for_retained_size(17, 64), 0);
    }
}
