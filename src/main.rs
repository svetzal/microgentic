use adaptive_agent_harness::cli;
use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    cli::main_entry(false).await
}
