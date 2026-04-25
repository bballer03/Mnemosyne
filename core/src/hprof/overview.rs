use serde::{Deserialize, Serialize};

// Placeholder for M7-1.A. Real OverviewSummary lands in slice M7-1.B.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OverviewSummary {
    /// Reserved for future overview-mode data. Empty until slice M7-1.B.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub placeholder: Option<()>,
}
