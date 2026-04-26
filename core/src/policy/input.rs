use crate::{AnalysisMode, AnalyzeResponse, OverviewSummary};

#[derive(Debug, Clone, Copy)]
pub enum PolicyInput<'a> {
    Deep(&'a AnalyzeResponse),
    Overview(&'a OverviewSummary),
}

impl<'a> PolicyInput<'a> {
    pub fn mode_used(&self) -> AnalysisMode {
        match self {
            Self::Deep(_) => AnalysisMode::Deep,
            Self::Overview(_) => AnalysisMode::Overview,
        }
    }
}
