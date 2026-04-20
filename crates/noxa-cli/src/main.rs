mod app;
mod cloud;
mod config;
mod setup;
mod theme;

pub(crate) use app::{Browser, Cli, OutputFormat, PdfModeArg};

#[tokio::main]
async fn main() {
    // Reset SIGPIPE to default so piping to head/less exits cleanly instead of panicking.
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
    app::run().await;
}
