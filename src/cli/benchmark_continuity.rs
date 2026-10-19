//! Socket-free continuity through the real CLI dispatcher and gateway boundary.
use super::test_support::*;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

const PROMPT: &str = "Inspect the project with list_tree, then finish. Unicode 世界.";
const SENTINEL: &str = "PROJECT-GUIDANCE-SENTINEL";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    /// Legacy verbs on the baseline, `benchmark <verb>` with ambient intake on
    /// the current binary, for every case.
    Benchmark,
    /// `run --prompt` with a contract on both binaries.
    Conversation,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Case {
    Contract,
    Goal,
    Sequential,
}

impl Case {
    fn name(self) -> &'static str {
        match self {
            Case::Contract => "contract",
            Case::Goal => "goal",
            Case::Sequential => "sequential",
        }
    }

    fn expected_requests(self) -> usize {
        if self == Case::Sequential { 4 } else { 2 }
    }
}

/// One isolated filesystem shared by the CLI dispatch paths being compared.
struct Experiment {
    workspace: Workspace,
    base: PathBuf,
    root: PathBuf,
    contract: PathBuf,
    config: PathBuf,
}

impl Experiment {
    fn new() -> Self {
        let workspace = Workspace::new();
        let base = workspace.base.clone();
        let root = workspace.project.clone();
        // Project guidance must never reach benchmark or contract runs.
        std::fs::write(
            root.join("AGENTS.md"),
            format!("{SENTINEL}: benchmark runs must never see this.\n"),
        )
        .expect("project guidance");
        let contract = base.join("contract.json");
        std::fs::write(
            &contract,
            json!({
                "profile": {"id": "terminal_work", "version": "terminal_work_profile.v1"},
                "guidance": PROMPT,
                "read_scope": ["./**"],
                "write_scope": ["./**"],
                "probes": [],
            })
            .to_string(),
        )
        .expect("contract");
        let config = workspace.config.clone();
        std::fs::write(
            &config,
            "invalid personal config: must never be read by benchmarks",
        )
        .expect("personal config");
        std::fs::write(base.join("task.md"), PROMPT).expect("goal");
        std::fs::write(base.join("expected.txt"), "expected artifact\n")
            .expect("expected artifact");
        Self {
            workspace,
            base,
            root,
            contract,
            config,
        }
    }
}

/// What one CLI invocation sent and recorded.
struct Capture {
    requests: Vec<Value>,
    packets: Vec<Value>,
    ledgers: Vec<Value>,
    summary: Value,
}

/// Each attempt inspects with an explicit tool-call ID, then finishes.
fn inspect_then_finish(wire: Wire) -> ScriptedGateway {
    let inspection = vec![
        content("Inspecting."),
        tool_with_id("inspection", "list_tree", json!({})),
    ];
    ScriptedGateway::turns(
        wire,
        vec![
            inspection.clone(),
            vec![content("DONE")],
            inspection,
            vec![content("DONE")],
        ],
    )
}

fn path(path: &Path) -> String {
    path.display().to_string()
}

fn capture(
    experiment: &Experiment,
    mode: Mode,
    case: Case,
    wire: Wire,
    side: &str,
    binary: &str,
) -> Capture {
    let provider = inspect_then_finish(wire);
    let trace_dir = experiment
        .base
        .join(format!("{}-{}-{side}", case.name(), wire.name()));
    let mut controls: Vec<String> = [
        "--experiment",
        &path(&experiment.base),
        "--trace-dir",
        &path(&trace_dir),
        "--provider",
        wire.name(),
        "--reasoning-effort",
        "low",
        "--max-tokens",
        "2048",
        "--max-model-interactions",
        "4",
        "--max-iterations",
        "3",
    ]
    .map(str::to_owned)
    .to_vec();
    if case != Case::Goal {
        controls.extend(["--contract".into(), path(&experiment.contract)]);
    }
    let verb = if case == Case::Sequential {
        "run-sequential"
    } else {
        "run"
    };
    if case == Case::Sequential {
        controls.extend(
            [
                "--artifact",
                "answer.txt",
                "--expected-artifact",
                "expected.txt",
                "--max-attempts",
                "2",
                "--max-consecutive-unchanged-attempts",
                "2",
            ]
            .map(str::to_owned),
        );
    }
    let current = side == "current";
    let mut stdin: &[u8] = b"";
    let args: Vec<String> = match mode {
        Mode::Benchmark => {
            if current {
                // Both historical file inputs may be supplied; the contract wins.
                if case != Case::Goal {
                    controls.extend(["--goal".into(), "missing-unused-goal.md".into()]);
                }
                stdin = b"ignore this ambient stdin";
                let mut args = vec![
                    "--config".into(),
                    path(&experiment.config),
                    "benchmark".into(),
                    verb.into(),
                ];
                args.extend(controls);
                args.extend(["--dir".into(), path(&experiment.root)]);
                args
            } else {
                let mut args = vec![verb.to_owned()];
                args.extend(controls);
                args.push("--json".into());
                args
            }
        }
        Mode::Conversation => {
            let mut args = vec!["run".to_owned()];
            args.extend(controls);
            args.extend(["--prompt".into(), PROMPT.into(), "--json".into()]);
            args
        }
    };
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = execute_input(binary, &args, &provider, &experiment.workspace, stdin);
    assert!(
        provider.metadata_models().is_empty(),
        "isolated runs must not discover windows"
    );
    let label = format!("{} {} {side}", case.name(), wire.name());
    assert!(output.status.success(), "{label}: {}", stderr(&output));
    let summary: Value = serde_json::from_str(&stdout(&output))
        .unwrap_or_else(|error| panic!("{label}: the summary is not JSON: {error}"));
    assert!(
        !experiment.workspace.state.exists(),
        "{label}: isolated run created personal state"
    );
    let trace_files: Vec<PathBuf> = if case == Case::Sequential {
        summary["attempts"]
            .as_array()
            .expect("sequential attempts")
            .iter()
            .map(|attempt| attempt["trace_file"].as_str().expect("trace file").into())
            .collect()
    } else {
        vec![summary["trace_file"].as_str().expect("trace file").into()]
    };
    // Unpaced attempts share one trace directory, each with its own file.
    let directories: std::collections::BTreeSet<_> =
        trace_files.iter().map(|file| file.parent()).collect();
    let distinct: std::collections::BTreeSet<_> = trace_files.iter().collect();
    assert_eq!(directories.len(), 1, "{label}: {trace_files:?}");
    assert_eq!(
        distinct.len(),
        trace_files.len(),
        "{label}: {trace_files:?}"
    );
    let records: Vec<Value> = trace_files
        .iter()
        .flat_map(|file| trace_records(file))
        .collect();
    let of_kind = |kind: &str| -> Vec<Value> {
        records
            .iter()
            .filter(|record| record["kind"] == kind)
            .map(|record| record["payload"].clone())
            .collect()
    };
    let packets = of_kind("llm.provider_request.assembled");
    let ledgers = of_kind("llm.context_assembly.ledger");
    assert!(
        !records.iter().any(|record| record["kind"]
            .as_str()
            .is_some_and(|kind| kind.starts_with("session."))),
        "{label}: a benchmark trace recorded session events"
    );
    assert!(
        !records
            .iter()
            .any(|record| record["kind"]
                .as_str()
                .is_some_and(|kind| kind.starts_with("context.residency")
                    || kind == "context.window.resolved")),
        "{label}: a benchmark trace recorded residency or a discovered window"
    );
    assert!(
        ledgers.iter().all(|ledger| ledger.get("residency").is_none()
            && ledger.get("known_window").is_none()),
        "{label}: a benchmark ledger carries residency fields"
    );
    let requests = provider.requests();
    assert_eq!(
        requests.len(),
        case.expected_requests(),
        "{label}: requests"
    );
    assert_eq!(
        requests[1]["messages"][2]["tool_calls"][0]["id"],
        "inspection"
    );
    assert_eq!(requests[1]["messages"][3]["role"], "tool");
    assert_eq!(
        requests[1]["messages"][3]["tool_calls"][0]["id"],
        "inspection"
    );
    assert_fixed_evidence(case, wire, &requests, &packets, &ledgers, &summary);
    assert_eq!(packets.len(), case.expected_requests(), "{label}: packets");
    if case == Case::Sequential {
        assert_eq!(
            requests[..2],
            requests[2..],
            "{label}: context leaked between task attempts"
        );
        assert_eq!(summary["stop_reason"], "consecutive_unchanged_attempts");
        assert_eq!(summary["confirmed_exact_success"], false);
    }
    assert!(
        !Value::from(requests.clone()).to_string().contains(SENTINEL),
        "{label}: project guidance reached a benchmark request"
    );
    Capture {
        requests,
        packets,
        ledgers,
        summary,
    }
}

/// Run one case on both binaries, assert continuity, and describe the result.
fn compare(baseline: &str, current: &str, mode: Mode, case: Case, wire: Wire) {
    let experiment = Experiment::new();
    let old = capture(&experiment, mode, case, wire, "baseline", baseline);
    let new = capture(&experiment, mode, case, wire, "current", current);
    let label = format!("{} {}", case.name(), wire.name());
    assert_eq!(
        old.requests, new.requests,
        "{label}: provider requests changed"
    );
    assert_eq!(
        old.packets, new.packets,
        "{label}: provider packets changed"
    );
    assert_eq!(old.ledgers, new.ledgers, "{label}: context ledgers changed");
    if case == Case::Sequential {
        let attempts =
            |summary: &Value| summary["attempts"].as_array().cloned().unwrap_or_default();
        let (left, right) = (attempts(&old.summary), attempts(&new.summary));
        assert_eq!(left.len(), right.len(), "{label}: attempt count changed");
        for (left, right) in left.iter().zip(&right) {
            for key in [
                "final_summary",
                "accepted_done",
                "independent_exact",
                "terminal_error",
                "pre_artifact",
                "post_artifact",
                "artifact_unchanged",
            ] {
                assert_eq!(
                    left[key], right[key],
                    "{label}: sequential attempt {key} changed"
                );
            }
        }
    } else {
        assert_eq!(
            old.summary["final_summary"], new.summary["final_summary"],
            "{label}: final summary changed"
        );
    }
}

/// Summary additions since 0.4.0 are optional for historical readers.
const ADDITIVE_SUMMARY_KEYS: &[&str] = &["outcome", "changes", "checks"];

/// Fixed expectations are independent of the two dispatch paths. No IDs, text,
/// paths, descriptors or completion controls are masked. The fixture factors
/// shared descriptors/controls once and expands them to complete gateway inputs.
fn assert_fixed_evidence(
    case: Case,
    wire: Wire,
    requests: &[Value],
    packets: &[Value],
    ledgers: &[Value],
    summary: &Value,
) {
    let expected: Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/benchmark_continuity.json"
    ))
    .unwrap();
    assert_eq!(
        expected["schema_version"],
        "microgentic_benchmark_continuity.v1"
    );
    let profile = if case == Case::Goal {
        "goal"
    } else {
        "contract"
    };
    let model = &expected["models"][wire.name()];
    let mut expected_requests = Vec::new();
    let mut expected_packets = Vec::new();
    let mut expected_ledgers = Vec::new();
    for i in 0..case.expected_requests() {
        let messages = &expected["messages"][profile][i % 2];
        let mut request = expected["request_controls"].clone();
        request["messages"] = messages.clone();
        request["model"] = model.clone();
        request["tools"] = expected["tools"].clone();
        expected_requests.push(request);

        let mut packet = expected["packet_controls"][wire.name()][i % 2].clone();
        packet["messages"] = messages.clone();
        packet["model"] = model.clone();
        packet["tools"] = expected["tools"].clone();
        expected_packets.push(packet);

        let mut ledger = expected["ledgers"][profile][i % 2].clone();
        ledger["model"] = model.clone();
        expected_ledgers.push(ledger);
    }
    assert_eq!(requests, expected_requests, "complete gateway requests");
    assert_eq!(packets, expected_packets, "complete assembled packets");
    assert_eq!(ledgers, expected_ledgers, "complete context ledgers");

    let key = if case == Case::Sequential {
        "sequential_summary_keys"
    } else {
        "summary_keys"
    };
    let required: Vec<&str> = expected[key]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    for key in &required {
        assert!(summary.get(key).is_some(), "removed summary field {key}");
    }
    for key in summary.as_object().unwrap().keys() {
        assert!(
            required.contains(&key.as_str())
                || (case != Case::Sequential && ADDITIVE_SUMMARY_KEYS.contains(&key.as_str())),
            "undocumented summary field {key}"
        );
    }
    if case == Case::Sequential {
        let attempts = summary["attempts"].as_array().unwrap();
        assert_eq!(attempts.len(), 2);
        for (i, attempt) in attempts.iter().enumerate() {
            // Every deterministic attempt field has a fixed value. Trace paths
            // are checked for distinctness above; elapsed time is not asserted.
            let mut fields = attempt.as_object().unwrap().clone();
            assert!(fields.remove("trace_file").is_some());
            assert!(fields.remove("runtime_seconds").is_some());
            assert_eq!(Value::Object(fields), expected["attempts"][i]);
        }
    } else {
        assert_eq!(summary["final_summary"], "Inspecting.\nDONE");
    }
}

fn self_check(case: Case, wire: Wire) {
    // Preserve top-level run dispatch as well as the compatibility executable.
    compare(microgentic(), microgentic(), Mode::Benchmark, case, wire);
    compare(legacy_binary(), microgentic(), Mode::Benchmark, case, wire);
}

#[test]
fn benchmark_packets_match_task_runs_and_ignore_ambient_intake_contract_omlx() {
    self_check(Case::Contract, Wire::Omlx);
}

#[test]
fn benchmark_packets_match_task_runs_and_ignore_ambient_intake_contract_ollama() {
    self_check(Case::Contract, Wire::Ollama);
}

#[test]
fn benchmark_packets_match_task_runs_and_ignore_ambient_intake_goal_omlx() {
    self_check(Case::Goal, Wire::Omlx);
}

#[test]
fn benchmark_packets_match_task_runs_and_ignore_ambient_intake_goal_ollama() {
    self_check(Case::Goal, Wire::Ollama);
}

#[test]
fn benchmark_packets_match_task_runs_and_ignore_ambient_intake_sequential_omlx() {
    self_check(Case::Sequential, Wire::Omlx);
}

#[test]
fn benchmark_packets_match_task_runs_and_ignore_ambient_intake_sequential_ollama() {
    self_check(Case::Sequential, Wire::Ollama);
}

#[test]
fn contract_conversation_isolation_both_providers() {
    for wire in [Wire::Omlx, Wire::Ollama] {
        compare(
            legacy_binary(),
            microgentic(),
            Mode::Conversation,
            Case::Contract,
            wire,
        );
    }
}
