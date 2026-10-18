use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{Value, json};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct TraceRecorder {
    path: PathBuf,
    file: Mutex<File>,
    conversation: Option<Arc<crate::conversation::ConversationRun>>,
    /// Ask for natural replies instead of DONE/FAIL tokens. True for the TUI
    /// conversation and everyday `run`.
    natural_replies: bool,
    /// Context residency recording; everyday runs and the conversation only.
    residency: Option<crate::residency::Residency>,
}

impl TraceRecorder {
    pub fn create(dir: &Path) -> Result<Self> {
        Self::create_for_conversation(dir, None, false)
    }

    pub(crate) fn create_for_conversation(
        dir: &Path,
        conversation: Option<Arc<crate::conversation::ConversationRun>>,
        natural_replies: bool,
    ) -> Result<Self> {
        std::fs::create_dir_all(dir)
            .with_context(|| format!("creating trace directory {}", dir.display()))?;
        let (path, file) = create_unique_trace(dir, Utc::now())?;
        Ok(Self {
            path,
            file: Mutex::new(file),
            conversation,
            natural_replies,
            residency: None,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn event(&self, kind: &str, payload: impl Serialize) -> Result<()> {
        let payload = serde_json::to_value(payload)?;
        self.value_event(kind, payload)
    }

    pub fn value_event(&self, kind: &str, mut payload: Value) -> Result<()> {
        if kind == "llm.context_assembly.ledger"
            && let Some(conversation) = &self.conversation
        {
            conversation.label_ledger(&mut payload);
        }
        let credential_redacted =
            crate::recovery::redact_value(&mut payload, &crate::recovery::credentials());
        let record = TraceEvent {
            timestamp: Utc::now(),
            kind,
            payload,
            credential_redacted: credential_redacted.then_some(true),
            exact: credential_redacted.then_some(false),
        };
        {
            let mut file = self.file.lock().expect("trace mutex poisoned");
            serde_json::to_writer(&mut *file, &record)?;
            file.write_all(b"\n")?;
            file.flush()?;
        }
        if let Some(conversation) = &self.conversation {
            conversation.observe(kind, &record.payload);
        }
        // Today's compaction passes, seen as residency demotions.
        if let Some(residency) = &self.residency {
            for (kind, payload) in residency.observe(kind, &record.payload) {
                self.value_event(kind, payload)?;
            }
        }
        Ok(())
    }

    pub(crate) fn conversation_checkpoint(&self, messages: &[mojentic::llm::models::LlmMessage]) {
        if let Some(conversation) = &self.conversation {
            conversation.checkpoint(messages);
        }
    }

    /// Stream the model's thinking to an open conversation view. Traces keep
    /// their own bounded previews; this does not change them.
    pub(crate) fn stream_thinking(&self, text: &str) {
        if let Some(conversation) = &self.conversation {
            conversation
                .emit(crate::conversation::ConversationEvent::ThinkingDelta { text: text.into() });
        }
    }

    /// Report a requested tool call to an open conversation view.
    pub(crate) fn tool_requested(&self, call: &mojentic::llm::models::LlmToolCall) {
        if let Some(conversation) = &self.conversation {
            conversation.tool_requested(call);
        }
    }

    /// Report a finished tool call, with the start of its result, to an open
    /// conversation view.
    pub(crate) fn tool_finished(
        &self,
        call: &mojentic::llm::models::LlmToolCall,
        ok: bool,
        content: &str,
    ) {
        if let Some(conversation) = &self.conversation {
            conversation.tool_finished(call, ok, content);
        }
    }

    /// Record context residency in this trace.
    pub(crate) fn with_residency(mut self, residency: crate::residency::Residency) -> Self {
        self.residency = Some(residency);
        self
    }

    pub(crate) fn residency(&self) -> Option<&crate::residency::Residency> {
        self.residency.as_ref()
    }

    pub(crate) fn natural_replies(&self) -> bool {
        self.natural_replies
    }

    pub(crate) fn reply_started(&self) {
        if let Some(conversation) = &self.conversation {
            conversation.emit(crate::conversation::ConversationEvent::ReplyStarted);
        }
    }

    pub(crate) fn stream_content(&self, text: &str) {
        if let Some(conversation) = &self.conversation {
            conversation
                .emit(crate::conversation::ConversationEvent::Content { text: text.into() });
        } else {
            eprint!("{text}");
            if let Some(last) = text.chars().last() {
                STDERR_MID_LINE.store(last != '\n', Ordering::Relaxed);
            }
        }
    }
}

/// How many numbered alternatives to try when runs start at the same instant.
const MAX_TRACE_NAME_ATTEMPTS: usize = 1000;

/// The trace file name for a run that started at `started`. The timestamp
/// has nanosecond precision so names sort in start order, which transcripts
/// rely on. `attempt` 0 is the plain name; later attempts add `_01`, `_02`,
/// and so on, which sort after the plain name and in order.
fn trace_file_name(started: DateTime<Utc>, attempt: usize) -> String {
    let stamp = started.format("%Y%m%dT%H%M%S%.9fZ");
    match attempt {
        0 => format!("run-{stamp}.jsonl"),
        n => format!("run-{stamp}_{n:02}.jsonl"),
    }
}

/// Create a new trace file in `dir` without overwriting one that exists.
/// Each name is claimed with an exclusive create, so runs that start at the
/// same instant, in this process or another, each get their own file and
/// never wait.
fn create_unique_trace(dir: &Path, started: DateTime<Utc>) -> Result<(PathBuf, File)> {
    for attempt in 0..MAX_TRACE_NAME_ATTEMPTS {
        let path = dir.join(trace_file_name(started, attempt));
        match OpenOptions::new().create_new(true).append(true).open(&path) {
            Ok(file) => return Ok((path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("creating trace file {}", path.display()));
            }
        }
    }
    anyhow::bail!(
        "creating a trace file in {}: {MAX_TRACE_NAME_ATTEMPTS} names for {} already exist",
        dir.display(),
        started.format("%Y%m%dT%H%M%S%.9fZ")
    )
}

/// Whether streamed model text has left standard error in the middle of a
/// line. Standard error is one process-wide stream, so this is too.
static STDERR_MID_LINE: AtomicBool = AtomicBool::new(false);

/// End a streamed line left open on standard error, so a message that
/// follows starts on its own line.
pub(crate) fn end_open_stderr_line() {
    if STDERR_MID_LINE.swap(false, Ordering::Relaxed) {
        eprintln!();
    }
}

#[derive(Debug, Serialize)]
struct TraceEvent<'a> {
    timestamp: DateTime<Utc>,
    kind: &'a str,
    payload: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    credential_redacted: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exact: Option<bool>,
}

pub fn error_payload(error: &anyhow::Error) -> Value {
    json!({ "error": error.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::collections::BTreeSet;

    fn instant() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 10, 17, 9, 30, 5).unwrap()
            + chrono::Duration::nanoseconds(123_456_789)
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn names_carry_nanoseconds_and_number_later_attempts() {
        assert_eq!(
            trace_file_name(instant(), 0),
            "run-20261017T093005.123456789Z.jsonl"
        );
        assert_eq!(
            trace_file_name(instant(), 1),
            "run-20261017T093005.123456789Z_01.jsonl"
        );
    }

    /// Runs that start at the same instant each get their own file, in
    /// creation order, and none waits.
    #[test]
    fn runs_at_the_same_instant_get_distinct_files_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let started = std::time::Instant::now();
        let created: Vec<PathBuf> = (0..12)
            .map(|_| create_unique_trace(dir.path(), instant()).unwrap().0)
            .collect();
        assert!(started.elapsed() < std::time::Duration::from_secs(1));
        let unique: BTreeSet<&PathBuf> = created.iter().collect();
        assert_eq!(unique.len(), 12, "{created:?}");
        // Name order is creation order, so transcripts keep run order.
        let mut sorted = created.clone();
        sorted.sort();
        assert_eq!(sorted, created);
        assert_eq!(
            names(dir.path())[11],
            "run-20261017T093005.123456789Z_11.jsonl"
        );
    }

    /// A name that is taken is never truncated or appended to.
    #[test]
    fn an_existing_trace_is_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let taken = dir.path().join(trace_file_name(instant(), 0));
        std::fs::write(&taken, "prior evidence\n").unwrap();
        let (path, mut file) = create_unique_trace(dir.path(), instant()).unwrap();
        writeln!(file, "new run").unwrap();
        assert_ne!(path, taken);
        assert_eq!(std::fs::read_to_string(&taken).unwrap(), "prior evidence\n");
    }

    /// Recorders created back to back, and from many threads at once, in
    /// one directory all succeed with their own files.
    #[test]
    fn rapid_and_concurrent_recorders_share_a_directory() {
        let dir = tempfile::tempdir().unwrap();
        let sequential: Vec<PathBuf> = (0..20)
            .map(|_| {
                TraceRecorder::create(dir.path())
                    .unwrap()
                    .path()
                    .to_path_buf()
            })
            .collect();
        let concurrent: Vec<PathBuf> = std::thread::scope(|scope| {
            let handles: Vec<_> = (0..16)
                .map(|_| {
                    scope.spawn(|| {
                        let recorder = TraceRecorder::create(dir.path()).unwrap();
                        recorder.event("run.started", json!({})).unwrap();
                        recorder.path().to_path_buf()
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect()
        });
        let all: BTreeSet<PathBuf> = sequential.iter().chain(&concurrent).cloned().collect();
        assert_eq!(all.len(), 36);
        assert_eq!(names(dir.path()).len(), 36);
        for path in &concurrent {
            let text = std::fs::read_to_string(path).unwrap();
            assert_eq!(text.lines().count(), 1, "{}: {text}", path.display());
        }
    }
}
