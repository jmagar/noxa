/// noxa-rag-daemon — watches an output directory for ExtractionResult JSON files
/// and indexes them via TEI + Qdrant.
///
/// Usage:
///   noxa-rag-daemon [--config <PATH>] [--log-level <LEVEL>] [--version]
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use clap::Parser;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::EnvFilter;

use noxa_rag::{
    build_embed_provider, build_vector_store, load_config,
    config::{EmbedProviderConfig, SourceConfig},
    pipeline::Pipeline,
};

#[derive(Parser)]
#[command(name = "noxa-rag-daemon", about = "noxa RAG indexing daemon")]
struct Args {
    /// Config file path
    #[arg(long, default_value = "noxa-rag.toml")]
    config: PathBuf,

    /// Log level (overrides RUST_LOG)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Print version and exit
    #[arg(long)]
    version: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if args.version {
        println!("noxa-rag-daemon {}", env!("CARGO_PKG_VERSION"));
        std::process::exit(0);
    }

    // Init tracing to stderr (stdout may be piped).
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new(&args.log_level)),
        )
        .with_writer(std::io::stderr)
        .init();

    if let Err(e) = run(args).await {
        eprintln!("[noxa-rag] fatal: {e}");
        std::process::exit(1);
    }
}

async fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let config_path = &args.config;

    // Warn if config file is world-readable (may contain api_key).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(config_path) {
            let mode = meta.permissions().mode();
            if mode & 0o004 != 0 {
                eprintln!(
                    "[noxa-rag] WARNING: config file is world-readable (mode {:o}). \
                     Consider: chmod 600 {}",
                    mode,
                    config_path.display()
                );
            }
        }
    }

    // Load config — fail fast with clear error.
    let config = load_config(config_path).map_err(|e| {
        format!("failed to load config from {}: {e}", config_path.display())
    })?;

    // Ensure watch_dir exists (create if missing — convenience for first-run).
    let watch_dir = match &config.source {
        SourceConfig::FsWatcher { watch_dir, .. } => watch_dir.clone(),
    };

    if !watch_dir.exists() {
        std::fs::create_dir_all(&watch_dir).map_err(|e| {
            format!(
                "watch_dir does not exist and could not be created ({}): {e}",
                watch_dir.display()
            )
        })?;
        eprintln!("[noxa-rag] created watch_dir: {}", watch_dir.display());
    }

    // Build embed provider — startup probe (exits 1 if TEI unavailable).
    let embed = build_embed_provider(&config)
        .await
        .map_err(|e| format!("embed provider startup failed: {e}"))?;

    // Probe embed dims dynamically by calling embed with a dummy text.
    // This avoids hardcoding and reuses the already-built provider.
    let probe_vecs = embed
        .embed(&["probe".to_string()])
        .await
        .map_err(|e| format!("embed dims probe failed: {e}"))?;
    let embed_dims = probe_vecs
        .first()
        .map(|v| v.len())
        .ok_or("embed probe returned empty vector")?;

    // Build vector store — collection create/validate.
    let store = build_vector_store(&config, embed_dims)
        .await
        .map_err(|e| format!("vector store startup failed: {e}"))?;

    // Load tokenizer.
    let tokenizer_model = match &config.embed_provider {
        EmbedProviderConfig::Tei { model, local_path, .. } => {
            (model.clone(), local_path.clone())
        }
        _ => ("Qwen/Qwen3-Embedding-0.6B".to_string(), None),
    };

    // Rust tokenizers crate has no from_pretrained — local_path is required.
    // Download tokenizer.json from HF Hub before running:
    //   huggingface-cli download Qwen/Qwen3-Embedding-0.6B tokenizer.json --local-dir ./
    let tokenizer = {
        let path = tokenizer_model.1.ok_or_else(|| {
            format!(
                "embed_provider.local_path is required — the Rust tokenizers crate cannot \
                 download from HF Hub. Set local_path to the directory containing tokenizer.json.\n\
                 Download: huggingface-cli download {} tokenizer.json --local-dir <dir>",
                tokenizer_model.0
            )
        })?;
        // If given a directory, look for tokenizer.json inside it.
        let tokenizer_file = if path.is_dir() {
            path.join("tokenizer.json")
        } else {
            path.clone()
        };
        tokenizers::Tokenizer::from_file(&tokenizer_file)
            .map_err(|e| format!("failed to load tokenizer from {}: {e}", tokenizer_file.display()))?
    };

    eprintln!("[noxa-rag] tokenizer: {} — loaded", tokenizer_model.0);

    let shutdown = CancellationToken::new();
    let pipeline = Pipeline::new(
        config,
        embed,
        store,
        Arc::new(tokenizer),
        shutdown.clone(),
    );

    // Signal handling: Ctrl-C + SIGTERM -> cancel.
    let shutdown_signal = shutdown.clone();
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm = signal(SignalKind::terminate())
                .expect("failed to register SIGTERM handler");
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = sigterm.recv() => {}
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
        eprintln!("[noxa-rag] shutdown signal received");
        shutdown_signal.cancel();
    });

    eprintln!("[noxa-rag] daemon started");

    // Run pipeline with 10s force-exit timeout after cancellation.
    match tokio::time::timeout(Duration::from_secs(10), pipeline.run()).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => eprintln!("[noxa-rag] pipeline error: {e}"),
        Err(_elapsed) => {
            eprintln!("[noxa-rag] WARNING: pipeline did not shut down within 10s, forcing exit");
            std::process::exit(0);
        }
    }

    eprintln!("[noxa-rag] daemon stopped");
    Ok(())
}
