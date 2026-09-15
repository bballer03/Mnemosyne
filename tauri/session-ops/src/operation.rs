use std::{
    collections::HashMap,
    fmt,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use serde::{Deserialize, Serialize};

pub const OPERATION_CANCELLED_CODE: &str = "operation_cancelled";

pub fn structured_operation_cancelled_error() -> String {
    format!("{OPERATION_CANCELLED_CODE}: Operation cancelled")
}

/// Correlation identity echoed by every operation event and response.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationContext {
    pub workspace_id: String,
    pub revision: u64,
    pub operation_id: String,
}

#[derive(Debug)]
struct RegisteredOperation {
    context: OperationContext,
    cancellation: Arc<AtomicBool>,
}

/// Active desktop operations keyed by opaque operation ID.
#[derive(Debug, Default)]
pub struct OperationRegistry {
    operations: Mutex<HashMap<String, RegisteredOperation>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationRegistryError {
    DuplicateOperationId { operation_id: String },
    Unavailable,
}

impl fmt::Display for OperationRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateOperationId { operation_id } => {
                write!(formatter, "operation_already_registered: {operation_id}")
            }
            Self::Unavailable => formatter.write_str("operation_registry_unavailable"),
        }
    }
}

impl std::error::Error for OperationRegistryError {}

/// Removes its operation from the registry on every terminal path.
#[derive(Debug)]
pub struct OperationRegistration<'a> {
    registry: &'a OperationRegistry,
    context: OperationContext,
    cancellation: Arc<AtomicBool>,
}

impl OperationRegistration<'_> {
    pub fn context(&self) -> &OperationContext {
        &self.context
    }

    pub fn cancellation_token(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.cancellation)
    }
}

impl Drop for OperationRegistration<'_> {
    fn drop(&mut self) {
        let Ok(mut operations) = self.registry.operations.lock() else {
            return;
        };
        let should_remove = operations
            .get(&self.context.operation_id)
            .is_some_and(|entry| Arc::ptr_eq(&entry.cancellation, &self.cancellation));
        if should_remove {
            operations.remove(&self.context.operation_id);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelOperationResult {
    pub operation_id: String,
    pub accepted: bool,
}

impl OperationRegistry {
    pub fn register(
        &self,
        context: OperationContext,
    ) -> Result<OperationRegistration<'_>, OperationRegistryError> {
        let mut operations = self
            .operations
            .lock()
            .map_err(|_| OperationRegistryError::Unavailable)?;
        if operations.contains_key(&context.operation_id) {
            return Err(OperationRegistryError::DuplicateOperationId {
                operation_id: context.operation_id,
            });
        }

        let cancellation = Arc::new(AtomicBool::new(false));
        operations.insert(
            context.operation_id.clone(),
            RegisteredOperation {
                context: context.clone(),
                cancellation: Arc::clone(&cancellation),
            },
        );
        Ok(OperationRegistration {
            registry: self,
            context,
            cancellation,
        })
    }

    pub fn cancel(&self, operation_id: &str) -> CancelOperationResult {
        let accepted = self
            .operations
            .lock()
            .ok()
            .and_then(|operations| {
                operations.get(operation_id).map(|entry| {
                    debug_assert_eq!(entry.context.operation_id, operation_id);
                    entry.cancellation.store(true, Ordering::Release);
                })
            })
            .is_some();
        CancelOperationResult {
            operation_id: operation_id.to_string(),
            accepted,
        }
    }

    #[cfg(test)]
    fn active_len(&self) -> Result<usize, OperationRegistryError> {
        self.operations
            .lock()
            .map(|operations| operations.len())
            .map_err(|_| OperationRegistryError::Unavailable)
    }
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

#[cfg(test)]
mod operation_registry_tests {
    use super::{
        structured_operation_cancelled_error, OperationContext, OperationRegistry,
        OperationRegistryError, OPERATION_CANCELLED_CODE,
    };
    use serde_json::json;
    use std::sync::atomic::Ordering;

    fn context(operation_id: &str, workspace_id: &str, revision: u64) -> OperationContext {
        OperationContext {
            workspace_id: workspace_id.to_string(),
            revision,
            operation_id: operation_id.to_string(),
        }
    }

    #[test]
    fn operation_registry_registers_context_and_rejects_duplicate_ids() {
        let registry = OperationRegistry::default();
        let first_context = context("operation-1", "workspace-1", 7);

        let registration = registry
            .register(first_context.clone())
            .expect("first registration must succeed");

        assert_eq!(registration.context(), &first_context);
        assert_eq!(registry.active_len().expect("registry available"), 1);
        assert!(matches!(
            registry.register(context("operation-1", "workspace-2", 8)),
            Err(OperationRegistryError::DuplicateOperationId { operation_id })
                if operation_id == "operation-1"
        ));
    }

    #[test]
    fn operation_registry_cancels_active_ids_and_rejects_unknown_or_finished_ids() {
        let registry = OperationRegistry::default();
        let registration = registry
            .register(context("operation-1", "workspace-1", 7))
            .expect("registration must succeed");
        let token = registration.cancellation_token();

        assert!(!token.load(Ordering::Acquire));
        assert!(!registry.cancel("missing").accepted);

        let response = registry.cancel("operation-1");
        assert_eq!(response.operation_id, "operation-1");
        assert!(response.accepted);
        assert_eq!(
            serde_json::to_value(&response).expect("cancel response must serialize"),
            json!({ "operationId": "operation-1", "accepted": true })
        );
        assert!(token.load(Ordering::Acquire));

        drop(registration);
        assert!(!registry.cancel("operation-1").accepted);
    }

    #[test]
    fn operation_registry_cleans_up_after_success_error_and_cancel() {
        fn finish(
            registry: &OperationRegistry,
            operation_id: &str,
            terminal: Result<(), &'static str>,
        ) -> Result<(), &'static str> {
            let registration = registry
                .register(context(operation_id, "workspace-1", 7))
                .expect("registration must succeed");
            if terminal == Err(OPERATION_CANCELLED_CODE) {
                assert!(registry.cancel(operation_id).accepted);
                assert!(registration.cancellation_token().load(Ordering::Acquire));
            }
            terminal
        }

        let registry = OperationRegistry::default();
        assert_eq!(finish(&registry, "success", Ok(())), Ok(()));
        assert_eq!(
            finish(&registry, "error", Err("operation_failed")),
            Err("operation_failed")
        );
        assert_eq!(
            finish(&registry, "cancel", Err(OPERATION_CANCELLED_CODE)),
            Err(OPERATION_CANCELLED_CODE)
        );

        assert_eq!(registry.active_len().expect("registry available"), 0);
        assert!(!registry.cancel("success").accepted);
        assert!(!registry.cancel("error").accepted);
        assert!(!registry.cancel("cancel").accepted);
        assert_eq!(
            structured_operation_cancelled_error(),
            "operation_cancelled: Operation cancelled"
        );
    }

    #[test]
    fn operation_registry_isolates_cancellation_between_operation_ids() {
        let registry = OperationRegistry::default();
        let first = registry
            .register(context("operation-1", "workspace-1", 7))
            .expect("first registration must succeed");
        let second = registry
            .register(context("operation-2", "workspace-1", 7))
            .expect("second registration must succeed");

        assert!(registry.cancel("operation-1").accepted);
        assert!(first.cancellation_token().load(Ordering::Acquire));
        assert!(!second.cancellation_token().load(Ordering::Acquire));
        assert_eq!(registry.active_len().expect("registry available"), 2);
    }
}
