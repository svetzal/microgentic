//! Run report coverage through real CLI dispatch and tool execution.
use super::test_support::*;
use mojentic::llm::gateway::StreamChunk;
use serde_json::json;

/// The `microgentic: …` lines on standard error.
fn report(output: &Output) -> Vec<String> {
    stderr(output)
        .lines()
        .filter(|line| line.starts_with("microgentic: "))
        .map(str::to_owned)
        .collect()
}

/// An everyday `microgentic run --prompt` in the project.
fn everyday(provider: &ScriptedGateway, workspace: &Workspace, extra: &[&str]) -> Output {
    let mut args = vec![
        "run",
        "--provider",
        provider.wire().name(),
        "--model",
        "test-model",
        "--prompt",
        "Please do the work.",
    ];
    args.extend_from_slice(extra);
    execute(microgentic(), &args, provider, workspace)
}

fn edit(path: &str, old: &str, new: &str) -> StreamChunk {
    tool(
        "edit_file",
        json!({"path": path, "edits": [{"kind": "replace_exact", "old": old, "new": new}]}),
    )
}

fn shell(command: &str) -> StreamChunk {
    tool(
        "shell_command",
        json!({"command": command, "timeout_secs": 30}),
    )
}

fn edit_then_grep(wire: Wire) -> ScriptedGateway {
    ScriptedGateway::turns(
        wire,
        vec![
            vec![edit("greeting.txt", "Hi there", "Hello there")],
            vec![shell("grep -c Hello greeting.txt")],
            vec![content("Changed the greeting.")],
        ],
    )
}

// ------------------------------------------------------- edit then check

fn edit_then_check(wire: Wire) {
    let workspace = Workspace::new();
    workspace.write("greeting.txt", "Hi there\n");
    let output = everyday(&edit_then_grep(wire), &workspace, &["--json"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(workspace.read("greeting.txt"), "Hello there\n");
    let json = summary(&output);
    assert_eq!(
        json["changes"],
        json!({
            "complete": true,
            "note": null,
            "files": [{"path": "greeting.txt", "change": "modified", "edited_before_run": false}],
            "edited_before_run": null,
        })
    );
    assert_eq!(
        json["checks"],
        json!({
            "declared": [],
            "observed": [{"command": "grep -c Hello greeting.txt", "exit_code": 0}],
            "unchecked_paths": [],
        })
    );
    assert!(report(&output).is_empty(), "{}", stderr(&output));

    workspace.write("greeting.txt", "Hi there\n");
    let output = everyday(&edit_then_grep(wire), &workspace, &[]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stdout(&output), "Changed the greeting.\n");
    assert_eq!(
        report(&output),
        [
            "microgentic: changed 1 file: greeting.txt (modified)",
            "microgentic: commands run: `grep -c Hello greeting.txt` exit 0",
        ],
        "{}",
        stderr(&output)
    );
}

#[test]
fn an_edit_then_a_check_reports_the_change_and_the_command_omlx() {
    edit_then_check(Wire::Omlx);
}

#[test]
fn an_edit_then_a_check_reports_the_change_and_the_command_ollama() {
    edit_then_check(Wire::Ollama);
}

// ------------------------------------------------------- dirty repository

#[test]
fn edits_already_in_the_working_tree_are_flagged_and_untouched_ones_are_not_changes() {
    let workspace = Workspace::new();
    workspace.write("a.txt", "one\n");
    workspace.git_init();
    workspace.write("a.txt", "one\ntwo\n");
    workspace.write("scratch.txt", "draft\n");
    let provider = ScriptedGateway::turns(
        Wire::Omlx,
        vec![
            vec![edit("a.txt", "two", "three")],
            vec![content("Updated a.txt.")],
        ],
    );
    let output = everyday(&provider, &workspace, &["--json"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(workspace.read("a.txt"), "one\nthree\n");
    let changes = &summary(&output)["changes"];
    assert_eq!(
        changes["edited_before_run"],
        json!(["a.txt", "scratch.txt"])
    );
    assert_eq!(
        changes["files"],
        json!([{"path": "a.txt", "change": "modified", "edited_before_run": true}])
    );
    assert_eq!(changes["complete"], true);

    workspace.write("a.txt", "one\ntwo\n");
    let provider = ScriptedGateway::turns(
        Wire::Omlx,
        vec![
            vec![edit("a.txt", "two", "three")],
            vec![content("Updated a.txt.")],
        ],
    );
    let output = everyday(&provider, &workspace, &[]);
    assert_eq!(
        report(&output),
        ["microgentic: changed 1 file: a.txt (modified, edited before the run)"],
        "{}",
        stderr(&output)
    );
}

// ------------------------------------------------------- shell changes

#[test]
fn files_a_shell_command_creates_and_deletes_are_reported() {
    let workspace = Workspace::new();
    workspace.write("old.txt", "old\n");
    workspace.write("keep.txt", "keep\n");
    let provider = ScriptedGateway::turns(
        Wire::Omlx,
        vec![
            vec![shell("printf new > new.txt && rm old.txt")],
            vec![content("Replaced old.txt with new.txt.")],
        ],
    );
    let output = everyday(&provider, &workspace, &["--json"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(workspace.read("new.txt"), "new");
    assert!(!workspace.project.join("old.txt").exists());
    assert_eq!(workspace.read("keep.txt"), "keep\n");
    let json = summary(&output);
    assert_eq!(
        json["changes"]["files"],
        json!([
            {"path": "new.txt", "change": "created", "edited_before_run": false},
            {"path": "old.txt", "change": "deleted", "edited_before_run": false},
        ])
    );
    assert_eq!(
        json["checks"]["observed"],
        json!([{"command": "printf new > new.txt && rm old.txt", "exit_code": 0}])
    );
}

// ------------------------------------------------------- declared probes

/// One declared probe passes, the other never runs, and the probe gate
/// withholds DONE when the one-turn budget runs out.
#[test]
fn declared_probes_report_which_passed_and_which_did_not_run() {
    let workspace = Workspace::new();
    let contract = workspace.base.join("contract.json");
    std::fs::write(
        &contract,
        json!({
            "profile": {"id": "terminal_work", "version": "terminal_work_profile.v1"},
            "guidance": "Run the checks, then finish.",
            "read_scope": ["./**"],
            "write_scope": ["./**"],
            "probes": [
                {"id": "test", "command": "true"},
                {"id": "lint", "command": "exit 0"},
            ],
        })
        .to_string(),
    )
    .expect("writing the contract");
    let provider = || {
        ScriptedGateway::turns(
            Wire::Omlx,
            vec![
                vec![tool("execute_probe", json!({"probe_id": "test"}))],
                vec![content("DONE")],
            ],
        )
    };
    let experiment = workspace.base.display().to_string();
    let traces = workspace.base.join("traces").display().to_string();
    let contract = contract.display().to_string();
    let args = [
        "run",
        "--experiment",
        &experiment,
        "--trace-dir",
        &traces,
        "--provider",
        "omlx",
        "--model",
        "test-model",
        "--contract",
        &contract,
        "--max-iterations",
        "1",
    ];
    let mut json_args = args.to_vec();
    json_args.push("--json");
    let scripted = provider();
    let output = execute(microgentic(), &json_args, &scripted, &workspace);
    assert_eq!(output.status.code(), Some(4), "{}", stderr(&output));
    let json = summary(&output);
    assert_eq!(json["outcome"]["gate"], "probes");
    assert_eq!(
        json["checks"],
        json!({
            "declared": [
                {"probe_id": "test", "command": "true", "status": "passed", "exit_code": 0},
                {"probe_id": "lint", "command": "exit 0", "status": "not_run", "exit_code": null},
            ],
            "observed": [],
            "unchecked_paths": [],
        })
    );
    assert_eq!(json["changes"]["files"], json!([]));

    // Plain output: the report comes before the outcome diagnostic.
    let scripted = provider();
    let output = execute(microgentic(), &args, &scripted, &workspace);
    assert_eq!(output.status.code(), Some(4), "{}", stderr(&output));
    let lines = report(&output);
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(
        lines[0],
        "microgentic: checks: probe `test` passed; probe `lint` not run"
    );
    assert!(
        lines[1].starts_with("microgentic: completion withheld by the probe gate"),
        "{}",
        lines[1]
    );
}

// ------------------------------------------------------- unchecked

/// A source edit with no passing check after it is unchecked. Everyday runs
/// have no declared probes, and a model-run command is observational, so it
/// does not check the edit.
#[test]
fn a_source_edit_without_a_passing_check_is_unchecked() {
    let workspace = Workspace::new();
    workspace.write("calc.py", "def add(a, b):\n    return a - b\n");
    let provider = || {
        ScriptedGateway::turns(
            Wire::Omlx,
            vec![
                vec![edit("calc.py", "a - b", "a + b")],
                vec![shell("grep -c 'a + b' calc.py")],
                vec![content("Fixed add.")],
            ],
        )
    };
    let output = everyday(&provider(), &workspace, &["--json"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let json = summary(&output);
    assert_eq!(
        workspace.read("calc.py"),
        "def add(a, b):\n    return a + b\n"
    );
    assert_eq!(json["checks"]["unchecked_paths"], json!(["calc.py"]));
    assert_eq!(json["checks"]["observed"][0]["exit_code"], 0);
    assert_eq!(
        json["changes"]["files"],
        json!([{"path": "calc.py", "change": "modified", "edited_before_run": false}])
    );

    workspace.write("calc.py", "def add(a, b):\n    return a - b\n");
    let output = everyday(&provider(), &workspace, &[]);
    assert_eq!(
        report(&output).last().map(String::as_str),
        Some("microgentic: unchecked: calc.py changed after the last passing check"),
        "{}",
        stderr(&output)
    );
}

// ------------------------------------------------------- plain reply

fn greeting(wire: Wire) {
    let workspace = Workspace::new();
    workspace.write("notes.txt", "notes\n");
    let provider = ScriptedGateway::replying(wire, vec![content("Hello there.")]);
    let output = everyday(&provider, &workspace, &["--json"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let json = summary(&output);
    assert_eq!(json["changes"]["files"], json!([]));
    assert_eq!(json["changes"]["complete"], true);
    assert_eq!(
        json["checks"],
        json!({"declared": [], "observed": [], "unchecked_paths": []})
    );

    let output = everyday(&provider, &workspace, &[]);
    assert_eq!(stdout(&output), "Hello there.\n");
    assert!(report(&output).is_empty(), "{}", stderr(&output));
}

#[test]
fn a_plain_reply_reports_nothing_omlx() {
    greeting(Wire::Omlx);
}

#[test]
fn a_plain_reply_reports_nothing_ollama() {
    greeting(Wire::Ollama);
}

// ------------------------------------------------------- trace directory

/// Traces written into the project, in a subdirectory or at its top level,
/// are the harness's own files, not changes the run made.
#[test]
fn a_trace_directory_inside_the_project_is_not_a_change() {
    let workspace = Workspace::new();
    for trace_dir in [workspace.project.join("traces"), workspace.project.clone()] {
        workspace.write("greeting.txt", "Hi there\n");
        let trace_dir = trace_dir.display().to_string();
        let output = everyday(
            &edit_then_grep(Wire::Omlx),
            &workspace,
            &["--json", "--trace-dir", &trace_dir],
        );
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
        let json = summary(&output);
        assert!(
            json["trace_file"]
                .as_str()
                .is_some_and(|trace| trace.starts_with(&trace_dir)),
            "{}",
            json["trace_file"]
        );
        assert!(std::path::Path::new(json["trace_file"].as_str().unwrap()).is_file());
        assert_eq!(workspace.read("greeting.txt"), "Hello there\n");
        assert_eq!(
            json["changes"]["files"],
            json!([{"path": "greeting.txt", "change": "modified", "edited_before_run": false}]),
            "trace directory {trace_dir}"
        );
    }
}

/// A file-assertion probe the model never runs is still a declared check, and
/// shows as not run with the file it asserts.
#[test]
fn an_unrun_file_assertion_probe_is_reported_as_not_run() {
    let workspace = Workspace::new();
    workspace.write("out/greeting.txt", "Hi there\n");
    let contract = workspace.base.join("contract.json");
    std::fs::write(
        &contract,
        json!({
            "profile": {"id": "terminal_work", "version": "terminal_work_profile.v1"},
            "guidance": "Run the checks, then finish.",
            "read_scope": ["out", "out/**"],
            "write_scope": ["out", "out/**"],
            "probes": [
                {"id": "test", "command": "true"},
                {"id": "greeting", "assertion": {
                    "kind": "file_text_equals",
                    "path": "out/greeting.txt",
                    "expected": "Hello there\n",
                }},
            ],
        })
        .to_string(),
    )
    .expect("writing the contract");
    let provider = || {
        ScriptedGateway::turns(
            Wire::Omlx,
            vec![
                vec![tool("execute_probe", json!({"probe_id": "test"}))],
                vec![content("DONE")],
            ],
        )
    };
    let experiment = workspace.base.display().to_string();
    let traces = workspace.base.join("traces").display().to_string();
    let contract = contract.display().to_string();
    let args = [
        "run",
        "--experiment",
        &experiment,
        "--trace-dir",
        &traces,
        "--provider",
        "omlx",
        "--model",
        "test-model",
        "--contract",
        &contract,
        "--max-iterations",
        "1",
    ];
    let mut json_args = args.to_vec();
    json_args.push("--json");
    let scripted = provider();
    let output = execute(microgentic(), &json_args, &scripted, &workspace);
    assert_eq!(output.status.code(), Some(4), "{}", stderr(&output));
    assert_eq!(
        summary(&output)["checks"]["declared"],
        json!([
            {"probe_id": "test", "command": "true", "status": "passed", "exit_code": 0},
            {"probe_id": "greeting", "command": "file_text_equals out/greeting.txt", "status": "not_run", "exit_code": null},
        ])
    );

    let scripted = provider();
    let output = execute(microgentic(), &args, &scripted, &workspace);
    assert_eq!(
        report(&output).first().map(String::as_str),
        Some("microgentic: checks: probe `test` passed; probe `greeting` not run"),
        "{}",
        stderr(&output)
    );
}
