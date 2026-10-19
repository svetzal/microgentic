use std::process::{Command, Stdio};

fn worker() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_microgentic"));
    command.stdin(Stdio::null());
    command
}

#[test]
fn missing_or_invalid_intake_never_starts_a_provider() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("config.json");
    for args in [
        vec![],
        vec!["run"],
        vec!["run", "--prompt", ""],
        vec!["run", "positional"],
        vec!["run", "--prompt", "one", "--prompt", "two"],
    ] {
        let output = worker()
            .arg("--config")
            .arg(&config)
            .args(args)
            .env("OMLX_HOST", "http://127.0.0.1:1")
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("Connection refused"));
    }
    assert!(!config.exists());
}

/// Bad input to `run` is a usage error, status 2, whether clap or the
/// intake itself finds it.
#[test]
fn usage_errors_exit_two() {
    use std::io::Write;
    let temp = tempfile::tempdir().unwrap();
    let file = temp.path().join("not-a-directory");
    std::fs::write(&file, "x").unwrap();
    let file = file.display().to_string();
    for (args, stdin, message) in [
        // Nothing on a closed stdin and no --prompt.
        (vec!["run"], "", "run requires a nonempty prompt"),
        (
            vec!["run", "--prompt", ""],
            "",
            "--prompt must not be empty",
        ),
        // Blank stdin is no prompt either.
        (vec!["run"], " \n\n", "run requires a nonempty prompt"),
        (
            vec!["run", "--prompt", "Hi", "--dir", &file],
            "",
            "--dir must name a project directory",
        ),
        (vec!["run", "positional"], "", "unexpected argument"),
    ] {
        let mut child = Command::new(env!("CARGO_BIN_EXE_microgentic"))
            .arg("--config")
            .arg(temp.path().join("config.json"))
            .args(&args)
            .env("OMLX_HOST", "http://127.0.0.1:1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(stdin.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {stderr}");
        assert!(stderr.contains(message), "{args:?}: {stderr}");
        assert!(output.stdout.is_empty());
    }
}

#[test]
fn benchmark_help_and_errors_explain_isolated_file_intake() {
    let help = worker()
        .args(["benchmark", "run", "--help"])
        .output()
        .unwrap();
    assert!(help.status.success());
    let text = String::from_utf8(help.stdout).unwrap();
    assert!(text.contains("--experiment <EXPERIMENT>"));
    assert!(text.contains("--goal <GOAL>"));
    assert!(text.contains("stdin, --config, and TUI history are ignored"));
    assert!(!text.contains("echo 'Hello!'"));
    let temp = tempfile::tempdir().unwrap();
    for args in [
        vec!["benchmark", "run"],
        vec!["benchmark", "run", "--experiment", ".", "--prompt", "hello"],
    ] {
        let output = worker()
            .current_dir(temp.path())
            .args(args)
            .env("OMLX_HOST", "http://127.0.0.1:1")
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!String::from_utf8_lossy(&output.stderr).contains("Connection refused"));
        assert!(!temp.path().join("traces").exists());
    }
}

#[test]
fn setup_previews_before_saving_and_persists_model_default_effort() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("nested/config.json");
    let args = [
        "setup",
        "--provider",
        "ollama",
        "--model",
        "test-model",
        "--reasoning-effort",
        "default",
        "--json",
    ];
    let preview = worker()
        .arg("--config")
        .arg(&config)
        .args(args)
        .output()
        .unwrap();
    assert!(preview.status.success());
    assert!(!config.exists());
    let save = worker()
        .arg("--config")
        .arg(&config)
        .args(args)
        .arg("--apply")
        .output()
        .unwrap();
    assert!(save.status.success());
    let saved: serde_json::Value = serde_json::from_slice(&std::fs::read(config).unwrap()).unwrap();
    assert!(saved["reasoning_effort"].is_null());
    assert_eq!(saved["model"], "test-model");
}

#[test]
fn prompt_must_match_an_authoritative_contract_before_a_run_is_created() {
    let temp = tempfile::tempdir().unwrap();
    let contract = temp.path().join("contract.json");
    std::fs::write(&contract, r#"{"guidance":"original prompt","probes":[]}"#).unwrap();
    let traces = temp.path().join("traces");
    let output = worker()
        .args(["run", "--prompt", "changed prompt", "--experiment"])
        .arg(temp.path())
        .arg("--contract")
        .arg(&contract)
        .arg("--trace-dir")
        .arg(&traces)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("must exactly match"));
    assert!(!traces.exists());
}

/// Without a terminal there is no conversation to hold: the binary says so,
/// points at `run`, and exits instead of waiting on input that cannot come.
#[test]
fn interactive_mode_without_a_terminal_exits_at_once_and_points_at_run() {
    let temp = tempfile::tempdir().unwrap();
    let output = worker()
        .arg("--config")
        .arg(temp.path().join("config.json"))
        .env("OMLX_HOST", "http://127.0.0.1:1")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("microgentic run"), "{stderr}");
}

#[test]
fn recovery_help_explains_wire_attempts_and_examples() {
    let output = worker().args(["run", "--help"]).output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("Wire attempts"));
    assert!(help.contains("Examples:"));
}
