//! Temporary compatibility entry point for preserved scripts and fixtures.
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    adaptive_agent_harness::cli::main_entry(true).await
}
