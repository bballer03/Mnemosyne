use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use mnemosyne_core::{
    analysis::{
        analyze_heap, detect_leaks, diff_heaps, AnalyzeRequest, LeakDetectionOptions, LeakSeverity,
    },
    config::{AppConfig, OutputFormat},
    heap::{parse_heap, HeapParseJob, HeapSummary},
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
            "Potential leak: {} (severity: {:?}) retained ~{:.2} MB",
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

async fn handle_serve(args: ServeArgs) -> Result<()> {
    warn!("MCP server is experimental; exiting after receiving shutdown signal");
    let options = McpServerOptions {
        host: args.host,
        port: args.port,
    };

    let _ = signal::ctrl_c().await;
    serve(
        options,
        AnalyzeRequest {
            heap_path: String::new(),
            config: AppConfig::default(),
            leak_options: LeakDetectionOptions::new(LeakSeverity::High),
            enable_ai: false,
        },
    )
    .await
    .ok();

    Ok(())
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
