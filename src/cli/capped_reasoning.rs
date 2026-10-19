//! Capped reasoning through the CLI dispatcher with an in-process gateway.
use super::test_support::*;
use mojentic::llm::gateway::StreamChunk;

#[test]
fn capped_partial_reasoning_is_not_retained_as_completed_native_history() {
    const SENTINEL: &str = "PARTIAL_REASONING_SENTINEL";
    let workspace = Workspace::new();
    std::fs::write(
        workspace.base.join("task.md"),
        "Inspect the workspace if needed, then finish with DONE.\n",
    )
    .unwrap();
    let gateway = ScriptedGateway::turns(
        Wire::Omlx,
        vec![
            vec![StreamChunk::Thinking(format!(
                "{SENTINEL} that exceeds the protective cap"
            ))],
            vec![content("DONE")],
        ],
    );
    let output = execute(
        legacy_binary(),
        &[
            "run",
            "--experiment",
            workspace.base.to_str().unwrap(),
            "--max-tokens",
            "8192",
            "--max-iterations",
            "3",
            "--reasoning-effort",
            "medium",
            "--preserve-reasoning",
            "--max-thinking-only-tokens",
            "1",
        ],
        &gateway,
        &workspace,
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let summary = summary(&output);
    assert_eq!(summary["final_summary"], "DONE");
    assert_eq!(summary["preserve_reasoning"], true);
    let requests = gateway.requests();
    assert_eq!(requests.len(), 2, "both complete gateway requests captured");
    assert!(
        !requests[1].to_string().contains(SENTINEL),
        "partial reasoning must not enter the complete second gateway request: {}",
        requests[1]
    );
    let trace = std::fs::read_to_string(summary["trace_file"].as_str().unwrap()).unwrap();
    assert!(trace.contains(SENTINEL));
    assert!(trace.contains("llm.thinking_only_stream.action_transitioned"));
}
