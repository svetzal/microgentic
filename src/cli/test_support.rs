//! Shared in-process CLI driver with a task-scoped gateway double.
use super::*;
use crate::outcome_test_io::{ACTIVE, OutcomeIo};
use mojentic::llm::{gateway::StreamChunk, models::LlmToolCall};
use serde_json::Value;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};
#[path = "../../tests/support/fixtures.rs"]
#[allow(dead_code)]
mod fixtures;
pub(super) use fixtures::*;
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Wire {
    Omlx,
    Ollama,
}
impl Wire {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Omlx => "omlx",
            Self::Ollama => "ollama",
        }
    }
}
pub(super) struct ScriptedGateway {
    wire: Wire,
    turns: Vec<Vec<StreamChunk>>,
    calls: AtomicUsize,
    requests: Mutex<Vec<Value>>,
    metadata_models: Mutex<Vec<String>>,
    metadata: Option<mojentic::llm::gateway::ModelContextWindow>,
}
impl ScriptedGateway {
    pub(super) fn turns(wire: Wire, turns: Vec<Vec<StreamChunk>>) -> Self {
        Self {
            wire,
            turns,
            calls: AtomicUsize::new(0),
            requests: Mutex::new(Vec::new()),
            metadata_models: Mutex::new(Vec::new()),
            metadata: None,
        }
    }
    pub(super) fn replying(wire: Wire, steps: Vec<StreamChunk>) -> Self {
        Self::turns(wire, vec![steps])
    }
    pub(super) fn requests(&self) -> Vec<Value> {
        self.requests.lock().unwrap().clone()
    }
    pub(super) fn metadata_models(&self) -> Vec<String> {
        self.metadata_models.lock().unwrap().clone()
    }
    pub(super) fn with_metadata(
        mut self,
        metadata: mojentic::llm::gateway::ModelContextWindow,
    ) -> Self {
        self.metadata = Some(metadata);
        self
    }
    pub(super) fn request_count(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
    pub(super) fn wire(&self) -> Wire {
        self.wire
    }
}
pub(super) fn content(text: &str) -> StreamChunk {
    StreamChunk::Content(text.into())
}
pub(super) fn tool(name: &str, arguments: Value) -> StreamChunk {
    StreamChunk::ToolCalls(vec![LlmToolCall {
        id: Some(format!("call-{name}")),
        name: name.into(),
        arguments: serde_json::from_value(arguments).unwrap(),
    }])
}
pub(super) fn microgentic() -> &'static str {
    "microgentic"
}
pub(super) fn legacy_binary() -> &'static str {
    "adaptive-agent-harness"
}
pub(super) struct Output {
    pub(super) status: Status,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}
pub(super) struct Status(ExitCode);
impl Status {
    pub(super) fn success(&self) -> bool {
        self.0 == ExitCode::SUCCESS
    }
    pub(super) fn code(&self) -> Option<i32> {
        (0..=255u8)
            .find(|n| self.0 == ExitCode::from(*n))
            .map(i32::from)
    }
}
pub(super) fn execute(
    binary: &str,
    args: &[&str],
    provider: &ScriptedGateway,
    workspace: &Workspace,
) -> Output {
    execute_input(binary, args, provider, workspace, b"")
}

pub(super) fn execute_input(
    binary: &str,
    args: &[&str],
    provider: &ScriptedGateway,
    workspace: &Workspace,
    stdin: &[u8],
) -> Output {
    let mut argv = vec![binary];
    if !args.contains(&"--config") {
        argv.extend(["--config", workspace.config.to_str().unwrap()]);
    }
    argv.extend_from_slice(args);
    if !args.contains(&"--dir") {
        argv.extend(["--dir", workspace.project.to_str().unwrap()]);
    }
    let cli =
        Cli::from_arg_matches(&command_definition().try_get_matches_from(argv).unwrap()).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    // State is scoped to this call, just like its gateway and stdin.
    let io = Arc::new(OutcomeIo {
        state: workspace.state.clone(),
        tool_root: workspace.project.clone(),
        streams: Mutex::new(provider.turns.clone().into()),
        calls: AtomicUsize::new(0),
        discoveries: AtomicUsize::new(0),
        stdin: (!stdin.is_empty()).then(|| String::from_utf8(stdin.to_vec()).unwrap()),
        metadata: provider.metadata.clone(),
        requests: Mutex::new(Vec::new()),
        metadata_models: Mutex::new(Vec::new()),
    });
    let everyday = binary != legacy_binary()
        && matches!(&cli.command, Some(Command::Run(run)) if run.experiment.is_none() && run.contract.is_none());
    let code = runtime
        .block_on(ACTIVE.scope(
            io.clone(),
            dispatch_cli(cli, binary == legacy_binary(), &mut stdout, &mut stderr),
        ))
        .unwrap();
    if everyday {
        assert_eq!(
            io.discoveries.load(Ordering::SeqCst),
            1,
            "everyday metadata lookup uses the in-process gateway"
        );
    }
    provider
        .calls
        .fetch_add(io.calls.load(Ordering::SeqCst), Ordering::SeqCst);
    provider
        .requests
        .lock()
        .unwrap()
        .extend(io.requests.lock().unwrap().iter().cloned());
    provider
        .metadata_models
        .lock()
        .unwrap()
        .extend(io.metadata_models.lock().unwrap().iter().cloned());
    if let Ok(summary) = serde_json::from_slice::<Value>(&stdout) {
        assert_eq!(summary["provider"], provider.wire.name());
    }
    Output {
        status: Status(code),
        stdout,
        stderr,
    }
}
pub(super) fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}

pub(super) fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

pub(super) fn summary(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "the summary is JSON: {error}\n{}\n{}",
            stdout(output),
            stderr(output)
        )
    })
}
