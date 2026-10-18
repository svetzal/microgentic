//! How `microgentic run` reports the way a run ended: the typed `outcome` in
//! the JSON summary, one diagnostic line on standard error, and the exit
//! status. The benchmark surface and the compatibility executable keep
//! exit 0 and gain only the `outcome` field. Mock providers, no model.
mod support;

use serde_json::{Value, json};
use std::process::Output;
use support::*;

fn env(provider: &MockProvider, workspace: &Workspace) -> Vec<(&'static str, String)> {
    let mut env = provider.env();
    env.push(("XDG_STATE_HOME", workspace.state.display().to_string()));
    env.push((
        "XDG_CONFIG_HOME",
        workspace.base.join("config").display().to_string(),
    ));
    env
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn summary(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "the summary is JSON: {error}\n{}\n{}",
            stdout(output),
            stderr(output)
        )
    })
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
    provider: &MockProvider,
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
    run_binary(
        microgentic(),
        &args,
        &workspace.project,
        &env(provider, workspace),
        b"",
    )
}

/// A task run through `binary`: `verb` is `run` or `benchmark run`, the
/// experiment root is the workspace base (holding `task.md` or a contract),
/// and the tool root is the project.
fn task(
    binary: &str,
    verb: &[&str],
    provider: &MockProvider,
    workspace: &Workspace,
    extra: &[&str],
) -> Output {
    // Trace files are named to the second, so runs in one test take
    // separate trace directories.
    static RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let run = RUNS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let experiment = workspace.base.display().to_string();
    let traces = workspace
        .base
        .join(format!("traces-{run}"))
        .display()
        .to_string();
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
    run_binary(
        binary,
        &args,
        &workspace.project,
        &env(provider, workspace),
        b"",
    )
}

fn write_task(workspace: &Workspace, text: &str) {
    std::fs::write(workspace.base.join("task.md"), text).expect("writing task.md");
}

// ---------------------------------------------------------------- replied

fn replied(wire: Wire) {
    let workspace = Workspace::new();
    let provider = MockProvider::replying(wire, vec![content("Hello there.")]);
    let output = everyday(&provider, &workspace, "Hello!", &["--json"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(summary(&output)["outcome"], json!({"kind": "replied"}));
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
    let provider = MockProvider::replying(wire, vec![content("DONE")]);
    let output = task(microgentic(), &["run"], &provider, &workspace, &["--json"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(summary(&output)["outcome"], json!({"kind": "done"}));
    assert_eq!(diagnostic(&output), None, "{}", stderr(&output));

    let output = task(legacy_binary(), &["run"], &provider, &workspace, &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(summary(&output)["outcome"], json!({"kind": "done"}));
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
    let provider = MockProvider::replying(wire, vec![content(FAIL_TEXT)]);

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
    assert_eq!(
        json["outcome"],
        json!({"kind": "failed", "reason": FAIL_TEXT})
    );
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
        &["--dir", &workspace.project.display().to_string()],
    );
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        summary(&output)["outcome"],
        json!({"kind": "failed", "reason": FAIL_TEXT})
    );
    assert_eq!(diagnostic(&output), None, "{}", stderr(&output));

    let output = task(legacy_binary(), &["run"], &provider, &workspace, &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(
        summary(&output)["outcome"],
        json!({"kind": "failed", "reason": FAIL_TEXT})
    );
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
    let provider = MockProvider::turns(
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
    assert_eq!(outcome["kind"], "withheld");
    assert_eq!(outcome["gate"], "validation");
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
    let provider = MockProvider::turns(
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
    assert_eq!(outcome["kind"], "withheld");
    assert_eq!(outcome["gate"], "stash");
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
    let provider = MockProvider::replying(Wire::Omlx, vec![content("DONE")]);
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
    assert_eq!(outcome["kind"], "withheld");
    assert_eq!(outcome["gate"], "probes");
    assert_eq!(outcome["detail"], "probe `check` missing");
}

// ---------------------------------------------------------------- hard stop

/// Two turns of looking around without a completion claim use up a two-turn
/// budget.
fn iteration_budget(wire: Wire) {
    let workspace = Workspace::new();
    workspace.write("notes.txt", "one\ntwo\n");
    write_task(&workspace, "Look around, then finish.\n");
    let provider = MockProvider::turns(
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
    assert_eq!(outcome["kind"], "hard_stop");
    assert_eq!(outcome["cause"], "iteration_budget");
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
    let provider = MockProvider::replying(Wire::Omlx, vec![]);
    let output = task(microgentic(), &["run"], &provider, &workspace, &["--json"]);
    assert_eq!(output.status.code(), Some(5), "{}", stderr(&output));
    let outcome = &summary(&output)["outcome"];
    assert_eq!(outcome["kind"], "hard_stop");
    assert_eq!(outcome["cause"], "empty_response");
    assert_eq!(
        outcome["detail"],
        "turn 3 produced 3 consecutive empty responses with no tool calls or final text"
    );
    assert_eq!(provider.request_count(), 3);
}
