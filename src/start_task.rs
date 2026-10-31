//! `start_task`: how the conversation hands engineering work to the
//! engineering subagent.
//!
//! The coordinator fills in a brief — goal, context, constraints and checks.
//! The fields help it think the work through; the subagent receives a run
//! contract whose guidance is the brief rendered as plain text and whose
//! probes are the checks. The subagent is the task-mode loop benchmark runs
//! use, and its outcome, changes and checks come back as the tool result.
use crate::agent::{AgentRunConfig, AgentRunSummary};
use crate::outcome::RunOutcome;
use crate::trace::TraceRecorder;
use anyhow::Context;
use mojentic::llm::LlmGateway;
use mojentic::llm::tools::{FunctionDescriptor, LlmTool, ToolDescriptor, ToolRunCtx};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio_util::sync::CancellationToken;

/// Whether a run offers `start_task`, and how a call proceeds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StartTaskMode {
    /// Not offered. Task mode, benchmark runs, the compatibility executable
    /// and (for now) the TUI conversation.
    #[default]
    Off,
    /// Offered; a call starts the subagent at once. Everyday `run`, where no
    /// one is present to review the brief first.
    Immediate,
}

/// System guidance added wherever the tool is offered, so runs without it
/// are unchanged.
pub const GUIDANCE: &str = "Engineering work that changes the project goes through start_task. Give it a clear goal, the relevant context from this conversation, any constraints, and the commands that show the work is done. Then report the result to the user.";

pub(crate) const NAME: &str = "start_task";

/// What the coordinator asked for, as it asked for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Brief {
    pub(crate) goal: String,
    pub(crate) context: String,
    pub(crate) constraints: String,
    pub(crate) checks: Vec<String>,
}

impl Brief {
    /// Read the tool arguments. Each error says what to send instead.
    pub(crate) fn from_arguments(args: &HashMap<String, Value>) -> Result<Self, String> {
        let text = |name: &str| -> Result<String, String> {
            match args.get(name) {
                None | Some(Value::Null) => Ok(String::new()),
                Some(Value::String(text)) => Ok(text.trim().to_owned()),
                Some(_) => Err(format!("\"{name}\" must be text")),
            }
        };
        let goal = text("goal")?;
        if goal.is_empty() {
            return Err(
                "give the work to do in \"goal\", such as {\"goal\": \"Add a --verbose flag to the CLI\"}"
                    .into(),
            );
        }
        let checks = match args.get("checks") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(items)) => items
                .iter()
                .map(|item| {
                    item.as_str().map(str::to_owned).ok_or_else(|| {
                        "\"checks\" must be a list of shell commands, such as [\"make test\", \"pytest\"]"
                            .to_owned()
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
            Some(_) => {
                return Err(
                    "\"checks\" must be a list of shell commands, such as [\"make test\", \"pytest\"]".into(),
                );
            }
        };
        let mut unique: Vec<String> = Vec::new();
        for (index, check) in checks.iter().enumerate() {
            let check = check.trim();
            if check.is_empty() {
                return Err(format!(
                    "check {} is empty; give a shell command such as \"make test\" or \"pytest\", or leave it out",
                    index + 1
                ));
            }
            if crate::agent::validation_command_masks_failure(check) {
                return Err(format!(
                    "check {check:?} hides its own failure (with `|| true`, `; true` or `exit 0`); give the command so that a failure shows"
                ));
            }
            if !unique.iter().any(|seen| seen == check) {
                unique.push(check.to_owned());
            }
        }
        Ok(Self {
            goal,
            context: text("context")?,
            constraints: text("constraints")?,
            checks: unique,
        })
    }

    /// The brief as the subagent reads it: the goal, then each section that
    /// has content.
    pub(crate) fn render(&self) -> String {
        let mut text = self.goal.clone();
        for (heading, body) in [
            ("Context:", &self.context),
            ("Constraints:", &self.constraints),
        ] {
            if !body.is_empty() {
                text.push_str(&format!("\n\n{heading}\n{body}"));
            }
        }
        if !self.checks.is_empty() {
            text.push_str("\n\nDone when these checks pass:");
            for check in &self.checks {
                text.push_str(&format!("\n- {check}"));
            }
        }
        text
    }

    /// The run contract: rendered guidance, and one probe per check.
    pub(crate) fn contract(&self) -> Value {
        json!({
            "guidance": self.render(),
            "probes": crate::profile::coding::synthesize_probe_ids(&self.checks),
        })
    }
}

/// The subagent's configuration: the coordinator's provider, model,
/// reasoning settings, budgets, guidance and context window, in task mode
/// and without `start_task`.
pub(crate) fn subagent_config(
    coordinator: &AgentRunConfig,
    context_window: Option<crate::context_window::ContextWindow>,
    task_dir: &Path,
    contract_file: &Path,
) -> AgentRunConfig {
    AgentRunConfig {
        experiment_dir: task_dir.to_path_buf(),
        // The subagent's trace goes in its own task directory.
        trace_dir: None,
        goal_file: contract_file.to_path_buf(),
        contract_file: Some(contract_file.to_path_buf()),
        conversational: false,
        calculator: false,
        start_task: StartTaskMode::Off,
        context_window,
        ..coordinator.clone()
    }
}

/// Everything a `start_task` call needs from the coordinator's run.
pub(crate) struct Subagent {
    /// The coordinator's configuration, from which each subagent's derives.
    pub(crate) coordinator: AgentRunConfig,
    /// The coordinator's resolved context window.
    pub(crate) context_window: Option<crate::context_window::ContextWindow>,
    /// The coordinator's gateway, shared so the subagent talks to the same
    /// provider through the same recovery and cancellation.
    pub(crate) gateway: Arc<dyn LlmGateway>,
    pub(crate) tool_root: PathBuf,
    pub(crate) evidence: Option<Arc<crate::recovery::Evidence>>,
    pub(crate) cancellation: CancellationToken,
    pub(crate) trace: Arc<TraceRecorder>,
    /// The coordinator's experiment directory, where task directories go.
    pub(crate) state_dir: PathBuf,
    pub(crate) started: AtomicUsize,
}

#[derive(Clone)]
pub(crate) struct StartTaskTool {
    subagent: Arc<Subagent>,
}

impl StartTaskTool {
    pub(crate) fn new(subagent: Subagent) -> Self {
        Self {
            subagent: Arc::new(subagent),
        }
    }
}

impl Subagent {
    async fn start(&self, brief: Brief) -> anyhow::Result<Value> {
        let contract = brief.contract();
        let contract_text = serde_json::to_string_pretty(&contract)?;
        // The explicit-contract path validates it before anything runs.
        crate::contract::resolve_contract(
            crate::contract::ContractSource::Explicit {
                source_path: None,
                json_text: contract_text.clone(),
            },
            crate::contract::Budgets::default(),
        )?;
        let number = self.started.fetch_add(1, Ordering::SeqCst) + 1;
        let task_dir = self.state_dir.join(format!("task-{number}"));
        std::fs::create_dir_all(&task_dir)
            .with_context(|| format!("creating task directory {}", task_dir.display()))?;
        let contract_file = task_dir.join("contract.json");
        std::fs::write(&contract_file, &contract_text)
            .with_context(|| format!("writing {}", contract_file.display()))?;
        let config = subagent_config(
            &self.coordinator,
            self.context_window.clone(),
            &task_dir,
            &contract_file,
        );
        let trace_dir = task_dir.join("traces");
        self.trace.event(
            crate::runtime_events::SUBAGENT_TASK_STARTED,
            json!({
                "task": number,
                "goal": brief.goal,
                "context": brief.context,
                "constraints": brief.constraints,
                "checks": brief.checks,
                "guidance": contract["guidance"],
                "contract_file": contract_file,
                "trace_dir": trace_dir,
            }),
        )?;
        // Streamed model text may have left standard error mid-line.
        crate::trace::end_open_stderr_line();
        let result = crate::agent::run_subagent(
            config,
            self.gateway.as_ref(),
            self.tool_root.clone(),
            self.evidence.clone(),
            self.cancellation.clone(),
        )
        .await;
        crate::trace::end_open_stderr_line();
        // Recovery evidence returns to the coordinator's trace.
        if let Some(evidence) = &self.evidence {
            evidence.bind(self.trace.clone());
        }
        let (value, outcome, trace_file) = match result {
            Ok(summary) => {
                let value = task_result(&summary);
                (value, summary.outcome, Some(summary.trace_file))
            }
            Err(error) => {
                let outcome = error
                    .downcast_ref::<RunOutcome>()
                    .cloned()
                    .unwrap_or_else(|| RunOutcome::Error {
                        message: format!("{error:#}"),
                    });
                let trace_file = only_trace(&trace_dir);
                let value = json!({
                    "outcome": outcome,
                    "final_summary": "",
                    "error": format!("{error:#}"),
                    "trace_file": trace_file,
                });
                (value, outcome, trace_file)
            }
        };
        self.trace.event(
            crate::runtime_events::SUBAGENT_TASK_FINISHED,
            json!({
                "task": number,
                "outcome": outcome,
                "trace_file": trace_file,
            }),
        )?;
        Ok(value)
    }
}

/// What the coordinator learns from a finished subagent.
fn task_result(summary: &AgentRunSummary) -> Value {
    json!({
        "outcome": summary.outcome,
        "final_summary": summary.final_summary,
        "changes": summary.changes,
        "checks": summary.checks,
        "trace_file": summary.trace_file,
    })
}

/// The one trace file a failed subagent left in its trace directory.
fn only_trace(dir: &Path) -> Option<PathBuf> {
    let mut traces = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"));
    let first = traces.next()?;
    traces.next().is_none().then_some(first)
}

#[async_trait::async_trait]
impl LlmTool for StartTaskTool {
    async fn run(
        &self,
        args: &HashMap<String, Value>,
        _ctx: &ToolRunCtx,
    ) -> mojentic::Result<Value> {
        let brief = Brief::from_arguments(args).map_err(mojentic::MojenticError::ToolError)?;
        self.subagent
            .start(brief)
            .await
            .map_err(|error| mojentic::MojenticError::ToolError(format!("{error:#}")))
    }

    fn descriptor(&self) -> ToolDescriptor {
        descriptor()
    }

    fn clone_box(&self) -> Box<dyn LlmTool> {
        Box::new(self.clone())
    }
}

pub(crate) fn descriptor() -> ToolDescriptor {
    ToolDescriptor {
        r#type: "function".to_string(),
        function: FunctionDescriptor {
            name: NAME.to_string(),
            description: "Hand engineering work that changes the project to an engineering agent. It does the work now and reports back. It sees only what you write here, not this conversation, so include what it needs to know. Returns the outcome, its summary, the files it changed and the results of the checks.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "goal": {
                        "type": "string",
                        "description": "The work to do, in a sentence or two."
                    },
                    "context": {
                        "type": "string",
                        "description": "What the agent needs from this conversation: findings, decisions, relevant files."
                    },
                    "constraints": {
                        "type": "string",
                        "description": "Limits to respect, such as files to leave alone or approaches to avoid."
                    },
                    "checks": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Shell commands that must pass for the work to count as done, such as \"make test\", \"pytest\" or \"cargo test\"."
                    }
                },
                "required": ["goal"]
            }),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arguments(value: Value) -> HashMap<String, Value> {
        serde_json::from_value(value).unwrap()
    }

    fn brief(value: Value) -> Brief {
        Brief::from_arguments(&arguments(value)).unwrap()
    }

    #[test]
    fn a_goal_alone_renders_as_itself() {
        assert_eq!(
            brief(json!({"goal": "  Add a --verbose flag.  "})).render(),
            "Add a --verbose flag."
        );
    }

    #[test]
    fn every_section_renders_in_order() {
        let brief = brief(json!({
            "goal": "Add a --verbose flag.",
            "context": "The CLI is in src/cli.rs.\nIt uses clap.",
            "constraints": "Do not change the default output.",
            "checks": ["cargo test", "cargo clippy -- -D warnings"],
        }));
        assert_eq!(
            brief.render(),
            "Add a --verbose flag.\n\n\
             Context:\nThe CLI is in src/cli.rs.\nIt uses clap.\n\n\
             Constraints:\nDo not change the default output.\n\n\
             Done when these checks pass:\n- cargo test\n- cargo clippy -- -D warnings"
        );
    }

    #[test]
    fn empty_sections_are_left_out() {
        let brief = brief(json!({
            "goal": "Fix the typo.",
            "context": "   ",
            "constraints": null,
            "checks": ["make check"],
        }));
        assert_eq!(
            brief.render(),
            "Fix the typo.\n\nDone when these checks pass:\n- make check"
        );
    }

    #[test]
    fn checks_become_probes_with_synthesized_ids() {
        let contract = brief(json!({
            "goal": "Fix it.",
            "checks": ["cargo test", " cargo test ", "make check"],
        }))
        .contract();
        assert_eq!(
            contract["probes"],
            json!([
                {"id": "cargo-test", "command": "cargo test"},
                {"id": "make-check", "command": "make check"},
            ])
        );
        assert_eq!(
            contract["guidance"],
            "Fix it.\n\nDone when these checks pass:\n- cargo test\n- make check"
        );
    }

    #[test]
    fn rejections_say_what_to_send() {
        let error = |value| Brief::from_arguments(&arguments(value)).unwrap_err();
        assert!(error(json!({})).contains("\"goal\""));
        assert!(error(json!({"goal": "  "})).contains("\"goal\""));
        assert!(error(json!({"goal": "x", "checks": "cargo test"})).contains("list"));
        assert!(error(json!({"goal": "x", "checks": [1]})).contains("list"));
        assert_eq!(
            error(json!({"goal": "x", "checks": ["cargo test", " "]})),
            "check 2 is empty; give a shell command such as \"make test\" or \"pytest\", or leave it out"
        );
        assert!(
            error(json!({"goal": "x", "checks": ["cargo test || true"]}))
                .contains("hides its own failure")
        );
        assert!(error(json!({"goal": "x", "context": 3})).contains("\"context\" must be text"));
    }

    fn coordinator_config() -> AgentRunConfig {
        use crate::agent::*;
        AgentRunConfig {
            recovery: Default::default(),
            provider: Provider::Ollama,
            experiment_dir: PathBuf::from("/state/run"),
            trace_dir: None,
            goal_file: PathBuf::from("/state/run/prompt.json"),
            contract_file: Some(PathBuf::from("/state/run/prompt.json")),
            model: "coordinator-model".into(),
            max_iterations: 7,
            max_model_interactions: 75,
            context_window_tokens: Some(32_768),
            packet_type: "multi-file-patch".into(),
            expected_output_tokens: 4_096,
            max_tokens: Some(16_384),
            reasoning_effort: Some(mojentic::llm::gateway::ReasoningEffort::Low),
            reasoning_profile: None,
            preserve_reasoning: false,
            max_thinking_only_tokens: 0,
            repair_exit_thinking_tokens: 0,
            repair_handoff_policy: RepairHandoffPolicy::TextOnly,
            action_boundary_interrupt_tokens: 0,
            reasoning_checkpoint_tokens: 0,
            transcript_policy: TranscriptPolicy::SummarizedTranscript,
            initial_context_catalog_file: None,
            semantic_advisor_model: None,
            acceptance_ledger: false,
            conversational: false,
            context_window: None,
            standing_context_limit: Some(0.2),
            guidance_fraction: None,
            calculator: false,
            project_guidance: false,
            user_guidance_file: None,
            start_task: StartTaskMode::Off,
        }
    }

    #[test]
    fn the_subagent_is_task_mode_without_start_task() {
        let mut coordinator = coordinator_config();
        coordinator.conversational = true;
        coordinator.calculator = true;
        coordinator.project_guidance = true;
        coordinator.user_guidance_file = Some(PathBuf::from("/config/AGENTS.md"));
        coordinator.start_task = StartTaskMode::Immediate;
        coordinator.trace_dir = Some(PathBuf::from("/elsewhere"));
        let window = crate::context_window::ContextWindow::configured(Some(4096), None);
        let child = subagent_config(
            &coordinator,
            Some(window.clone()),
            Path::new("/state/run/task-1"),
            Path::new("/state/run/task-1/contract.json"),
        );
        assert!(!child.conversational);
        assert!(!child.calculator);
        assert_eq!(child.start_task, StartTaskMode::Off);
        assert!(child.project_guidance);
        assert_eq!(
            child.user_guidance_file,
            Some(PathBuf::from("/config/AGENTS.md"))
        );
        assert_eq!(child.context_window, Some(window));
        assert_eq!(child.experiment_dir, PathBuf::from("/state/run/task-1"));
        assert_eq!(child.trace_dir, None);
        assert_eq!(
            child.contract_file,
            Some(PathBuf::from("/state/run/task-1/contract.json"))
        );
        assert_eq!(child.model, coordinator.model);
        assert_eq!(child.provider, coordinator.provider);
        assert_eq!(child.max_iterations, coordinator.max_iterations);
        assert_eq!(child.reasoning_effort, coordinator.reasoning_effort);
        assert_eq!(child.max_tokens, coordinator.max_tokens);
        assert_eq!(
            child.standing_context_limit,
            coordinator.standing_context_limit
        );
    }
}
