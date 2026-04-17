use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use tracing_subscriber::EnvFilter;

use noxa_rag::mcp_bridge::{
    BridgeConfig, McpBridge, McpSource, ProcessMcporterExecutor, SyncReport,
};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SourceArg {
    Linkding,
    Memos,
    Bytestash,
    Paperless,
}

impl From<SourceArg> for McpSource {
    fn from(value: SourceArg) -> Self {
        match value {
            SourceArg::Linkding => McpSource::Linkding,
            SourceArg::Memos => McpSource::Memos,
            SourceArg::Bytestash => McpSource::Bytestash,
            SourceArg::Paperless => McpSource::Paperless,
        }
    }
}

#[derive(Parser)]
#[command(
    name = "noxa-rag-mcp-sync",
    about = "Fetch MCP source content through mcporter and write normalized ExtractionResult JSON into a noxa-rag watch_dir"
)]
struct Args {
    /// Source to sync. Run the command once per source so platform URLs stay source-specific.
    #[arg(value_enum)]
    source: SourceArg,

    /// Directory watched by noxa-rag-daemon for ExtractionResult JSON files.
    #[arg(long)]
    watch_dir: PathBuf,

    /// MCP server prefix passed to mcporter.
    #[arg(long, default_value = "lab")]
    server: String,

    /// mcporter executable path.
    #[arg(long, default_value = "mcporter")]
    mcporter: String,

    /// Batch size for paginated list actions.
    #[arg(long, default_value_t = 100)]
    page_size: u32,

    /// Platform base URL used to derive stable MCP-native document URLs.
    /// Required for memos, bytestash, and paperless. Optional for linkding.
    #[arg(long)]
    platform_base_url: Option<String>,

    /// Log level (overrides RUST_LOG when unset).
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&args.log_level)),
        )
        .with_writer(std::io::stderr)
        .init();

    if let Err(error) = run(args).await {
        eprintln!("[noxa-rag-mcp-sync] fatal: {error}");
        std::process::exit(1);
    }
}

async fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    if !args.watch_dir.exists() {
        std::fs::create_dir_all(&args.watch_dir)?;
    }

    let source: McpSource = args.source.into();
    if source != McpSource::Linkding && args.platform_base_url.is_none() {
        return Err(format!(
            "{} requires --platform-base-url so metadata.url stays valid for noxa-rag ingest",
            source.as_str()
        )
        .into());
    }

    let executor = ProcessMcporterExecutor::new(args.mcporter);
    let bridge = McpBridge::new(
        executor,
        BridgeConfig {
            server: args.server,
            watch_dir: args.watch_dir,
            page_size: args.page_size.max(1),
            platform_base_url: args.platform_base_url,
        },
    );

    let report = bridge.sync(source).await?;
    print_report(source, report);
    Ok(())
}

fn print_report(source: McpSource, report: SyncReport) {
    println!(
        "{}: fetched={}, written={}, skipped={}",
        source.as_str(),
        report.fetched,
        report.written,
        report.skipped
    );
}
