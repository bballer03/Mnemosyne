//! Dependency-neutral progress observation and cooperative cancellation.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use serde::{Deserialize, Serialize};

/// Named phases shared by core operations and host progress events.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationPhase {
    Accepted,
    Opening,
    Parsing,
    BuildingGraph,
    ComputingDominators,
    Analyzing,
    Rendering,
    Committing,
    Cancelling,
    Cancelled,
    Complete,
    Failed,
}

/// Generic progress payload emitted by dependency-neutral core operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationProgressSnapshot {
    pub phase: OperationPhase,
    pub completed: Option<u64>,
    pub total: Option<u64>,
    pub unit: Option<String>,
    pub indeterminate: bool,
    pub elapsed_ms: u64,
}

/// Cloneable cooperative cancellation signal shared across operation layers.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

/// Receives bounded progress updates and exposes cooperative cancellation.
pub trait OperationObserver: Send + Sync {
    fn progress(&self, event: OperationProgressSnapshot);
    fn is_cancelled(&self) -> bool;
}

/// Observer used by unchanged CLI and MCP call sites.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopOperationObserver;

impl OperationObserver for NoopOperationObserver {
    fn progress(&self, _event: OperationProgressSnapshot) {}

    fn is_cancelled(&self) -> bool {
        false
    }
}
