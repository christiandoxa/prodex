use super::*;

/// Only an interactive foreground TUI owns terminal input modes. Piped execs
/// and app-server children must never send terminal controls to machine output.
pub(super) struct ChildTerminalHandoff {
    enabled: bool,
    #[cfg(unix)]
    saved: Option<libc::termios>,
}

impl ChildTerminalHandoff {
    pub(super) fn new(plan: &ChildProcessPlan) -> Self {
        let enabled = plan.reset_terminal_keyboard_enhancement
            && io::stdin().is_terminal()
            && io::stdout().is_terminal();
        Self {
            enabled,
            #[cfg(unix)]
            saved: enabled
                .then(|| capture_termios(libc::STDIN_FILENO))
                .flatten(),
        }
    }
}

impl Drop for ChildTerminalHandoff {
    fn drop(&mut self) {
        if !self.enabled {
            return;
        }
        let _ = reset_child_terminal_protocols(&mut io::stdout());
        #[cfg(unix)]
        if let Some(saved) = self.saved.as_ref() {
            // TCSANOW intentionally does not flush queued user input.
            restore_termios(libc::STDIN_FILENO, saved);
        }
    }
}

#[cfg(unix)]
fn capture_termios(fd: libc::c_int) -> Option<libc::termios> {
    let mut state = std::mem::MaybeUninit::<libc::termios>::uninit();
    if unsafe { libc::tcgetattr(fd, state.as_mut_ptr()) } == 0 {
        Some(unsafe { state.assume_init() })
    } else {
        None
    }
}

#[cfg(unix)]
fn restore_termios(fd: libc::c_int, saved: &libc::termios) {
    unsafe {
        libc::tcsetattr(fd, libc::TCSANOW, saved);
    }
}

fn reset_child_terminal_protocols(writer: &mut impl io::Write) -> io::Result<()> {
    crossterm::execute!(
        writer,
        crossterm::terminal::LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture,
        crossterm::event::DisableBracketedPaste,
        crossterm::event::DisableFocusChange,
        crossterm::event::PopKeyboardEnhancementFlags,
        crossterm::cursor::Show,
    )
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn child_terminal_cleanup_disables_mouse_paste_and_alt_screen_without_input_flush() {
        let mut output = Vec::new();
        reset_child_terminal_protocols(&mut output).unwrap();
        let text = String::from_utf8(output).unwrap();
        for control in [
            "\x1b[?1049l",
            "\x1b[?1000l",
            "\x1b[?1006l",
            "\x1b[?2004l",
            "\x1b[?1004l",
            "\x1b[?25h",
        ] {
            assert!(
                text.contains(control),
                "missing terminal protocol cleanup {control:?}: {text:?}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn child_terminal_saved_mode_is_restorable_on_an_owned_pty() {
        use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
        let (mut master, mut slave) = (-1, -1);
        assert_eq!(
            unsafe {
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            },
            0
        );
        let _master = unsafe { OwnedFd::from_raw_fd(master) };
        let slave = unsafe { OwnedFd::from_raw_fd(slave) };
        let original = capture_termios(slave.as_raw_fd()).unwrap();
        let mut raw = original;
        unsafe {
            libc::cfmakeraw(&mut raw);
        }
        assert_eq!(
            unsafe { libc::tcsetattr(slave.as_raw_fd(), libc::TCSANOW, &raw) },
            0
        );
        assert_eq!(
            capture_termios(slave.as_raw_fd()).unwrap().c_lflag & libc::ICANON,
            0
        );
        let input = b"queued-before-exit\n";
        assert_eq!(
            unsafe { libc::write(_master.as_raw_fd(), input.as_ptr().cast(), input.len()) },
            input.len() as isize
        );
        restore_termios(slave.as_raw_fd(), &original);
        assert_eq!(
            capture_termios(slave.as_raw_fd()).unwrap().c_lflag,
            original.c_lflag
        );
        let mut wait = libc::pollfd {
            fd: slave.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        assert_eq!(
            unsafe { libc::poll(&mut wait, 1, 1_000) },
            1,
            "queued input must not be flushed during recovery"
        );
        let mut readback = [0_u8; 64];
        let count = unsafe {
            libc::read(
                slave.as_raw_fd(),
                readback.as_mut_ptr().cast(),
                readback.len(),
            )
        };
        assert_eq!(count, input.len() as isize);
        assert_eq!(&readback[..count as usize], input);
    }
}
