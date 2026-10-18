//! What a run changed in the project and which checks passed, failed or did
//! not run.
//!
//! The runtime measures the workspace at the start and end of a run and keeps
//! two kinds of check apart: declared probes, which the executor owns and
//! which carry acceptance authority, and commands the model chose to run,
//! which are observational. This module holds the report types, the pure
//! difference and rendering logic, and one Git status capture at its edge.
//! It changes no prompt, provider request, stop rule, gate or exit status.
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// A snapshot walk stops here and reports the changes as unknown.
pub const MAX_SNAPSHOT_FILES: usize = 50_000;

const MAX_LISTED_PATHS: usize = 10;
const MAX_LISTED_COMMANDS: usize = 5;
const MAX_COMMAND_CHARS: usize = 60;

/// How one file differs between the start and the end of a run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeKind {
    Created,
    Modified,
    Deleted,
}

impl ChangeKind {
    fn name(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Modified => "modified",
            Self::Deleted => "deleted",
        }
    }
}

/// One project file the run changed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
    /// Project-relative path.
    pub path: String,
    pub change: ChangeKind,
    /// Git reported this path as changed or untracked before the run.
    pub edited_before_run: bool,
}

/// What changed in the project relative to the workspace at the start of the
/// run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceChanges {
    /// False when a snapshot could not be taken or was cut short; `files` is
    /// then empty and `note` says why.
    pub complete: bool,
    pub note: Option<String>,
    /// Changed files, sorted by path.
    pub files: Vec<FileChange>,
    /// Paths Git reported as modified, added, deleted, renamed or untracked
    /// at the start of the run, sorted; `None` outside a Git repository or
    /// when `git status` failed.
    pub edited_before_run: Option<Vec<String>>,
}

impl Default for WorkspaceChanges {
    /// Nothing changed and nothing is unknown.
    fn default() -> Self {
        Self {
            complete: true,
            note: None,
            files: Vec::new(),
            edited_before_run: None,
        }
    }
}

/// The state of one executor-owned check at the end of the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Passed,
    Failed,
    NotRun,
    /// It passed, then the source changed.
    Stale,
}

/// A declared probe or requested validation command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclaredCheck {
    pub probe_id: Option<String>,
    pub command: String,
    pub status: CheckStatus,
    pub exit_code: Option<i32>,
}

/// A shell command the model chose to run that is not a declared probe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedCommand {
    pub command: String,
    /// `None` when the command was killed, timed out or was cancelled.
    pub exit_code: Option<i32>,
}

/// Which checks passed, failed or did not run.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RunChecks {
    /// Executor-owned checks; only these have acceptance authority.
    pub declared: Vec<DeclaredCheck>,
    /// Model-run shell commands, in run order; observational only.
    pub observed: Vec<ObservedCommand>,
    /// Source paths changed after the last passing check, sorted.
    pub unchecked_paths: Vec<String>,
}

/// A snapshot of project files by path, or why it could not be taken.
pub(crate) type Snapshot<F> = Result<BTreeMap<String, F>, String>;

/// Compare two snapshots. Either side missing makes the result incomplete.
pub(crate) fn workspace_changes<F: PartialEq>(
    before: &Snapshot<F>,
    after: &Snapshot<F>,
    edited_before_run: Option<Vec<String>>,
) -> WorkspaceChanges {
    let (before, after) = match (before, after) {
        (Ok(before), Ok(after)) => (before, after),
        (Err(note), _) | (_, Err(note)) => {
            return WorkspaceChanges {
                complete: false,
                note: Some(note.clone()),
                files: Vec::new(),
                edited_before_run,
            };
        }
    };
    let dirty: BTreeSet<&str> = edited_before_run
        .iter()
        .flatten()
        .map(String::as_str)
        .collect();
    let paths: BTreeSet<&String> = before.keys().chain(after.keys()).collect();
    let files = paths
        .into_iter()
        .filter_map(|path| {
            let change = match (before.get(path), after.get(path)) {
                (None, Some(_)) => ChangeKind::Created,
                (Some(_), None) => ChangeKind::Deleted,
                (Some(old), Some(new)) if old != new => ChangeKind::Modified,
                _ => return None,
            };
            Some(FileChange {
                path: path.clone(),
                change,
                edited_before_run: dirty.contains(path.as_str()),
            })
        })
        .collect();
    WorkspaceChanges {
        complete: true,
        note: None,
        files,
        edited_before_run,
    }
}

/// Paths in `git status --porcelain=v1 -z` output, relative to the project.
///
/// `prefix` is the project's path inside the repository (`git rev-parse
/// --show-prefix`, empty at the top level or ending in `/`). Entries outside
/// it are dropped. A rename or copy names both its new and original paths.
pub(crate) fn parse_porcelain_z(output: &[u8], prefix: &str) -> Vec<String> {
    let mut fields = output
        .split(|byte| *byte == 0)
        .filter(|field| !field.is_empty());
    let mut paths = BTreeSet::new();
    while let Some(entry) = fields.next() {
        if entry.len() < 4 {
            continue;
        }
        let (status, path) = (&entry[..2], &entry[3..]);
        let mut entry_paths = vec![path];
        if status.contains(&b'R') || status.contains(&b'C') {
            entry_paths.extend(fields.next());
        }
        for path in entry_paths {
            let path = String::from_utf8_lossy(path);
            if let Some(relative) = path.strip_prefix(prefix)
                && !relative.is_empty()
            {
                paths.insert(relative.trim_end_matches('/').to_owned());
            }
        }
    }
    paths.into_iter().collect()
}

/// The report as lines, in the order the command line and conversation view
/// show them. An ordinary reply that touched nothing renders no lines.
pub fn report_lines(changes: &WorkspaceChanges, checks: &RunChecks) -> Vec<String> {
    let mut lines = Vec::new();
    if !changes.complete {
        lines.push(match &changes.note {
            Some(note) => format!("changes not fully known: {note}"),
            None => "changes not fully known".to_owned(),
        });
    } else if !changes.files.is_empty() {
        let count = changes.files.len();
        let listed = changes.files.iter().map(|file| {
            if file.edited_before_run {
                format!(
                    "{} ({}, edited before the run)",
                    file.path,
                    file.change.name()
                )
            } else {
                format!("{} ({})", file.path, file.change.name())
            }
        });
        lines.push(format!(
            "changed {count} {}: {}",
            if count == 1 { "file" } else { "files" },
            capped(listed, count, MAX_LISTED_PATHS, ", ")
        ));
    }
    if !checks.declared.is_empty() {
        let entries = checks.declared.iter().map(|check| {
            let subject = match &check.probe_id {
                Some(id) => format!("probe `{id}`"),
                None => format!("`{}`", one_line(&check.command)),
            };
            let state = match (check.status, check.exit_code) {
                (CheckStatus::Passed, _) => "passed".to_owned(),
                (CheckStatus::Failed, Some(code)) => format!("failed (exit {code})"),
                (CheckStatus::Failed, None) => "failed".to_owned(),
                (CheckStatus::NotRun, _) => "not run".to_owned(),
                (CheckStatus::Stale, _) => "stale (passed before later edits)".to_owned(),
            };
            format!("{subject} {state}")
        });
        lines.push(format!(
            "checks: {}",
            entries.collect::<Vec<_>>().join("; ")
        ));
    }
    if !checks.observed.is_empty() {
        let count = checks.observed.len();
        let entries = checks.observed.iter().map(|observed| {
            let command = shortened(&one_line(&observed.command));
            match observed.exit_code {
                Some(code) => format!("`{command}` exit {code}"),
                None => format!("`{command}` no exit status"),
            }
        });
        lines.push(format!(
            "commands run: {}",
            capped(entries, count, MAX_LISTED_COMMANDS, "; ")
        ));
    }
    if !checks.unchecked_paths.is_empty() {
        let count = checks.unchecked_paths.len();
        lines.push(format!(
            "unchecked: {} changed after the last passing check",
            capped(
                checks.unchecked_paths.iter().cloned(),
                count,
                MAX_LISTED_PATHS,
                ", "
            )
        ));
    }
    lines
}

/// The first `limit` items joined, then `{separator}and M more`.
fn capped(
    items: impl Iterator<Item = String>,
    count: usize,
    limit: usize,
    separator: &str,
) -> String {
    let mut text = items.take(limit).collect::<Vec<_>>().join(separator);
    if count > limit {
        text.push_str(&format!("{separator}and {} more", count - limit));
    }
    text
}

/// Command text on one line: runs of whitespace that contain a line break
/// become one space.
fn one_line(command: &str) -> String {
    if !command.contains(['\n', '\r']) {
        return command.to_owned();
    }
    command.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// At most 60 characters, the last an ellipsis when the text was cut.
fn shortened(command: &str) -> String {
    if command.chars().count() <= MAX_COMMAND_CHARS {
        return command.to_owned();
    }
    let mut text: String = command.chars().take(MAX_COMMAND_CHARS - 1).collect();
    text.push('…');
    text
}

/// The project paths Git reports as changed or untracked, or `None` when the
/// project is not in a Git repository or `git status` fails.
pub(crate) async fn edited_before_run(root: &Path) -> Option<Vec<String>> {
    let prefix = crate::stash::git(root, &["rev-parse", "--show-prefix"]).await?;
    if !prefix.status.success() {
        return None;
    }
    let prefix = String::from_utf8(prefix.stdout).ok()?;
    let status = crate::stash::git(
        root,
        &[
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--",
            ".",
        ],
    )
    .await?;
    status
        .status
        .success()
        .then(|| parse_porcelain_z(&status.stdout, prefix.trim_end_matches('\n')))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(files: &[(&str, u32)]) -> Snapshot<u32> {
        Ok(files
            .iter()
            .map(|(path, value)| ((*path).to_owned(), *value))
            .collect())
    }

    fn change(path: &str, change: ChangeKind, edited_before_run: bool) -> FileChange {
        FileChange {
            path: path.into(),
            change,
            edited_before_run,
        }
    }

    fn observed(command: &str, exit_code: Option<i32>) -> ObservedCommand {
        ObservedCommand {
            command: command.into(),
            exit_code,
        }
    }

    fn files(paths: &[&str]) -> WorkspaceChanges {
        WorkspaceChanges {
            files: paths
                .iter()
                .map(|path| change(path, ChangeKind::Modified, false))
                .collect(),
            ..WorkspaceChanges::default()
        }
    }

    #[test]
    fn the_difference_names_created_modified_and_deleted_files_in_path_order() {
        let before = snapshot(&[("b.rs", 1), ("gone.txt", 2), ("same.md", 3)]);
        let after = snapshot(&[("a/new.md", 9), ("b.rs", 4), ("same.md", 3)]);
        let changes = workspace_changes(&before, &after, Some(vec!["b.rs".into()]));
        assert!(changes.complete);
        assert_eq!(changes.note, None);
        assert_eq!(
            changes.files,
            [
                change("a/new.md", ChangeKind::Created, false),
                change("b.rs", ChangeKind::Modified, true),
                change("gone.txt", ChangeKind::Deleted, false),
            ]
        );
        assert_eq!(changes.edited_before_run, Some(vec!["b.rs".to_owned()]));
    }

    #[test]
    fn a_missing_snapshot_makes_the_changes_incomplete_with_its_reason() {
        let taken = snapshot(&[("a.rs", 1)]);
        let failed: Snapshot<u32> = Err("reading directory x: denied".into());
        for (before, after) in [(&failed, &taken), (&taken, &failed)] {
            let changes = workspace_changes(before, after, None);
            assert!(!changes.complete);
            assert_eq!(changes.note.as_deref(), Some("reading directory x: denied"));
            assert!(changes.files.is_empty());
        }
    }

    #[test]
    fn porcelain_entries_name_modified_added_deleted_renamed_and_untracked_paths() {
        let output = b" M src/parser.rs\0A  added.rs\0 D removed.rs\0R  new name.rs\0old name.rs\0?? scratch.txt\0?? notes/deep/x.md\0";
        assert_eq!(
            parse_porcelain_z(output, ""),
            [
                "added.rs",
                "new name.rs",
                "notes/deep/x.md",
                "old name.rs",
                "removed.rs",
                "scratch.txt",
                "src/parser.rs",
            ]
        );
    }

    #[test]
    fn porcelain_paths_are_made_relative_to_a_project_inside_the_repository() {
        let output = b" M app/src/lib.rs\0?? other/x.txt\0RM app/b.rs\0other/a.rs\0";
        assert_eq!(parse_porcelain_z(output, "app/"), ["b.rs", "src/lib.rs"]);
        assert!(parse_porcelain_z(b"", "").is_empty());
    }

    #[test]
    fn changed_files_render_with_their_change_and_prior_edits() {
        let changes = WorkspaceChanges {
            files: vec![
                change("src/parser.rs", ChangeKind::Modified, true),
                change("notes/new.md", ChangeKind::Created, false),
            ],
            ..WorkspaceChanges::default()
        };
        assert_eq!(
            report_lines(&changes, &RunChecks::default()),
            [
                "changed 2 files: src/parser.rs (modified, edited before the run), notes/new.md (created)"
            ]
        );
        let one = WorkspaceChanges {
            files: vec![change("old.txt", ChangeKind::Deleted, false)],
            ..WorkspaceChanges::default()
        };
        assert_eq!(
            report_lines(&one, &RunChecks::default()),
            ["changed 1 file: old.txt (deleted)"]
        );
    }

    #[test]
    fn more_than_ten_changed_files_are_counted_after_the_first_ten() {
        let paths: Vec<String> = (0..13).map(|n| format!("f{n:02}.txt")).collect();
        let changes = files(&paths.iter().map(String::as_str).collect::<Vec<_>>());
        let line = &report_lines(&changes, &RunChecks::default())[0];
        assert!(
            line.starts_with("changed 13 files: f00.txt (modified), "),
            "{line}"
        );
        assert!(line.ends_with("f09.txt (modified), and 3 more"), "{line}");
        assert!(!line.contains("f10.txt"), "{line}");
    }

    #[test]
    fn incomplete_changes_render_one_line_with_the_reason() {
        let changes = WorkspaceChanges {
            complete: false,
            note: Some("more than 50000 files; changes not tracked".into()),
            ..WorkspaceChanges::default()
        };
        assert_eq!(
            report_lines(&changes, &RunChecks::default()),
            ["changes not fully known: more than 50000 files; changes not tracked"]
        );
    }

    #[test]
    fn declared_checks_render_each_status() {
        let declared = |probe_id: Option<&str>, command: &str, status, exit_code| DeclaredCheck {
            probe_id: probe_id.map(str::to_owned),
            command: command.into(),
            status,
            exit_code,
        };
        let checks = RunChecks {
            declared: vec![
                declared(Some("test"), "cargo test", CheckStatus::Passed, Some(0)),
                declared(Some("lint"), "cargo clippy", CheckStatus::NotRun, None),
                declared(None, "cargo test", CheckStatus::Failed, Some(101)),
                declared(Some("x"), "make x", CheckStatus::Stale, Some(0)),
                declared(Some("y"), "make y", CheckStatus::Failed, None),
            ],
            ..RunChecks::default()
        };
        assert_eq!(
            report_lines(&WorkspaceChanges::default(), &checks),
            [
                "checks: probe `test` passed; probe `lint` not run; `cargo test` failed (exit 101); probe `x` stale (passed before later edits); probe `y` failed"
            ]
        );
    }

    #[test]
    fn observed_commands_render_in_order_with_exit_statuses() {
        let checks = RunChecks {
            observed: vec![
                observed("grep -c Hello greeting.txt", Some(0)),
                observed("cargo test", Some(101)),
                observed("sleep 100", None),
            ],
            ..RunChecks::default()
        };
        assert_eq!(
            report_lines(&WorkspaceChanges::default(), &checks),
            [
                "commands run: `grep -c Hello greeting.txt` exit 0; `cargo test` exit 101; `sleep 100` no exit status"
            ]
        );
    }

    #[test]
    fn long_commands_are_cut_and_more_than_five_are_counted() {
        let long = format!("echo {}", "a".repeat(80));
        let mut list = vec![observed(&long, Some(0))];
        list.extend((1..7).map(|n| observed(&format!("true {n}"), Some(0))));
        let line = report_lines(
            &WorkspaceChanges::default(),
            &RunChecks {
                observed: list,
                ..RunChecks::default()
            },
        )
        .remove(0);
        let cut = format!("`echo {}…` exit 0", "a".repeat(54));
        assert!(
            line.starts_with(&format!("commands run: {cut}; `true 1` exit 0")),
            "{line}"
        );
        assert!(line.ends_with("`true 4` exit 0; and 2 more"), "{line}");
        assert_eq!(shortened(&long).chars().count(), 60);
        assert_eq!(
            one_line("printf 'a'\n  && echo b"),
            "printf 'a' && echo b",
            "a command renders on one line"
        );
    }

    #[test]
    fn unchecked_paths_render_after_the_other_lines() {
        let checks = RunChecks {
            observed: vec![observed("ls", Some(0))],
            unchecked_paths: vec!["src/lib.rs".into(), "src/main.rs".into()],
            ..RunChecks::default()
        };
        assert_eq!(
            report_lines(&files(&["src/lib.rs"]), &checks),
            [
                "changed 1 file: src/lib.rs (modified)",
                "commands run: `ls` exit 0",
                "unchecked: src/lib.rs, src/main.rs changed after the last passing check",
            ]
        );
        let many = RunChecks {
            unchecked_paths: (0..12).map(|n| format!("s{n:02}.rs")).collect(),
            ..RunChecks::default()
        };
        let line = report_lines(&WorkspaceChanges::default(), &many).remove(0);
        assert!(
            line.ends_with("s09.rs, and 2 more changed after the last passing check"),
            "{line}"
        );
    }

    #[test]
    fn a_run_that_touched_nothing_renders_no_lines() {
        assert!(report_lines(&WorkspaceChanges::default(), &RunChecks::default()).is_empty());
        let dirty_but_untouched = WorkspaceChanges {
            edited_before_run: Some(vec!["a.txt".into()]),
            ..WorkspaceChanges::default()
        };
        assert!(report_lines(&dirty_but_untouched, &RunChecks::default()).is_empty());
    }

    #[test]
    fn reports_read_back_from_partial_json_with_defaults() {
        let changes: WorkspaceChanges = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(changes, WorkspaceChanges::default());
        let checks: RunChecks =
            serde_json::from_value(serde_json::json!({"unchecked_paths": ["a.rs"]})).unwrap();
        assert_eq!(checks.unchecked_paths, ["a.rs"]);
        let json = serde_json::to_value(WorkspaceChanges::default()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({"complete": true, "note": null, "files": [], "edited_before_run": null})
        );
    }
}
