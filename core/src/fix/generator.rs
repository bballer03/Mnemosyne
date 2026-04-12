use crate::{
    analysis::{
        analyze_heap, focus_leaks, validate_leak_id, AnalyzeRequest, LeakInsight, LeakSeverity,
        ProvenanceKind, ProvenanceMarker,
    },
    config::AppConfig,
    errors::CoreResult,
    HistogramGroupBy,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FixStyle {
    Minimal,
    Defensive,
    Comprehensive,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixRequest {
    pub heap_path: String,
    pub leak_id: Option<String>,
    pub style: FixStyle,
    pub project_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixSuggestion {
    pub leak_id: String,
    pub class_name: String,
    pub target_file: String,
    pub description: String,
    pub diff: String,
    pub confidence: f32,
    pub style: FixStyle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixResponse {
    pub suggestions: Vec<FixSuggestion>,
    pub project_root: Option<PathBuf>,
    /// Provenance markers (e.g. synthetic / placeholder when real fix pipeline is not wired).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub provenance: Vec<ProvenanceMarker>,
}

/// Generate heuristic fix suggestions for a heap + leak combination. The fixes
/// are intentionally lightweight placeholders until the real static analysis
/// pipeline is wired up.
pub async fn propose_fix(request: FixRequest) -> CoreResult<FixResponse> {
    let mut config = AppConfig::default();
    config.ai.enabled = true;

    let analysis = analyze_heap(AnalyzeRequest {
        heap_path: request.heap_path.clone(),
        config: config.clone(),
        leak_options: crate::analysis::LeakDetectionOptions::new(LeakSeverity::Low),
        enable_ai: true,
        histogram_group_by: HistogramGroupBy::Class,
        ..AnalyzeRequest::default()
    })
    .await?;

    if let Some(ref target) = request.leak_id {
        validate_leak_id(&analysis.leaks, target)?;
    }

    let leaks = focus_leaks(&analysis.leaks, request.leak_id.as_deref());
    let suggestions = leaks
        .into_iter()
        .take(1)
        .map(|leak| build_suggestion(&leak, request.project_root.as_deref(), &request.style))
        .collect();

    Ok(FixResponse {
        suggestions,
        project_root: request.project_root,
        provenance: vec![
            ProvenanceMarker::new(
                ProvenanceKind::Synthetic,
                "Fix suggestions are generated heuristically from leak summaries.",
            ),
            ProvenanceMarker::new(
                ProvenanceKind::Placeholder,
                "Static-analysis-backed remediation is not wired yet; this is placeholder guidance.",
            ),
        ],
    })
}

fn build_suggestion(
    leak: &LeakInsight,
    project_root: Option<&Path>,
    style: &FixStyle,
) -> FixSuggestion {
    let target_file = resolve_file_hint(leak, project_root);
    let (description, diff) = match style {
        FixStyle::Minimal => minimal_fix(leak, &target_file),
        FixStyle::Defensive => defensive_fix(leak, &target_file),
        FixStyle::Comprehensive => comprehensive_fix(leak, &target_file),
    };

    let base_confidence = match leak.severity {
        LeakSeverity::Low => 0.45,
        LeakSeverity::Medium => 0.55,
        LeakSeverity::High => 0.65,
        LeakSeverity::Critical => 0.72,
    };

    FixSuggestion {
        leak_id: leak.id.clone(),
        class_name: leak.class_name.clone(),
        target_file,
        description,
        diff,
        confidence: (base_confidence + 0.1_f32).min(0.95_f32),
        style: style.clone(),
    }
}

fn resolve_file_hint(leak: &LeakInsight, project_root: Option<&Path>) -> String {
    let relative = leak.class_name.replace('.', "/") + ".java";
    if let Some(root) = project_root {
        root.join("src/main/java")
            .join(&relative)
            .display()
            .to_string()
    } else {
        relative
    }
}

fn minimal_fix(leak: &LeakInsight, file: &str) -> (String, String) {
    let description = format!(
        "Add guard clauses so {} releases references when exceeding safe capacity.",
        leak.class_name
    );
    let diff = format!(
        "--- a/{file}\n+++ b/{file}\n@@\n-// TODO: release retained objects\n+if (cache.size() > SAFE_CAPACITY) {{\n+    cache.clear();\n+}}\n"
    );
    (description, diff)
}

fn defensive_fix(leak: &LeakInsight, file: &str) -> (String, String) {
    let description = format!(
        "Wrap {} allocations in try-with-resources / finally blocks to avoid lingering references.",
        leak.class_name
    );
    let diff = format!(
        "--- a/{file}\n+++ b/{file}\n@@ public void retain(...)\n-Resource r = allocator.acquire();\n+try (Resource r = allocator.acquire()) {{\n+    // existing logic\n+}}\n"
    );
    (description, diff)
}

fn comprehensive_fix(leak: &LeakInsight, file: &str) -> (String, String) {
    let description = format!(
        "Refactor {} to use weak references and scheduled cleanup handling to break root chains.",
        leak.class_name
    );
    let diff = format!(
        "--- a/{file}\n+++ b/{file}\n@@\n-Map<String, Object> cache = new HashMap<>();\n+Map<String, Object> cache = new WeakHashMap<>();\n+ScheduledExecutorService reap = Executors.newSingleThreadScheduledExecutor();\n+reap.scheduleAtFixedRate(this::cleanup, 1, 1, TimeUnit.MINUTES);\n"
    );
    (description, diff)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{LeakKind, LeakSeverity};

    #[test]
    fn builds_suggestion() {
        let leak = LeakInsight {
            id: "com.example.Cache::deadbeef".into(),
            class_name: "com.example.Cache".into(),
            leak_kind: LeakKind::Cache,
            severity: LeakSeverity::High,
            retained_size_bytes: 10,
            shallow_size_bytes: None,
            suspect_score: None,
            instances: 2,
            description: String::new(),
            provenance: Vec::new(),
        };
        let suggestion = build_suggestion(&leak, None, &FixStyle::Minimal);
        assert!(suggestion.diff.contains("SAFE_CAPACITY"));
        assert_eq!(suggestion.leak_id, leak.id);
    }

    #[tokio::test]
    async fn propose_fix_runs_analysis() {
        // Write a tiny fake heap file so analyze_heap has something to parse.
        let mut file = tempfile::NamedTempFile::new().unwrap();
        write_minimal_hprof(&mut file);
        let path = file.path().to_string_lossy().into_owned();

        let response = propose_fix(FixRequest {
            heap_path: path,
            leak_id: None,
            style: FixStyle::Minimal,
            project_root: None,
        })
        .await
        .unwrap();

        assert!(!response.suggestions.is_empty());
        assert!(
            response
                .provenance
                .iter()
                .any(|m| m.kind == ProvenanceKind::Synthetic),
            "fix response must carry Synthetic provenance"
        );
        assert!(
            response
                .provenance
                .iter()
                .any(|m| m.kind == ProvenanceKind::Placeholder),
            "fix response must carry Placeholder provenance"
        );
    }

    fn write_minimal_hprof(file: &mut tempfile::NamedTempFile) {
        use std::io::Write;
        file.write_all(b"JAVA PROFILE 1.0.2\0").unwrap();
        file.write_all(&4u32.to_be_bytes()).unwrap();
        file.write_all(&0u64.to_be_bytes()).unwrap();
        file.flush().unwrap();
    }
}
