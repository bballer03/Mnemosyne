use crate::{
    analysis::{AnalyzeResponse, ProvenanceKind},
    config::OutputFormat,
    errors::CoreResult,
};
use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;
use std::fmt::Write as _;

fn escape_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(ch),
        }
    }
    out
}

fn escape_toon_value(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn provenance_label(kind: ProvenanceKind) -> &'static str {
    match kind {
        ProvenanceKind::Synthetic => "SYNTHETIC",
        ProvenanceKind::Partial => "PARTIAL",
        ProvenanceKind::Fallback => "FALLBACK",
        ProvenanceKind::Placeholder => "PLACEHOLDER",
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportRequest {
    pub analysis: AnalyzeResponse,
    pub format: OutputFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportArtifact {
    pub mime_type: String,
    pub contents: String,
}

/// Generate a textual artifact from the provided analysis output.
pub fn render_report(request: &ReportRequest) -> CoreResult<ReportArtifact> {
    let (contents, mime_type) = match request.format {
        OutputFormat::Text => (render_text(&request.analysis), "text/plain"),
        OutputFormat::Toon => (render_toon(&request.analysis), "application/x-toon"),
        OutputFormat::Markdown => (render_markdown(&request.analysis), "text/markdown"),
        OutputFormat::Html => (render_html(&request.analysis), "text/html"),
        OutputFormat::Json => (render_json(&request.analysis)?, "application/json"),
    };

    Ok(ReportArtifact {
        mime_type: mime_type.into(),
        contents,
    })
}

fn render_json(analysis: &AnalyzeResponse) -> CoreResult<String> {
    Ok(to_string_pretty(analysis)?)
}

fn render_toon(analysis: &AnalyzeResponse) -> String {
    let mut doc = String::new();
    doc.push_str("TOON v1\n");

    doc.push_str("section summary\n");
    push_kv(&mut doc, 2, "heap", &analysis.summary.heap_path);
    push_kv(&mut doc, 2, "objects", analysis.summary.total_objects);
    push_kv(&mut doc, 2, "bytes", analysis.summary.total_size_bytes);
    push_kv(
        &mut doc,
        2,
        "size_gb",
        format!(
            "{:.2}",
            analysis.summary.total_size_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
        ),
    );
    push_kv(&mut doc, 2, "graph_nodes", analysis.graph.node_count);
    push_kv(&mut doc, 2, "leak_count", analysis.leaks.len());

    doc.push_str("section leaks\n");
    if analysis.leaks.is_empty() {
        push_kv(&mut doc, 2, "status", "empty");
    } else {
        for (idx, leak) in analysis.leaks.iter().enumerate() {
            let header = format!("  leak#{idx}");
            doc.push_str(&header);
            doc.push('\n');
            push_kv(&mut doc, 4, "id", &leak.id);
            push_kv(&mut doc, 4, "class", &leak.class_name);
            push_kv(&mut doc, 4, "kind", format!("{:?}", leak.leak_kind));
            push_kv(&mut doc, 4, "severity", format!("{:?}", leak.severity));
            push_kv(
                &mut doc,
                4,
                "retained_mb",
                format!("{:.2}", leak.retained_size_bytes as f64 / (1024.0 * 1024.0)),
            );
            if let Some(shallow_size) = leak.shallow_size_bytes {
                push_kv(
                    &mut doc,
                    4,
                    "shallow_mb",
                    format!("{:.2}", shallow_size as f64 / (1024.0 * 1024.0)),
                );
            }
            if let Some(score) = leak.suspect_score {
                push_kv(&mut doc, 4, "suspect_score", format!("{score:.2}"));
            }
            push_kv(&mut doc, 4, "instances", leak.instances);
            push_kv(&mut doc, 4, "description", &leak.description);
            for (pidx, marker) in leak.provenance.iter().enumerate() {
                let detail = marker.detail.as_deref().unwrap_or("");
                push_kv(
                    &mut doc,
                    4,
                    &format!("provenance#{pidx}"),
                    format!("{}: {}", provenance_label(marker.kind), detail),
                );
            }
        }
    }

    if let Some(histogram) = &analysis.histogram {
        doc.push_str("section histogram\n");
        push_kv(&mut doc, 2, "group_by", format!("{:?}", histogram.group_by));
        push_kv(&mut doc, 2, "total_instances", histogram.total_instances);
        push_kv(
            &mut doc,
            2,
            "total_shallow_size",
            histogram.total_shallow_size,
        );
        for (idx, entry) in histogram.entries.iter().take(10).enumerate() {
            doc.push_str(&format!("  entry#{idx}\n"));
            push_kv(&mut doc, 4, "key", &entry.key);
            push_kv(&mut doc, 4, "instance_count", entry.instance_count);
            push_kv(&mut doc, 4, "shallow_size", entry.shallow_size);
            push_kv(&mut doc, 4, "retained_size", entry.retained_size);
        }
    }

    if let Some(unreachable) = &analysis.unreachable {
        doc.push_str("section unreachable\n");
        push_kv(&mut doc, 2, "total_count", unreachable.total_count);
        push_kv(
            &mut doc,
            2,
            "total_shallow_size",
            unreachable.total_shallow_size,
        );
        for (idx, entry) in unreachable.by_class.iter().take(10).enumerate() {
            doc.push_str(&format!("  class#{idx}\n"));
            push_kv(&mut doc, 4, "class_name", &entry.class_name);
            push_kv(&mut doc, 4, "count", entry.count);
            push_kv(&mut doc, 4, "shallow_size", entry.shallow_size);
        }
    }

    doc.push_str("section dominators\n");
    if analysis.graph.dominators.is_empty() {
        push_kv(&mut doc, 2, "status", "empty");
    } else {
        for (idx, dom) in analysis.graph.dominators.iter().enumerate() {
            doc.push_str(&format!("  dominator#{idx}\n"));
            let parent = dom.immediate_dominator.as_deref().unwrap_or("<heap-root>");
            push_kv(&mut doc, 4, "name", &dom.name);
            push_kv(&mut doc, 4, "parent", parent);
            push_kv(&mut doc, 4, "descendants", dom.dominates);
        }
    }

    doc.push_str("section ai\n");
    if let Some(ai) = &analysis.ai {
        push_kv(&mut doc, 2, "model", &ai.model);
        push_kv(
            &mut doc,
            2,
            "confidence_pct",
            format!("{:.0}", ai.confidence * 100.0),
        );
        push_kv(&mut doc, 2, "summary", &ai.summary);
        if ai.recommendations.is_empty() {
            push_kv(&mut doc, 2, "recommendations", "none");
        } else {
            for (idx, rec) in ai.recommendations.iter().enumerate() {
                doc.push_str(&format!("  rec#{idx}\n"));
                push_kv(&mut doc, 4, "text", rec);
            }
        }
    } else {
        push_kv(&mut doc, 2, "status", "disabled");
    }

    if !analysis.provenance.is_empty() {
        doc.push_str("section provenance\n");
        for (idx, marker) in analysis.provenance.iter().enumerate() {
            doc.push_str(&format!("  marker#{idx}\n"));
            push_kv(&mut doc, 4, "kind", provenance_label(marker.kind));
            if let Some(detail) = &marker.detail {
                push_kv(&mut doc, 4, "detail", detail);
            }
        }
    }

    doc
}

fn push_kv<T: std::fmt::Display>(buf: &mut String, indent: usize, key: &str, value: T) {
    for _ in 0..indent {
        buf.push(' ');
    }
    let raw = value.to_string();
    let _ = writeln!(buf, "{}={}", key, escape_toon_value(&raw));
}

fn render_text(analysis: &AnalyzeResponse) -> String {
    let mut body = format!(
        "Mnemosyne Analysis\n=====================\nHeap: {}\nTotal Objects: {}\nTotal Size: {:.2} GB\nDetected Leaks: {}\nGraph Nodes: {}\n",
        analysis.summary.heap_path,
        analysis.summary.total_objects,
        analysis.summary.total_size_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
        analysis.leaks.len(),
        analysis.graph.node_count
    );

    if !analysis.leaks.is_empty() {
        body.push_str("\nLeak Details\n------------\n");
        for leak in &analysis.leaks {
            let retained_mb = leak.retained_size_bytes as f64 / (1024.0 * 1024.0);
            body.push_str(&format!(
                "[{}] {} ({:?}) → ~{:.2} MB across {} instances\n  {}\n",
                leak.id,
                leak.class_name,
                leak.severity,
                retained_mb,
                leak.instances,
                leak.description
            ));
            for marker in &leak.provenance {
                let detail = marker.detail.as_deref().unwrap_or("");
                body.push_str(&format!(
                    "    [{}] {}\n",
                    provenance_label(marker.kind),
                    detail
                ));
            }
        }
    }

    if !analysis.graph.dominators.is_empty() {
        body.push_str("\nDominators\n----------\n");
        for dom in &analysis.graph.dominators {
            let parent = dom.immediate_dominator.as_deref().unwrap_or("<heap-root>");
            body.push_str(&format!(
                "{} dominated by {} ({} descendants)\n",
                dom.name, parent, dom.dominates
            ));
        }
    }

    if let Some(histogram) = &analysis.histogram {
        body.push_str("\nHistogram\n---------\n");
        body.push_str(&format!("Grouped by {:?}\n", histogram.group_by));
        for entry in histogram.entries.iter().take(10) {
            body.push_str(&format!(
                "{}: {} instances, shallow {} bytes, retained {} bytes\n",
                entry.key, entry.instance_count, entry.shallow_size, entry.retained_size
            ));
        }
    }

    if let Some(unreachable) = &analysis.unreachable {
        body.push_str("\nUnreachable Objects\n-------------------\n");
        body.push_str(&format!(
            "Total unreachable: {} objects / {} bytes\n",
            unreachable.total_count, unreachable.total_shallow_size
        ));
        for entry in unreachable.by_class.iter().take(10) {
            body.push_str(&format!(
                "{}: {} objects, {} bytes\n",
                entry.class_name, entry.count, entry.shallow_size
            ));
        }
    }

    if let Some(ai) = &analysis.ai {
        body.push_str("\nAI Insights\n-----------\n");
        body.push_str(&format!(
            "Model {} (confidence {:.0}%)\n{}\n",
            ai.model,
            ai.confidence * 100.0,
            ai.summary
        ));
        for rec in &ai.recommendations {
            body.push_str(&format!("- {}\n", rec));
        }
    }

    if !analysis.provenance.is_empty() {
        body.push_str("\nProvenance\n----------\n");
        for marker in &analysis.provenance {
            let detail = marker.detail.as_deref().unwrap_or("");
            body.push_str(&format!("[{}] {}\n", provenance_label(marker.kind), detail));
        }
    }

    body
}

fn render_markdown(analysis: &AnalyzeResponse) -> String {
    let mut doc = String::new();
    doc.push_str("# Mnemosyne Analysis\n\n");
    doc.push_str(&format!("- **Heap:** {}\n", analysis.summary.heap_path));
    doc.push_str(&format!(
        "- **Total Objects:** {}\n",
        analysis.summary.total_objects
    ));
    doc.push_str(&format!(
        "- **Total Size:** {:.2} GB\n",
        analysis.summary.total_size_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    ));
    doc.push_str(&format!(
        "- **Graph Nodes:** {}\n\n",
        analysis.graph.node_count
    ));

    doc.push_str("## Detected Leaks\n\n");
    if analysis.leaks.is_empty() {
        doc.push_str("_No leaks detected during this run._\n");
    } else {
        for leak in &analysis.leaks {
            doc.push_str(&format!(
                "- [`{}`] `{}` ({:?}): ~{:.2} MB across {} instances — {}\n",
                leak.id,
                leak.class_name,
                leak.severity,
                leak.retained_size_bytes as f64 / (1024.0 * 1024.0),
                leak.instances,
                leak.description
            ));
            for marker in &leak.provenance {
                let detail = marker.detail.as_deref().unwrap_or("");
                doc.push_str(&format!(
                    "  > **{}**: {}\n",
                    provenance_label(marker.kind),
                    detail
                ));
            }
        }
    }

    if !analysis.graph.dominators.is_empty() {
        doc.push_str("\n## Dominator Highlights\n");
        for dom in &analysis.graph.dominators {
            let parent = dom.immediate_dominator.as_deref().unwrap_or("<heap-root>");
            doc.push_str(&format!(
                "- `{}` immediately dominated by `{}` ({} descendants)\n",
                dom.name, parent, dom.dominates
            ));
        }
    }

    if let Some(histogram) = &analysis.histogram {
        doc.push_str("\n## Histogram\n");
        doc.push_str(&format!("- Grouped by `{:?}`\n", histogram.group_by));
        for entry in histogram.entries.iter().take(10) {
            doc.push_str(&format!(
                "- `{}`: {} instances, shallow {} bytes, retained {} bytes\n",
                entry.key, entry.instance_count, entry.shallow_size, entry.retained_size
            ));
        }
    }

    if let Some(unreachable) = &analysis.unreachable {
        doc.push_str("\n## Unreachable Objects\n");
        doc.push_str(&format!(
            "- Total unreachable: {} objects / {} bytes\n",
            unreachable.total_count, unreachable.total_shallow_size
        ));
        for entry in unreachable.by_class.iter().take(10) {
            doc.push_str(&format!(
                "- `{}`: {} objects, {} bytes\n",
                entry.class_name, entry.count, entry.shallow_size
            ));
        }
    }

    if let Some(ai) = &analysis.ai {
        doc.push_str("\n## AI Insights\n");
        doc.push_str(&format!(
            "- Model `{}` confidence {:.0}%\n",
            ai.model,
            ai.confidence * 100.0
        ));
        doc.push_str(&format!("  {}\n", ai.summary));
        if !ai.recommendations.is_empty() {
            doc.push_str("  ### Recommendations\n");
            for rec in &ai.recommendations {
                doc.push_str(&format!("  - {}\n", rec));
            }
        }
    }

    if !analysis.provenance.is_empty() {
        doc.push_str("\n## Provenance\n\n");
        for marker in &analysis.provenance {
            let detail = marker.detail.as_deref().unwrap_or("");
            doc.push_str(&format!(
                "- **{}**: {}\n",
                provenance_label(marker.kind),
                detail
            ));
        }
    }

    doc
}

fn render_html(analysis: &AnalyzeResponse) -> String {
    let mut leak_list = String::new();
    if analysis.leaks.is_empty() {
        leak_list.push_str("<p>No leaks detected.</p>");
    } else {
        leak_list.push_str("<ul>");
        for leak in &analysis.leaks {
            let prov_spans: String = leak
                .provenance
                .iter()
                .map(|m| {
                    let detail = m.detail.as_deref().unwrap_or("");
                    format!(
                        " <span class=\"provenance {}\">[{}] {}</span>",
                        provenance_label(m.kind).to_lowercase(),
                        escape_html(provenance_label(m.kind)),
                        escape_html(detail),
                    )
                })
                .collect();
            leak_list.push_str(&format!(
                "<li><strong>{}</strong> [{}]: {:?} (~{:.2} MB, {} instances){}</li>",
                escape_html(&leak.class_name),
                escape_html(&leak.id),
                leak.severity,
                leak.retained_size_bytes as f64 / (1024.0 * 1024.0),
                leak.instances,
                prov_spans
            ));
        }
        leak_list.push_str("</ul>");
    }

    let ai_block = analysis.ai.as_ref().map(|ai| {
            let recs = if ai.recommendations.is_empty() {
                String::from("<p>No explicit recommendations.</p>")
            } else {
                let items: String = ai
                    .recommendations
                    .iter()
                    .map(|rec| format!("<li>{}</li>", escape_html(rec)))
                    .collect();
                format!("<ul>{}</ul>", items)
            };
            format!(
                "<section><h2>AI Insights</h2><p><strong>Model:</strong> {model} (confidence {confidence:.0}%)</p><p>{summary}</p>{recs}</section>",
                model = escape_html(&ai.model),
                confidence = ai.confidence * 100.0,
                summary = escape_html(&ai.summary),
                recs = recs,
            )
        }).unwrap_or_default();

    let provenance_block = if analysis.provenance.is_empty() {
        String::new()
    } else {
        let items: String = analysis
            .provenance
            .iter()
            .map(|m| {
                let detail = m.detail.as_deref().unwrap_or("");
                format!(
                    "<li class=\"provenance-{}\">[{}] {}</li>",
                    provenance_label(m.kind).to_lowercase(),
                    escape_html(provenance_label(m.kind)),
                    escape_html(detail),
                )
            })
            .collect();
        format!("<section class=\"provenance\"><h2>Provenance</h2><ul>{items}</ul></section>")
    };

    let histogram_block = analysis.histogram.as_ref().map(|histogram| {
        let items: String = histogram
            .entries
            .iter()
            .take(10)
            .map(|entry| {
                format!(
                    "<li><strong>{}</strong>: {} instances, shallow {} bytes, retained {} bytes</li>",
                    escape_html(&entry.key),
                    entry.instance_count,
                    entry.shallow_size,
                    entry.retained_size
                )
            })
            .collect();
        format!(
            "<section><h2>Histogram</h2><p><strong>Grouped by:</strong> {:?}</p><ul>{}</ul></section>",
            histogram.group_by,
            items
        )
    }).unwrap_or_default();

    let unreachable_block = analysis.unreachable.as_ref().map(|unreachable| {
        let items: String = unreachable
            .by_class
            .iter()
            .take(10)
            .map(|entry| {
                format!(
                    "<li><strong>{}</strong>: {} objects, {} bytes</li>",
                    escape_html(&entry.class_name),
                    entry.count,
                    entry.shallow_size
                )
            })
            .collect();
        format!(
            "<section><h2>Unreachable Objects</h2><p><strong>Total:</strong> {} objects / {} bytes</p><ul>{}</ul></section>",
            unreachable.total_count,
            unreachable.total_shallow_size,
            items
        )
    }).unwrap_or_default();

    format!(
        r#"<section>
  <h1>Mnemosyne Analysis</h1>
  <p><strong>Heap:</strong> {heap}</p>
  <p><strong>Total Objects:</strong> {objects}</p>
  <p><strong>Total Size:</strong> {size:.2} GB</p>
    <p><strong>Leak Count:</strong> {leaks}</p>
    <p><strong>Graph Nodes:</strong> {nodes}</p>
    <div><strong>Leaks:</strong> {leak_list}</div>
            {histogram_block}
            {unreachable_block}
      {ai_block}
            {provenance_block}
</section>"#,
        heap = escape_html(&analysis.summary.heap_path),
        objects = analysis.summary.total_objects,
        size = analysis.summary.total_size_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
        leaks = analysis.leaks.len(),
        nodes = analysis.graph.node_count,
        leak_list = leak_list,
        histogram_block = histogram_block,
        unreachable_block = unreachable_block,
        provenance_block = provenance_block
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_escaping_prevents_xss() {
        let input = r#"<script>alert("xss")</script> & 'quotes'"#;
        let escaped = escape_html(input);
        assert!(!escaped.contains('<'));
        assert!(!escaped.contains('>'));
        assert!(!escaped.contains('"'));
        assert!(escaped.contains("&amp;"));
        assert!(escaped.contains("&lt;"));
        assert!(escaped.contains("&gt;"));
        assert!(escaped.contains("&quot;"));
        assert!(escaped.contains("&#x27;"));
    }

    #[test]
    fn toon_escaping_handles_control_chars() {
        let input = "line1\nline2\r\nwith\\backslash";
        let escaped = escape_toon_value(input);
        assert!(!escaped.contains('\n'));
        assert!(!escaped.contains('\r'));
        assert_eq!(escaped, "line1\\nline2\\r\\nwith\\\\backslash");
    }

    #[test]
    fn text_report_renders_provenance() {
        use crate::analysis::{
            LeakInsight, LeakKind, LeakSeverity, ProvenanceKind, ProvenanceMarker,
        };
        use crate::graph::GraphMetrics;
        use crate::hprof::HeapSummary;
        use std::time::{Duration, SystemTime};

        let response = AnalyzeResponse {
            summary: HeapSummary {
                heap_path: "test.hprof".into(),
                total_objects: 100,
                total_size_bytes: 1024,
                classes: Vec::new(),
                generated_at: SystemTime::now(),
                header: None,
                total_records: 0,
                record_stats: Vec::new(),
            },
            leaks: vec![LeakInsight {
                id: "test::leak".into(),
                class_name: "TestClass".into(),
                leak_kind: LeakKind::Cache,
                severity: LeakSeverity::High,
                retained_size_bytes: 512,
                shallow_size_bytes: None,
                suspect_score: None,
                instances: 5,
                description: "test leak".into(),
                provenance: vec![ProvenanceMarker::new(
                    ProvenanceKind::Synthetic,
                    "test provenance",
                )],
            }],
            recommendations: Vec::new(),
            elapsed: Duration::from_millis(42),
            graph: GraphMetrics::default(),
            ai: None,
            histogram: None,
            unreachable: None,
            thread_report: None,
            collection_report: None,
            string_report: None,
            top_instances: None,
            provenance: vec![ProvenanceMarker::new(
                ProvenanceKind::Partial,
                "response provenance",
            )],
        };

        let text = render_text(&response);
        assert!(text.contains("[SYNTHETIC]"), "leak provenance missing");
        assert!(
            text.contains("test provenance"),
            "leak provenance detail missing"
        );
        assert!(text.contains("[PARTIAL]"), "response provenance missing");
        assert!(
            text.contains("response provenance"),
            "response provenance detail missing"
        );
    }

    #[test]
    fn toon_report_renders_provenance() {
        use crate::analysis::{
            LeakInsight, LeakKind, LeakSeverity, ProvenanceKind, ProvenanceMarker,
        };
        use crate::graph::GraphMetrics;
        use crate::hprof::HeapSummary;
        use std::time::{Duration, SystemTime};

        let response = AnalyzeResponse {
            summary: HeapSummary {
                heap_path: "test.hprof".into(),
                total_objects: 100,
                total_size_bytes: 1024,
                classes: Vec::new(),
                generated_at: SystemTime::now(),
                header: None,
                total_records: 0,
                record_stats: Vec::new(),
            },
            leaks: vec![LeakInsight {
                id: "test::leak".into(),
                class_name: "TestClass".into(),
                leak_kind: LeakKind::Cache,
                severity: LeakSeverity::High,
                retained_size_bytes: 512,
                shallow_size_bytes: None,
                suspect_score: None,
                instances: 5,
                description: "test leak".into(),
                provenance: vec![ProvenanceMarker::new(
                    ProvenanceKind::Synthetic,
                    "synth detail",
                )],
            }],
            recommendations: Vec::new(),
            elapsed: Duration::from_millis(42),
            graph: GraphMetrics::default(),
            ai: None,
            histogram: None,
            unreachable: None,
            thread_report: None,
            collection_report: None,
            string_report: None,
            top_instances: None,
            provenance: vec![ProvenanceMarker::new(
                ProvenanceKind::Partial,
                "response detail",
            )],
        };

        let toon = render_toon(&response);
        assert!(
            toon.contains("SYNTHETIC: synth detail"),
            "leak provenance missing in TOON"
        );
        assert!(
            toon.contains("section provenance"),
            "response provenance section missing in TOON"
        );
        assert!(
            toon.contains("kind=PARTIAL"),
            "response provenance kind missing in TOON"
        );
    }

    #[test]
    fn html_report_renders_provenance() {
        use crate::analysis::{
            LeakInsight, LeakKind, LeakSeverity, ProvenanceKind, ProvenanceMarker,
        };
        use crate::graph::GraphMetrics;
        use crate::hprof::HeapSummary;
        use std::time::{Duration, SystemTime};

        let response = AnalyzeResponse {
            summary: HeapSummary {
                heap_path: "test.hprof".into(),
                total_objects: 100,
                total_size_bytes: 1024,
                classes: Vec::new(),
                generated_at: SystemTime::now(),
                header: None,
                total_records: 0,
                record_stats: Vec::new(),
            },
            leaks: vec![LeakInsight {
                id: "test::leak".into(),
                class_name: "TestClass".into(),
                leak_kind: LeakKind::Cache,
                severity: LeakSeverity::High,
                retained_size_bytes: 512,
                shallow_size_bytes: None,
                suspect_score: None,
                instances: 5,
                description: "test leak".into(),
                provenance: vec![ProvenanceMarker::new(
                    ProvenanceKind::Synthetic,
                    "html synth detail",
                )],
            }],
            recommendations: Vec::new(),
            elapsed: Duration::from_millis(42),
            graph: GraphMetrics::default(),
            ai: None,
            histogram: None,
            unreachable: None,
            thread_report: None,
            collection_report: None,
            string_report: None,
            top_instances: None,
            provenance: vec![ProvenanceMarker::new(
                ProvenanceKind::Partial,
                "html response detail",
            )],
        };

        let html = render_html(&response);
        assert!(
            html.contains("provenance synthetic"),
            "leak provenance class missing in HTML"
        );
        assert!(
            html.contains("[SYNTHETIC]"),
            "leak provenance label missing in HTML"
        );
        assert!(
            html.contains("provenance-partial"),
            "response provenance class missing in HTML"
        );
    }
}
