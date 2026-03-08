use super::engine::LeakInsight;
use crate::{config::AiConfig, hprof::HeapSummary, CoreError, CoreResult};
use serde::{Deserialize, Serialize};
use std::fmt::Write as _;

fn escape_toon_value(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiInsights {
    pub model: String,
    pub summary: String,
    pub recommendations: Vec<String>,
    pub confidence: f32,
    pub wire: AiWireExchange,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiWireExchange {
    pub format: AiWireFormat,
    pub prompt: String,
    pub response: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub enum AiWireFormat {
    #[default]
    Toon,
}

/// Narrow the leak set to a specific identifier; falls back to the full list if
/// no matching leak is found so downstream callers always have context.
pub fn focus_leaks(leaks: &[LeakInsight], leak_id: Option<&str>) -> Vec<LeakInsight> {
    if leaks.is_empty() {
        return Vec::new();
    }

    if let Some(target) = leak_id {
        let matches: Vec<LeakInsight> = leaks
            .iter()
            .filter(|leak| leak.id == target || leak.class_name == target)
            .cloned()
            .collect();
        if !matches.is_empty() {
            return matches;
        }
    }

    leaks.to_vec()
}

/// Validate that a given leak ID exists in the leak set.
/// Returns an error if the ID is specified but not found.
pub fn validate_leak_id(leaks: &[LeakInsight], leak_id: &str) -> CoreResult<()> {
    if leaks
        .iter()
        .any(|leak| leak.id == leak_id || leak.class_name == leak_id)
    {
        Ok(())
    } else {
        Err(CoreError::InvalidInput(format!(
            "no leak found matching identifier '{leak_id}'"
        )))
    }
}

/// Generate a deterministic, heuristic "AI" insight so that higher layers can
/// exercise the UX before real LLM integration is available.
pub fn generate_ai_insights(
    summary: &HeapSummary,
    leaks: &[LeakInsight],
    config: &AiConfig,
) -> AiInsights {
    let top = leaks
        .iter()
        .max_by_key(|leak| leak.retained_size_bytes)
        .cloned();

    let summary_text = match &top {
        Some(leak) => format!(
            "{class} is retaining ~{size:.2} MB via {instances} instances; prioritize freeing it to reclaim {percent:.1}% of the heap.",
            class = leak.class_name,
            size = bytes_to_mb(leak.retained_size_bytes),
            instances = leak.instances,
            percent = retained_percent(leak.retained_size_bytes, summary.total_size_bytes),
        ),
        None => format!(
            "Heap `{}` looks healthy; continue monitoring but no blockers were detected.",
            summary.heap_path
        ),
    };

    let mut recs = Vec::new();
    if let Some(leak) = &top {
        recs.push(format!(
            "Guard {} lifetimes: ensure cleanup hooks dispose unused entries.",
            leak.class_name
        ));
        recs.push("Add targeted instrumentation (counters, timers) around the suspected allocation sites.".into());
        if leak.severity >= crate::analysis::LeakSeverity::High {
            recs.push(
                "Review threading / coroutine lifecycles anchoring these objects to a GC root."
                    .into(),
            );
        }
    } else {
        recs.push("Capture a heap dump under load to validate steady-state behavior.".into());
    }

    let confidence = (0.55 + leaks.len() as f32 * 0.05 - config.temperature * 0.1).clamp(0.3, 0.92);

    AiInsights {
        model: config.model.clone(),
        summary: summary_text,
        recommendations: recs,
        confidence,
        wire: AiWireExchange {
            format: AiWireFormat::Toon,
            prompt: build_toon_prompt(summary, leaks),
            response: build_toon_response(summary, &top, confidence, config),
        },
    }
}

fn build_toon_prompt(summary: &HeapSummary, leaks: &[LeakInsight]) -> String {
    let mut body = String::from("TOON v1\n");
    body.push_str("section request\n");
    push_kv(&mut body, 2, "intent", "explain_leak");
    push_kv(&mut body, 2, "heap_path", &summary.heap_path);
    push_kv(&mut body, 2, "total_bytes", summary.total_size_bytes);
    push_kv(&mut body, 2, "total_objects", summary.total_objects);
    push_kv(&mut body, 2, "leak_sampled", leaks.len());

    body.push_str("section leaks\n");
    if leaks.is_empty() {
        push_kv(&mut body, 2, "status", "empty");
    } else {
        for (idx, leak) in leaks.iter().enumerate().take(3) {
            body.push_str(&format!("  leak#{idx}\n"));
            push_kv(&mut body, 4, "id", &leak.id);
            push_kv(&mut body, 4, "class", &leak.class_name);
            push_kv(&mut body, 4, "kind", format!("{:?}", leak.leak_kind));
            push_kv(&mut body, 4, "severity", format!("{:?}", leak.severity));
            push_kv(
                &mut body,
                4,
                "retained_mb",
                format!("{:.2}", bytes_to_mb(leak.retained_size_bytes)),
            );
            push_kv(&mut body, 4, "instances", leak.instances);
            push_kv(&mut body, 4, "description", &leak.description);
        }
    }

    body
}

fn build_toon_response(
    summary: &HeapSummary,
    top: &Option<LeakInsight>,
    confidence: f32,
    config: &AiConfig,
) -> String {
    let mut body = String::from("TOON v1\n");
    body.push_str("section response\n");
    push_kv(&mut body, 2, "model", &config.model);
    push_kv(
        &mut body,
        2,
        "confidence_pct",
        format!("{:.0}", confidence * 100.0),
    );

    match top {
        Some(leak) => {
            push_kv(
                &mut body,
                2,
                "summary",
                format!(
                    "{class} retains ~{size:.2} MB via {instances} instances (severity {severity:?}).",
                    class = leak.class_name,
                    size = bytes_to_mb(leak.retained_size_bytes),
                    instances = leak.instances,
                    severity = leak.severity
                ),
            );
            body.push_str("section remediation\n");
            push_kv(&mut body, 2, "priority", "high");
            push_kv(
                &mut body,
                2,
                "retained_percent",
                format!(
                    "{:.1}",
                    retained_percent(leak.retained_size_bytes, summary.total_size_bytes)
                ),
            );
        }
        None => {
            push_kv(
                &mut body,
                2,
                "summary",
                format!("Heap `{}` currently looks healthy.", summary.heap_path),
            );
            body.push_str("section remediation\n");
            push_kv(&mut body, 2, "priority", "observe");
        }
    }

    body
}

fn push_kv<T: std::fmt::Display>(buf: &mut String, indent: usize, key: &str, value: T) {
    for _ in 0..indent {
        buf.push(' ');
    }
    let raw = value.to_string();
    let _ = writeln!(buf, "{}={}", key, escape_toon_value(&raw));
}

fn bytes_to_mb(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0)
}

fn retained_percent(retained: u64, total: u64) -> f64 {
    if total == 0 {
        return 0.0;
    }
    (retained as f64 / total as f64) * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::{LeakKind, LeakSeverity};
    use crate::hprof::HeapSummary;
    use std::time::SystemTime;

    #[test]
    fn generates_summary_with_leak() {
        let summary = HeapSummary {
            heap_path: "heap.hprof".into(),
            total_objects: 10,
            total_size_bytes: 512 * 1024 * 1024,
            classes: Vec::new(),
            generated_at: SystemTime::now(),
            header: None,
            total_records: 0,
            record_stats: Vec::new(),
        };
        let leak = LeakInsight {
            id: "com.example.Leak::deadbeef".into(),
            class_name: "com.example.Leak".into(),
            leak_kind: LeakKind::Cache,
            severity: LeakSeverity::High,
            retained_size_bytes: 256 * 1024 * 1024,
            shallow_size_bytes: None,
            suspect_score: None,
            instances: 42,
            description: "Half the heap".into(),
            provenance: Vec::new(),
        };
        let config = AiConfig::default();

        let insights = generate_ai_insights(&summary, &[leak], &config);
        assert!(insights.summary.contains("com.example.Leak"));
        assert!(insights.recommendations.len() >= 2);
        assert!(insights.confidence > 0.5);
        assert!(insights.wire.prompt.starts_with("TOON v1"));
        assert!(insights.wire.response.contains("section response"));
    }

    #[test]
    fn handles_empty_leaks() {
        let summary = HeapSummary::placeholder("heap");
        let config = AiConfig::default();

        let insights = generate_ai_insights(&summary, &[], &config);
        assert!(insights.summary.contains("looks healthy"));
        assert_eq!(insights.recommendations.len(), 1);
        assert_eq!(insights.wire.format, AiWireFormat::Toon);
    }

    #[test]
    fn focuses_on_matching_leak() {
        let leaks = vec![
            LeakInsight {
                id: "LeakA::1".into(),
                class_name: "LeakA".into(),
                leak_kind: LeakKind::Cache,
                severity: LeakSeverity::Low,
                retained_size_bytes: 1,
                shallow_size_bytes: None,
                suspect_score: None,
                instances: 1,
                description: String::new(),
                provenance: Vec::new(),
            },
            LeakInsight {
                id: "LeakB::2".into(),
                class_name: "LeakB".into(),
                leak_kind: LeakKind::Thread,
                severity: LeakSeverity::High,
                retained_size_bytes: 2,
                shallow_size_bytes: None,
                suspect_score: None,
                instances: 2,
                description: String::new(),
                provenance: Vec::new(),
            },
        ];

        let focused = focus_leaks(&leaks, Some("LeakB::2"));
        assert_eq!(focused.len(), 1);
        assert_eq!(focused[0].class_name, "LeakB");

        // Fallback to all leaks when no match.
        let fallback = focus_leaks(&leaks, Some("missing"));
        assert_eq!(fallback.len(), leaks.len());
    }

    #[test]
    fn validates_matching_leak_id() {
        let leaks = vec![LeakInsight {
            id: "LeakA::1".into(),
            class_name: "LeakA".into(),
            leak_kind: LeakKind::Cache,
            severity: LeakSeverity::Low,
            retained_size_bytes: 1,
            shallow_size_bytes: None,
            suspect_score: None,
            instances: 1,
            description: String::new(),
            provenance: Vec::new(),
        }];

        assert!(validate_leak_id(&leaks, "LeakA::1").is_ok());
        assert!(validate_leak_id(&leaks, "LeakA").is_ok());
    }

    #[test]
    fn rejects_unknown_leak_id() {
        let leaks = vec![LeakInsight {
            id: "LeakA::1".into(),
            class_name: "LeakA".into(),
            leak_kind: LeakKind::Cache,
            severity: LeakSeverity::Low,
            retained_size_bytes: 1,
            shallow_size_bytes: None,
            suspect_score: None,
            instances: 1,
            description: String::new(),
            provenance: Vec::new(),
        }];

        let err = validate_leak_id(&leaks, "missing").unwrap_err();
        assert!(err
            .to_string()
            .contains("no leak found matching identifier"));
    }
}
