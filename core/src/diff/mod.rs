pub mod class;
pub mod object;

use crate::{
    errors::{CoreError, CoreResult},
    hprof::HeapDiff,
};

pub use class::compute_class_level_diff;
pub use object::{
    DiffMode, IdentityStrategy, MatchQuality, ObjectDelta, ObjectDeltaKind, ObjectDiffReport,
    ObjectDiffTotals, ObjectFingerprint, Risk,
};

#[derive(Debug, Clone)]
pub struct DiffRequest {
    pub before_path: String,
    pub after_path: String,
    pub mode: DiffMode,
    pub identity_strategy: IdentityStrategy,
    pub retained_bucket_bits: u8,
    pub min_retained_bytes: u64,
    pub retained_change_threshold: u64,
    pub top_n: usize,
    pub retain_field_data: bool,
}

impl DiffRequest {
    pub(crate) fn class(before_path: &str, after_path: &str) -> Self {
        Self {
            before_path: before_path.into(),
            after_path: after_path.into(),
            mode: DiffMode::Class,
            identity_strategy: IdentityStrategy::default(),
            retained_bucket_bits: 10,
            min_retained_bytes: object::types::DEFAULT_OBJECT_DIFF_MIN_RETAINED_BYTES,
            retained_change_threshold: object::types::DEFAULT_RETAINED_CHANGE_THRESHOLD,
            top_n: object::types::DEFAULT_OBJECT_DIFF_TOP_N,
            retain_field_data: false,
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
            let Some((before_graph, before_dom)) = crate::analysis::engine::try_build_dominator(
                &request.before_path,
                request.retain_field_data,
            ) else {
                return Err(CoreError::Unsupported(
                    "object diff requires graph-backed analysis for the before heap".into(),
                ));
            };

            let Some((after_graph, after_dom)) = crate::analysis::engine::try_build_dominator(
                &request.after_path,
                request.retain_field_data,
            ) else {
                return Err(CoreError::Unsupported(
                    "object diff requires graph-backed analysis for the after heap".into(),
                ));
            };

            let report = object::engine::diff_object_graphs_with_limit(
                &before_graph,
                &before_dom,
                &after_graph,
                &after_dom,
                object::engine::ObjectDiffEngineConfig {
                    strategy: request.identity_strategy,
                    bucket_bits: request.retained_bucket_bits,
                    min_retained_bytes: request.min_retained_bytes,
                    retained_change_threshold: request.retained_change_threshold,
                    top_n: request.top_n,
                    max_fingerprints: object::engine::MAX_OBJECT_DIFF_FINGERPRINTS,
                },
            )
            .map_err(|error| error.to_core_error())?;

            Ok(DiffResult::Object(report))
        }
    }
}
