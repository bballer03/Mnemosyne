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

fn escape_toon_value(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

#[derive(Debug, Clone, PartialEq)]
struct ProviderFixDraft {
    description: String,
    diff: String,
    confidence: f32,
}

#[derive(Debug, Clone, PartialEq)]
struct SourceSnippet {
    target_file: String,
    diff_target_file: String,
    snippet: String,
}

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

/// Generate fix suggestions for a heap + leak combination. The current path
/// falls back to lightweight placeholder guidance until provider-backed
/// source-aware remediation is available.
pub async fn propose_fix(request: FixRequest) -> CoreResult<FixResponse> {
    let mut config = AppConfig::default();
    config.ai.enabled = true;
    propose_fix_with_config(request, &config).await
}

pub async fn propose_fix_with_config(
    request: FixRequest,
    base_config: &AppConfig,
) -> CoreResult<FixResponse> {
    let config = base_config.clone();

    let mut analysis_config = config.clone();
    analysis_config.ai.enabled = false;

    let analysis = analyze_heap(AnalyzeRequest {
        heap_path: request.heap_path.clone(),
        config: analysis_config,
        leak_options: crate::analysis::LeakDetectionOptions::new(LeakSeverity::Low),
        enable_ai: false,
        histogram_group_by: HistogramGroupBy::Class,
        ..AnalyzeRequest::default()
    })
    .await?;

    if let Some(ref target) = request.leak_id {
        validate_leak_id(&analysis.leaks, target)?;
    }

    let leaks = focus_leaks(&analysis.leaks, request.leak_id.as_deref());
    let Some(leak) = leaks.into_iter().next() else {
        return Ok(FixResponse {
            suggestions: Vec::new(),
            project_root: request.project_root,
            provenance: Vec::new(),
        });
    };

    let mut provenance = Vec::new();
    let suggestion = if config.ai.enabled
        && matches!(config.ai.mode, crate::config::AiMode::Provider)
    {
        if let Some(root) = request.project_root.as_deref() {
            if let Some(source) = source_snippet_for_leak(&leak, root) {
                let prompt = build_provider_fix_prompt(
                    &leak,
                    &request.style,
                    &source.diff_target_file,
                    &source.snippet,
                    &request.heap_path,
                );
                let prompt = crate::analysis::redact_provider_prompt(prompt, &config.ai)?;
                let ai_config = config.ai.clone();
                let draft_result = tokio::task::spawn_blocking(move || {
                    crate::analysis::complete_provider_prompt(prompt, &ai_config)
                        .and_then(|raw| parse_provider_fix_response(&raw))
                })
                .await;
                match draft_result.map_err(|err| crate::CoreError::Other(err.into())) {
                    Ok(Ok(draft))
                        if validate_provider_fix_diff(&draft.diff, &source.diff_target_file) =>
                    {
                        FixSuggestion {
                            leak_id: leak.id.clone(),
                            class_name: leak.class_name.clone(),
                            target_file: source.target_file,
                            description: draft.description,
                            diff: draft.diff,
                            confidence: draft.confidence,
                            style: request.style.clone(),
                        }
                    }
                    Ok(Ok(_)) => {
                        provenance = fallback_provenance(
                            "Provider-backed fix generation returned a diff that failed local validation.",
                        );
                        build_suggestion(&leak, request.project_root.as_deref(), &request.style)
                    }
                    Ok(Err(_)) | Err(_) => {
                        provenance = fallback_provenance(
                            "Provider-backed fix generation was unavailable; returned heuristic guidance instead.",
                        );
                        build_suggestion(&leak, request.project_root.as_deref(), &request.style)
                    }
                }
            } else {
                provenance = fallback_provenance(
                    "Provider-backed fix generation was skipped because source targeting or snippet extraction was unavailable.",
                );
                build_suggestion(&leak, request.project_root.as_deref(), &request.style)
            }
        } else {
            provenance = fallback_provenance(
                "Provider-backed fix generation was skipped because project_root was not provided.",
            );
            build_suggestion(&leak, request.project_root.as_deref(), &request.style)
        }
    } else {
        provenance = fallback_provenance(
            "Provider-backed fix generation was skipped because AI provider mode is not active.",
        );
        build_suggestion(&leak, request.project_root.as_deref(), &request.style)
    };

    Ok(FixResponse {
        suggestions: vec![suggestion],
        project_root: request.project_root,
        provenance,
    })
}

fn parse_provider_fix_response(response: &str) -> CoreResult<ProviderFixDraft> {
    if !response.starts_with("TOON v1") {
        return Err(crate::CoreError::InvalidInput(
            "provider returned malformed TOON output".into(),
        ));
    }

    let mut section = String::new();
    let mut description = None;
    let mut diff = None;
    let mut confidence = None;

    for raw_line in response.lines().skip(1) {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("section ") {
            section = rest.trim().to_string();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let value = unescape_toon_value(value);

        match (section.as_str(), key.trim()) {
            ("response", "confidence_pct") => {
                let pct: f32 = value.parse().map_err(|_| {
                    crate::CoreError::InvalidInput(
                        "provider returned invalid confidence_pct".into(),
                    )
                })?;
                confidence = Some((pct / 100.0).clamp(0.0, 1.0));
            }
            ("response", "description") => description = Some(value),
            ("patch", "diff") => diff = Some(value),
            _ => {}
        }
    }

    let description = description.ok_or_else(|| {
        crate::CoreError::InvalidInput("provider TOON output missing response description".into())
    })?;
    let diff = diff.ok_or_else(|| {
        crate::CoreError::InvalidInput("provider TOON output missing section patch diff".into())
    })?;
    let confidence = confidence.ok_or_else(|| {
        crate::CoreError::InvalidInput(
            "provider TOON output missing response confidence_pct".into(),
        )
    })?;

    Ok(ProviderFixDraft {
        description,
        diff,
        confidence,
    })
}

fn fallback_provenance(reason: &str) -> Vec<ProvenanceMarker> {
    vec![
        ProvenanceMarker::new(
            ProvenanceKind::Synthetic,
            "Fix suggestions are generated heuristically from leak summaries.",
        ),
        ProvenanceMarker::new(ProvenanceKind::Fallback, reason),
        ProvenanceMarker::new(
            ProvenanceKind::Placeholder,
            "Static-analysis-backed remediation is not wired yet; this is placeholder guidance.",
        ),
    ]
}

fn extract_source_snippet(file: &Path, line: u32) -> CoreResult<String> {
    let contents = std::fs::read_to_string(file)?;
    let lines: Vec<&str> = contents.lines().collect();
    if lines.is_empty() {
        return Ok(String::new());
    }

    let idx = line.saturating_sub(1) as usize;
    let start = idx.saturating_sub(5);
    let end = usize::min(lines.len(), idx.saturating_add(6));

    Ok(lines[start..end].join("\n"))
}

fn build_provider_fix_prompt(
    leak: &LeakInsight,
    style: &FixStyle,
    target_file: &str,
    source_snippet: &str,
    heap_path: &str,
) -> String {
    let mut body = String::from("TOON v1\n");
    body.push_str("section request\n");
    body.push_str("  intent=generate_fix\n");
    body.push_str(&format!("  heap_path={}\n", escape_toon_value(heap_path)));
    body.push_str(&format!("  leak_id={}\n", escape_toon_value(&leak.id)));
    body.push_str(&format!(
        "  class_name={}\n",
        escape_toon_value(&leak.class_name)
    ));
    body.push_str(&format!("  severity={:?}\n", leak.severity));
    body.push_str(&format!("  retained_bytes={}\n", leak.retained_size_bytes));
    body.push_str(&format!("  style={style:?}\n"));
    body.push_str(&format!(
        "  target_file={}\n",
        escape_toon_value(target_file)
    ));
    body.push_str(&format!(
        "  leak_description={}\n",
        escape_toon_value(&leak.description)
    ));
    body.push_str("section source\n");
    body.push_str(&format!(
        "  source_snippet={}\n",
        escape_toon_value(source_snippet)
    ));
    body.push_str("section instructions\n");
    body.push_str("  item#0=Return only TOON v1 output.\n");
    body.push_str("  item#1=Return section response with confidence_pct and description fields.\n");
    body.push_str("  item#2=Return section patch with a unified diff in the diff field.\n");
    body.push_str("  item#3=Patch only the provided target_file.\n");
    body.push_str("  item#4=Stay consistent with the requested fix style.\n");
    body.push_str(
        "  item#5=If context is weak, keep the patch minimal and confidence lower rather than inventing files.\n",
    );
    body
}

fn unescape_toon_value(input: &str) -> String {
    let mut out = String::new();
    let mut chars = input.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn validate_provider_fix_diff(diff: &str, target_file: &str) -> bool {
    let normalized = target_file.replace('\\', "/");
    let target = normalized.trim_start_matches("./").to_string();
    if target.is_empty() || diff.trim().is_empty() {
        return false;
    }

    let mut before_paths = Vec::new();
    let mut after_paths = Vec::new();
    for line in diff.lines() {
        if let Some(path) = line.strip_prefix("--- a/") {
            before_paths.push(path.trim().replace('\\', "/"));
        } else if let Some(path) = line.strip_prefix("+++ b/") {
            after_paths.push(path.trim().replace('\\', "/"));
        }
    }

    before_paths.len() == 1
        && after_paths.len() == 1
        && before_paths[0] == target
        && after_paths[0] == target
}

fn source_snippet_for_leak(leak: &LeakInsight, project_root: &Path) -> Option<SourceSnippet> {
    let mapped = crate::mapper::map_to_code(&crate::mapper::MapToCodeRequest {
        leak_id: leak.id.clone(),
        class_name: Some(leak.class_name.clone()),
        project_root: project_root.to_path_buf(),
        include_git_info: false,
    })
    .ok()?;

    let location = mapped.locations.into_iter().next()?;
    if !location.file.exists() {
        return None;
    }

    let snippet = extract_source_snippet(&location.file, location.line).ok()?;
    let diff_target_file = location
        .file
        .strip_prefix(project_root)
        .ok()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_else(|| location.file.display().to_string().replace('\\', "/"));

    Some(SourceSnippet {
        target_file: location.file.display().to_string(),
        diff_target_file,
        snippet,
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

        let response = propose_fix_with_config(
            FixRequest {
                heap_path: path,
                leak_id: None,
                style: FixStyle::Minimal,
                project_root: None,
            },
            &AppConfig::default(),
        )
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

    #[tokio::test]
    async fn propose_fix_uses_passed_app_config_for_ai_mode() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        write_minimal_hprof(&mut file);
        let path = file.path().to_string_lossy().into_owned();

        let mut config = AppConfig::default();
        config.ai.enabled = true;
        config.ai.mode = crate::config::AiMode::Provider;

        let response = propose_fix_with_config(
            FixRequest {
                heap_path: path,
                leak_id: None,
                style: FixStyle::Minimal,
                project_root: None,
            },
            &config,
        )
        .await
        .unwrap();

        assert!(
            response.provenance.iter().any(|marker| {
                marker.kind == ProvenanceKind::Fallback
                    && marker
                        .detail
                        .as_deref()
                        .is_some_and(|detail| detail.contains("project_root was not provided"))
            }),
            "provider-mode config should drive the project_root fallback path"
        );
    }

    #[tokio::test]
    async fn propose_fix_does_not_attempt_provider_when_ai_is_disabled() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        write_minimal_hprof(&mut file);
        let path = file.path().to_string_lossy().into_owned();

        let mut config = AppConfig::default();
        config.ai.enabled = false;
        config.ai.mode = crate::config::AiMode::Provider;

        let response = propose_fix_with_config(
            FixRequest {
                heap_path: path,
                leak_id: None,
                style: FixStyle::Minimal,
                project_root: None,
            },
            &config,
        )
        .await
        .unwrap();

        assert!(response.suggestions[0].diff.contains("SAFE_CAPACITY"));
        assert!(response.provenance.iter().any(|marker| {
            marker.kind == ProvenanceKind::Fallback
                && marker
                    .detail
                    .as_deref()
                    .is_some_and(|detail| detail.contains("AI provider mode is not active"))
        }));
    }

    #[test]
    fn parse_provider_fix_response_parses_valid_toon_payload() {
        let parsed = parse_provider_fix_response(
            "TOON v1\nsection response\n  confidence_pct=81\n  description=Release retained cache entries.\nsection patch\n  diff=--- a/src/main/java/com/example/Cache.java\\n+++ b/src/main/java/com/example/Cache.java\\n@@ ...\n",
        )
        .unwrap();

        assert_eq!(parsed.description, "Release retained cache entries.");
        assert!((parsed.confidence - 0.81).abs() < f32::EPSILON);
        assert!(parsed
            .diff
            .contains("--- a/src/main/java/com/example/Cache.java"));
    }

    #[test]
    fn parse_provider_fix_response_rejects_missing_patch_section() {
        let err = parse_provider_fix_response(
            "TOON v1\nsection response\n  confidence_pct=81\n  description=Missing patch section\n",
        )
        .unwrap_err();

        assert!(err.to_string().contains("section patch") || err.to_string().contains("diff"));
    }

    #[test]
    fn parse_provider_fix_response_preserves_literal_backslashes() {
        let diff = "--- a/src/main/java/com/example/Cache.java\n+++ b/src/main/java/com/example/Cache.java\n@@ ...\n+String pattern = \\\"\\\\n\\\";\n";
        let parsed = parse_provider_fix_response(&format!(
            "TOON v1\nsection response\n  confidence_pct=81\n  description=Preserve escapes\nsection patch\n  diff={}\n",
            escape_toon_value(diff)
        ))
        .unwrap();

        assert_eq!(parsed.diff, diff);
    }

    #[test]
    fn fallback_provenance_includes_fallback_and_placeholder_markers() {
        let provenance = fallback_provenance("provider fix generation was skipped");

        assert!(provenance
            .iter()
            .any(|m| m.kind == ProvenanceKind::Synthetic));
        assert!(provenance
            .iter()
            .any(|m| m.kind == ProvenanceKind::Fallback));
        assert!(provenance
            .iter()
            .any(|m| m.kind == ProvenanceKind::Placeholder));
    }

    #[test]
    fn extract_source_snippet_reads_small_window_around_line() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("Example.java");
        std::fs::write(
            &file,
            "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\nline11\n",
        )
        .unwrap();

        let snippet = extract_source_snippet(&file, 6).unwrap();
        assert!(snippet.contains("line1"));
        assert!(snippet.contains("line6"));
        assert!(snippet.contains("line11"));
    }

    #[test]
    fn validate_provider_fix_diff_requires_relative_target_match() {
        let target = "src/main/java/com/example/CacheLeak.java";

        assert!(validate_provider_fix_diff(
            "--- a/src/main/java/com/example/CacheLeak.java\n+++ b/src/main/java/com/example/CacheLeak.java\n@@ ...\n",
            target,
        ));

        assert!(!validate_provider_fix_diff(
            "--- a/src/main/java/com/other/CacheLeak.java\n+++ b/src/main/java/com/other/CacheLeak.java\n@@ ...\n",
            target,
        ));
    }

    #[test]
    fn validate_provider_fix_diff_rejects_multi_file_patches() {
        let target = "src/main/java/com/example/CacheLeak.java";

        assert!(!validate_provider_fix_diff(
            "--- a/src/main/java/com/example/CacheLeak.java\n+++ b/src/main/java/com/example/CacheLeak.java\n@@ ...\n--- a/src/main/java/com/example/OtherLeak.java\n+++ b/src/main/java/com/example/OtherLeak.java\n@@ ...\n",
            target,
        ));
    }

    #[test]
    fn validate_provider_fix_diff_accepts_project_root_relative_target() {
        let target = "com/example/CacheLeak.java";

        assert!(validate_provider_fix_diff(
            "--- a/com/example/CacheLeak.java\n+++ b/com/example/CacheLeak.java\n@@ ...\n",
            target,
        ));
    }

    #[test]
    fn build_provider_fix_prompt_describes_required_toon_sections() {
        let leak = LeakInsight {
            id: "leak-usersession-1".into(),
            class_name: "com.example.UserSessionCache".into(),
            leak_kind: LeakKind::Collection,
            severity: LeakSeverity::High,
            retained_size_bytes: 1024,
            shallow_size_bytes: Some(256),
            suspect_score: None,
            instances: 3,
            description: "Session cache retains stale entries".into(),
            provenance: Vec::new(),
        };

        let prompt = build_provider_fix_prompt(
            &leak,
            &FixStyle::Minimal,
            "src/main/java/com/example/UserSessionCache.java",
            "class UserSessionCache {}",
            "heap.hprof",
        );

        assert!(prompt.contains("section response"), "{prompt}");
        assert!(prompt.contains("confidence_pct"), "{prompt}");
        assert!(prompt.contains("description"), "{prompt}");
        assert!(prompt.contains("section patch"), "{prompt}");
        assert!(prompt.contains("diff field"), "{prompt}");
    }

    #[tokio::test]
    async fn propose_fix_keeps_public_single_argument_entrypoint() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        write_minimal_hprof(&mut file);

        let response = propose_fix(FixRequest {
            heap_path: file.path().to_string_lossy().into_owned(),
            leak_id: None,
            style: FixStyle::Minimal,
            project_root: None,
        })
        .await
        .unwrap();

        assert_eq!(response.suggestions.len(), 1);
    }

    #[test]
    fn propose_fix_returns_provider_backed_fix_when_source_context_is_available() {
        use std::io::{Read, Write};
        use std::net::TcpListener;
        use std::thread;

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let response_body = serde_json::json!({
            "choices": [
                {
                    "message": {
                        "content": "TOON v1\nsection response\n  confidence_pct=82\n  description=Evict idle cache entries before they accumulate.\nsection patch\n  diff=--- a/src/main/java/com/example/CacheLeak.java\\n+++ b/src/main/java/com/example/CacheLeak.java\\n@@ ...\n"
                    }
                }
            ]
        })
        .to_string();

        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0_u8; 8192];
            let read = stream.read(&mut buf).unwrap();
            let request = String::from_utf8_lossy(&buf[..read]).into_owned();
            assert!(request.contains("intent=generate_fix"), "{request}");
            assert!(request.contains("target_file="), "{request}");
            assert!(request.contains("source_snippet="), "{request}");

            let reply = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream.write_all(reply.as_bytes()).unwrap();
            stream.flush().unwrap();
        });

        let mut heap = tempfile::NamedTempFile::new().unwrap();
        write_minimal_hprof(&mut heap);

        let project = tempfile::tempdir().unwrap();
        let source_dir = project
            .path()
            .join("src")
            .join("main")
            .join("java")
            .join("com")
            .join("example");
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::write(
            source_dir.join("CacheLeak.java"),
            "package com.example;\npublic class CacheLeak {\n  void retain() {}\n}\n",
        )
        .unwrap();

        let mut config = AppConfig::default();
        config.ai.enabled = true;
        config.ai.mode = crate::config::AiMode::Provider;
        config.ai.provider = crate::config::AiProvider::Local;
        config.ai.endpoint = Some(format!("http://{addr}/v1"));
        config.ai.api_key_env = Some("MNEMOSYNE_TEST_FIX_LOCAL_KEY".into());
        config.ai.timeout_secs = 2;
        std::env::set_var("MNEMOSYNE_TEST_FIX_LOCAL_KEY", "dummy-key");

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let response = runtime
            .block_on(propose_fix_with_config(
                FixRequest {
                    heap_path: heap.path().to_string_lossy().into_owned(),
                    leak_id: None,
                    style: FixStyle::Minimal,
                    project_root: Some(project.path().to_path_buf()),
                },
                &config,
            ))
            .unwrap();

        assert_eq!(response.suggestions.len(), 1);
        assert_eq!(
            response.suggestions[0].description,
            "Evict idle cache entries before they accumulate."
        );
        assert!(response.suggestions[0].diff.contains("CacheLeak.java"));
        assert!(response.provenance.is_empty());

        std::env::remove_var("MNEMOSYNE_TEST_FIX_LOCAL_KEY");
        server.join().unwrap();
    }

    #[tokio::test]
    async fn propose_fix_falls_back_to_template_when_provider_fix_is_unavailable() {
        let mut heap = tempfile::NamedTempFile::new().unwrap();
        write_minimal_hprof(&mut heap);

        let mut config = AppConfig::default();
        config.ai.enabled = true;
        config.ai.mode = crate::config::AiMode::Provider;

        let response = propose_fix_with_config(
            FixRequest {
                heap_path: heap.path().to_string_lossy().into_owned(),
                leak_id: None,
                style: FixStyle::Minimal,
                project_root: None,
            },
            &config,
        )
        .await
        .unwrap();

        assert_eq!(response.suggestions.len(), 1);
        assert!(response.suggestions[0].diff.contains("SAFE_CAPACITY"));
        assert!(response
            .provenance
            .iter()
            .any(|m| m.kind == ProvenanceKind::Fallback));
        assert!(response
            .provenance
            .iter()
            .any(|m| m.kind == ProvenanceKind::Placeholder));
    }

    fn write_minimal_hprof(file: &mut tempfile::NamedTempFile) {
        use std::io::Write;
        file.write_all(b"JAVA PROFILE 1.0.2\0").unwrap();
        file.write_all(&4u32.to_be_bytes()).unwrap();
        file.write_all(&0u64.to_be_bytes()).unwrap();
        file.flush().unwrap();
    }
}
