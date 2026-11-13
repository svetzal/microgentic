//! User-entered shell commands, independent of the agent and its tools.
use super::*;
use std::path::Path;

pub(super) struct ShellRun {
    pid: Option<u32>,
    pub result: JoinHandle<std::io::Result<Output>>,
}

impl ShellRun {
    pub fn cancel(&self) {
        #[cfg(unix)]
        if let Some(pid) = self.pid.and_then(|pid| i32::try_from(pid).ok()) {
            // The shell owns a separate process group, including pipeline children.
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
        }
        #[cfg(not(unix))]
        self.result.abort();
    }
}

impl Drop for ShellRun {
    fn drop(&mut self) {
        if !self.result.is_finished() {
            self.cancel();
        }
    }
}

fn start(root: &Path, command: &str) -> std::io::Result<ShellRun> {
    let shell = std::env::var_os("SHELL")
        .filter(|path| Path::new(path).is_file())
        .unwrap_or_else(|| "/bin/sh".into());
    let mut process = tokio::process::Command::new(shell);
    process
        .arg("-c")
        .arg(command)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    process.process_group(0);
    let child = process.spawn()?;
    Ok(ShellRun {
        pid: child.id(),
        result: tokio::spawn(child.wait_with_output()),
    })
}

/// Shell mode owns Enter and Ctrl-C before ordinary conversation handling.
pub(super) fn key(
    view: &mut View,
    root: &Path,
    run: &mut Option<ShellRun>,
    key: event::KeyEvent,
    agent_busy: bool,
) -> bool {
    if key.code == KeyCode::Char('!')
        && !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        && view.input.is_empty()
        && !view.shell_mode
    {
        view.shell_mode = true;
        return true;
    }
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        if view.shell_mode {
            view.shell_mode = false;
            view.input.clear();
            return true;
        }
        if let Some(run) = run {
            run.cancel();
            return true;
        }
    }
    if key.code == KeyCode::Tab && view.shell_mode {
        return true;
    }
    if key.code != KeyCode::Enter || key.modifiers.contains(KeyModifiers::ALT) {
        return false;
    }
    if run.is_some() || (agent_busy && view.shell_mode) {
        return true;
    }
    if !view.shell_mode {
        return false;
    }
    view.shell_mode = false;
    let command = std::mem::take(&mut view.input);
    if command.trim().is_empty() {
        return true;
    }
    view.history.start_block();
    view.history
        .push(Role::UserLabel, &format!("! {}\n", command.trim()));
    view.scroll = 0;
    match start(root, &command) {
        Ok(started) => {
            *run = Some(started);
            view.status = Status::new(Phase::Working, "Running shell command · Ctrl-C to cancel");
        }
        Err(error) => {
            view.history.push(
                Role::Error,
                &format!("Could not start shell command: {error}\n"),
            );
            view.status = ready_for_next();
        }
    }
    true
}

pub(super) async fn finish(view: &mut View, run: &mut Option<ShellRun>, session: &mut Session) {
    let mut finished = run.take().expect("finished shell command");
    match (&mut finished.result).await {
        Ok(Ok(output)) => {
            for (bytes, role) in [(&output.stdout, Role::Plain), (&output.stderr, Role::Error)] {
                view.history.push(role, &String::from_utf8_lossy(bytes));
                view.history.start_line();
            }
            view.history.push(
                if output.status.success() {
                    Role::Muted
                } else {
                    Role::Error
                },
                &format!("Shell command: {}\n", output.status),
            );
        }
        result => view
            .history
            .push(Role::Error, &format!("Shell command failed: {result:?}\n")),
    }
    view.status = ready_for_next();
    view.scroll = 0;
    session.refresh_git();
    session.refresh_guidance();
    session.refresh_summary();
}
