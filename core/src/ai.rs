use crate::{analysis::LeakInsight, config::AiConfig, heap::HeapSummary};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiInsights {
    pub model: String,
    pub summary: String,
    pub recommendations: Vec<String>,
    pub confidence: f32,
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
    }
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
    use crate::heap::HeapSummary;
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
            instances: 42,
            description: "Half the heap".into(),
        };
        let config = AiConfig::default();

        let insights = generate_ai_insights(&summary, &[leak], &config);
        assert!(insights.summary.contains("com.example.Leak"));
        assert!(insights.recommendations.len() >= 2);
        assert!(insights.confidence > 0.5);
    }

    #[test]
    fn handles_empty_leaks() {
        let summary = HeapSummary::placeholder("heap");
        let config = AiConfig::default();

        let insights = generate_ai_insights(&summary, &[], &config);
        assert!(insights.summary.contains("looks healthy"));
        assert_eq!(insights.recommendations.len(), 1);
    }
}
