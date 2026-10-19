//! Waiting for terminal input without reading it.
//!
//! crossterm's Unix reader loops on `read(2)` until it has parsed an event.
//! A descriptor at end of file, or in error, returns at once with nothing,
//! so once the terminal has gone (its emulator quit, or the parent that held
//! the pseudo-terminal exited) `event::poll` never returns and the process
//! spins on one core. The wait here is therefore a `poll(2)` on standard
//! input, which reports a hang-up or an invalid descriptor without reading.
//! crossterm reads only after the wait, and only with a zero timeout.
//!
//! Standard input is the right descriptor because the conversation refuses
//! to start unless it is a terminal, and crossterm reads standard input
//! whenever it is one (it opens `/dev/tty` only otherwise).
use std::io;
use std::time::Duration;

/// What a wait for input found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Wait {
    /// Bytes are ready for crossterm to read.
    Ready,
    /// The wait ended with nothing to read.
    Idle,
    /// The terminal has gone: its other end hung up, it reported an error,
    /// or standard input is not an open descriptor.
    Closed,
}

/// Wait up to `timeout` for input on standard input.
#[cfg(unix)]
pub(super) fn wait(timeout: Duration) -> io::Result<Wait> {
    wait_on(libc::STDIN_FILENO, timeout)
}

/// Wait up to `timeout` for input on `fd`.
#[cfg(unix)]
fn wait_on(fd: std::os::fd::RawFd, timeout: Duration) -> io::Result<Wait> {
    wait_on_with(
        fd,
        timeout,
        std::time::Instant::now,
        |descriptor, millis| {
            // SAFETY: descriptor points to one initialized pollfd that stays valid
            // throughout poll; the count describes exactly that one descriptor.
            let ready = unsafe { libc::poll(descriptor, 1, millis) };
            if ready >= 0 {
                Ok(())
            } else {
                Err(io::Error::last_os_error())
            }
        },
    )
}

#[cfg(unix)]
fn wait_on_with(
    fd: std::os::fd::RawFd,
    timeout: Duration,
    mut now: impl FnMut() -> std::time::Instant,
    mut poll: impl FnMut(&mut libc::pollfd, libc::c_int) -> io::Result<()>,
) -> io::Result<Wait> {
    let deadline = now() + timeout;
    loop {
        let mut descriptor = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let left = deadline.saturating_duration_since(now());
        let millis =
            libc::c_int::try_from(left.as_micros().div_ceil(1_000)).unwrap_or(libc::c_int::MAX);
        match poll(&mut descriptor, millis) {
            Ok(()) => return Ok(classify(descriptor.revents)),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
}

/// Wait up to `timeout` for input, through crossterm where `poll(2)` is
/// not available.
#[cfg(not(unix))]
pub(super) fn wait(timeout: Duration) -> io::Result<Wait> {
    Ok(if crossterm::event::poll(timeout)? {
        Wait::Ready
    } else {
        Wait::Idle
    })
}

/// What the returned events of a `poll(2)` on the terminal mean. A hang-up
/// wins over readable bytes: nobody is left to see their effect.
#[cfg(unix)]
fn classify(revents: libc::c_short) -> Wait {
    if revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0 {
        Wait::Closed
    } else if revents & libc::POLLIN != 0 {
        Wait::Ready
    } else {
        Wait::Idle
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::fd::AsRawFd;

    #[test]
    fn readable_input_is_ready() {
        assert_eq!(classify(libc::POLLIN), Wait::Ready);
    }

    #[test]
    fn nothing_reported_is_idle() {
        assert_eq!(classify(0), Wait::Idle);
    }

    #[test]
    fn a_hang_up_closes_even_with_bytes_waiting() {
        assert_eq!(classify(libc::POLLIN | libc::POLLHUP), Wait::Closed);
        assert_eq!(classify(libc::POLLHUP), Wait::Closed);
    }

    #[test]
    fn an_error_or_invalid_descriptor_closes() {
        assert_eq!(classify(libc::POLLERR), Wait::Closed);
        assert_eq!(classify(libc::POLLNVAL), Wait::Closed);
    }

    #[test]
    fn an_empty_pipe_is_idle_until_the_timeout() {
        let (reader, _writer) = io::pipe().expect("a pipe");
        let epoch = std::time::Instant::now();
        let mut times = [epoch, epoch, epoch + Duration::from_millis(12)].into_iter();
        let mut waits = Vec::new();
        let state = wait_on_with(
            reader.as_raw_fd(),
            Duration::from_millis(30),
            || times.next().expect("unexpected clock read"),
            |descriptor, millis| {
                assert_eq!(descriptor.fd, reader.as_raw_fd());
                assert_eq!(descriptor.events, libc::POLLIN);
                waits.push(millis);
                if waits.len() == 1 {
                    Err(io::Error::from(io::ErrorKind::Interrupted))
                } else {
                    Ok(())
                }
            },
        )
        .unwrap();
        assert_eq!(state, Wait::Idle);
        assert_eq!(waits, [30, 18], "a signal must not restart the timeout");
        assert_eq!(
            wait_on(reader.as_raw_fd(), Duration::ZERO).unwrap(),
            Wait::Idle
        );
    }

    #[test]
    fn a_pipe_with_bytes_is_ready_at_once() {
        let (reader, mut writer) = io::pipe().expect("a pipe");
        writer.write_all(b"x").expect("writing");
        let state = wait_on(reader.as_raw_fd(), Duration::from_secs(5)).expect("polling");
        assert_eq!(state, Wait::Ready);
    }

    #[test]
    fn a_pipe_whose_writer_is_gone_is_closed_even_with_bytes_waiting() {
        let (reader, mut writer) = io::pipe().expect("a pipe");
        writer.write_all(b"x").expect("writing");
        drop(writer);
        let state = wait_on(reader.as_raw_fd(), Duration::from_secs(5)).expect("polling");
        assert_eq!(state, Wait::Closed);
    }
}
