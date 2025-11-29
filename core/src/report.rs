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
    format!(
        "Mnemosyne Analysis\n=====================\nHeap: {}\nTotal Objects: {}\nTotal Size: {:.2} GB\nDetected Leaks: {}\n",
        analysis.summary.heap_path,
        analysis.summary.total_objects,
        analysis.summary.total_size_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
        analysis.leaks.len()
    )
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
        "- **Total Size:** {:.2} GB\n\n",
        analysis.summary.total_size_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    ));

    doc.push_str("## Detected Leaks\n");
    for leak in &analysis.leaks {
        doc.push_str(&format!(
            "- `{}` ({:?}): ~{:.2} MB across {} instances — {}\n",
            leak.class_name,
            leak.severity,
            leak.retained_size_bytes as f64 / (1024.0 * 1024.0),
            leak.instances,
            leak.description
        ));
    }

    doc
}

fn render_html(analysis: &AnalyzeResponse) -> String {
    format!(
        r#"<section>
  <h1>Mnemosyne Analysis</h1>
  <p><strong>Heap:</strong> {heap}</p>
  <p><strong>Total Objects:</strong> {objects}</p>
  <p><strong>Total Size:</strong> {size:.2} GB</p>
  <p><strong>Leak Count:</strong> {leaks}</p>
</section>"#,
        heap = analysis.summary.heap_path,
        objects = analysis.summary.total_objects,
        size = analysis.summary.total_size_bytes as f64 / (1024.0 * 1024.0 * 1024.0),
        leaks = analysis.leaks.len()
    )
}
