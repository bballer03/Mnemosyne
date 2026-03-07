use std::{fs, path::PathBuf};

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
mod config_loader;
use config_loader::{load_app_config, ConfigOrigin, LoadedConfig};
use mnemosyne_core::{
    analysis::{
        analyze_heap, detect_leaks, diff_heaps, AnalyzeRequest, LeakDetectionOptions, LeakKind,
        LeakSeverity,
    },
    config::{AppConfig, OutputFormat},
    fix::{propose_fix, FixRequest, FixStyle},
    focus_leaks,
    gc_path::{find_gc_path, GcPathRequest},
    generate_ai_insights,
    heap::{parse_heap, HeapParseJob, HeapSummary},
    mapper::{map_to_code, MapToCodeRequest},
    mcp::{serve, McpServerOptions},
    report::{render_report, ReportRequest},
};
use tokio::signal;
use tracing::{info, warn};
use tracing_subscriber::util::SubscriberInitExt;

#[derive(Parser, Debug)]
#[command(author, version, about = "Mnemosyne JVM memory debugging copilot")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Increase verbosity (can be passed multiple times)
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Explicit config file path
    #[arg(short = 'c', long = "config", value_name = "FILE", global = true)]
    config: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Parse a heap dump and print a summary.
    Parse(ParseArgs),
    /// Detect potential memory leaks in a heap dump.
    Leaks(LeakArgs),
    /// Run the full AI-assisted analysis pipeline.
    Analyze(AnalyzeArgs),
    /// Compare two heap dumps and highlight changes.
    Diff(DiffArgs),
    /// Map a leak candidate to likely source files.
    Map(MapArgs),
    /// Find a path from an object to its GC root.
    GcPath(GcPathArgs),
    /// Generate AI explanations for a leak candidate.
    Explain(ExplainArgs),
    /// Generate patch suggestions for a leak candidate.
    Fix(FixArgs),
    /// Start the Model Context Protocol (MCP) server.
    Serve(ServeArgs),
    /// Show the effective configuration.
    Config,
}

#[derive(Debug, Parser)]
struct ParseArgs {
    heap: PathBuf,
}

#[derive(Debug, Parser)]
struct LeakArgs {
    heap: PathBuf,
    #[arg(long, value_enum)]
    min_severity: Option<SeverityArg>,
    #[arg(long = "package", value_name = "PKG", value_delimiter = ',')]
    packages: Vec<String>,
    #[arg(long = "leak-kind", value_enum, value_delimiter = ',')]
    leak_kind: Vec<LeakKindArg>,
}

#[derive(Debug, Parser)]
struct AnalyzeArgs {
    heap: PathBuf,
    #[arg(long, value_enum, default_value_t = OutputFormatArg::Text)]
    format: OutputFormatArg,
    #[arg(short = 'o', long = "output-file", value_name = "FILE")]
    output: Option<PathBuf>,
    #[arg(long)]
    ai: bool,
    #[arg(long = "package", value_name = "PKG", value_delimiter = ',')]
    packages: Vec<String>,
    #[arg(long = "leak-kind", value_enum, value_delimiter = ',')]
    leak_kind: Vec<LeakKindArg>,
}

#[derive(Debug, Parser)]
struct DiffArgs {
    before: PathBuf,
    after: PathBuf,
}

#[derive(Debug, Parser)]
struct MapArgs {
    leak_id: String,
    #[arg(long)]
    class: Option<String>,
    #[arg(long = "project-root")]
    project_root: PathBuf,
    #[arg(long = "no-git", action = clap::ArgAction::SetTrue)]
    disable_git: bool,
}

#[derive(Debug, Parser)]
struct GcPathArgs {
    heap: PathBuf,
    #[arg(long = "object-id")]
    object_id: String,
    #[arg(long)]
    max_depth: Option<u32>,
}

#[derive(Debug, Parser)]
struct ExplainArgs {
    heap: PathBuf,
    #[arg(long = "leak-id")]
    leak_id: Option<String>,
    #[arg(long, value_enum)]
    min_severity: Option<SeverityArg>,
    #[arg(long = "package", value_name = "PKG", value_delimiter = ',')]
    packages: Vec<String>,
    #[arg(long = "leak-kind", value_enum, value_delimiter = ',')]
    leak_kind: Vec<LeakKindArg>,
}

#[derive(Debug, Parser)]
struct FixArgs {
    heap: PathBuf,
    #[arg(long = "leak-id")]
    leak_id: Option<String>,
    #[arg(long = "project-root")]
    project_root: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = FixStyleArg::Minimal)]
    style: FixStyleArg,
}

#[derive(Debug, Parser)]
struct ServeArgs {
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    #[arg(long, default_value_t = 0)]
    port: u16,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum SeverityArg {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum OutputFormatArg {
    Text,
    Toon,
    Markdown,
    Html,
    Json,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum FixStyleArg {
    Minimal,
    Defensive,
    Comprehensive,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum LeakKindArg {
    Unknown,
    Cache,
    Coroutine,
    Thread,
    HttpResponse,
    ClassLoader,
    Collection,
    Listener,
}

impl From<SeverityArg> for LeakSeverity {
    fn from(value: SeverityArg) -> Self {
        match value {
            SeverityArg::Low => LeakSeverity::Low,
            SeverityArg::Medium => LeakSeverity::Medium,
            SeverityArg::High => LeakSeverity::High,
            SeverityArg::Critical => LeakSeverity::Critical,
        }
    }
}

impl From<OutputFormatArg> for OutputFormat {
    fn from(value: OutputFormatArg) -> Self {
        match value {
            OutputFormatArg::Text => OutputFormat::Text,
            OutputFormatArg::Toon => OutputFormat::Toon,
            OutputFormatArg::Markdown => OutputFormat::Markdown,
            OutputFormatArg::Html => OutputFormat::Html,
            OutputFormatArg::Json => OutputFormat::Json,
        }
    }
}

impl From<FixStyleArg> for FixStyle {
    fn from(value: FixStyleArg) -> Self {
        match value {
            FixStyleArg::Minimal => FixStyle::Minimal,
            FixStyleArg::Defensive => FixStyle::Defensive,
            FixStyleArg::Comprehensive => FixStyle::Comprehensive,
        }
    }
}

impl From<LeakKindArg> for LeakKind {
    fn from(value: LeakKindArg) -> Self {
        match value {
            LeakKindArg::Unknown => LeakKind::Unknown,
            LeakKindArg::Cache => LeakKind::Cache,
            LeakKindArg::Coroutine => LeakKind::Coroutine,
            LeakKindArg::Thread => LeakKind::Thread,
            LeakKindArg::HttpResponse => LeakKind::HttpResponse,
            LeakKindArg::ClassLoader => LeakKind::ClassLoader,
            LeakKindArg::Collection => LeakKind::Collection,
            LeakKindArg::Listener => LeakKind::Listener,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    install_tracing();

    let cli = Cli::parse();
    let loaded_config = load_app_config(cli.config.as_deref())?;

    match cli.command {
        Commands::Parse(args) => handle_parse(args, &loaded_config.data).await?,
        Commands::Leaks(args) => handle_leaks(args, &loaded_config.data).await?,
        Commands::Analyze(args) => handle_analyze(args, &loaded_config.data).await?,
        Commands::Diff(args) => handle_diff(args).await?,
        Commands::Map(args) => handle_map(args).await?,
        Commands::GcPath(args) => handle_gc_path(args).await?,
        Commands::Explain(args) => handle_explain(args, &loaded_config.data).await?,
        Commands::Fix(args) => handle_fix(args).await?,
        Commands::Serve(args) => handle_serve(args, &loaded_config.data).await?,
        Commands::Config => handle_config(&loaded_config)?,
    }

    Ok(())
}

async fn handle_parse(args: ParseArgs, cfg: &AppConfig) -> Result<()> {
    let job = HeapParseJob {
        path: args.heap.to_string_lossy().into(),
        include_strings: false,
        max_objects: cfg.parser.max_objects,
    };
    let summary = parse_heap(&job)?;
    print_summary(&summary);
    Ok(())
}

async fn handle_leaks(args: LeakArgs, cfg: &AppConfig) -> Result<()> {
    let mut options = LeakDetectionOptions::from(&cfg.analysis);
    if let Some(sev) = args.min_severity {
        options.min_severity = sev.into();
    }
    if !args.packages.is_empty() {
        options.package_filters = args.packages.clone();
    }
    if !args.leak_kind.is_empty() {
        options.leak_types = args.leak_kind.iter().copied().map(LeakKind::from).collect();
    }

    let leaks = detect_leaks(args.heap.to_string_lossy().as_ref(), options).await?;
    for leak in leaks {
        println!(
            "Potential leak [{}]: {} (severity: {:?}) retained ~{:.2} MB",
            leak.id,
            leak.class_name,
            leak.severity,
            leak.retained_size_bytes as f64 / (1024.0 * 1024.0)
        );
        for marker in &leak.provenance {
            let detail = marker.detail.as_deref().unwrap_or("");
            let kind = format!("{:?}", marker.kind).to_uppercase();
            println!("    [{kind}] {detail}");
        }
    }
    Ok(())
}

async fn handle_analyze(args: AnalyzeArgs, base_config: &AppConfig) -> Result<()> {
    let mut config = base_config.clone();
    config.output = args.format.into();
    let use_ai = args.ai || config.ai.enabled;
    config.ai.enabled = use_ai;
    if !args.packages.is_empty() {
        config.analysis.packages = args.packages.clone();
    }
    if !args.leak_kind.is_empty() {
        config.analysis.leak_types = args.leak_kind.iter().copied().map(LeakKind::from).collect();
    }
    let leak_options = LeakDetectionOptions::from(&config.analysis);

    let response = analyze_heap(AnalyzeRequest {
        heap_path: args.heap.to_string_lossy().into(),
        config: config.clone(),
        leak_options,
        enable_ai: use_ai,
    })
    .await?;

    let report = render_report(&ReportRequest {
        analysis: response,
        format: config.output,
    })?;

    if let Some(path) = args.output {
        fs::write(&path, &report.contents)?;
        println!(
            "Report ({}) written to {}",
            report.mime_type,
            path.display()
        );
    } else {
        println!("{}", report.contents);
    }
    Ok(())
}

async fn handle_diff(args: DiffArgs) -> Result<()> {
    let diff = diff_heaps(
        args.before.to_string_lossy().as_ref(),
        args.after.to_string_lossy().as_ref(),
    )
    .await?;
    println!("Heap diff: {} -> {}", diff.before, diff.after);
    println!(
        "  Delta size: {:+.2} MB",
        diff.delta_bytes as f64 / (1024.0 * 1024.0)
    );
    println!("  Delta objects: {:+}", diff.delta_objects);

    if diff.changed_classes.is_empty() {
        println!("  No dominant class or record shifts detected.");
    } else {
        println!("  Top changes:");
        for entry in &diff.changed_classes {
            let delta = entry.after_bytes as i64 - entry.before_bytes as i64;
            let before_mb = entry.before_bytes as f64 / (1024.0 * 1024.0);
            let after_mb = entry.after_bytes as f64 / (1024.0 * 1024.0);
            println!(
                "    - {}: {:+.2} MB (before {:.2} MB -> after {:.2} MB)",
                entry.name,
                delta as f64 / (1024.0 * 1024.0),
                before_mb,
                after_mb
            );
        }
    }
    Ok(())
}

async fn handle_map(args: MapArgs) -> Result<()> {
    let response = map_to_code(&MapToCodeRequest {
        leak_id: args.leak_id,
        class_name: args.class,
        project_root: args.project_root,
        include_git_info: !args.disable_git,
    })?;

    println!("Source candidates for `{}`:", response.leak_id);
    for location in response.locations {
        println!(
            "- {}:{} ({})",
            location.file.display(),
            location.line,
            location.symbol
        );
        for line in location.code_snippet.lines() {
            println!("    {}", line.trim_end());
        }
        if let Some(git) = &location.git {
            println!(
                "    Git: {} @ {} ({}) — {}",
                git.author, git.commit, git.date, git.message
            );
        }
    }

    Ok(())
}

async fn handle_gc_path(args: GcPathArgs) -> Result<()> {
    let response = find_gc_path(&GcPathRequest {
        heap_path: args.heap.to_string_lossy().into(),
        object_id: args.object_id,
        max_depth: args.max_depth,
    })?;

    println!("GC path for {}:", response.object_id);
    for (idx, node) in response.path.iter().enumerate() {
        let marker = if node.is_root {
            "ROOT".to_string()
        } else {
            format!("#{idx}")
        };
        println!(
            "{} -> {} [{}] via {}",
            marker,
            node.class_name,
            node.object_id,
            node.field.clone().unwrap_or_else(|| "<direct>".into())
        );
    }

    if !response.provenance.is_empty() {
        println!();
        for marker in &response.provenance {
            let detail = marker.detail.as_deref().unwrap_or("");
            let kind = format!("{:?}", marker.kind).to_uppercase();
            println!("  [{}] {}", kind, detail);
        }
    }

    Ok(())
}

async fn handle_explain(args: ExplainArgs, base_config: &AppConfig) -> Result<()> {
    let mut config = base_config.clone();
    config.ai.enabled = true;
    if !args.packages.is_empty() {
        config.analysis.packages = args.packages.clone();
    }
    if !args.leak_kind.is_empty() {
        config.analysis.leak_types = args.leak_kind.iter().copied().map(LeakKind::from).collect();
    }
    let mut leak_options = LeakDetectionOptions::from(&config.analysis);
    if let Some(sev) = args.min_severity {
        leak_options.min_severity = sev.into();
    }

    let response = analyze_heap(AnalyzeRequest {
        heap_path: args.heap.to_string_lossy().into(),
        config: config.clone(),
        leak_options,
        enable_ai: true,
    })
    .await?;

    let targeted = focus_leaks(&response.leaks, args.leak_id.as_deref());
    let ai = generate_ai_insights(&response.summary, &targeted, &config.ai);

    println!(
        "Model: {} (confidence {:.0}%)",
        ai.model,
        ai.confidence * 100.0
    );
    println!("{}", ai.summary);
    if !ai.recommendations.is_empty() {
        println!("Recommendations:");
        for rec in ai.recommendations {
            println!("- {}", rec);
        }
    }

    Ok(())
}

async fn handle_fix(args: FixArgs) -> Result<()> {
    let response = propose_fix(FixRequest {
        heap_path: args.heap.to_string_lossy().into_owned(),
        leak_id: args.leak_id,
        style: args.style.into(),
        project_root: args.project_root,
    })
    .await?;

    if response.suggestions.is_empty() {
        println!("No fix suggestions available for the provided criteria.");
        return Ok(());
    }

    for suggestion in response.suggestions {
        println!(
            "Fix for {} [{}] ({:?}, confidence {:.0}%):",
            suggestion.class_name,
            suggestion.leak_id,
            suggestion.style,
            suggestion.confidence * 100.0
        );
        println!("File: {}", suggestion.target_file);
        println!("{}", suggestion.description);
        println!("Patch:\n{}", suggestion.diff);
    }

    if !response.provenance.is_empty() {
        println!();
        for marker in &response.provenance {
            let detail = marker.detail.as_deref().unwrap_or("");
            let kind = format!("{:?}", marker.kind).to_uppercase();
            println!("  [{}] {}", kind, detail);
        }
    }

    Ok(())
}

async fn handle_serve(args: ServeArgs, cfg: &AppConfig) -> Result<()> {
    warn!("Starting MCP server; press Ctrl+C to stop");
    let options = McpServerOptions {
        host: args.host,
        port: args.port,
    };

    tokio::select! {
        res = serve(options, cfg.clone()) => {
            res?;
            Ok(())
        }
        _ = signal::ctrl_c() => {
            warn!("Received interrupt signal; shutting down MCP server");
            Ok(())
        }
    }
}

fn handle_config(loaded: &LoadedConfig) -> Result<()> {
    println!("{}", serde_json::to_string_pretty(&loaded.data)?);
    match (&loaded.origin, &loaded.path) {
        (ConfigOrigin::Default, _) => println!("Using built-in defaults (no config file found)."),
        (_, Some(path)) => println!(
            "Loaded configuration from {} ({}).",
            path.display(),
            loaded.origin.label()
        ),
        _ => {}
    }
    Ok(())
}

fn install_tracing() {
    let _ = tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .finish()
        .try_init();
    info!("Tracing initialized");
}

fn print_summary(summary: &HeapSummary) {
    println!("Heap path: {}", summary.heap_path);
    println!(
        "File size: {:.2} GB",
        summary.total_size_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    );
    if let Some(header) = &summary.header {
        println!(
            "Format: {} | Identifier bytes: {} | Timestamp(ms): {}",
            header.format.trim(),
            header.identifier_size,
            header.timestamp_millis
        );
    }
    println!("Estimated objects: {}", summary.total_objects);
    println!("Total HPROF records: {}", summary.total_records);

    if !summary.classes.is_empty() {
        println!("Top classes by retained size:");
        for (idx, class) in summary.classes.iter().take(5).enumerate() {
            println!(
                "  {}. {} — {:.2} MB ({:.1}%, {} instances)",
                idx + 1,
                class.name,
                class.total_size_bytes as f64 / (1024.0 * 1024.0),
                class.percentage,
                class.instances
            );
        }
    }

    if !summary.record_stats.is_empty() {
        println!("Top record tags:");
        for stat in summary.record_stats.iter().take(5) {
            println!(
                "  - {} (tag 0x{:02X}): {} entries, {:.2} MB",
                stat.name,
                stat.tag,
                stat.count,
                stat.bytes as f64 / (1024.0 * 1024.0)
            );
        }
    }
}
