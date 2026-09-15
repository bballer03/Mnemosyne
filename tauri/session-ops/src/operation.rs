use serde::{Deserialize, Serialize};

/// Correlation identity echoed by every operation event and response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationContext {
    pub workspace_id: String,
    pub revision: u64,
    pub operation_id: String,
}

/// Desktop progress wire payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationProgress {
    pub context: OperationContext,
    pub kind: String,
    pub phase: mnemosyne_core::OperationPhase,
    pub completed: Option<u64>,
    pub total: Option<u64>,
    pub unit: Option<String>,
    pub indeterminate: bool,
    pub elapsed_ms: u64,
}

impl OperationProgress {
    pub fn from_snapshot(
        context: OperationContext,
        kind: impl Into<String>,
        snapshot: mnemosyne_core::OperationProgressSnapshot,
    ) -> Self {
        Self {
            context,
            kind: kind.into(),
            phase: snapshot.phase,
            completed: snapshot.completed,
            total: snapshot.total,
            unit: snapshot.unit,
            indeterminate: snapshot.indeterminate,
            elapsed_ms: snapshot.elapsed_ms,
        }
    }
}

/// Keeps only monotonic updates within the same correlated operation phase.
#[derive(Debug, Default)]
pub struct OperationProgressCoalescer {
    latest: Option<OperationProgress>,
}

impl OperationProgressCoalescer {
    pub fn coalesce(&mut self, next: OperationProgress) -> Option<OperationProgress> {
        if let Some(previous) = self.latest.as_ref() {
            let same_phase = previous.context == next.context
                && previous.kind == next.kind
                && previous.phase == next.phase;
            let moved_backward = match (previous.completed, next.completed) {
                (Some(_), None) => true,
                (Some(previous), Some(next)) => next < previous,
                _ => false,
            };
            if same_phase && moved_backward {
                return None;
            }
        }

        self.latest = Some(next.clone());
        Some(next)
    }

    pub fn latest(&self) -> Option<&OperationProgress> {
        self.latest.as_ref()
    }
}

#[cfg(test)]
mod operation_progress_tests {
    use super::{OperationContext, OperationProgress, OperationProgressCoalescer};
    use mnemosyne_core::{
        CancellationToken, CoreError, NoopOperationObserver, OperationObserver, OperationPhase,
        OperationProgressSnapshot,
    };
    use serde_json::json;

    fn context() -> OperationContext {
        OperationContext {
            workspace_id: "workspace-test".to_string(),
            revision: 7,
            operation_id: "operation-test".to_string(),
        }
    }

    fn snapshot(
        phase: OperationPhase,
        completed: Option<u64>,
        elapsed_ms: u64,
    ) -> OperationProgressSnapshot {
        OperationProgressSnapshot {
            phase,
            completed,
            total: Some(100),
            unit: Some("objects".to_string()),
            indeterminate: false,
            elapsed_ms,
        }
    }

    fn progress(
        phase: OperationPhase,
        completed: Option<u64>,
        elapsed_ms: u64,
    ) -> OperationProgress {
        OperationProgress::from_snapshot(
            context(),
            "analyze",
            snapshot(phase, completed, elapsed_ms),
        )
    }

    #[test]
    fn operation_progress_serializes_camel_case_and_echoes_context() {
        let event = progress(OperationPhase::BuildingGraph, Some(25), 1250);

        assert_eq!(
            serde_json::to_value(event).expect("progress must serialize"),
            json!({
                "context": {
                    "workspaceId": "workspace-test",
                    "revision": 7,
                    "operationId": "operation-test"
                },
                "kind": "analyze",
                "phase": "building-graph",
                "completed": 25,
                "total": 100,
                "unit": "objects",
                "indeterminate": false,
                "elapsedMs": 1250
            })
        );
    }

    #[test]
    fn operation_progress_serializes_every_known_phase_string() {
        let phases = [
            (OperationPhase::Accepted, "accepted"),
            (OperationPhase::Opening, "opening"),
            (OperationPhase::Parsing, "parsing"),
            (OperationPhase::BuildingGraph, "building-graph"),
            (OperationPhase::ComputingDominators, "computing-dominators"),
            (OperationPhase::Analyzing, "analyzing"),
            (OperationPhase::Rendering, "rendering"),
            (OperationPhase::Committing, "committing"),
            (OperationPhase::Cancelling, "cancelling"),
            (OperationPhase::Cancelled, "cancelled"),
            (OperationPhase::Complete, "complete"),
            (OperationPhase::Failed, "failed"),
        ];

        for (phase, expected) in phases {
            assert_eq!(
                serde_json::to_value(phase).expect("phase must serialize"),
                json!(expected)
            );
        }
    }

    #[test]
    fn operation_progress_coalescer_rejects_backward_progress_within_phase() {
        let mut coalescer = OperationProgressCoalescer::default();
        let first = progress(OperationPhase::Parsing, Some(10), 100);
        assert_eq!(coalescer.coalesce(first.clone()), Some(first));

        assert_eq!(
            coalescer.coalesce(progress(OperationPhase::Parsing, Some(9), 200)),
            None
        );
        assert_eq!(
            coalescer.latest().and_then(|event| event.completed),
            Some(10)
        );
    }

    #[test]
    fn operation_progress_coalescer_accepts_new_phase_and_context() {
        let mut coalescer = OperationProgressCoalescer::default();
        assert!(coalescer
            .coalesce(progress(OperationPhase::Parsing, Some(90), 100))
            .is_some());
        assert!(coalescer
            .coalesce(progress(OperationPhase::BuildingGraph, Some(0), 200))
            .is_some());

        let mut next_context = progress(OperationPhase::BuildingGraph, Some(0), 10);
        next_context.context.operation_id = "operation-next".to_string();
        assert!(coalescer.coalesce(next_context).is_some());
    }

    #[test]
    fn operation_progress_core_control_types_share_cancellation_and_noop_safely() {
        let token = CancellationToken::new();
        let clone = token.clone();
        assert!(!token.is_cancelled());
        clone.cancel();
        assert!(token.is_cancelled());

        let noop = NoopOperationObserver;
        noop.progress(snapshot(OperationPhase::Accepted, None, 0));
        assert!(!noop.is_cancelled());
        assert_eq!(
            CoreError::OperationCancelled.to_string(),
            "Operation cancelled"
        );
    }
}
