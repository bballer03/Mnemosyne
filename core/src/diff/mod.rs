pub mod class;
pub mod object;

use crate::{
    errors::{CoreError, CoreResult},
    hprof::HeapDiff,
};

pub use class::compute_class_level_diff;
pub use object::{DiffMode, IdentityStrategy, ObjectDelta, ObjectDiffReport, ObjectFingerprint};

#[derive(Debug, Clone)]
pub(crate) struct DiffRequest {
    pub before_path: String,
    pub after_path: String,
    pub mode: DiffMode,
    pub identity_strategy: IdentityStrategy,
    pub retained_bucket_bits: u8,
    pub min_retained_bytes: u64,
}

impl DiffRequest {
    pub(crate) fn class(before_path: &str, after_path: &str) -> Self {
        Self {
            before_path: before_path.into(),
            after_path: after_path.into(),
            mode: DiffMode::Class,
            identity_strategy: IdentityStrategy::ClassRetained,
            retained_bucket_bits: 10,
            min_retained_bytes: 0,
        }
    }
}

pub(crate) enum DiffResult {
    Class(HeapDiff),
    Object(ObjectDiffReport),
}

pub(crate) async fn run_diff(request: DiffRequest) -> CoreResult<DiffResult> {
    match request.mode {
        DiffMode::Class => Ok(DiffResult::Class(
            crate::analysis::engine::diff_heaps_class(&request.before_path, &request.after_path)
                .await?,
        )),
        DiffMode::Object => {
            if request.identity_strategy != IdentityStrategy::ClassRetained {
                return Err(CoreError::Unsupported(
                    "Slice 8-1.A only supports IdentityStrategy::ClassRetained".into(),
                ));
            }

            let Some((before_graph, before_dom)) =
                crate::analysis::engine::try_build_dominator(&request.before_path, false)
            else {
                return Err(CoreError::Unsupported(
                    "object diff requires graph-backed analysis for the before heap".into(),
                ));
            };

            let Some((after_graph, after_dom)) =
                crate::analysis::engine::try_build_dominator(&request.after_path, false)
            else {
                return Err(CoreError::Unsupported(
                    "object diff requires graph-backed analysis for the after heap".into(),
                ));
            };

            let report = object::engine::diff_object_graphs(
                &before_graph,
                &before_dom,
                &after_graph,
                &after_dom,
                request.identity_strategy,
                request.retained_bucket_bits,
                request.min_retained_bytes,
            )?;

            Ok(DiffResult::Object(report))
        }
    }
}
