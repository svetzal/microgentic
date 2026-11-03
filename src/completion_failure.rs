//! Actionable presentation of provider output limits, without changing stop policy.
use mojentic::error::MojenticError;
use mojentic::llm::recovery::{CompletionDecodeError, Outcome};
use serde::Serialize;
use std::{error::Error, fmt};

#[derive(Debug, Serialize)]
pub(crate) struct OutputLimit {
    pub(crate) generated_tokens: Option<u64>,
    pub(crate) configured_max_tokens: Option<usize>,
    pub(crate) reasoning_bytes: u64,
    pub(crate) content_bytes: u64,
    pub(crate) tool_fragments: u64,
    pub(crate) completed_tool_calls: u64,
}

fn is_length(cause: &(dyn Error + 'static)) -> bool {
    if let Some(decode) = cause.downcast_ref::<CompletionDecodeError>() {
        return *decode == CompletionDecodeError::Length;
    }
    // IO wrappers can retain a concrete decoding cause in gateway doubles and
    // embedded transports. Never classify failures by their display strings.
    if let Some(io) = cause.downcast_ref::<std::io::Error>()
        && let Some(inner) = io.get_ref()
    {
        return is_length(inner);
    }
    cause.source().is_some_and(is_length)
}

impl OutputLimit {
    pub(crate) fn from_error(error: &MojenticError, max_tokens: Option<usize>) -> Option<Self> {
        let MojenticError::Recovery(recovery) = error else {
            return None;
        };
        if recovery.outcome == Outcome::Cancelled
            || !recovery
                .failure
                .inspect_cause()
                .is_some_and(|cause| is_length(cause))
        {
            return None;
        }
        let observed = &recovery.failure.progress.observed;
        Some(Self {
            generated_tokens: recovery.stream_metrics.as_ref().and_then(|m| m.eval_count),
            configured_max_tokens: max_tokens,
            reasoning_bytes: observed.reasoning_bytes,
            content_bytes: observed.content_bytes,
            tool_fragments: observed.tool_fragments,
            completed_tool_calls: observed.completed_tool_calls,
        })
    }

    fn reasoning_only(&self) -> bool {
        self.reasoning_bytes > 0
            && self.content_bytes == 0
            && self.tool_fragments == 0
            && self.completed_tool_calls == 0
    }

    pub(crate) fn message(&self, conversation: bool) -> String {
        let mut message = self.to_string();
        message.push_str(
            "\nThe unfinished reply was not retried automatically. Unfinished tool calls were not executed.",
        );
        if conversation {
            message.push_str("\nAsk to continue with a smaller step; completed conversation history is retained. A follow-up starts a fresh request, not a continuation of the interrupted stream.");
            if self.reasoning_bytes > 0 {
                message.push_str("\nTo reduce thinking for the next request, try /effort low or /effort disabled (model support varies). Your conversation history is kept.");
            }
        } else {
            message.push_str("\nTry a smaller task, or rerun with a larger --max-tokens limit.");
            if self.reasoning_bytes > 0 {
                message.push_str(" To reduce thinking, try --reasoning-effort low or --reasoning-effort disabled (model support varies).");
            }
        }
        message
    }
}

impl fmt::Display for OutputLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "The model reached its output token limit before completing the reply."
        )?;
        match (self.generated_tokens, self.configured_max_tokens) {
            (Some(generated), Some(limit)) => write!(
                f,
                " Generated {generated} tokens; configured limit: {limit} (including reasoning)."
            )?,
            (Some(generated), None) => {
                write!(f, " Generated {generated} tokens (including reasoning).")?
            }
            (None, Some(limit)) => write!(
                f,
                " Configured limit: {limit} tokens (including reasoning)."
            )?,
            (None, None) => {}
        }
        if self.reasoning_only() {
            write!(
                f,
                " All observed output was reasoning; no answer text or tool calls were produced."
            )?;
        } else {
            write!(f, " Any partial reply is incomplete.")?;
        }
        Ok(())
    }
}

/// Preserve the original typed error for callers and traces, while presenting
/// the useful diagnosis without the recovery implementation's generic label.
pub(crate) fn message(error: &anyhow::Error, conversation: bool) -> String {
    error.downcast_ref::<OutputLimit>().map_or_else(
        || {
            if conversation {
                format!("{error:#}")
            } else {
                format!("{error:?}")
            }
        },
        |limit| limit.message(conversation),
    )
}
