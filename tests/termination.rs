//! Real CLI signal dispatch with a socket-free gateway and pipe handshakes.
#![cfg(all(
    unix,
    feature = "termination-test-provider",
    microgentic_termination_test_build
))]

#[path = "support/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;
use fixtures::Workspace;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, ExitStatus, Stdio};

struct Run {
    child: Child,
    events: BufReader<File>,
    control: File,
    workspace: Workspace,
}

impl Run {
    fn start(provider: &str, ignore_hup: bool, project_command: &str) -> Self {
        let workspace = Workspace::new();
        for name in ["events", "control", "command-events", "command-control"] {
            assert!(
                Command::new("mkfifo")
                    .arg(workspace.base.join(name))
                    .status()
                    .unwrap()
                    .success()
            );
        }
        std::fs::write(workspace.base.join("command"), project_command).unwrap();
        let control = OpenOptions::new()
            .read(true)
            .write(true)
            .open(workspace.base.join("control"))
            .unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_microgentic"));
        command
            .arg("--config")
            .arg(&workspace.config)
            .args([
                "run",
                "--provider",
                provider,
                "--model",
                "test-model",
                "--context-window-tokens",
                "32768",
                "--prompt",
                "Run the requested work.",
            ])
            .current_dir(&workspace.project)
            .env("MICROGENTIC_TERMINATION_SCRIPT", &workspace.base)
            .env("XDG_STATE_HOME", &workspace.state)
            .env("XDG_CONFIG_HOME", workspace.base.join("config"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        if ignore_hup {
            // SAFETY: only the child's disposition table is changed before
            // exec, as nohup does; signal performs no allocation.
            unsafe {
                command.pre_exec(|| {
                    libc::signal(libc::SIGHUP, libc::SIG_IGN);
                    Ok(())
                });
            }
        }
        let child = command.spawn().unwrap();
        let events = BufReader::new(File::open(workspace.base.join("events")).unwrap());
        Self {
            child,
            events,
            control,
            workspace,
        }
    }

    fn event(&mut self, expected: &str) {
        let mut line = String::new();
        assert_ne!(
            self.events.read_line(&mut line).unwrap(),
            0,
            "run ended before {expected}"
        );
        assert_eq!(line.trim(), expected);
    }

    fn signal(&self, signal: i32) {
        // SAFETY: the child owns this live pid; kill touches no memory.
        assert_eq!(unsafe { libc::kill(self.child.id() as i32, signal) }, 0);
    }

    fn stop(&mut self, signal: i32) -> (ExitStatus, String) {
        self.signal(signal);
        let status = self.child.wait().unwrap();
        let mut stderr = String::new();
        self.child
            .stderr
            .take()
            .unwrap()
            .read_to_string(&mut stderr)
            .unwrap();
        (status, stderr)
    }
}

#[test]
fn run_started_under_nohup_keeps_ignoring_sighup() {
    let mut run = Run::start("omlx", true, "");
    run.event("stream-ready");
    run.signal(libc::SIGHUP);
    run.control.write_all(b"+").unwrap();
    run.event("stream-advanced");
    let result = run.stop(libc::SIGTERM);
    assert_cancelled(result, libc::SIGTERM, "SIGTERM");
}

// A surviving descendant reports itself after CLI completion. This release
// handshake makes a missing group kill fail without polling or a timeout.
fn command_cancellation(provider: &str) {
    let command = r#"exec 3>"$MICROGENTIC_TERMINATION_SCRIPT/command-events"
sh -c 'printf "descendant-started\n" >&3
read -r release < "$MICROGENTIC_TERMINATION_SCRIPT/command-control"
printf "survived\n" >&3' &
wait"#;
    let mut run = Run::start(provider, false, command);
    let command_control = run.workspace.base.join("command-control");
    let mut release = OpenOptions::new()
        .read(true)
        .write(true)
        .open(command_control)
        .unwrap();
    let mut events = BufReader::new(File::open(run.workspace.base.join("command-events")).unwrap());
    let mut line = String::new();
    events.read_line(&mut line).unwrap();
    assert_eq!(line, "descendant-started\n");
    let result = run.stop(libc::SIGTERM);
    release.write_all(b"release\n").unwrap();
    let mut remaining = String::new();
    events.read_to_string(&mut remaining).unwrap();
    assert!(
        remaining.is_empty(),
        "a project process survived cancellation: {remaining}"
    );
    assert_cancelled(result, libc::SIGTERM, "SIGTERM");
}

#[test]
fn run_stops_on_sigterm_while_a_command_runs_omlx() {
    command_cancellation("omlx");
}

fn assert_cancelled((status, stderr): (ExitStatus, String), signal: i32, name: &str) {
    assert_eq!(status.code(), Some(128 + signal), "{stderr}");
    assert!(stderr.contains(&format!("cancelled by {name}")), "{stderr}");
}

impl Drop for Run {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn model_cancellation(provider: &str, signal: i32, name: &str) {
    let mut run = Run::start(provider, false, "");
    run.event("stream-ready");
    let result = run.stop(signal);
    assert_cancelled(result, signal, name);
}

#[test]
fn run_stops_on_sigterm_while_the_model_is_answering_omlx() {
    model_cancellation("omlx", libc::SIGTERM, "SIGTERM");
}

#[test]
fn run_stops_on_sigterm_while_the_model_is_answering_ollama() {
    model_cancellation("ollama", libc::SIGTERM, "SIGTERM");
}

#[test]
fn run_stops_on_sighup_while_the_model_is_answering() {
    model_cancellation("omlx", libc::SIGHUP, "SIGHUP");
}

#[test]
fn run_stops_on_sigterm_while_a_command_runs_ollama() {
    command_cancellation("ollama");
}
