use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use mnemosyne_core::{
    analysis::{
        analyze_heap, detect_leaks, diff_heaps, AnalyzeRequest, LeakDetectionOptions, LeakSeverity,
    },
    config::{AppConfig, OutputFormat},
    gc_path::{find_gc_path, GcPathRequest},
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
    #[arg(long, value_enum, default_value_t = SeverityArg::High)]
    min_severity: SeverityArg,
    #[arg(long)]
    package: Option<String>,
}

#[derive(Debug, Parser)]
struct AnalyzeArgs {
    heap: PathBuf,
    #[arg(long, value_enum, default_value_t = OutputFormatArg::Text)]
    format: OutputFormatArg,
    #[arg(long)]
    ai: bool,
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
    Json,
    Markdown,
    Html,
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
            OutputFormatArg::Json => OutputFormat::Json,
            OutputFormatArg::Markdown => OutputFormat::Markdown,
            OutputFormatArg::Html => OutputFormat::Html,
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    install_tracing();

    let cli = Cli::parse();
    match cli.command {
        Commands::Parse(args) => handle_parse(args).await?,
        Commands::Leaks(args) => handle_leaks(args).await?,
        Commands::Analyze(args) => handle_analyze(args).await?,
        Commands::Diff(args) => handle_diff(args).await?,
        Commands::Map(args) => handle_map(args).await?,
        Commands::GcPath(args) => handle_gc_path(args).await?,
        Commands::Serve(args) => handle_serve(args).await?,
        Commands::Config => handle_config()?,
    }

    Ok(())
}

async fn handle_parse(args: ParseArgs) -> Result<()> {
    let job = HeapParseJob {
        path: args.heap.to_string_lossy().into(),
        include_strings: false,
        max_objects: None,
    };
    let summary = parse_heap(&job)?;
    print_summary(&summary);
    Ok(())
}

async fn handle_leaks(args: LeakArgs) -> Result<()> {
    let mut options = LeakDetectionOptions::new(args.min_severity.into());
    options.package_filter = args.package;

    let leaks = detect_leaks(args.heap.to_string_lossy().as_ref(), options).await?;
    for leak in leaks {
        println!(
            "Potential leak [{}]: {} (severity: {:?}) retained ~{:.2} MB",
            leak.id,
            leak.class_name,
            leak.severity,
            leak.retained_size_bytes as f64 / (1024.0 * 1024.0)
        );
    }
    Ok(())
}

async fn handle_analyze(args: AnalyzeArgs) -> Result<()> {
    let mut config = AppConfig::default();
    config.output = args.format.into();
    config.ai.enabled = args.ai;

    let response = analyze_heap(AnalyzeRequest {
        heap_path: args.heap.to_string_lossy().into(),
        config: config.clone(),
        leak_options: LeakDetectionOptions::new(LeakSeverity::High),
        enable_ai: args.ai,
    })
    .await?;

    let report = render_report(&ReportRequest {
        analysis: response,
        format: config.output,
    })?;

    println!("{}", report.contents);
    Ok(())
}

async fn handle_diff(args: DiffArgs) -> Result<()> {
    let diff = diff_heaps(
        args.before.to_string_lossy().as_ref(),
        args.after.to_string_lossy().as_ref(),
    )
    .await?;
    println!("Heap diff result:\n{diff:#?}");
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

    Ok(())
}

async fn handle_serve(args: ServeArgs) -> Result<()> {
    warn!("Starting MCP server; press Ctrl+C to stop");
    let options = McpServerOptions {
        host: args.host,
        port: args.port,
    };

    tokio::select! {
        res = serve(options) => {
            res?;
            Ok(())
        }
        _ = signal::ctrl_c() => {
            warn!("Received interrupt signal; shutting down MCP server");
            Ok(())
        }
    }
}

fn handle_config() -> Result<()> {
    let cfg = AppConfig::default();
    println!("{}", serde_json::to_string_pretty(&cfg)?);
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
