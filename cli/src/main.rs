use std::{
    fs,
    path::{Path, PathBuf},
    process,
    time::Duration,
};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use comfy_table::{
    presets::ASCII_BORDERS_ONLY_CONDENSED, Attribute, Cell, CellAlignment, ContentArrangement,
    Table,
};
use console::{style, StyledObject};
mod config_loader;
use config_loader::{load_app_config, ConfigOrigin, LoadedConfig};
use indicatif::{ProgressBar, ProgressStyle};
use mnemosyne_core::{
    analysis::{
        analyze_heap, detect_leaks, diff_heaps, focus_leaks, generate_ai_chat_turn_async,
        generate_ai_insights_async, validate_leak_id, AnalyzeRequest, LeakDetectionOptions,
        LeakKind, LeakSeverity, ProvenanceKind,
    },
    config::{AnalysisProfile, AppConfig, OutputFormat},
    fix::{propose_fix, FixRequest, FixStyle},
    graph::{find_gc_path, GcPathRequest, HistogramGroupBy},
    hprof::{parse_heap, HeapParseJob, HeapSummary},
    mapper::{map_to_code, MapToCodeRequest},
    mcp::{serve, McpServerOptions},
    parse_hprof_file,
    query::{execute_query, parse_query, CellValue},
    report::{render_report, ReportRequest},
    CoreError,
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
    /// Execute an OQL-style query against the heap graph.
    Query(QueryArgs),
    /// Generate AI explanations for a leak candidate.
    Explain(ExplainArgs),
    /// Start a bounded leak-focused chat session.
    Chat(ChatArgs),
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
    #[arg(long)]
    profile: Option<ProfileArg>,
    #[arg(long = "group-by", value_enum, default_value_t = GroupByArg::Class)]
    group_by: GroupByArg,
    #[arg(short = 'o', long = "output-file", value_name = "FILE")]
    output: Option<PathBuf>,
    #[arg(long)]
    ai: bool,
    /// Enable thread inspection
    #[arg(long)]
    threads: bool,
    /// Enable string analysis (duplicate detection, waste quantification)
    #[arg(long)]
    strings: bool,
    /// Enable collection inspection (fill ratio, waste detection)
    #[arg(long)]
    collections: bool,
    /// Enable classloader analysis
    #[arg(long = "classloaders")]
    classloaders: bool,
    /// Show top-N largest instances
    #[arg(long = "top-instances")]
    top_instances: bool,
    /// Number of results for top-N queries (threads, strings, top-instances)
    #[arg(long = "top-n", default_value_t = 10)]
    top_n: usize,
    /// Minimum collection backing capacity to report
    #[arg(long = "min-capacity", default_value_t = 16)]
    min_capacity: usize,
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
struct QueryArgs {
    heap: PathBuf,
    query: String,
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
struct ChatArgs {
    heap: PathBuf,
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

#[derive(Copy, Clone, Debug, ValueEnum)]
enum GroupByArg {
    Class,
    Package,
    #[value(name = "classloader")]
    Classloader,
}

#[derive(Copy, Clone, Debug, ValueEnum)]
enum ProfileArg {
    #[value(name = "overview")]
    Overview,
    #[value(name = "incident-response")]
    IncidentResponse,
    #[value(name = "ci-regression")]
    CiRegression,
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

impl From<GroupByArg> for HistogramGroupBy {
    fn from(value: GroupByArg) -> Self {
        match value {
            GroupByArg::Class => HistogramGroupBy::Class,
            GroupByArg::Package => HistogramGroupBy::Package,
            GroupByArg::Classloader => HistogramGroupBy::ClassLoader,
        }
    }
}

impl From<ProfileArg> for AnalysisProfile {
    fn from(value: ProfileArg) -> Self {
        match value {
            ProfileArg::Overview => AnalysisProfile::Overview,
            ProfileArg::IncidentResponse => AnalysisProfile::IncidentResponse,
            ProfileArg::CiRegression => AnalysisProfile::CiRegression,
        }
    }
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("{} {err:#}", style("Error:").red().bold());

        if let Some(core_err) = err.downcast_ref::<CoreError>() {
            match core_err {
                CoreError::NotAnHprof { detail, .. } => {
                    eprintln!("  {} {detail}", style("hint:").yellow().bold());
                }
                _ => {
                    if let Some(hint) = core_err.suggestion() {
                        eprintln!("  {} {hint}", style("hint:").yellow().bold());
                    }
                }
            }
        }

        process::exit(1);
    }
}

async fn run() -> Result<()> {
    install_tracing();

    let cli = Cli::parse();
    let loaded_config = load_app_config(cli.config.as_deref()).map_err(map_config_error)?;

    match cli.command {
        Commands::Parse(args) => handle_parse(args, &loaded_config.data).await?,
        Commands::Leaks(args) => handle_leaks(args, &loaded_config.data).await?,
        Commands::Analyze(args) => handle_analyze(args, &loaded_config.data).await?,
        Commands::Diff(args) => handle_diff(args).await?,
        Commands::Map(args) => handle_map(args).await?,
        Commands::GcPath(args) => handle_gc_path(args).await?,
        Commands::Query(args) => handle_query(args).await?,
        Commands::Explain(args) => handle_explain(args, &loaded_config.data).await?,
        Commands::Chat(args) => handle_chat(args, &loaded_config.data).await?,
        Commands::Fix(args) => handle_fix(args).await?,
        Commands::Serve(args) => handle_serve(args, &loaded_config.data).await?,
        Commands::Config => handle_config(&loaded_config)?,
    }

    Ok(())
}

async fn handle_parse(args: ParseArgs, cfg: &AppConfig) -> Result<()> {
    validate_heap_file(&args.heap)?;

    let job = HeapParseJob {
        path: args.heap.to_string_lossy().into(),
        include_strings: false,
        max_objects: cfg.parser.max_objects,
    };
    let pb = start_spinner("Parsing heap dump...");
    let summary = parse_heap(&job)
        .with_context(|| format!("Failed to parse heap dump: {}", args.heap.display()))?;
    finish_spinner(&pb, "Parsed heap dump.");
    print_summary(&summary);
    Ok(())
}

async fn handle_leaks(args: LeakArgs, cfg: &AppConfig) -> Result<()> {
    validate_heap_file(&args.heap)?;

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

    let pb = start_spinner("Detecting leaks...");
    let leaks = detect_leaks(args.heap.to_string_lossy().as_ref(), options)
        .await
        .with_context(|| {
            format!(
                "Failed to detect leaks from heap dump: {}",
                args.heap.display()
            )
        })?;
    finish_spinner(&pb, "Leak detection complete.");
    if !leaks.is_empty() {
        println!("{}", bold_label("Potential leaks:"));
        let (table, truncated_leak_ids, truncated_class_names) = build_leaks_table(&leaks);
        println!("{table}");
        print_full_value_section("Full leak IDs for truncated rows:", &truncated_leak_ids);
        print_full_value_section(
            "Full class names for truncated leak rows:",
            &truncated_class_names,
        );
        for leak in &leaks {
            println!("  {} {}", bold_label("Leak:"), leak.id);
            println!(
                "    {} {}",
                bold_label("Description:"),
                leak.description.trim()
            );
            if !leak.provenance.is_empty() {
                println!("    {}", bold_label("Provenance:"));
                for marker in &leak.provenance {
                    let detail = marker.detail.as_deref().unwrap_or("");
                    println!("      [{}] {detail}", styled_provenance(marker.kind));
                }
            }
            println!();
        }
    } else {
        println!("{}", bold_label("No leak suspects detected."));
    }
    Ok(())
}

async fn handle_analyze(args: AnalyzeArgs, base_config: &AppConfig) -> Result<()> {
    validate_heap_file(&args.heap)?;

    let mut config = base_config.clone();
    config.output = args.format.into();
    let profile = args.profile.map(AnalysisProfile::from);
    let use_ai = args.ai || config.ai.enabled;
    config.ai.enabled = use_ai;
    if !args.packages.is_empty() {
        config.analysis.packages = args.packages.clone();
    }
    if !args.leak_kind.is_empty() {
        config.analysis.leak_types = args.leak_kind.iter().copied().map(LeakKind::from).collect();
    }
    let leak_options = LeakDetectionOptions::from(&config.analysis);

    let mut enable_threads = args.threads;
    let mut enable_strings = args.strings;
    let mut enable_collections = args.collections;
    let mut enable_classloaders = args.classloaders;
    let mut enable_top_instances = args.top_instances;
    let mut top_n = args.top_n;
    let mut min_capacity = args.min_capacity;

    if let Some(profile) = profile {
        match profile {
            AnalysisProfile::Overview => {
                enable_threads = false;
                enable_strings = false;
                enable_collections = false;
                enable_classloaders = false;
                enable_top_instances = false;
                top_n = 10;
                min_capacity = 16;
            }
            AnalysisProfile::IncidentResponse => {
                enable_threads = true;
                enable_strings = true;
                enable_collections = true;
                enable_classloaders = true;
                enable_top_instances = true;
                top_n = top_n.max(15);
                min_capacity = min_capacity.max(32);
            }
            AnalysisProfile::CiRegression => {
                enable_threads = false;
                enable_strings = false;
                enable_collections = false;
                enable_classloaders = false;
                enable_top_instances = true;
                top_n = 5;
                min_capacity = 64;
            }
        }
    }

    let pb = start_spinner("Analyzing heap dump...");
    if use_ai {
        pb.println("AI insights enabled...");
    }

    let response = analyze_heap(AnalyzeRequest {
        heap_path: args.heap.to_string_lossy().into(),
        config: config.clone(),
        leak_options,
        enable_ai: use_ai,
        histogram_group_by: args.group_by.into(),
        enable_classloaders,
        enable_threads,
        enable_strings,
        enable_collections,
        enable_top_instances,
        top_n,
        min_collection_capacity: min_capacity,
        min_duplicate_count: 2,
    })
    .await
    .with_context(|| format!("Failed to analyze heap dump: {}", args.heap.display()))?;
    finish_spinner(&pb, "Analysis complete.");

    let output_format = config.output.clone();
    let report = render_report(&ReportRequest {
        analysis: response.clone(),
        format: output_format.clone(),
    })?;

    if let Some(path) = args.output {
        fs::write(&path, &report.contents)?;
        println!(
            "{}",
            style(format!(
                "Report ({}) written to {}",
                report.mime_type,
                path.display()
            ))
            .green()
        );
    } else {
        println!("{}", report.contents);
        if matches!(output_format, OutputFormat::Text) {
            if let Some(histogram) = &response.histogram {
                println!();
                println!("{}", bold_label("Histogram:"));
                println!("{}", build_histogram_table(histogram));
            }

            if let Some(top_instances) = &response.top_instances {
                println!();
                println!("{}", bold_label("Top Instances by Size:"));
                println!("{}", build_top_instances_table(top_instances));
            }

            if let Some(threads) = &response.thread_report {
                println!();
                println!(
                    "{}",
                    bold_label(&format!(
                        "Thread Report ({} threads):",
                        threads.total_thread_count
                    ))
                );
                println!(
                    "  {} {}",
                    bold_label("Total retained:"),
                    format_megabytes(threads.total_thread_retained)
                );
                println!("{}", build_thread_table(threads));
                print_thread_stacks(threads);
            }

            if let Some(classloaders) = &response.classloader_report {
                println!();
                println!("{}", bold_label("ClassLoader Report:"));
                println!("{}", build_classloader_table(classloaders));
            }

            if let Some(strings) = &response.string_report {
                println!();
                println!(
                    "{}",
                    bold_label(&format!(
                        "String Analysis ({} strings, {} unique):",
                        strings.total_strings, strings.unique_strings
                    ))
                );
                println!(
                    "  {} {}",
                    bold_label("Total duplicate waste:"),
                    format_megabytes(strings.total_duplicate_waste)
                );
                println!("{}", build_string_duplicates_table(strings));
            }

            if let Some(collections) = &response.collection_report {
                println!();
                println!(
                    "{}",
                    bold_label(&format!(
                        "Collection Report ({} collections):",
                        collections.total_collections
                    ))
                );
                println!(
                    "  {} {}",
                    bold_label("Total waste:"),
                    format_megabytes(collections.total_waste_bytes)
                );
                println!(
                    "  {} {}",
                    bold_label("Empty collections:"),
                    collections.empty_collections
                );
                println!("{}", build_collection_table(collections));
            }
        }
    }
    Ok(())
}

async fn handle_diff(args: DiffArgs) -> Result<()> {
    validate_heap_file(&args.before)?;
    validate_heap_file(&args.after)?;

    let pb = start_spinner("Diffing heap dumps...");
    let diff = diff_heaps(
        args.before.to_string_lossy().as_ref(),
        args.after.to_string_lossy().as_ref(),
    )
    .await
    .with_context(|| {
        format!(
            "Failed to diff heap dumps: {} -> {}",
            args.before.display(),
            args.after.display()
        )
    })?;
    finish_spinner(&pb, "Heap diff complete.");
    println!(
        "{} {} -> {}",
        section_label("Heap diff:"),
        diff.before,
        diff.after
    );
    println!(
        "  {} {}",
        bold_label("Delta size:"),
        styled_delta_megabytes(diff.delta_bytes)
    );
    println!(
        "  {} {}",
        bold_label("Delta objects:"),
        styled_delta_count(diff.delta_objects)
    );

    if diff.changed_classes.is_empty() {
        println!("  No dominant class or record shifts detected.");
    } else {
        println!("  {}", bold_label("Top changes:"));
        for entry in &diff.changed_classes {
            let delta = entry.after_bytes as i64 - entry.before_bytes as i64;
            let before_mb = entry.before_bytes as f64 / (1024.0 * 1024.0);
            let after_mb = entry.after_bytes as f64 / (1024.0 * 1024.0);
            println!(
                "    - {}: {} (before {:.2} MB -> after {:.2} MB)",
                entry.name,
                styled_delta_megabytes(delta),
                before_mb,
                after_mb
            );
        }
    }

    if let Some(class_diff) = &diff.class_diff {
        if !class_diff.is_empty() {
            println!("  {}", bold_label("Class-level retained deltas:"));
            println!("{}", build_class_diff_table(class_diff));
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
    validate_heap_file(&args.heap)?;

    let pb = start_spinner("Tracing GC path...");
    let response = find_gc_path(&GcPathRequest {
        heap_path: args.heap.to_string_lossy().into(),
        object_id: args.object_id,
        max_depth: args.max_depth,
    })
    .with_context(|| {
        format!(
            "Failed to trace GC path from heap dump: {}",
            args.heap.display()
        )
    })?;
    finish_spinner(&pb, "GC path trace complete.");

    println!("{} {}:", section_label("GC path for"), response.object_id);
    for (idx, node) in response.path.iter().enumerate() {
        let marker = if node.is_root {
            style("ROOT").bold().to_string()
        } else {
            format!("#{idx}")
        };
        println!(
            "{} -> {} [{}] via {}",
            marker,
            style(node.class_name.as_str()).cyan(),
            node.object_id,
            node.field.clone().unwrap_or_else(|| "<direct>".into())
        );
    }

    if !response.provenance.is_empty() {
        println!();
        for marker in &response.provenance {
            let detail = marker.detail.as_deref().unwrap_or("");
            println!("  [{}] {}", styled_provenance(marker.kind), detail);
        }
    }

    Ok(())
}

async fn handle_query(args: QueryArgs) -> Result<()> {
    validate_heap_file(&args.heap)?;

    let pb = start_spinner("Executing query...");
    let graph = parse_hprof_file(args.heap.to_string_lossy().as_ref())
        .with_context(|| format!("Failed to parse heap dump: {}", args.heap.display()))?;
    let dominator = mnemosyne_core::build_dominator_tree(&graph);
    let query = parse_query(&args.query).map_err(|err| anyhow::anyhow!(err.to_string()))?;
    let result = execute_query(&query, &graph, Some(&dominator))?;
    finish_spinner(&pb, "Query complete.");

    println!("{} {}", bold_label("Columns:"), result.columns.join(", "));
    println!("{} {}", bold_label("Matched:"), result.total_matched);
    for row in &result.rows {
        println!("{}", format_query_row(row));
    }
    if result.truncated {
        println!("{} result set truncated by LIMIT", bold_label("Note:"));
    }

    Ok(())
}

async fn handle_explain(args: ExplainArgs, base_config: &AppConfig) -> Result<()> {
    validate_heap_file(&args.heap)?;

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

    let pb = start_spinner("Generating explanations...");
    let response = analyze_heap(AnalyzeRequest {
        heap_path: args.heap.to_string_lossy().into(),
        config: config.clone(),
        leak_options,
        enable_ai: true,
        histogram_group_by: HistogramGroupBy::Class,
        ..AnalyzeRequest::default()
    })
    .await
    .with_context(|| {
        format!(
            "Failed to generate explanation from heap dump: {}",
            args.heap.display()
        )
    })?;

    if let Some(ref target) = args.leak_id {
        validate_leak_id(&response.leaks, target)?;
    }

    let targeted = focus_leaks(&response.leaks, args.leak_id.as_deref());
    let ai = generate_ai_insights_async(&response.summary, &targeted, &config.ai).await?;
    finish_spinner(&pb, "Explanation complete.");

    println!(
        "{} {} (confidence {:.0}%)",
        bold_label("Model:"),
        ai.model,
        ai.confidence * 100.0
    );
    println!("{}", ai.summary);
    if !ai.recommendations.is_empty() {
        println!("{}", bold_label("Recommendations:"));
        for rec in ai.recommendations {
            println!("- {rec}");
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct ChatSession {
    summary: HeapSummary,
    leaks: Vec<mnemosyne_core::analysis::LeakInsight>,
    focus_leak_id: Option<String>,
    history: Vec<mnemosyne_core::analysis::AiChatTurn>,
}

fn print_chat_help() {
    println!("Type a question about the current leak context.");
    println!("Commands: /focus <leak-id>, /list, /help, /exit");
}

fn print_chat_shortlist(leaks: &[mnemosyne_core::analysis::LeakInsight]) {
    let shortlist: Vec<_> = leaks.iter().take(3).cloned().collect();
    println!("{}", bold_label("Top leak candidates:"));
    if shortlist.is_empty() {
        println!(
            "No leak suspects detected. Ask questions about the healthy-heap summary or type /exit."
        );
        return;
    }

    let (table, truncated_ids, truncated_classes) = build_leaks_table(&shortlist);
    println!("{table}");
    print_full_value_section("Full leak IDs for truncated rows:", &truncated_ids);
    print_full_value_section(
        "Full class names for truncated leak rows:",
        &truncated_classes,
    );
}

fn chat_shortlist(
    leaks: &[mnemosyne_core::analysis::LeakInsight],
) -> Vec<mnemosyne_core::analysis::LeakInsight> {
    leaks.iter().take(3).cloned().collect()
}

async fn handle_chat(args: ChatArgs, base_config: &AppConfig) -> Result<()> {
    use std::io::{self, Write};

    validate_heap_file(&args.heap)?;

    let mut config = base_config.clone();
    let mut startup_config = config.clone();
    startup_config.ai.enabled = false;

    let pb = start_spinner("Analyzing heap for chat...");
    let response = analyze_heap(AnalyzeRequest {
        heap_path: args.heap.to_string_lossy().into(),
        config: startup_config,
        leak_options: LeakDetectionOptions::from(&config.analysis),
        enable_ai: false,
        histogram_group_by: HistogramGroupBy::Class,
        ..AnalyzeRequest::default()
    })
    .await
    .with_context(|| {
        format!(
            "Failed to start chat from heap dump: {}",
            args.heap.display()
        )
    })?;
    finish_spinner(&pb, "Chat context ready.");

    config.ai.enabled = true;

    let mut session = ChatSession {
        summary: response.summary,
        leaks: response.leaks,
        focus_leak_id: None,
        history: Vec::new(),
    };

    println!("{} {}", bold_label("Analyzed heap:"), args.heap.display());
    print_chat_shortlist(&session.leaks);
    print_chat_help();

    let stdin = io::stdin();
    let mut stdout = io::stdout();

    loop {
        write!(stdout, "chat> ")?;
        stdout.flush()?;

        let mut line = String::new();
        if stdin.read_line(&mut line)? == 0 {
            break;
        }

        let input = line.trim();
        if input.is_empty() {
            continue;
        }
        if input == "/exit" {
            break;
        }
        if input == "/help" {
            print_chat_help();
            continue;
        }
        if input == "/list" {
            print_chat_shortlist(&session.leaks);
            continue;
        }
        if input == "/focus" {
            println!("{} /focus <leak-id>", bold_label("Usage:"));
            continue;
        }
        if let Some(target) = input.strip_prefix("/focus ") {
            let target = target.trim();
            match validate_leak_id(&session.leaks, target) {
                Ok(()) => {
                    session.focus_leak_id = Some(target.to_string());
                    println!("{} {}", bold_label("Focused leak:"), target);
                }
                Err(err) => {
                    let detail = match &err {
                        CoreError::InvalidInput(detail) => detail.clone(),
                        _ => err.to_string(),
                    };
                    println!("{} {detail}", bold_label("Focus error:"));
                }
            }
            continue;
        }

        let targeted = match session.focus_leak_id.as_deref() {
            Some(leak_id) => focus_leaks(&session.leaks, Some(leak_id)),
            None => chat_shortlist(&session.leaks),
        };
        println!("{} {}", bold_label("Question:"), input);
        let ai = generate_ai_chat_turn_async(
            &session.summary,
            &targeted,
            input,
            &session.history,
            session.focus_leak_id.as_deref(),
            &config.ai,
        )
        .await?;
        println!("{}", bold_label("Answer:"));
        println!("{}", ai.summary);
        if !ai.recommendations.is_empty() {
            println!("{}", bold_label("Recommendations:"));
            for rec in &ai.recommendations {
                println!("- {rec}");
            }
        }

        session.history.push(mnemosyne_core::analysis::AiChatTurn {
            question: input.to_string(),
            answer_summary: ai.summary.clone(),
        });
        if session.history.len() > 3 {
            let excess = session.history.len() - 3;
            session.history.drain(0..excess);
        }
    }

    Ok(())
}

async fn handle_fix(args: FixArgs) -> Result<()> {
    validate_heap_file(&args.heap)?;

    let pb = start_spinner("Generating fixes...");
    let response = propose_fix(FixRequest {
        heap_path: args.heap.to_string_lossy().into_owned(),
        leak_id: args.leak_id,
        style: args.style.into(),
        project_root: args.project_root,
    })
    .await
    .with_context(|| {
        format!(
            "Failed to generate fixes from heap dump: {}",
            args.heap.display()
        )
    })?;
    finish_spinner(&pb, "Fix generation complete.");

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
        println!("{} {}", bold_label("File:"), suggestion.target_file);
        println!("{}", suggestion.description);
        println!("{}\n{}", bold_label("Patch:"), suggestion.diff);
    }

    if !response.provenance.is_empty() {
        println!();
        for marker in &response.provenance {
            let detail = marker.detail.as_deref().unwrap_or("");
            println!("  [{}] {}", styled_provenance(marker.kind), detail);
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
        .with_writer(std::io::stderr)
        .finish()
        .try_init();
    info!("Tracing initialized");
}

fn print_summary(summary: &HeapSummary) {
    println!("{} {}", bold_label("Heap path:"), summary.heap_path);
    println!(
        "{} {:.2} GB",
        bold_label("File size:"),
        summary.total_size_bytes as f64 / (1024.0 * 1024.0 * 1024.0)
    );
    if let Some(header) = &summary.header {
        println!(
            "{} {} | Identifier bytes: {} | Timestamp(ms): {}",
            bold_label("Format:"),
            header.format.trim(),
            header.identifier_size,
            header.timestamp_millis
        );
    }
    println!(
        "{} {}",
        bold_label("Estimated objects:"),
        summary.total_objects
    );
    println!(
        "{} {}",
        bold_label("Total HPROF records:"),
        summary.total_records
    );

    if !summary.classes.is_empty() {
        println!(
            "{}",
            bold_label("Top heap record categories by aggregate bytes:")
        );
        let (table, truncated_categories) = build_parse_summary_table(summary);
        println!("{table}");
        print_full_value_section(
            "Full record category names for truncated rows:",
            &truncated_categories,
        );
    }

    if !summary.record_stats.is_empty() {
        println!("{}", bold_label("Top record tags:"));
        println!("{}", build_record_stats_table(summary));
    }
}

fn validate_heap_file(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(CoreError::FileNotFound {
            path: path.display().to_string(),
            suggestion: suggest_heap_file(path),
        }
        .into());
    }

    if path.is_dir() {
        anyhow::bail!(
            "Expected an HPROF heap dump file, but '{}' is a directory.\n  {} Specify a heap dump file path instead.",
            path.display(),
            style("hint:").yellow().bold()
        );
    }

    if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
        let ext_lower = ext.to_ascii_lowercase();
        match ext_lower.as_str() {
            "hprof" | "bin" => {}
            "jar" | "war" | "ear" | "zip" => {
                return Err(CoreError::NotAnHprof {
                    path: path.display().to_string(),
                    detail: format!(
                        "Expected an HPROF heap dump, but this appears to be a {} archive. Use `jmap` or your JVM's heap dump utility to generate an .hprof file.",
                        ext_lower.to_ascii_uppercase()
                    ),
                }
                .into());
            }
            "class" => {
                return Err(CoreError::NotAnHprof {
                    path: path.display().to_string(),
                    detail: "Expected an HPROF heap dump, but this appears to be a compiled Java class file. Use `jmap -dump:format=b,file=heap.hprof <pid>` to capture a heap dump.".into(),
                }
                .into());
            }
            "log" | "txt" | "csv" => {
                return Err(CoreError::NotAnHprof {
                    path: path.display().to_string(),
                    detail: format!(
                        "Expected an HPROF heap dump, but this file has a .{ext_lower} extension. HPROF heap dumps typically have a .hprof extension."
                    ),
                }
                .into());
            }
            _ => {}
        }
    }

    if let Err(err) = std::fs::metadata(path) {
        anyhow::bail!(
            "Cannot read heap dump '{}': {}\n  {} Check file permissions.",
            path.display(),
            err,
            style("hint:").yellow().bold()
        );
    }

    Ok(())
}

fn suggest_heap_file(path: &Path) -> Option<String> {
    if path.extension().is_none() {
        let with_hprof = path.with_extension("hprof");
        if with_hprof.exists() {
            return Some(format!("Did you mean '{}' ?", with_hprof.display()).replace("' ?", "'?"));
        }
    }

    if let Some(parent) = path.parent() {
        if parent.is_dir() {
            let hprof_files: Vec<_> = std::fs::read_dir(parent)
                .ok()?
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry
                        .path()
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .is_some_and(|ext| ext.eq_ignore_ascii_case("hprof"))
                })
                .take(3)
                .collect();

            if hprof_files.len() == 1 {
                return Some(
                    format!("Did you mean '{}' ?", hprof_files[0].path().display())
                        .replace("' ?", "'?"),
                );
            }
            if !hprof_files.is_empty() {
                let names: Vec<_> = hprof_files
                    .iter()
                    .map(|entry| entry.file_name().to_string_lossy().into_owned())
                    .collect();
                return Some(format!(
                    "Found .hprof files in the same directory: {}",
                    names.join(", ")
                ));
            }
        }
    }

    Some("Check the file path and try again.".into())
}

fn map_config_error(err: anyhow::Error) -> anyhow::Error {
    let detail = format!("{err:#}");
    let suggestion = if detail.contains("does not exist") {
        Some("Create the config file, remove the explicit config override, or point --config / MNEMOSYNE_CONFIG at an existing file.".into())
    } else if detail.contains("invalid TOML") {
        Some("Fix the TOML syntax in the config file and try again.".into())
    } else {
        Some("Review the configuration file contents and try again.".into())
    };

    CoreError::ConfigError { detail, suggestion }.into()
}

fn start_spinner(message: impl Into<String>) -> ProgressBar {
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .expect("valid spinner template"),
    );
    pb.enable_steady_tick(Duration::from_millis(80));
    pb.set_message(message.into());
    pb
}

fn finish_spinner(pb: &ProgressBar, message: &str) {
    pb.finish_with_message(format!("✓ {message}"));
}

const PARSE_SUMMARY_NAME_WIDTH: usize = 44;
const RECORD_TAG_NAME_WIDTH: usize = 34;
const LEAK_ID_WIDTH: usize = 20;
const LEAK_CLASS_NAME_WIDTH: usize = 34;
const TOP_INSTANCE_CLASS_WIDTH: usize = 40;
const THREAD_NAME_WIDTH: usize = 32;
const STRING_VALUE_WIDTH: usize = 36;
const COLLECTION_TYPE_WIDTH: usize = 38;
const CLASSLOADER_CLASS_WIDTH: usize = 36;

fn build_parse_summary_table(summary: &HeapSummary) -> (Table, Vec<(String, String)>) {
    let mut table = base_table();
    let mut truncated_categories = Vec::new();
    table.set_header(vec![
        header_cell("#", CellAlignment::Right),
        header_cell("Record Category", CellAlignment::Left),
        header_cell("Bytes", CellAlignment::Right),
        header_cell("Share", CellAlignment::Right),
        header_cell("Entries", CellAlignment::Right),
    ]);

    for (idx, class) in summary.classes.iter().take(5).enumerate() {
        let class_cell = truncate_for_table(&class.name, PARSE_SUMMARY_NAME_WIDTH);
        if let Some(full_name) = &class_cell.full_value {
            truncated_categories.push((format!("#{}", idx + 1), full_name.clone()));
        }
        table.add_row(vec![
            right_cell(idx + 1),
            Cell::new(class_cell.display).set_alignment(CellAlignment::Left),
            right_cell(format_megabytes(class.total_size_bytes)),
            right_cell(format!("{:.1}%", class.percentage)),
            right_cell(class.instances),
        ]);
    }

    (table, truncated_categories)
}

fn build_record_stats_table(summary: &HeapSummary) -> Table {
    let mut table = base_table();
    table.set_header(vec![
        header_cell("Record Tag", CellAlignment::Left),
        header_cell("Hex", CellAlignment::Right),
        header_cell("Entries", CellAlignment::Right),
        header_cell("Size", CellAlignment::Right),
    ]);

    for stat in summary.record_stats.iter().take(5) {
        table.add_row(vec![
            Cell::new(truncate_for_table(&stat.name, RECORD_TAG_NAME_WIDTH).display)
                .set_alignment(CellAlignment::Left),
            right_cell(format!("0x{:02X}", stat.tag)),
            right_cell(stat.count),
            right_cell(format_megabytes(stat.bytes)),
        ]);
    }

    table
}

type TruncatedTableValues = Vec<(String, String)>;
type LeaksTableBuild = (Table, TruncatedTableValues, TruncatedTableValues);

fn build_leaks_table(leaks: &[mnemosyne_core::analysis::LeakInsight]) -> LeaksTableBuild {
    let mut table = base_table();
    let mut truncated_leak_ids = Vec::new();
    let mut truncated_classes = Vec::new();
    table.set_header(vec![
        header_cell("Leak ID", CellAlignment::Left),
        header_cell("Class", CellAlignment::Left),
        header_cell("Kind", CellAlignment::Left),
        header_cell("Severity", CellAlignment::Left),
        header_cell("Retained", CellAlignment::Right),
        header_cell("Instances", CellAlignment::Right),
    ]);

    for (idx, leak) in leaks.iter().enumerate() {
        let row_label = format!("row {}", idx + 1);
        let leak_id_cell = truncate_for_table(&leak.id, LEAK_ID_WIDTH);
        if let Some(full_id) = &leak_id_cell.full_value {
            truncated_leak_ids.push((
                format!("{row_label} | {}", leak_id_cell.display),
                full_id.clone(),
            ));
        }
        let class_cell = truncate_for_table(&leak.class_name, LEAK_CLASS_NAME_WIDTH);
        if let Some(full_name) = &class_cell.full_value {
            truncated_classes.push((
                format!("{row_label} | {}", class_cell.display),
                full_name.clone(),
            ));
        }
        table.add_row(vec![
            Cell::new(leak_id_cell.display).set_alignment(CellAlignment::Left),
            Cell::new(class_cell.display).set_alignment(CellAlignment::Left),
            Cell::new(format!("{:?}", leak.leak_kind)).set_alignment(CellAlignment::Left),
            severity_cell(leak.severity),
            right_cell(format_megabytes(leak.retained_size_bytes)),
            right_cell(leak.instances),
        ]);
    }

    (table, truncated_leak_ids, truncated_classes)
}

fn build_histogram_table(histogram: &mnemosyne_core::HistogramResult) -> Table {
    let mut table = base_table();
    table.set_header(vec![
        header_cell("Group", CellAlignment::Left),
        header_cell("Instances", CellAlignment::Right),
        header_cell("Shallow", CellAlignment::Right),
        header_cell("Retained", CellAlignment::Right),
    ]);

    for entry in histogram.entries.iter().take(10) {
        table.add_row(vec![
            Cell::new(entry.key.as_str()).set_alignment(CellAlignment::Left),
            right_cell(entry.instance_count),
            right_cell(format_megabytes(entry.shallow_size)),
            right_cell(format_megabytes(entry.retained_size)),
        ]);
    }

    table
}

fn build_top_instances_table(report: &mnemosyne_core::analysis::TopInstancesReport) -> Table {
    let mut table = base_table();
    table.set_header(vec![
        header_cell("Rank", CellAlignment::Right),
        header_cell("Class", CellAlignment::Left),
        header_cell("Shallow", CellAlignment::Right),
        header_cell("Retained", CellAlignment::Right),
    ]);

    for (idx, instance) in report.instances.iter().enumerate() {
        let class_cell = truncate_for_table(&instance.class_name, TOP_INSTANCE_CLASS_WIDTH);
        table.add_row(vec![
            right_cell(idx + 1),
            Cell::new(class_cell.display).set_alignment(CellAlignment::Left),
            right_cell(format_megabytes(instance.shallow_size)),
            right_cell(format_megabytes(
                instance.retained_size.unwrap_or(instance.shallow_size),
            )),
        ]);
    }

    table
}

fn build_thread_table(report: &mnemosyne_core::analysis::ThreadReport) -> Table {
    let mut table = base_table();
    table.set_header(vec![
        header_cell("Name", CellAlignment::Left),
        header_cell("Retained", CellAlignment::Right),
        header_cell("Thread Locals", CellAlignment::Right),
    ]);

    for thread in &report.top_retainers {
        let name_cell = truncate_for_table(&thread.name, THREAD_NAME_WIDTH);
        table.add_row(vec![
            Cell::new(name_cell.display).set_alignment(CellAlignment::Left),
            right_cell(format_megabytes(thread.retained_bytes)),
            right_cell(thread.thread_local_count),
        ]);
    }

    table
}

fn print_thread_stacks(report: &mnemosyne_core::analysis::ThreadReport) {
    for thread in &report.top_retainers {
        let Some(stack_trace) = &thread.stack_trace else {
            continue;
        };

        println!();
        println!("{} {}", bold_label("Stack:"), thread.name);
        for frame in stack_trace {
            match (&frame.source_file, frame.line_number) {
                (Some(source_file), line) if line > 0 => {
                    println!(
                        "  at {}.{}({}:{})",
                        frame.class_name, frame.method_name, source_file, line
                    );
                }
                (Some(source_file), _) => {
                    println!(
                        "  at {}.{}({})",
                        frame.class_name, frame.method_name, source_file
                    );
                }
                (None, _) => {
                    println!("  at {}.{}", frame.class_name, frame.method_name);
                }
            }
        }
    }
}

fn build_string_duplicates_table(report: &mnemosyne_core::analysis::StringReport) -> Table {
    let mut table = base_table();
    table.set_header(vec![
        header_cell("Value", CellAlignment::Left),
        header_cell("Count", CellAlignment::Right),
        header_cell("Waste", CellAlignment::Right),
    ]);

    for group in report.duplicate_groups.iter().take(10) {
        let value_cell = truncate_for_table(&group.value, STRING_VALUE_WIDTH);
        table.add_row(vec![
            Cell::new(value_cell.display).set_alignment(CellAlignment::Left),
            right_cell(group.count),
            right_cell(format_megabytes(group.total_wasted_bytes)),
        ]);
    }

    table
}

fn build_collection_table(report: &mnemosyne_core::analysis::CollectionReport) -> Table {
    let mut table = base_table();
    table.set_header(vec![
        header_cell("Type", CellAlignment::Left),
        header_cell("Size", CellAlignment::Right),
        header_cell("Capacity", CellAlignment::Right),
        header_cell("Fill", CellAlignment::Right),
        header_cell("Waste", CellAlignment::Right),
    ]);

    for collection in report.oversized_collections.iter().take(10) {
        let type_cell = truncate_for_table(&collection.collection_type, COLLECTION_TYPE_WIDTH);
        table.add_row(vec![
            Cell::new(type_cell.display).set_alignment(CellAlignment::Left),
            right_cell(collection.size),
            right_cell(collection.capacity.unwrap_or(0)),
            right_cell(format!(
                "{:.0}%",
                collection.fill_ratio.unwrap_or(0.0) * 100.0
            )),
            right_cell(format_megabytes(collection.waste_bytes)),
        ]);
    }

    table
}

fn build_classloader_table(report: &mnemosyne_core::analysis::ClassLoaderReport) -> Table {
    let mut table = base_table();
    table.set_header(vec![
        header_cell("Loader", CellAlignment::Left),
        header_cell("Classes", CellAlignment::Right),
        header_cell("Instances", CellAlignment::Right),
        header_cell("Shallow", CellAlignment::Right),
        header_cell("Retained", CellAlignment::Right),
    ]);

    for loader in report.loaders.iter().take(10) {
        let class_cell = truncate_for_table(&loader.class_name, CLASSLOADER_CLASS_WIDTH);
        table.add_row(vec![
            Cell::new(class_cell.display).set_alignment(CellAlignment::Left),
            right_cell(loader.loaded_class_count),
            right_cell(loader.instance_count),
            right_cell(format_megabytes(loader.total_shallow_bytes)),
            right_cell(format_megabytes(loader.retained_bytes.unwrap_or(0))),
        ]);
    }

    table
}

fn build_class_diff_table(class_diff: &[mnemosyne_core::ClassLevelDelta]) -> Table {
    let mut table = base_table();
    table.set_header(vec![
        header_cell("Class", CellAlignment::Left),
        header_cell("Instances", CellAlignment::Right),
        header_cell("Shallow", CellAlignment::Right),
        header_cell("Retained Delta", CellAlignment::Right),
    ]);

    for entry in class_diff.iter().take(10) {
        let instance_delta = entry.after_instances as i64 - entry.before_instances as i64;
        let retained_delta = entry.after_retained_bytes as i64 - entry.before_retained_bytes as i64;
        table.add_row(vec![
            Cell::new(entry.class_name.as_str()).set_alignment(CellAlignment::Left),
            right_cell(format_signed_count(instance_delta)),
            right_cell(format!(
                "{:.2} -> {:.2} MB",
                entry.before_shallow_bytes as f64 / (1024.0 * 1024.0),
                entry.after_shallow_bytes as f64 / (1024.0 * 1024.0)
            )),
            right_cell(format_signed_megabytes(retained_delta)),
        ]);
    }

    table
}

fn base_table() -> Table {
    let mut table = Table::new();
    table.load_preset(ASCII_BORDERS_ONLY_CONDENSED);
    table.set_content_arrangement(ContentArrangement::Dynamic);
    table
}

fn header_cell(label: &str, alignment: CellAlignment) -> Cell {
    Cell::new(label)
        .add_attribute(Attribute::Bold)
        .set_alignment(alignment)
}

fn right_cell(value: impl ToString) -> Cell {
    Cell::new(value.to_string()).set_alignment(CellAlignment::Right)
}

fn severity_cell(severity: LeakSeverity) -> Cell {
    let label = format!("{severity:?}");
    Cell::new(label).set_alignment(CellAlignment::Left)
}

fn format_megabytes(bytes: u64) -> String {
    format!("{:.2} MB", bytes as f64 / (1024.0 * 1024.0))
}

fn format_query_row(row: &[CellValue]) -> String {
    row.iter()
        .map(|cell| match cell {
            CellValue::Id(id) => format!("0x{id:08X}"),
            CellValue::Str(value) => value.clone(),
            CellValue::Int(value) => value.to_string(),
            CellValue::Null => "null".into(),
        })
        .collect::<Vec<_>>()
        .join(" | ")
}

fn print_full_value_section(title: &str, values: &[(String, String)]) {
    if values.is_empty() {
        return;
    }

    println!("{}", bold_label(title));
    for (label, full_value) in values {
        println!("  {label} -> {full_value}");
    }
}

struct TruncatedTableValue {
    display: String,
    full_value: Option<String>,
}

fn truncate_for_table(value: &str, max_width: usize) -> TruncatedTableValue {
    if value.chars().count() <= max_width {
        return TruncatedTableValue {
            display: value.to_string(),
            full_value: None,
        };
    }

    if max_width <= 3 {
        return TruncatedTableValue {
            display: ".".repeat(max_width),
            full_value: Some(value.to_string()),
        };
    }

    let truncated: String = value.chars().take(max_width - 3).collect();
    TruncatedTableValue {
        display: format!("{truncated}..."),
        full_value: Some(value.to_string()),
    }
}

fn section_label(label: &str) -> StyledObject<&str> {
    style(label).bold().cyan()
}

fn bold_label(label: &str) -> StyledObject<&str> {
    style(label).bold()
}

fn styled_provenance(kind: ProvenanceKind) -> StyledObject<String> {
    let text = format!("{kind:?}").to_uppercase();
    match kind {
        ProvenanceKind::Synthetic => style(text).red(),
        ProvenanceKind::Partial => style(text).yellow(),
        ProvenanceKind::Fallback => style(text).yellow(),
        ProvenanceKind::Placeholder => style(text).dim(),
    }
}

fn styled_delta_megabytes(delta_bytes: i64) -> StyledObject<String> {
    let text = format!("{:+.2} MB", delta_bytes as f64 / (1024.0 * 1024.0));
    match delta_bytes.cmp(&0) {
        std::cmp::Ordering::Greater => style(text).red(),
        std::cmp::Ordering::Less => style(text).green(),
        std::cmp::Ordering::Equal => style(text),
    }
}

fn styled_delta_count(delta: i64) -> StyledObject<String> {
    let text = format!("{delta:+}");
    match delta.cmp(&0) {
        std::cmp::Ordering::Greater => style(text).red(),
        std::cmp::Ordering::Less => style(text).green(),
        std::cmp::Ordering::Equal => style(text),
    }
}

fn format_signed_megabytes(delta_bytes: i64) -> String {
    format!("{:+.2} MB", delta_bytes as f64 / (1024.0 * 1024.0))
}

fn format_signed_count(delta: i64) -> String {
    format!("{delta:+}")
}
