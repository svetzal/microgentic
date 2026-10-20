//! A pseudo-terminal driver that renders output into a screen model.
//!
//! `portable-pty` runs the binary on a real pseudo-terminal (ConPTY on
//! Windows), and `vt100` parses everything it writes into a cell grid, so
//! tests assert on rows, cells, attributes and colour rather than bytes. A
//! reader thread drains the terminal continuously, so the binary never
//! blocks on a full buffer, and keeps the raw bytes and a history of rendered
//! screens for the few tests that need them.
//!
//! Completed DEC synchronized updates are observable render events. Waiters
//! block on a condition variable, with a watchdog only for missing events.
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use std::ffi::OsStr;
use std::io::{Read, Write};
use std::num::NonZeroU16;
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

/// How long a wait lasts before the test fails with the current screen.
pub const WAIT: Duration = Duration::from_secs(20);

/// Most rendered screens kept in the history.
const MAX_FRAMES: usize = 20_000;
const BEGIN_UPDATE: &[u8] = b"\x1b[?2026h";
const END_UPDATE: &[u8] = b"\x1b[?2026l";

struct Shared {
    raw: Vec<u8>,
    parser: vt100::Parser,
    /// The screen's text after each chunk of output that leaves no
    /// synchronized update open.
    frames: Vec<String>,
    /// A synchronized update has begun and not yet ended.
    in_update: bool,
    /// Last completed update, retained while the next frame arrives.
    last_frame: Option<Snapshot>,
    ended: bool,
}

impl Shared {
    /// Take in one chunk of output. A chunk can hold the end of one frame
    /// and the start of the next, so the screen is recorded at every
    /// update's end, not only at the end of the chunk.
    fn absorb(&mut self, chunk: &[u8]) {
        // Markers can be split across chunks, so search from a few bytes back.
        let carry = self.raw.len().min(BEGIN_UPDATE.len() - 1);
        self.raw.extend_from_slice(chunk);
        let window = &self.raw[self.raw.len() - chunk.len() - carry..];
        let mut fed = 0;
        for (end, opens) in update_markers(window) {
            // `end` is just past the marker; markers that end in bytes
            // already fed were handled with the previous chunk.
            let Some(end) = end.checked_sub(carry).filter(|end| *end > 0) else {
                continue;
            };
            self.parser.process(&chunk[fed..end]);
            fed = end;
            self.in_update = opens;
            if !opens {
                self.record_frame();
            }
        }
        if fed < chunk.len() {
            self.parser.process(&chunk[fed..]);
            if !self.in_update {
                self.record_frame();
            }
        }
    }

    fn record_frame(&mut self) {
        self.last_frame = Some(Snapshot(self.parser.screen().clone()));
        if self.frames.len() < MAX_FRAMES {
            self.frames.push(self.parser.screen().contents());
        }
    }
}

/// Every synchronized-update marker in `bytes`, in order, as the index just
/// past it and whether it opens an update.
fn update_markers(bytes: &[u8]) -> Vec<(usize, bool)> {
    (0..bytes.len())
        .filter_map(|start| {
            let rest = &bytes[start..];
            if rest.starts_with(BEGIN_UPDATE) {
                Some((start + BEGIN_UPDATE.len(), true))
            } else if rest.starts_with(END_UPDATE) {
                Some((start + END_UPDATE.len(), false))
            } else {
                None
            }
        })
        .collect()
}

pub struct Tui {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    shared: Arc<Mutex<Shared>>,
    changed: Arc<Condvar>,
    reader: Option<JoinHandle<()>>,
    /// Frames before this index are ignored by `wait_for_frame`.
    frame_cursor: usize,
    #[cfg(unix)]
    initial_modes: Option<[libc::tcflag_t; 3]>,
}

/// Builds a terminal session.
pub struct Launch {
    command: CommandBuilder,
    rows: u16,
    cols: u16,
}

impl Launch {
    pub fn new(binary: &str) -> Self {
        let mut command = CommandBuilder::new(binary);
        command.env("TERM", "xterm-256color");
        // Colour stays on unless a test asks otherwise.
        command.env_remove("NO_COLOR");
        Self {
            command,
            rows: 30,
            cols: 120,
        }
    }

    pub fn arg(mut self, arg: impl AsRef<OsStr>) -> Self {
        self.command.arg(arg);
        self
    }

    pub fn cwd(mut self, directory: &Path) -> Self {
        self.command.cwd(directory);
        self
    }

    pub fn env(mut self, key: &str, value: impl AsRef<OsStr>) -> Self {
        self.command.env(key, value);
        self
    }

    pub fn env_remove(mut self, key: &str) -> Self {
        self.command.env_remove(key);
        self
    }

    pub fn size(mut self, rows: u16, cols: u16) -> Self {
        self.rows = rows;
        self.cols = cols;
        self
    }

    pub fn spawn(self) -> Tui {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: self.rows,
                cols: self.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("opening a pseudo-terminal");
        #[cfg(unix)]
        let initial_modes = modes(pair.master.as_ref());
        let child = pair
            .slave
            .spawn_command(self.command)
            .expect("starting the binary on the pseudo-terminal");
        drop(pair.slave);
        let mut reader = pair.master.try_clone_reader().expect("terminal reader");
        let writer = pair.master.take_writer().expect("terminal writer");
        let shared = Arc::new(Mutex::new(Shared {
            raw: Vec::new(),
            parser: vt100::Parser::new(screen_dimension(self.rows), screen_dimension(self.cols), 0),
            frames: Vec::new(),
            in_update: false,
            last_frame: None,
            ended: false,
        }));
        let sink = shared.clone();
        let changed = Arc::new(Condvar::new());
        let notify = changed.clone();
        let reader = std::thread::spawn(move || {
            let mut buffer = [0u8; 65_536];
            while let Ok(count) = reader.read(&mut buffer) {
                if count == 0 {
                    break;
                }
                sink.lock()
                    .expect("terminal state")
                    .absorb(&buffer[..count]);
                notify.notify_all();
            }
            sink.lock().unwrap().ended = true;
            notify.notify_all();
        });
        Tui {
            master: pair.master,
            writer,
            child,
            shared,
            changed,
            reader: Some(reader),
            frame_cursor: 0,
            #[cfg(unix)]
            initial_modes,
        }
    }
}

impl Launch {
    /// Start the binary, wait until its output contains `needle`, then close
    /// every handle on the terminal's controlling side, as a terminal
    /// emulator does when it quits or a parent process that owns the
    /// pseudo-terminal exits. Returns the exit status if the binary ends
    /// after the child reports its exit.
    #[cfg(unix)]
    pub fn spawn_and_hang_up(self, needle: &str) -> portable_pty::ExitStatus {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: self.rows,
                cols: self.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("opening a pseudo-terminal");
        let mut child = pair
            .slave
            .spawn_command(self.command)
            .expect("starting the binary on the pseudo-terminal");
        drop(pair.slave);
        // Reads happen only after poll(2) reports bytes, so no reader thread
        // holds a handle on the terminal when it is time to close it.
        let fd = pair.master.as_raw_fd().expect("the terminal's descriptor");
        let mut reader = pair.master.try_clone_reader().expect("terminal reader");
        let mut seen = Vec::new();
        while !String::from_utf8_lossy(&seen).contains(needle) {
            let mut fds = [libc::pollfd {
                fd,
                events: libc::POLLIN,
                revents: 0,
            }];
            // SAFETY: `fds` is one initialised pollfd that outlives the call.
            let ready = unsafe { libc::poll(fds.as_mut_ptr(), 1, -1) };
            if ready <= 0 {
                continue;
            }
            let mut buffer = [0u8; 4096];
            let count = reader.read(&mut buffer).expect("reading the terminal");
            seen.extend_from_slice(&buffer[..count]);
        }
        drop(reader);
        drop(pair.master);
        child.wait().expect("waiting for terminal hangup")
    }
}

#[cfg(unix)]
fn modes(master: &dyn MasterPty) -> Option<[libc::tcflag_t; 3]> {
    let fd = master.as_raw_fd()?;
    // SAFETY: `termios` is plain data that `tcgetattr` fully initialises on
    // success, and `fd` is the open master descriptor owned by `master`.
    unsafe {
        let mut termios: libc::termios = std::mem::zeroed();
        (libc::tcgetattr(fd, &mut termios) == 0).then_some([
            termios.c_iflag,
            termios.c_oflag,
            termios.c_lflag,
        ])
    }
}

/// A copy of the rendered screen at one moment.
#[derive(Clone)]
pub struct Snapshot(vt100::Screen);

impl Snapshot {
    /// The screen's rows and columns.
    fn size(&self) -> (u16, u16) {
        let (rows, cols) = self.0.size();
        (rows.get(), cols.get())
    }

    /// Row `row`, with trailing blanks removed.
    pub fn row(&self, row: u16) -> String {
        let (_, cols) = self.size();
        self.0
            .rows(0, cols)
            .nth(usize::from(row))
            .unwrap_or_default()
            .trim_end()
            .to_owned()
    }

    pub fn rows(&self) -> Vec<String> {
        let (rows, _) = self.size();
        (0..rows).map(|row| self.row(row)).collect()
    }

    pub fn text(&self) -> String {
        self.rows().join("\n")
    }

    /// Every row from column `col` to the right edge, with trailing blanks
    /// removed, for a pane that sits beside other text.
    pub fn columns_from(&self, col: u16) -> Vec<String> {
        let (_, cols) = self.size();
        self.0
            .rows(col, cols.saturating_sub(col))
            .map(|row| row.trim_end().to_owned())
            .collect()
    }

    pub fn contains(&self, needle: &str) -> bool {
        self.rows().iter().any(|row| row.contains(needle))
    }

    /// The first row and column where `needle` starts.
    pub fn locate(&self, needle: &str) -> Option<(u16, u16)> {
        let (rows, cols) = self.size();
        for row in 0..rows {
            let mut text = String::new();
            let mut starts = Vec::new();
            for col in 0..cols {
                let cell = self.0.cell(row, col)?;
                if cell.is_wide_continuation() {
                    continue;
                }
                starts.push((text.len(), col));
                text.push_str(if cell.has_contents() {
                    cell.contents()
                } else {
                    " "
                });
            }
            if let Some(byte) = text.find(needle) {
                let col = starts
                    .iter()
                    .rev()
                    .find(|(offset, _)| *offset <= byte)
                    .map_or(0, |(_, col)| *col);
                return Some((row, col));
            }
        }
        None
    }

    pub fn cell(&self, row: u16, col: u16) -> &vt100::Cell {
        self.0.cell(row, col).expect("cell inside the screen")
    }
}

impl std::fmt::Display for Snapshot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (index, row) in self.rows().iter().enumerate() {
            writeln!(f, "{index:2}|{row}")?;
        }
        Ok(())
    }
}

/// A screen dimension for `vt100`, which rejects an empty screen.
fn screen_dimension(cells: u16) -> NonZeroU16 {
    NonZeroU16::new(cells).expect("a terminal needs at least one row and column")
}

/// A named 256-colour index, as crossterm writes ANSI colours.
pub fn ansi(index: u8) -> vt100::Color {
    vt100::Color::Idx(index)
}

impl Tui {
    pub fn snapshot(&self) -> Snapshot {
        Snapshot(
            self.shared
                .lock()
                .expect("terminal state")
                .parser
                .screen()
                .clone(),
        )
    }

    /// Everything the binary has written, escape sequences included.
    pub fn raw(&self) -> Vec<u8> {
        self.shared.lock().expect("terminal state").raw.clone()
    }

    /// Rendered screens recorded since the last `mark_frames`.
    pub fn frames_since_mark(&self) -> Vec<String> {
        let shared = self.shared.lock().expect("terminal state");
        shared.frames[self.frame_cursor.min(shared.frames.len())..].to_vec()
    }

    /// Start a new window for `wait_for_frame` and `frames_since_mark`.
    pub fn mark_frames(&mut self) {
        self.frame_cursor = self.shared.lock().expect("terminal state").frames.len();
    }

    /// Wait until a completed frame satisfies `test`, and return that screen.
    pub fn wait_for(&self, what: &str, test: impl Fn(&Snapshot) -> bool) -> Snapshot {
        let mut shared = self.shared.lock().unwrap();
        loop {
            if let Some(snapshot) = &shared.last_frame
                && test(snapshot)
            {
                return snapshot.clone();
            }
            assert!(
                !shared.ended,
                "terminal ended before {what}: {}",
                shared.parser.screen().contents()
            );
            let (next, timeout) = self.changed.wait_timeout(shared, WAIT).unwrap();
            shared = next;
            assert!(
                !timeout.timed_out(),
                "no terminal event for {what}: {}",
                shared.parser.screen().contents()
            );
        }
    }
    /// Wait until the screen shows `needle`.
    pub fn wait_text(&self, needle: &str) -> Snapshot {
        self.wait_for(&format!("{needle:?}"), |screen| screen.contains(needle))
    }

    /// Wait until some screen rendered since the last mark satisfies `test`,
    /// so a transient state is not missed between polls. Returns that frame.
    pub fn wait_for_frame(&self, what: &str, test: impl Fn(&str) -> bool) -> String {
        let mut shared = self.shared.lock().unwrap();
        loop {
            if let Some(frame) = shared.frames[self.frame_cursor..]
                .iter()
                .find(|frame| test(frame))
            {
                return frame.clone();
            }
            assert!(!shared.ended, "terminal ended before {what}");
            let (next, timeout) = self.changed.wait_timeout(shared, WAIT).unwrap();
            shared = next;
            assert!(
                !timeout.timed_out(),
                "no terminal event for {what}: {}",
                shared.parser.screen().contents()
            );
        }
    }

    pub fn send(&mut self, bytes: &[u8]) {
        self.writer
            .write_all(bytes)
            .expect("writing to the terminal");
        self.writer.flush().expect("flushing the terminal");
    }

    /// Bracketed paste, as a terminal sends pasted text.
    pub fn paste(&mut self, text: &str) {
        self.send(format!("\x1b[200~{text}\x1b[201~").as_bytes());
    }

    /// Paste a line and press Enter.
    pub fn submit(&mut self, text: &str) {
        self.paste(text);
        self.send(b"\r");
    }

    pub fn esc(&mut self) {
        self.send(b"\x1b");
    }

    pub fn ctrl_c(&mut self) {
        self.send(b"\x03");
    }

    pub fn resize(&mut self, rows: u16, cols: u16) {
        self.master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("resizing the terminal");
        self.shared
            .lock()
            .expect("terminal state")
            .parser
            .screen_mut()
            .set_size(screen_dimension(rows), screen_dimension(cols));
    }

    /// Wait for the binary to exit, and return whether it succeeded.
    pub fn wait_exit(&mut self) -> bool {
        self.child.wait().expect("waiting for binary").success()
    }

    pub fn exit_status(&mut self) -> portable_pty::ExitStatus {
        self.child.wait().expect("waiting for binary")
    }

    /// Send the binary a signal, as `kill` would.
    #[cfg(unix)]
    pub fn signal(&self, signal: i32) {
        let pid = self.child.process_id().expect("the binary's process id");
        // SAFETY: kill(2) takes a pid and a signal number and touches no memory.
        let sent = unsafe { libc::kill(pid as libc::pid_t, signal) };
        assert_eq!(
            sent,
            0,
            "signalling the binary: {}",
            std::io::Error::last_os_error()
        );
    }

    /// Press Esc on an idle screen and require a clean exit that restores
    /// the terminal's modes.
    pub fn quit(&mut self) {
        self.esc();
        assert!(self.wait_exit(), "the binary exited with an error");
        self.assert_terminal_restored();
    }

    /// The terminal's input, output and local modes match those before launch.
    pub fn assert_terminal_restored(&self) {
        #[cfg(unix)]
        {
            assert!(
                self.initial_modes.is_some(),
                "could not read the terminal modes"
            );
            assert_eq!(
                modes(self.master.as_ref()),
                self.initial_modes,
                "raw terminal mode was not restored"
            );
        }
    }

    /// The binary has put the terminal in raw mode, so the restore check
    /// has something to compare.
    pub fn assert_raw_mode_active(&self) {
        #[cfg(unix)]
        assert_ne!(
            modes(self.master.as_ref()),
            self.initial_modes,
            "the terminal is not in raw mode"
        );
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        // The reader ends at end of file once the child and the writer are gone.
        drop(self.reader.take());
    }
}
