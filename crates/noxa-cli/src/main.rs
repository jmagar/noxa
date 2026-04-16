mod app;
mod cloud;
mod config;
mod setup;
mod theme;

pub(crate) use app::{Browser, Cli, OutputFormat, PdfModeArg};

#[tokio::main]
async fn main() {
    app::run().await;
}
