//! Executable journeys with a gateway double and an event-driven PTY.
#[path = "../support/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;
mod gateway;
mod pty;
pub use fixtures::*;
pub use gateway::*;
pub use pty::*;
pub fn microgentic() -> &'static str {
    static BINARY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    BINARY.get_or_init(|| {
        std::env::var("MICROGENTIC_TEST_BINARY")
            .unwrap_or_else(|_| env!("CARGO_BIN_EXE_microgentic").into())
    })
}
pub fn legacy_binary() -> &'static str {
    env!("CARGO_BIN_EXE_adaptive-agent-harness")
}
