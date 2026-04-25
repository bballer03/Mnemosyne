use crate::{
    analysis::{AnalysisMode, AnalyzeResponse, ProvenanceKind},
    config::OutputFormat,
    errors::CoreResult,
    hprof::{GcRootKind, OverviewSummary},
};
use serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;
use std::fmt::Write as _;

const OVERVIEW_BANNER: &str = "Overview mode (streaming, no object graph)";
const OVERVIEW_LIMITATION_NOTE: &str =
    "Approximate shallow sizes only. Retained sizes, dominator tree, and leak suspects are not available.";

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
    if matches!(request.analysis.mode, AnalysisMode::Overview) {
        if let Some(overview) = &request.analysis.overview {
            return render_overview_report(overview, request.format.clone());
        }
    }

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

pub fn render_overview_report(
    summary: &OverviewSummary,
    format: OutputFormat,
) -> CoreResult<ReportArtifact> {
    let (contents, mime_type) = match format {
        OutputFormat::Text => (render_overview_text(summary), "text/plain"),
        OutputFormat::Toon => (render_overview_toon(summary), "application/x-toon"),
        OutputFormat::Markdown => (render_overview_markdown(summary), "text/markdown"),
        OutputFormat::Html => (render_overview_html(summary), "text/html"),
        OutputFormat::Json => (render_overview_json(summary)?, "application/json"),
    };

    Ok(ReportArtifact {
        mime_type: mime_type.into(),
        contents,
    })
}

fn render_overview_json(summary: &OverviewSummary) -> CoreResult<String> {
    Ok(to_string_pretty(summary)?)
}

fn render_overview_text(summary: &OverviewSummary) -> String {
    let mut body = String::new();
    let _ = writeln!(body, "{OVERVIEW_BANNER}");
    let _ = writeln!(body, "{}", "=".repeat(OVERVIEW_BANNER.len()));
    let _ = writeln!(body, "Heap: {}", summary.heap_path);
    let _ = writeln!(
        body,
        "Bytes processed: {}",
        format_byte_count(summary.total_bytes_processed)
    );
    let _ = writeln!(body, "HPROF records: {}", summary.total_record_count);
    let _ = writeln!(body, "{OVERVIEW_LIMITATION_NOTE}");
    let _ = writeln!(body, "Retained sizes: not available in overview mode");
    let _ = writeln!(body, "Leak suspects: not available in overview mode");
    let _ = writeln!(body, "Dominator tree: not available in overview mode");

    body.push_str(
        "\nTop Classes by Approximate Shallow Bytes\n---------------------------------------\n",
    );
    if summary.class_stats.entries.is_empty() {
        body.push_str("No class aggregates captured.\n");
    } else {
        for entry in &summary.class_stats.entries {
            let _ = writeln!(
                body,
                "- {}: {} instances, {} approx shallow",
                entry.class_name,
                entry.instance_count,
                format_byte_count(entry.approx_shallow_bytes)
            );
        }
    }

    body.push_str(
        "\nLargest Single Instances (approximate shallow size, not retained)\n---------------------------------------------------------------\n",
    );
    if summary.top_instances.is_empty() {
        body.push_str("No instance-level samples captured.\n");
    } else {
        for instance in &summary.top_instances {
            let _ = writeln!(
                body,
                "- {} {}: {}",
                format_object_id(instance.object_id),
                instance.class_name,
                format_byte_count(instance.approx_retained_bytes)
            );
        }
    }

    body.push_str("\nGC Root Counts\n--------------\n");
    for (label, count) in ordered_gc_root_counts(summary) {
        let _ = writeln!(body, "- {label}: {count}");
    }

    body.push_str("\nThread Frame Sample (capped)\n---------------------------\n");
    if summary.thread_frames.is_empty() {
        body.push_str("No thread frames captured.\n");
    } else {
        for frame in &summary.thread_frames {
            let _ = writeln!(body, "- {}", format_thread_frame(frame));
        }
    }

    body.push_str("\nTruncation Flags\n----------------\n");
    let _ = writeln!(body, "- truncated: {}", yes_no(summary.truncated));
    let _ = writeln!(
        body,
        "- class table cap: {}",
        summary.options.max_class_table_size
    );
    let _ = writeln!(
        body,
        "- thread frame cap: {}",
        summary.options.max_thread_frames
    );

    body
}

fn render_overview_markdown(summary: &OverviewSummary) -> String {
    let mut doc = String::new();
    doc.push_str("# Overview mode (streaming, no object graph)\n\n");
    doc.push_str(&format!("- **Heap:** {}\n", summary.heap_path));
    doc.push_str(&format!(
        "- **Bytes processed:** {}\n",
        format_byte_count(summary.total_bytes_processed)
    ));
    doc.push_str(&format!(
        "- **HPROF records:** {}\n",
        summary.total_record_count
    ));
    doc.push_str(&format!("- **Note:** {OVERVIEW_LIMITATION_NOTE}\n"));
    doc.push_str("- **Retained sizes:** not available in overview mode\n");
    doc.push_str("- **Leak suspects:** not available in overview mode\n");
    doc.push_str("- **Dominator tree:** not available in overview mode\n");

    doc.push_str("\n## Top Classes by Approximate Shallow Bytes\n\n");
    if summary.class_stats.entries.is_empty() {
        doc.push_str("_No class aggregates captured._\n");
    } else {
        for entry in &summary.class_stats.entries {
            doc.push_str(&format!(
                "- `{}`: {} instances, {} approx shallow\n",
                entry.class_name,
                entry.instance_count,
                format_byte_count(entry.approx_shallow_bytes)
            ));
        }
    }

    doc.push_str("\n## Largest Single Instances (approximate shallow size, not retained)\n\n");
    if summary.top_instances.is_empty() {
        doc.push_str("_No instance-level samples captured._\n");
    } else {
        for instance in &summary.top_instances {
            doc.push_str(&format!(
                "- `{}` `{}`: {}\n",
                format_object_id(instance.object_id),
                instance.class_name,
                format_byte_count(instance.approx_retained_bytes)
            ));
        }
    }

    doc.push_str("\n## GC Root Counts\n\n");
    for (label, count) in ordered_gc_root_counts(summary) {
        doc.push_str(&format!("- **{label}:** {count}\n"));
    }

    doc.push_str("\n## Thread Frame Sample (capped)\n\n");
    if summary.thread_frames.is_empty() {
        doc.push_str("_No thread frames captured._\n");
    } else {
        for frame in &summary.thread_frames {
            doc.push_str(&format!("- `{}`\n", format_thread_frame(frame)));
        }
    }

    doc.push_str("\n## Truncation Flags\n\n");
    doc.push_str(&format!("- **truncated:** {}\n", yes_no(summary.truncated)));
    doc.push_str(&format!(
        "- **class table cap:** {}\n",
        summary.options.max_class_table_size
    ));
    doc.push_str(&format!(
        "- **thread frame cap:** {}\n",
        summary.options.max_thread_frames
    ));

    doc
}

fn render_overview_toon(summary: &OverviewSummary) -> String {
    let mut doc = String::new();
    doc.push_str("TOON v1\n");
    doc.push_str("section overview\n");
    push_kv(&mut doc, 2, "mode", "overview");
    push_kv(&mut doc, 2, "note", OVERVIEW_LIMITATION_NOTE);
    push_kv(&mut doc, 2, "heap", &summary.heap_path);
    push_kv(
        &mut doc,
        2,
        "bytes_processed",
        summary.total_bytes_processed,
    );
    push_kv(&mut doc, 2, "record_count", summary.total_record_count);
    push_kv(&mut doc, 2, "retained_sizes", "not_available");
    push_kv(&mut doc, 2, "leak_suspects", "not_available");
    push_kv(&mut doc, 2, "dominator_tree", "not_available");
    push_kv(&mut doc, 2, "truncated", yes_no(summary.truncated));

    doc.push_str("section top_classes\n");
    if summary.class_stats.entries.is_empty() {
        push_kv(&mut doc, 2, "status", "empty");
    } else {
        for (idx, entry) in summary.class_stats.entries.iter().enumerate() {
            doc.push_str(&format!("  class#{idx}\n"));
            push_kv(&mut doc, 4, "class_name", &entry.class_name);
            push_kv(&mut doc, 4, "instance_count", entry.instance_count);
            push_kv(
                &mut doc,
                4,
                "approx_shallow_bytes",
                entry.approx_shallow_bytes,
            );
        }
    }

    doc.push_str("section top_instances\n");
    if summary.top_instances.is_empty() {
        push_kv(&mut doc, 2, "status", "empty");
    } else {
        for (idx, instance) in summary.top_instances.iter().enumerate() {
            doc.push_str(&format!("  instance#{idx}\n"));
            push_kv(
                &mut doc,
                4,
                "object_id",
                format_object_id(instance.object_id),
            );
            push_kv(&mut doc, 4, "class_name", &instance.class_name);
            push_kv(
                &mut doc,
                4,
                "approx_shallow_bytes",
                instance.approx_retained_bytes,
            );
        }
    }

    doc.push_str("section gc_roots\n");
    for (label, count) in ordered_gc_root_counts(summary) {
        push_kv(&mut doc, 2, label, count);
    }

    doc.push_str("section thread_frames\n");
    if summary.thread_frames.is_empty() {
        push_kv(&mut doc, 2, "status", "empty");
    } else {
        for (idx, frame) in summary.thread_frames.iter().enumerate() {
            doc.push_str(&format!("  frame#{idx}\n"));
            push_kv(&mut doc, 4, "thread_serial", frame.thread_serial);
            push_kv(&mut doc, 4, "frame", format_thread_frame(frame));
        }
    }

    doc
}

fn render_overview_html(summary: &OverviewSummary) -> String {
    let top_classes = if summary.class_stats.entries.is_empty() {
        String::from("<p>No class aggregates captured.</p>")
    } else {
        let items: String = summary
            .class_stats
            .entries
            .iter()
            .map(|entry| {
                format!(
                    "<li><strong>{}</strong>: {} instances, {} approx shallow</li>",
                    escape_html(&entry.class_name),
                    entry.instance_count,
                    format_byte_count(entry.approx_shallow_bytes)
                )
            })
            .collect();
        format!("<ul>{items}</ul>")
    };

    let top_instances = if summary.top_instances.is_empty() {
        String::from("<p>No instance-level samples captured.</p>")
    } else {
        let items: String = summary
            .top_instances
            .iter()
            .map(|instance| {
                format!(
                    "<li><strong>{}</strong> {}: {}</li>",
                    escape_html(&format_object_id(instance.object_id)),
                    escape_html(&instance.class_name),
                    format_byte_count(instance.approx_retained_bytes)
                )
            })
            .collect();
        format!("<ul>{items}</ul>")
    };

    let gc_roots: String = ordered_gc_root_counts(summary)
        .into_iter()
        .map(|(label, count)| format!("<li><strong>{}</strong>: {count}</li>", escape_html(label)))
        .collect();

    let thread_frames = if summary.thread_frames.is_empty() {
        String::from("<p>No thread frames captured.</p>")
    } else {
        let items: String = summary
            .thread_frames
            .iter()
            .map(|frame| format!("<li>{}</li>", escape_html(&format_thread_frame(frame))))
            .collect();
        format!("<ul>{items}</ul>")
    };

    format!(
        r#"<section>
  <h1>{banner}</h1>
  <p><strong>Heap:</strong> {heap}</p>
  <p><strong>Bytes processed:</strong> {bytes}</p>
  <p><strong>HPROF records:</strong> {records}</p>
  <p><strong>Note:</strong> {note}</p>
  <ul>
    <li><strong>Retained sizes:</strong> not available in overview mode</li>
    <li><strong>Leak suspects:</strong> not available in overview mode</li>
    <li><strong>Dominator tree:</strong> not available in overview mode</li>
  </ul>
  <section>
    <h2>Top Classes by Approximate Shallow Bytes</h2>
    {top_classes}
  </section>
  <section>
    <h2>Largest Single Instances (approximate shallow size, not retained)</h2>
    {top_instances}
  </section>
  <section>
    <h2>GC Root Counts</h2>
    <ul>{gc_roots}</ul>
  </section>
  <section>
    <h2>Thread Frame Sample (capped)</h2>
    {thread_frames}
  </section>
  <section>
    <h2>Truncation Flags</h2>
    <ul>
      <li><strong>truncated:</strong> {truncated}</li>
      <li><strong>class table cap:</strong> {class_cap}</li>
      <li><strong>thread frame cap:</strong> {thread_cap}</li>
    </ul>
  </section>
</section>"#,
        banner = escape_html(OVERVIEW_BANNER),
        heap = escape_html(&summary.heap_path),
        bytes = format_byte_count(summary.total_bytes_processed),
        records = summary.total_record_count,
        note = escape_html(OVERVIEW_LIMITATION_NOTE),
        top_classes = top_classes,
        top_instances = top_instances,
        gc_roots = gc_roots,
        thread_frames = thread_frames,
        truncated = yes_no(summary.truncated),
        class_cap = summary.options.max_class_table_size,
        thread_cap = summary.options.max_thread_frames,
    )
}

fn format_byte_count(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0usize;

    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes} {}", UNITS[unit])
    } else {
        format!("{value:.2} {}", UNITS[unit])
    }
}

fn ordered_gc_root_counts(summary: &OverviewSummary) -> Vec<(&'static str, u64)> {
    [
        GcRootKind::JniGlobal,
        GcRootKind::JniLocal,
        GcRootKind::JavaFrame,
        GcRootKind::NativeStack,
        GcRootKind::StickyClass,
        GcRootKind::ThreadBlock,
        GcRootKind::MonitorUsed,
        GcRootKind::ThreadObject,
        GcRootKind::Unknown,
    ]
    .into_iter()
    .map(|kind| {
        (
            gc_root_kind_label(kind),
            summary.gc_root_counts.get(&kind).copied().unwrap_or(0),
        )
    })
    .collect()
}

fn gc_root_kind_label(kind: GcRootKind) -> &'static str {
    match kind {
        GcRootKind::JniGlobal => "jni_global",
        GcRootKind::JniLocal => "jni_local",
        GcRootKind::JavaFrame => "java_frame",
        GcRootKind::NativeStack => "native_stack",
        GcRootKind::StickyClass => "sticky_class",
        GcRootKind::ThreadBlock => "thread_block",
        GcRootKind::MonitorUsed => "monitor_used",
        GcRootKind::ThreadObject => "thread_object",
        GcRootKind::Unknown => "unknown",
    }
}

fn format_object_id(object_id: u64) -> String {
    format!("0x{object_id:016X}")
}

fn format_thread_frame(frame: &crate::hprof::OverviewThreadFrame) -> String {
    let source = if frame.source_file.is_empty() {
        String::from("Unknown Source")
    } else if frame.line_number > 0 {
        format!("{}:{}", frame.source_file, frame.line_number)
    } else {
        frame.source_file.clone()
    };

    format!(
        "thread#{} {}.{}({})",
        frame.thread_serial, frame.class_name, frame.method_name, source
    )
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
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

    if let Some(classloaders) = &analysis.classloader_report {
        doc.push_str("section classloaders\n");
        push_kv(&mut doc, 2, "total_loaders", classloaders.loaders.len());
        for (idx, loader) in classloaders.loaders.iter().enumerate() {
            doc.push_str(&format!("  loader#{idx}\n"));
            push_kv(&mut doc, 4, "object_id", loader.object_id);
            push_kv(&mut doc, 4, "class_name", &loader.class_name);
            push_kv(&mut doc, 4, "loaded_class_count", loader.loaded_class_count);
            push_kv(&mut doc, 4, "instance_count", loader.instance_count);
            push_kv(
                &mut doc,
                4,
                "total_shallow_bytes",
                loader.total_shallow_bytes,
            );
            if let Some(retained_bytes) = loader.retained_bytes {
                push_kv(&mut doc, 4, "retained_bytes", retained_bytes);
            }
            if let Some(parent_loader) = loader.parent_loader {
                push_kv(&mut doc, 4, "parent_loader", parent_loader);
            }
        }

        if !classloaders.potential_leaks.is_empty() {
            doc.push_str("section classloader_leaks\n");
            for (idx, leak) in classloaders.potential_leaks.iter().enumerate() {
                doc.push_str(&format!("  leak#{idx}\n"));
                push_kv(&mut doc, 4, "object_id", leak.object_id);
                push_kv(&mut doc, 4, "class_name", &leak.class_name);
                push_kv(&mut doc, 4, "retained_bytes", leak.retained_bytes);
                push_kv(&mut doc, 4, "loaded_class_count", leak.loaded_class_count);
                push_kv(&mut doc, 4, "reason", &leak.reason);
            }
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

    if let Some(classloaders) = &analysis.classloader_report {
        body.push_str("\nClassLoader Report\n------------------\n");
        for loader in &classloaders.loaders {
            body.push_str(&format!(
                "{} [{} classes, {} instances, {} shallow bytes",
                loader.class_name,
                loader.loaded_class_count,
                loader.instance_count,
                loader.total_shallow_bytes
            ));
            if let Some(retained_bytes) = loader.retained_bytes {
                body.push_str(&format!(", {retained_bytes} retained bytes"));
            }
            body.push_str("]\n");
            if let Some(parent_loader) = loader.parent_loader {
                body.push_str(&format!("  Parent loader: {parent_loader}\n"));
            }
        }

        if !classloaders.potential_leaks.is_empty() {
            body.push_str("\nPotential ClassLoader Leaks\n---------------------------\n");
            for leak in &classloaders.potential_leaks {
                body.push_str(&format!(
                    "{} [{}]: {}\n",
                    leak.class_name, leak.object_id, leak.reason
                ));
            }
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
            body.push_str(&format!("- {rec}\n"));
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

    if let Some(classloaders) = &analysis.classloader_report {
        doc.push_str("\n## ClassLoader Report\n");
        for loader in &classloaders.loaders {
            doc.push_str(&format!(
                "- `{}`: {} classes, {} instances, {} shallow bytes",
                loader.class_name,
                loader.loaded_class_count,
                loader.instance_count,
                loader.total_shallow_bytes
            ));
            if let Some(retained_bytes) = loader.retained_bytes {
                doc.push_str(&format!(", {retained_bytes} retained bytes"));
            }
            doc.push('\n');
            if let Some(parent_loader) = loader.parent_loader {
                doc.push_str(&format!("  - parent loader: `{parent_loader}`\n"));
            }
        }

        if !classloaders.potential_leaks.is_empty() {
            doc.push_str("\n### Potential ClassLoader Leaks\n");
            for leak in &classloaders.potential_leaks {
                doc.push_str(&format!(
                    "- `{}` [`{}`]: {}\n",
                    leak.class_name, leak.object_id, leak.reason
                ));
            }
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
                doc.push_str(&format!("  - {rec}\n"));
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
                format!("<ul>{items}</ul>")
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

    let classloader_block = analysis
        .classloader_report
        .as_ref()
        .map(|classloaders| {
            let loaders: String = classloaders
                .loaders
                .iter()
                .map(|loader| {
                    let retained = loader
                        .retained_bytes
                        .map(|bytes| format!(", {bytes} retained bytes"))
                        .unwrap_or_default();
                    let parent = loader
                        .parent_loader
                        .map(|id| format!(" <span class=\"parent-loader\">parent {id}</span>"))
                        .unwrap_or_default();
                    format!(
                    "<li><strong>{}</strong>: {} classes, {} instances, {} shallow bytes{}{}</li>",
                    escape_html(&loader.class_name),
                    loader.loaded_class_count,
                    loader.instance_count,
                    loader.total_shallow_bytes,
                    retained,
                    parent
                )
                })
                .collect();
            let leak_block = if classloaders.potential_leaks.is_empty() {
                String::new()
            } else {
                let leaks: String = classloaders
                    .potential_leaks
                    .iter()
                    .map(|leak| {
                        format!(
                            "<li><strong>{}</strong> [{}]: {}</li>",
                            escape_html(&leak.class_name),
                            leak.object_id,
                            escape_html(&leak.reason)
                        )
                    })
                    .collect();
                format!("<section><h3>Potential ClassLoader Leaks</h3><ul>{leaks}</ul></section>")
            };
            format!("<section><h2>ClassLoader Report</h2><ul>{loaders}</ul>{leak_block}</section>")
        })
        .unwrap_or_default();

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
            {classloader_block}
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
        classloader_block = classloader_block,
        provenance_block = provenance_block
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_classloader_response() -> AnalyzeResponse {
        use crate::analysis::{
            ClassLoaderInfo, ClassLoaderLeakCandidate, ClassLoaderReport, ProvenanceMarker,
        };
        use crate::graph::GraphMetrics;
        use crate::hprof::HeapSummary;
        use std::time::{Duration, SystemTime};

        AnalyzeResponse {
            mode: crate::analysis::AnalysisMode::Deep,
            overview: None,
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
            leaks: Vec::new(),
            recommendations: Vec::new(),
            elapsed: Duration::from_millis(42),
            graph: GraphMetrics::default(),
            ai: None,
            histogram: None,
            unreachable: None,
            thread_report: None,
            classloader_report: Some(ClassLoaderReport {
                loaders: vec![ClassLoaderInfo {
                    object_id: 5000,
                    class_name: "com.example.PluginClassLoader".into(),
                    loaded_class_count: 2,
                    instance_count: 3,
                    total_shallow_bytes: 448,
                    retained_bytes: Some(512),
                    parent_loader: Some(42),
                }],
                potential_leaks: vec![ClassLoaderLeakCandidate {
                    object_id: 7000,
                    class_name: "com.example.LeakyPluginClassLoader".into(),
                    retained_bytes: 10 * 1024 * 1024,
                    loaded_class_count: 1,
                    reason: "Retains 10.00 MB but loads only 1 classes".into(),
                }],
            }),
            collection_report: None,
            string_report: None,
            top_instances: None,
            provenance: vec![ProvenanceMarker::bare(ProvenanceKind::Partial)],
        }
    }

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
            mode: crate::analysis::AnalysisMode::Deep,
            overview: None,
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
            classloader_report: None,
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
            mode: crate::analysis::AnalysisMode::Deep,
            overview: None,
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
            classloader_report: None,
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
            mode: crate::analysis::AnalysisMode::Deep,
            overview: None,
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
            classloader_report: None,
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

    #[test]
    fn reports_render_classloader_sections() {
        let response = sample_classloader_response();

        let text = render_text(&response);
        assert!(text.contains("ClassLoader Report"));
        assert!(text.contains("com.example.PluginClassLoader"));
        assert!(text.contains("Potential ClassLoader Leaks"));

        let markdown = render_markdown(&response);
        assert!(markdown.contains("## ClassLoader Report"));
        assert!(markdown.contains("com.example.PluginClassLoader"));
        assert!(markdown.contains("### Potential ClassLoader Leaks"));

        let html = render_html(&response);
        assert!(html.contains("<h2>ClassLoader Report</h2>"));
        assert!(html.contains("com.example.PluginClassLoader"));
        assert!(html.contains("Potential ClassLoader Leaks"));

        let toon = render_toon(&response);
        assert!(toon.contains("section classloaders"));
        assert!(toon.contains("class_name=com.example.PluginClassLoader"));
        assert!(toon.contains("section classloader_leaks"));
    }
}
