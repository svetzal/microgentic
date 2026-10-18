//! CLI outcome coverage migrated from tests/run_outcomes.rs. All requests terminate
//! at a task-scoped LlmGateway double; no sockets, servers, or Python harnesses.
use super::test_support::*;
use serde_json::{Value, json};
fn contract_case(name: &str) -> Value {
    let contract: Value = serde_json::from_str(include_str!(
        "../../benchmarks/fixtures/terminal_outcome_contract.json"
    ))
    .expect("terminal outcome contract JSON");
    assert_eq!(
        contract["schema_version"],
        "microgentic_terminal_outcome_contract.v1"
    );
    contract["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == name)
        .unwrap_or_else(|| panic!("no contract case {name}"))
        .clone()
}

/// The run's terminal trace event carries the same outcome as the summary,
/// and its identifying fields match the named case of the contract that the
/// benchmark classifier reads (benchmarks/fixtures/terminal_outcome_contract.json).
fn assert_terminal_contract(case: &str, summary: &Value) {
    let expected = contract_case(case);
    let trace = std::path::PathBuf::from(summary["trace_file"].as_str().expect("trace path"));
    let terminal = trace_records(&trace)
        .into_iter()
        .rfind(|record| record["kind"] == "run.finished" || record["kind"] == "run.failed")
        .expect("a terminal trace event");
    assert_eq!(terminal["kind"], expected["terminal_kind"], "{case}");
    let outcome = &terminal["payload"]["outcome"];
    assert_eq!(
        outcome, &summary["outcome"],
        "{case}: trace and summary differ"
    );
    for (field, value) in expected["outcome"].as_object().expect("contract outcome") {
        assert_eq!(&outcome[field], value, "{case}: outcome {field}");
    }
}

/// The `microgentic: …` line the public executable writes when a run did not
/// finish its work.
fn diagnostic(output: &Output) -> Option<String> {
    stderr(output)
        .lines()
        .find(|line| line.starts_with("microgentic: "))
        .map(str::to_owned)
}

/// An everyday `microgentic run --prompt`.
fn everyday(
    provider: &ScriptedGateway,
    workspace: &Workspace,
    prompt: &str,
    extra: &[&str],
) -> Output {
    let mut args = vec![
        "run",
        "--provider",
        provider.wire().name(),
        "--model",
        "test-model",
        "--prompt",
        prompt,
    ];
    args.extend_from_slice(extra);
    execute(microgentic(), &args, provider, workspace)
}

/// A task run through `binary`: `verb` is `run` or `benchmark run`, the
/// experiment root is the workspace base (holding `task.md` or a contract),
/// and the tool root is the project.
fn task(
    binary: &str,
    verb: &[&str],
    provider: &ScriptedGateway,
    workspace: &Workspace,
    extra: &[&str],
) -> Output {
    // Every run in a test shares one trace directory, as scripts do.
    let experiment = workspace.base.display().to_string();
    let traces = workspace.base.join("traces").display().to_string();
    let mut args: Vec<&str> = verb.to_vec();
    args.extend([
        "--experiment",
        &experiment,
        "--trace-dir",
        &traces,
        "--provider",
        provider.wire().name(),
        "--model",
        "test-model",
    ]);
    args.extend_from_slice(extra);
    execute(binary, &args, provider, workspace)
}

fn write_task(workspace: &Workspace, text: &str) {
    std::fs::write(workspace.base.join("task.md"), text).expect("writing task.md");
}

fn replied(wire: Wire) {
    let workspace = Workspace::new();
    let provider = ScriptedGateway::replying(wire, vec![content("Hello there.")]);
    let output = everyday(&provider, &workspace, "Hello!", &["--json"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let summary = summary(&output);
    assert_eq!(summary["outcome"], json!({"kind": "replied"}));
    let trace = trace_records(std::path::Path::new(
        summary["trace_file"].as_str().unwrap(),
    ));
    let terminal = trace
        .iter()
        .rfind(|event| event["kind"] == "run.finished")
        .unwrap();
    assert_eq!(terminal["payload"]["outcome"], summary["outcome"]);
    assert_eq!(diagnostic(&output), None, "{}", stderr(&output));

    let output = everyday(&provider, &workspace, "Hello!", &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stdout(&output), "Hello there.\n");
    assert_eq!(diagnostic(&output), None, "{}", stderr(&output));
}

#[test]
fn a_completed_reply_exits_zero_with_a_replied_outcome_omlx() {
    replied(Wire::Omlx);
}

#[test]
fn a_completed_reply_exits_zero_with_a_replied_outcome_ollama() {
    replied(Wire::Ollama);
}

// ---------------------------------------------------------------- done

fn done(wire: Wire) {
    let workspace = Workspace::new();
    write_task(&workspace, "Finish with DONE.\n");
    let provider = ScriptedGateway::replying(wire, vec![content("DONE")]);
    let output = task(microgentic(), &["run"], &provider, &workspace, &["--json"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        summary(&output)["outcome"],
        contract_case("done")["outcome"]
    );
    assert_terminal_contract("done", &summary(&output));
    assert_eq!(diagnostic(&output), None, "{}", stderr(&output));

    let output = task(legacy_binary(), &["run"], &provider, &workspace, &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        summary(&output)["outcome"],
        contract_case("done")["outcome"]
    );
}

#[test]
fn an_accepted_done_exits_zero_with_a_done_outcome_omlx() {
    done(Wire::Omlx);
}

#[test]
fn an_accepted_done_exits_zero_with_a_done_outcome_ollama() {
    done(Wire::Ollama);
}

// ---------------------------------------------------------------- failed

const FAIL_TEXT: &str = "FAIL the fixture file is missing";

fn failed(wire: Wire) {
    let workspace = Workspace::new();
    write_task(&workspace, "Read the fixture and finish.\n");
    let provider = ScriptedGateway::replying(wire, vec![content(FAIL_TEXT)]);

    // Plain output: stdout carries the FAIL text, stderr says how it ended.
    let output = task(microgentic(), &["run"], &provider, &workspace, &[]);
    assert_eq!(output.status.code(), Some(3), "{}", stderr(&output));
    assert_eq!(stdout(&output), format!("{FAIL_TEXT}\n"));
    let line = diagnostic(&output).expect("a diagnostic line");
    let evidence = line
        .strip_prefix(&format!(
            "microgentic: FAIL returned: {FAIL_TEXT}; evidence: "
        ))
        .unwrap_or_else(|| panic!("unexpected diagnostic: {line}"));
    assert!(
        std::path::Path::new(evidence).is_file(),
        "the evidence path is the trace: {evidence}"
    );

    // JSON output: the outcome is in the summary and the evidence is its trace.
    let output = task(microgentic(), &["run"], &provider, &workspace, &["--json"]);
    assert_eq!(output.status.code(), Some(3), "{}", stderr(&output));
    let json = summary(&output);
    assert_eq!(json["outcome"], {
        let mut outcome = contract_case("failed")["outcome"].clone();
        outcome["reason"] = json!(FAIL_TEXT);
        outcome
    });
    assert_terminal_contract("failed", &json);
    assert_eq!(
        diagnostic(&output).as_deref(),
        Some(
            format!(
                "microgentic: FAIL returned: {FAIL_TEXT}; evidence: {}",
                json["trace_file"].as_str().expect("trace path")
            )
            .as_str()
        )
    );

    // Benchmark mode and the compatibility executable keep exit 0.
    let output = task(
        microgentic(),
        &["benchmark", "run"],
        &provider,
        &workspace,
        &[],
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(summary(&output)["outcome"], {
        let mut outcome = contract_case("failed")["outcome"].clone();
        outcome["reason"] = json!(FAIL_TEXT);
        outcome
    });
    // Benchmark adapters read the same contract from the benchmark trace.
    assert_terminal_contract("failed", &summary(&output));
    assert_eq!(diagnostic(&output), None, "{}", stderr(&output));

    let output = task(legacy_binary(), &["run"], &provider, &workspace, &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(summary(&output)["outcome"], {
        let mut outcome = contract_case("failed")["outcome"].clone();
        outcome["reason"] = json!(FAIL_TEXT);
        outcome
    });
    assert_eq!(diagnostic(&output), None, "{}", stderr(&output));
}

#[test]
fn a_fail_reply_exits_three_except_in_benchmark_and_compatibility_modes_omlx() {
    failed(Wire::Omlx);
}

#[test]
fn a_fail_reply_exits_three_except_in_benchmark_and_compatibility_modes_ollama() {
    failed(Wire::Ollama);
}

// ---------------------------------------------------------------- withheld

/// A source edit followed by DONE in the same turn, under a task that
/// declares a validation command: the validation gate withholds the
/// completion, and the one-turn budget runs out.
fn withheld_by_validation(wire: Wire) {
    let workspace = Workspace::new();
    workspace.write("greeting.py", "print('Hi there')\n");
    write_task(
        &workspace,
        "Change the greeting in greeting.py to Hello there, then validate:\n\n```sh\npytest\n```\n",
    );
    let provider = ScriptedGateway::turns(
        wire,
        vec![
            vec![tool(
                "edit_file",
                json!({"path": "greeting.py", "edits": [{"kind": "replace_exact", "old": "Hi there", "new": "Hello there"}]}),
            )],
            vec![content("DONE")],
        ],
    );
    let output = task(
        microgentic(),
        &["run"],
        &provider,
        &workspace,
        &["--json", "--max-iterations", "1"],
    );
    assert_eq!(output.status.code(), Some(4), "{}", stderr(&output));
    let outcome = &summary(&output)["outcome"];
    assert_terminal_contract("withheld_validation", &summary(&output));
    assert_eq!(workspace.read("greeting.py"), "print('Hello there')\n");
    assert_eq!(
        outcome["detail"],
        "edits were not validated before the reply"
    );
    assert!(
        diagnostic(&output).is_some_and(|line| line.starts_with(
            "microgentic: completion withheld by the validation gate: edits were not validated before the reply; evidence: "
        )),
        "{}",
        stderr(&output)
    );
}

#[test]
fn an_unvalidated_edit_before_done_exits_four_as_withheld_by_validation_omlx() {
    withheld_by_validation(Wire::Omlx);
}

#[test]
fn an_unvalidated_edit_before_done_exits_four_as_withheld_by_validation_ollama() {
    withheld_by_validation(Wire::Ollama);
}

/// A stash created during the run blocks DONE on every remaining turn.
#[test]
fn a_new_stash_before_done_exits_four_as_withheld_by_the_stash_gate() {
    let workspace = Workspace::new();
    workspace.write("tracked.txt", "seed\n");
    workspace.git_init();
    workspace.write("tracked.txt", "changed\n");
    write_task(&workspace, "Tidy the working tree and finish.\n");
    let provider = ScriptedGateway::turns(
        Wire::Omlx,
        vec![
            vec![tool(
                "shell_command",
                json!({"command": "git stash push -q", "timeout_secs": 30}),
            )],
            vec![content("DONE")],
        ],
    );
    let output = task(
        microgentic(),
        &["run"],
        &provider,
        &workspace,
        &["--json", "--max-iterations", "2"],
    );
    assert_eq!(output.status.code(), Some(4), "{}", stderr(&output));
    let outcome = &summary(&output)["outcome"];
    assert_terminal_contract("withheld_stash", &summary(&output));
    let json = summary(&output);
    assert_eq!(
        diagnostic(&output),
        Some(format!(
            "microgentic: completion withheld by the Git stash gate: {}; evidence: {}",
            json["outcome"]["detail"].as_str().unwrap(),
            json["trace_file"].as_str().unwrap(),
        ))
    );
    let detail = outcome["detail"].as_str().expect("detail");
    assert!(
        detail.starts_with("new stash object IDs: [\"")
            && detail.contains("repository changed: false"),
        "{detail}"
    );
    assert_eq!(
        provider.request_count(),
        3,
        "the stash, DONE, then DONE again"
    );
    // The stash the model made is left for the user, never dropped.
    let stashes = std::process::Command::new("git")
        .args(["stash", "list"])
        .current_dir(&workspace.project)
        .output()
        .expect("git stash list");
    assert_eq!(String::from_utf8_lossy(&stashes.stdout).lines().count(), 1);
}

/// A declared probe the model never runs withholds DONE.
#[test]
fn an_unrun_probe_before_done_exits_four_as_withheld_by_the_probe_gate() {
    let workspace = Workspace::new();
    let contract = workspace.base.join("contract.json");
    std::fs::write(
        &contract,
        json!({
            "profile": {"id": "terminal_work", "version": "terminal_work_profile.v1"},
            "guidance": "Finish.",
            "read_scope": ["./**"],
            "write_scope": ["./**"],
            "probes": [{"id": "check", "command": "true"}],
        })
        .to_string(),
    )
    .expect("writing the contract");
    let provider = ScriptedGateway::replying(Wire::Omlx, vec![content("DONE")]);
    let output = task(
        microgentic(),
        &["run"],
        &provider,
        &workspace,
        &[
            "--json",
            "--contract",
            &contract.display().to_string(),
            "--max-iterations",
            "1",
        ],
    );
    assert_eq!(output.status.code(), Some(4), "{}", stderr(&output));
    let outcome = &summary(&output)["outcome"];
    assert_terminal_contract("withheld_probes", &summary(&output));
    let json = summary(&output);
    assert_eq!(
        diagnostic(&output),
        Some(format!(
            "microgentic: completion withheld by the probe gate: {}; evidence: {}",
            json["outcome"]["detail"].as_str().unwrap(),
            json["trace_file"].as_str().unwrap(),
        ))
    );
    assert_eq!(outcome["detail"], "probe `check` missing");
}

// ---------------------------------------------------------------- hard stop

/// Two turns of looking around without a completion claim use up a two-turn
/// budget.
fn iteration_budget(wire: Wire) {
    let workspace = Workspace::new();
    workspace.write("notes.txt", "one\ntwo\n");
    write_task(&workspace, "Look around, then finish.\n");
    let provider = ScriptedGateway::turns(
        wire,
        vec![
            vec![tool("list_tree", json!({"path": ".", "max_depth": 1}))],
            vec![content("Still looking.")],
            vec![tool(
                "read_file",
                json!({"path": "notes.txt", "line_start": 1, "line_end": 2}),
            )],
            vec![content("Still looking.")],
        ],
    );
    let output = task(
        microgentic(),
        &["run"],
        &provider,
        &workspace,
        &["--json", "--max-iterations", "2"],
    );
    assert_eq!(output.status.code(), Some(5), "{}", stderr(&output));
    let outcome = &summary(&output)["outcome"];
    assert_terminal_contract("iteration_budget", &summary(&output));
    assert_eq!(outcome["detail"], "2 turns used; last: Still looking.");
    assert_eq!(
        provider.request_count(),
        4,
        "two tool calls and two replies"
    );
    assert!(
        diagnostic(&output).is_some_and(|line| line.starts_with(
            "microgentic: stopped by the runtime (iteration budget): 2 turns used; last: Still looking.; evidence: "
        )),
        "{}",
        stderr(&output)
    );
}

#[test]
fn an_exhausted_iteration_budget_exits_five_omlx() {
    iteration_budget(Wire::Omlx);
}

#[test]
fn an_exhausted_iteration_budget_exits_five_ollama() {
    iteration_budget(Wire::Ollama);
}

/// Empty replies, turn after turn, reach the runtime's escalation.
#[test]
fn repeated_empty_responses_exit_five_as_an_empty_response_hard_stop() {
    let workspace = Workspace::new();
    write_task(&workspace, "Say anything, then finish.\n");
    let provider = ScriptedGateway::replying(Wire::Omlx, vec![]);
    let output = task(microgentic(), &["run"], &provider, &workspace, &["--json"]);
    assert_eq!(output.status.code(), Some(5), "{}", stderr(&output));
    let outcome = &summary(&output)["outcome"];
    assert_terminal_contract("empty_response", &summary(&output));
    let json = summary(&output);
    assert_eq!(
        diagnostic(&output),
        Some(format!(
            "microgentic: stopped by the runtime (empty response): {}; evidence: {}",
            json["outcome"]["detail"].as_str().unwrap(),
            json["trace_file"].as_str().unwrap(),
        ))
    );
    assert_eq!(
        outcome["detail"],
        "turn 3 produced 3 consecutive empty responses with no tool calls or final text"
    );
    assert_eq!(provider.request_count(), 3);
}
