//! How a run ended, as one typed value.
//!
//! The runtime establishes the outcome at its boundary; the run summary
//! carries it into `--json`, the benchmark summary and the `run.finished`
//! trace event; the command line maps it to an exit status and the
//! conversation view shows it. A model claim alone never sets `Done`: the
//! runtime's gates decided it.
use serde::{Deserialize, Serialize};
use std::fmt;

/// How a run ended, as the runtime established it. A model claim alone never
/// sets Done: the runtime's gates decided it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunOutcome {
    /// An everyday reply finished: visible text with no tool calls, after the
    /// runtime's checks.
    Replied,
    /// The model said DONE and the runtime accepted it (stash, validation,
    /// probe and ledger gates passed).
    Done,
    /// The model returned FAIL. `reason` is the FAIL text (the final summary).
    Failed { reason: String },
    /// The model claimed completion, a gate withheld it, and the iteration
    /// budget ran out before it was satisfied.
    Withheld { gate: WithheldGate, detail: String },
    /// The runtime stopped the run by one of its rules.
    HardStop {
        cause: HardStopCause,
        detail: String,
    },
    /// A termination signal ended the run (SIGINT 2, SIGTERM 15, SIGHUP 1).
    Cancelled { signal: i32 },
    /// A provider, environment or internal error; the run has no summary.
    Error { message: String },
    /// Bad command-line input; nothing ran.
    Usage { message: String },
}

/// The gate that withheld a claimed completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WithheldGate {
    Stash,
    Validation,
    Probes,
    AcceptanceLedger,
}

/// The runtime rule that stopped a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HardStopCause {
    ActionBoundary,
    ThinkingOnly,
    EmptyResponse,
    RepairNoAction,
    RepairDepth,
    InspectionLoop,
    NoContentStream,
    IterationBudget,
}

impl RunOutcome {
    /// The exit status of a public `microgentic run`:
    ///
    /// | outcome                 | code             |
    /// | ----------------------- | ---------------- |
    /// | `Replied`, `Done`       | 0                |
    /// | `Error`                 | 1                |
    /// | `Usage`                 | 2                |
    /// | `Failed`                | 3                |
    /// | `Withheld`              | 4                |
    /// | `HardStop`              | 5                |
    /// | `Cancelled { signal }`  | 128 + signal     |
    ///
    /// Cancellation follows the shell convention: 130 for SIGINT, 143 for
    /// SIGTERM, 129 for SIGHUP.
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Replied | Self::Done => 0,
            Self::Error { .. } => 1,
            Self::Usage { .. } => 2,
            Self::Failed { .. } => 3,
            Self::Withheld { .. } => 4,
            Self::HardStop { .. } => 5,
            Self::Cancelled { signal } => u8::try_from(128 + signal).unwrap_or(u8::MAX),
        }
    }

    /// Whether the run finished its work: a completed reply or an accepted DONE.
    pub fn is_success(&self) -> bool {
        matches!(self, Self::Replied | Self::Done)
    }
}

impl WithheldGate {
    fn name(self) -> &'static str {
        match self {
            Self::Stash => "Git stash",
            Self::Validation => "validation",
            Self::Probes => "probe",
            Self::AcceptanceLedger => "acceptance ledger",
        }
    }
}

impl HardStopCause {
    fn name(self) -> &'static str {
        match self {
            Self::ActionBoundary => "action boundary",
            Self::ThinkingOnly => "thinking only",
            Self::EmptyResponse => "empty response",
            Self::RepairNoAction => "repair without action",
            Self::RepairDepth => "repair depth",
            Self::InspectionLoop => "inspection loop",
            Self::NoContentStream => "no-content stream",
            Self::IterationBudget => "iteration budget",
        }
    }
}

/// One line for standard error and the conversation view.
impl fmt::Display for RunOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Replied => f.write_str("reply completed"),
            Self::Done => f.write_str("DONE accepted"),
            Self::Failed { reason } => write!(f, "FAIL returned: {}", first_line(reason)),
            Self::Withheld { gate, detail } => {
                write!(
                    f,
                    "completion withheld by the {} gate: {detail}",
                    gate.name()
                )
            }
            Self::HardStop { cause, detail } => {
                write!(f, "stopped by the runtime ({}): {detail}", cause.name())
            }
            Self::Cancelled { signal } => {
                write!(f, "cancelled by {}", crate::signals::name(*signal))
            }
            Self::Error { message } => write!(f, "run error: {message}"),
            Self::Usage { message } => write!(f, "usage error: {message}"),
        }
    }
}

/// A failed run carries its outcome through `anyhow`, as the error itself
/// (a usage error) or as context on the underlying cause (a hard stop in a
/// stream, a cancellation). `anyhow::Error::downcast_ref::<RunOutcome>()`
/// finds either.
impl std::error::Error for RunOutcome {}

/// The first non-blank line of a model reply, which may open with blank
/// lines left over from thinking.
fn first_line(text: &str) -> &str {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn withheld() -> RunOutcome {
        RunOutcome::Withheld {
            gate: WithheldGate::Stash,
            detail: "new stash object IDs: [\"abc\"]".into(),
        }
    }

    #[test]
    fn exit_codes_follow_the_documented_table() {
        assert_eq!(RunOutcome::Replied.exit_code(), 0);
        assert_eq!(RunOutcome::Done.exit_code(), 0);
        assert_eq!(
            RunOutcome::Error {
                message: "x".into()
            }
            .exit_code(),
            1
        );
        assert_eq!(
            RunOutcome::Usage {
                message: "x".into()
            }
            .exit_code(),
            2
        );
        assert_eq!(
            RunOutcome::Failed {
                reason: "FAIL".into()
            }
            .exit_code(),
            3
        );
        assert_eq!(withheld().exit_code(), 4);
        assert_eq!(
            RunOutcome::HardStop {
                cause: HardStopCause::IterationBudget,
                detail: String::new(),
            }
            .exit_code(),
            5
        );
        assert_eq!(RunOutcome::Cancelled { signal: 2 }.exit_code(), 130);
        assert_eq!(RunOutcome::Cancelled { signal: 15 }.exit_code(), 143);
        assert_eq!(RunOutcome::Cancelled { signal: 1 }.exit_code(), 129);
    }

    #[test]
    fn only_a_completed_reply_or_an_accepted_done_is_a_success() {
        assert!(RunOutcome::Replied.is_success());
        assert!(RunOutcome::Done.is_success());
        assert!(!withheld().is_success());
        assert!(
            !RunOutcome::Failed {
                reason: "FAIL".into()
            }
            .is_success()
        );
        assert!(!RunOutcome::Cancelled { signal: 15 }.is_success());
    }

    #[test]
    fn display_is_one_line_per_outcome() {
        assert_eq!(RunOutcome::Replied.to_string(), "reply completed");
        assert_eq!(RunOutcome::Done.to_string(), "DONE accepted");
        assert_eq!(
            RunOutcome::Failed {
                reason: "\n\nFAIL the fixture file is missing\nmore detail".into()
            }
            .to_string(),
            "FAIL returned: FAIL the fixture file is missing"
        );
        assert_eq!(
            withheld().to_string(),
            "completion withheld by the Git stash gate: new stash object IDs: [\"abc\"]"
        );
        assert_eq!(
            RunOutcome::Withheld {
                gate: WithheldGate::Validation,
                detail: "edits were not validated before the reply".into(),
            }
            .to_string(),
            "completion withheld by the validation gate: edits were not validated before the reply"
        );
        assert_eq!(
            RunOutcome::Withheld {
                gate: WithheldGate::Probes,
                detail: "probe `build` missing".into(),
            }
            .to_string(),
            "completion withheld by the probe gate: probe `build` missing"
        );
        assert_eq!(
            RunOutcome::Withheld {
                gate: WithheldGate::AcceptanceLedger,
                detail: "x".into(),
            }
            .to_string(),
            "completion withheld by the acceptance ledger gate: x"
        );
        for (cause, name) in [
            (HardStopCause::ActionBoundary, "action boundary"),
            (HardStopCause::ThinkingOnly, "thinking only"),
            (HardStopCause::EmptyResponse, "empty response"),
            (HardStopCause::RepairNoAction, "repair without action"),
            (HardStopCause::RepairDepth, "repair depth"),
            (HardStopCause::InspectionLoop, "inspection loop"),
            (HardStopCause::NoContentStream, "no-content stream"),
            (HardStopCause::IterationBudget, "iteration budget"),
        ] {
            assert_eq!(
                RunOutcome::HardStop {
                    cause,
                    detail: "why".into()
                }
                .to_string(),
                format!("stopped by the runtime ({name}): why")
            );
        }
        assert_eq!(
            RunOutcome::Cancelled { signal: 15 }.to_string(),
            "cancelled by SIGTERM"
        );
        assert_eq!(
            RunOutcome::Cancelled { signal: 1 }.to_string(),
            "cancelled by SIGHUP"
        );
        assert_eq!(
            RunOutcome::Error {
                message: "connection refused".into()
            }
            .to_string(),
            "run error: connection refused"
        );
        assert_eq!(
            RunOutcome::Usage {
                message: "--prompt must not be empty".into()
            }
            .to_string(),
            "usage error: --prompt must not be empty"
        );
    }

    #[test]
    fn serde_round_trips_the_tagged_shape() {
        let json = serde_json::to_value(RunOutcome::Withheld {
            gate: WithheldGate::Stash,
            detail: "...".into(),
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({"kind": "withheld", "gate": "stash", "detail": "..."})
        );
        let back: RunOutcome = serde_json::from_value(json).unwrap();
        assert_eq!(
            back,
            RunOutcome::Withheld {
                gate: WithheldGate::Stash,
                detail: "...".into(),
            }
        );
        assert_eq!(
            serde_json::to_value(RunOutcome::Replied).unwrap(),
            serde_json::json!({"kind": "replied"})
        );
        assert_eq!(
            serde_json::to_value(RunOutcome::HardStop {
                cause: HardStopCause::IterationBudget,
                detail: "2 turns used".into(),
            })
            .unwrap(),
            serde_json::json!({"kind": "hard_stop", "cause": "iteration_budget", "detail": "2 turns used"})
        );
        assert_eq!(
            serde_json::to_value(RunOutcome::Cancelled { signal: 15 }).unwrap(),
            serde_json::json!({"kind": "cancelled", "signal": 15})
        );
    }

    #[test]
    fn anyhow_finds_the_outcome_as_the_error_or_as_context() {
        let usage = anyhow::Error::new(RunOutcome::Usage {
            message: "bad".into(),
        });
        assert_eq!(
            usage
                .downcast_ref::<RunOutcome>()
                .map(RunOutcome::exit_code),
            Some(2)
        );
        let stopped = anyhow::anyhow!("turn 3 looped").context(RunOutcome::HardStop {
            cause: HardStopCause::InspectionLoop,
            detail: "turn 3 looped".into(),
        });
        assert_eq!(
            stopped
                .downcast_ref::<RunOutcome>()
                .map(RunOutcome::exit_code),
            Some(5)
        );
        assert_eq!(
            stopped.to_string(),
            "stopped by the runtime (inspection loop): turn 3 looped"
        );
        assert_eq!(stopped.root_cause().to_string(), "turn 3 looped");
        let plain = anyhow::anyhow!("connection refused");
        assert!(plain.downcast_ref::<RunOutcome>().is_none());
    }
}
