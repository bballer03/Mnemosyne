use crate::{analysis::AnalyzeResponse, config::OutputFormat, errors::CoreResult};
use serde::{Deserialize, Serialize};

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
    let body = match request.format {
        OutputFormat::Text => render_text(&request.analysis),
        OutputFormat::Json => serde_json::to_string_pretty(&request.analysis)?,
        OutputFormat::Markdown => render_markdown(&request.analysis),
        OutputFormat::Html => render_html(&request.analysis),
    };

    Ok(ReportArtifact {
        mime_type: match request.format {
            OutputFormat::Text => "text/plain",
            OutputFormat::Json => "application/json",
            OutputFormat::Markdown => "text/markdown",
            OutputFormat::Html => "text/html",
        }
        .into(),
        contents: body,
    })
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

    doc
}

fn render_html(analysis: &AnalyzeResponse) -> String {
    let mut leak_list = String::new();
    if analysis.leaks.is_empty() {
        leak_list.push_str("<p>No leaks detected.</p>");
    } else {
        leak_list.push_str("<ul>");
        for leak in &analysis.leaks {
            leak_list.push_str(&format!(
                "<li><strong>{}</strong> [{}]: {:?} (~{:.2} MB, {} instances)</li>",
                leak.class_name,
                leak.id,
                leak.severity,
                leak.retained_size_bytes as f64 / (1024.0 * 1024.0),
                leak.instances
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
                    .map(|rec| format!("<li>{}</li>", rec))
                    .collect();
                format!("<ul>{}</ul>", items)
            };
            format!(
                "<section><h2>AI Insights</h2><p><strong>Model:</strong> {model} (confidence {confidence:.0}%)</p><p>{summary}</p>{recs}</section>",
                model = ai.model,
                confidence = ai.confidence * 100.0,
                summary = ai.summary,
                recs = recs,
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
      {ai_block}
</section>"#,
        heap = analysis.summary.heap_path,
        objects = analysis.summary.total_objects,
        size = analysis.summary.total_size_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
        leaks = analysis.leaks.len(),
        nodes = analysis.graph.node_count,
        leak_list = leak_list
    )
}
