//! `start_task` through the production CLI dispatcher: everyday `run` hands
//! engineering work to the task-mode subagent, which shares the in-process
//! gateway double, and the result returns to the coordinator.
use super::everyday_runs::{last_tool_result, system_message, tool_names, user_messages};
use super::test_support::*;
use serde_json::{Value, json};
use std::path::PathBuf;

const GOAL: &str = "Create hello.txt containing hi.";
const BRIEF: &str = "Create hello.txt containing hi.\n\n\
                     Context:\nThe user wants a greeting file at the project root.\n\n\
                     Constraints:\nTouch no other file.\n\n\
                     Done when these checks pass:\n- grep -q hi hello.txt";

fn everyday(provider: &ScriptedGateway, workspace: &Workspace) -> Value {
    let output = execute(
        microgentic(),
        &[
            "run",
            "--prompt",
            "Please add a hello file.",
            "--json",
            "--provider",
            provider.wire().name(),
            "--model",
            "test-model",
        ],
        provider,
        workspace,
    );
    assert!(output.status.success(), "{}", stderr(&output));
    summary(&output)
}

fn path(value: &Value) -> PathBuf {
    PathBuf::from(value.as_str().expect("a path"))
}

fn offers_start_task(request: &Value) -> bool {
    tool_names(request).contains(&"start_task".to_owned())
}

fn task_mode_system(request: &Value) -> bool {
    !system_message(request).contains("conversational assistant")
}

fn delegate(wire: Wire) {
    let workspace = Workspace::new();
    let provider = ScriptedGateway::turns(
        wire,
        vec![
            // The coordinator writes the brief.
            vec![tool(
                "start_task",
                json!({
                    "goal": GOAL,
                    "context": "The user wants a greeting file at the project root.",
                    "constraints": "Touch no other file.",
                    "checks": ["grep -q hi hello.txt"],
                }),
            )],
            // The subagent does the work, runs its probe and finishes.
            vec![tool(
                "write_file",
                json!({"path": "hello.txt", "content": "hi\n"}),
            )],
            vec![tool(
                "execute_probe",
                json!({"probe_id": "grep-q-hi-hello-txt"}),
            )],
            vec![content("DONE")],
            // The coordinator reports the result.
            vec![content("I added hello.txt and its check passed.")],
        ],
    );
    let summary = everyday(&provider, &workspace);
    let requests = provider.requests();
    assert_eq!(
        requests.len(),
        5,
        "two coordinator and three subagent requests"
    );
    assert_eq!(workspace.read("hello.txt"), "hi\n");
    assert_eq!(summary["outcome"]["kind"], "replied");
    assert_eq!(
        summary["final_summary"],
        "I added hello.txt and its check passed."
    );

    // The coordinator is offered start_task, with its guidance.
    let coordinator = &requests[0];
    assert!(
        offers_start_task(coordinator),
        "{:?}",
        tool_names(coordinator)
    );
    assert!(
        system_message(coordinator).contains(crate::start_task::GUIDANCE),
        "{}",
        system_message(coordinator)
    );

    // The subagent is task mode: its own system guidance, the rendered brief
    // as the first message with the declared probe, and no start_task or
    // calculate.
    for request in &requests[1..4] {
        assert!(task_mode_system(request), "{}", system_message(request));
        assert!(!offers_start_task(request), "{:?}", tool_names(request));
        assert!(!tool_names(request).contains(&"calculate".to_owned()));
    }
    let first = &user_messages(&requests[1])[0];
    let profile = crate::profile::default_profile();
    assert!(
        first.starts_with(&profile.run_guidance(BRIEF)),
        "the brief is the task: {first}"
    );
    assert!(first.contains("grep-q-hi-hello-txt"), "{first}");
    assert!(
        !requests[1]["messages"]
            .to_string()
            .contains("Please add a hello file."),
        "the subagent starts from the brief, not the conversation"
    );

    // The coordinator receives the subagent's result and replies.
    let last = &requests[4];
    assert!(!task_mode_system(last));
    let result: Value = serde_json::from_str(&last_tool_result(last)).expect("JSON result");
    assert_eq!(result["outcome"], json!({"kind": "done"}));
    assert_eq!(result["final_summary"], "DONE");
    assert_eq!(
        result["checks"]["declared"],
        json!([{
            "probe_id": "grep-q-hi-hello-txt",
            "command": "grep -q hi hello.txt",
            "status": "passed",
            "exit_code": 0,
        }])
    );
    assert!(
        result["changes"]["files"].to_string().contains("hello.txt"),
        "{}",
        result["changes"]
    );
    // What the subagent changed is what the run changed.
    assert_eq!(summary["changes"]["files"], result["changes"]["files"]);

    // Both traces exist; the coordinator's links the subagent's.
    let child_trace = path(&result["trace_file"]);
    let coordinator_trace = path(&summary["trace_file"]);
    assert_ne!(child_trace, coordinator_trace);
    let started = trace_payloads(
        &coordinator_trace,
        crate::runtime_events::SUBAGENT_TASK_STARTED,
    );
    assert_eq!(started.len(), 1);
    let started = &started[0];
    assert_eq!(started["task"], 1);
    assert_eq!(started["goal"], GOAL);
    assert_eq!(started["checks"], json!(["grep -q hi hello.txt"]));
    assert_eq!(started["guidance"], BRIEF);
    let contract_file = path(&started["contract_file"]);
    assert!(child_trace.starts_with(path(&started["trace_dir"])));
    let finished = trace_payloads(
        &coordinator_trace,
        crate::runtime_events::SUBAGENT_TASK_FINISHED,
    );
    assert_eq!(
        finished,
        [json!({"task": 1, "outcome": {"kind": "done"}, "trace_file": child_trace})]
    );
    // The contract and the subagent's trace stay out of the project.
    for file in [&contract_file, &child_trace] {
        assert!(file.is_file(), "{}", file.display());
        assert!(!file.starts_with(&workspace.project), "{}", file.display());
    }
    let contract: Value =
        serde_json::from_str(&std::fs::read_to_string(&contract_file).unwrap()).unwrap();
    assert_eq!(
        contract,
        json!({
            "guidance": BRIEF,
            "probes": [{"id": "grep-q-hi-hello-txt", "command": "grep -q hi hello.txt"}],
        })
    );
    let delivered = trace_payloads(&child_trace, "agent.contract.probes.delivered");
    assert_eq!(delivered[0]["probe_ids"], json!(["grep-q-hi-hello-txt"]));
    let resolved = trace_payloads(&child_trace, "agent.contract.resolved");
    assert_eq!(resolved[0]["adapter_kind"], "explicit");
}

#[test]
fn everyday_run_hands_engineering_work_to_the_subagent_omlx() {
    delegate(Wire::Omlx);
}

#[test]
fn everyday_run_hands_engineering_work_to_the_subagent_ollama() {
    delegate(Wire::Ollama);
}

/// A check that hides its own failure, or an empty one, is a tool error the
/// model can act on; nothing starts.
#[test]
fn a_masking_or_empty_check_is_rejected_before_anything_starts() {
    let workspace = Workspace::new();
    let provider = ScriptedGateway::turns(
        Wire::Omlx,
        vec![
            vec![tool(
                "start_task",
                json!({"goal": GOAL, "checks": ["grep -q hi hello.txt || true"]}),
            )],
            vec![tool("start_task", json!({"goal": GOAL, "checks": ["  "]}))],
            vec![content("I could not start that.")],
        ],
    );
    let summary = everyday(&provider, &workspace);
    let requests = provider.requests();
    assert_eq!(requests.len(), 3, "no subagent request");
    let error = |request: &Value| -> String {
        let result: Value = serde_json::from_str(&last_tool_result(request)).unwrap();
        result["error"].as_str().expect("a tool error").to_owned()
    };
    assert!(
        error(&requests[1]).contains("hides its own failure"),
        "{}",
        error(&requests[1])
    );
    assert_eq!(
        error(&requests[2]),
        "Tool error: check 1 is empty; give a shell command such as \"make test\" or \"pytest\", or leave it out"
    );
    let trace = path(&summary["trace_file"]);
    assert!(trace_payloads(&trace, crate::runtime_events::SUBAGENT_TASK_STARTED).is_empty());
    assert!(!workspace.project.join("hello.txt").exists());
}

fn contract(workspace: &Workspace) -> String {
    let contract = workspace.base.join("contract.json");
    std::fs::write(
        &contract,
        json!({"guidance": "Finish.", "probes": []}).to_string(),
    )
    .expect("writing the contract");
    contract.display().to_string()
}

fn first_request(
    binary: &str,
    args: &[&str],
    workspace: &Workspace,
    script: Vec<Vec<mojentic::llm::gateway::StreamChunk>>,
) -> Value {
    let provider = ScriptedGateway::turns(Wire::Omlx, script);
    let output = execute(binary, args, &provider, workspace);
    assert!(output.status.success(), "{}", stderr(&output));
    provider.requests().remove(0)
}

/// Only everyday `run` offers start_task: not benchmark or task mode, not
/// the compatibility executable, and not the TUI conversation.
#[test]
fn start_task_is_offered_only_to_everyday_run() {
    let workspace = Workspace::new();
    let experiment = workspace.base.display().to_string();
    let traces = workspace.base.join("traces").display().to_string();
    let contract = contract(&workspace);
    let model = ["--provider", "omlx", "--model", "test-model"];
    let task = [
        "--experiment",
        &experiment,
        "--trace-dir",
        &traces,
        "--contract",
        &contract,
        "--max-iterations",
        "1",
    ];
    let done = || vec![vec![content("DONE")]];

    let benchmark = [&["benchmark", "run"][..], &task, &model].concat();
    let request = first_request(microgentic(), &benchmark, &workspace, done());
    assert!(!offers_start_task(&request), "{:?}", tool_names(&request));

    let task_run = [&["run"][..], &task, &model].concat();
    let request = first_request(microgentic(), &task_run, &workspace, done());
    assert!(!offers_start_task(&request), "{:?}", tool_names(&request));

    let legacy = [
        &["run", "--prompt", "Hello!", "--max-iterations", "1"][..],
        &model,
    ]
    .concat();
    let request = first_request(legacy_binary(), &legacy, &workspace, done());
    assert!(!offers_start_task(&request), "{:?}", tool_names(&request));

    let session = workspace.base.join("session").join("conversation.json");
    std::fs::create_dir_all(session.parent().unwrap()).expect("session directory");
    let session = session.display().to_string();
    let worker = [
        &[
            "conversation-worker",
            "--prompt",
            "Hi.",
            "--session-state",
            &session,
        ][..],
        &model,
    ]
    .concat();
    let request = first_request(
        microgentic(),
        &worker,
        &workspace,
        vec![vec![content("Hello.")]],
    );
    assert!(!offers_start_task(&request), "{:?}", tool_names(&request));
    assert!(!system_message(&request).contains(crate::start_task::GUIDANCE));

    let everyday_run = [&["run", "--prompt", "Hi."][..], &model].concat();
    let request = first_request(
        microgentic(),
        &everyday_run,
        &workspace,
        vec![vec![content("Hello.")]],
    );
    assert!(offers_start_task(&request), "{:?}", tool_names(&request));
}
