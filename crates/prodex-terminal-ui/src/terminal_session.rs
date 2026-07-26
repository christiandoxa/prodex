use anyhow::{Context, Result};
use crossterm::cursor::{Hide, Show};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::{Backend, ClearType, CrosstermBackend, WindowSize};
use ratatui::buffer::Cell;
use ratatui::layout::{Position, Size};
use std::io::{self, Write};
use std::ops::{Deref, DerefMut};

#[derive(Clone, Copy)]
enum TerminalOutput {
    Stdout,
    Stderr,
}

#[derive(Clone, Copy)]
struct TerminalOperations {
    enable_raw_mode: fn() -> io::Result<()>,
    enter_alternate_screen: fn(TerminalOutput) -> io::Result<()>,
    leave_alternate_screen: fn(TerminalOutput) -> io::Result<()>,
    disable_raw_mode: fn() -> io::Result<()>,
}

impl Default for TerminalOperations {
    fn default() -> Self {
        Self {
            enable_raw_mode,
            enter_alternate_screen,
            leave_alternate_screen,
            disable_raw_mode,
        }
    }
}

struct TerminalSessionGuard {
    output: TerminalOutput,
    operations: TerminalOperations,
    raw_mode_enabled: bool,
    alternate_screen_entered: bool,
}

impl TerminalSessionGuard {
    fn enter(label: &str, output: TerminalOutput) -> Result<Self> {
        Self::enter_with_operations(label, output, TerminalOperations::default())
    }

    fn enter_with_operations(
        label: &str,
        output: TerminalOutput,
        operations: TerminalOperations,
    ) -> Result<Self> {
        (operations.enable_raw_mode)()
            .with_context(|| format!("failed to enable {label} raw mode"))?;
        let mut guard = Self {
            output,
            operations,
            raw_mode_enabled: true,
            alternate_screen_entered: false,
        };
        (operations.enter_alternate_screen)(output)
            .with_context(|| format!("failed to enter {label} alternate screen"))?;
        guard.alternate_screen_entered = true;
        Ok(guard)
    }
}

impl Drop for TerminalSessionGuard {
    fn drop(&mut self) {
        if self.raw_mode_enabled {
            let _ = (self.operations.disable_raw_mode)();
            self.raw_mode_enabled = false;
        }
        if self.alternate_screen_entered {
            let _ = (self.operations.leave_alternate_screen)(self.output);
            self.alternate_screen_entered = false;
        }
    }
}

/// A Ratatui terminal whose raw-mode and alternate-screen lifecycle is restored on every exit.
pub struct AlternateScreenTerminal<W: Write> {
    pub terminal: Terminal<OutputCrosstermBackend<W>>,
    _session: TerminalSessionGuard,
}

#[doc(hidden)]
pub struct OutputCrosstermBackend<W: Write> {
    inner: CrosstermBackend<W>,
    output: TerminalOutput,
}

impl<W: Write> OutputCrosstermBackend<W> {
    fn new(writer: W, output: TerminalOutput) -> Self {
        Self {
            inner: CrosstermBackend::new(writer),
            output,
        }
    }
}

impl<W: Write> Backend for OutputCrosstermBackend<W> {
    type Error = io::Error;

    fn draw<'a, I>(&mut self, content: I) -> io::Result<()>
    where
        I: Iterator<Item = (u16, u16, &'a Cell)>,
    {
        self.inner.draw(content)
    }

    fn append_lines(&mut self, lines: u16) -> io::Result<()> {
        self.inner.append_lines(lines)
    }

    fn hide_cursor(&mut self) -> io::Result<()> {
        self.inner.hide_cursor()
    }

    fn show_cursor(&mut self) -> io::Result<()> {
        self.inner.show_cursor()
    }

    fn get_cursor_position(&mut self) -> io::Result<Position> {
        self.inner.get_cursor_position()
    }

    fn set_cursor_position<P: Into<Position>>(&mut self, position: P) -> io::Result<()> {
        self.inner.set_cursor_position(position)
    }

    fn clear(&mut self) -> io::Result<()> {
        self.inner.clear()
    }

    fn clear_region(&mut self, clear_type: ClearType) -> io::Result<()> {
        self.inner.clear_region(clear_type)
    }

    fn size(&self) -> io::Result<Size> {
        match self.output {
            TerminalOutput::Stdout => self.inner.size(),
            TerminalOutput::Stderr => Ok(stderr_terminal_window_size().columns_rows),
        }
    }

    fn window_size(&mut self) -> io::Result<WindowSize> {
        match self.output {
            TerminalOutput::Stdout => self.inner.window_size(),
            TerminalOutput::Stderr => Ok(stderr_terminal_window_size()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        Backend::flush(&mut self.inner)
    }
}

impl AlternateScreenTerminal<io::Stdout> {
    pub fn stdout(label: &str) -> Result<Self> {
        Self::new(io::stdout(), TerminalOutput::Stdout, label)
    }
}

impl AlternateScreenTerminal<io::Stderr> {
    pub fn stderr(label: &str) -> Result<Self> {
        Self::new(io::stderr(), TerminalOutput::Stderr, label)
    }
}

impl<W: Write> AlternateScreenTerminal<W> {
    fn new(writer: W, output: TerminalOutput, label: &str) -> Result<Self> {
        let session = TerminalSessionGuard::enter(label, output)?;
        let terminal = Terminal::new(OutputCrosstermBackend::new(writer, output))
            .with_context(|| format!("failed to initialize {label} terminal"))?;
        Ok(Self {
            terminal,
            _session: session,
        })
    }
}

impl<W: Write> Deref for AlternateScreenTerminal<W> {
    type Target = Terminal<OutputCrosstermBackend<W>>;

    fn deref(&self) -> &Self::Target {
        &self.terminal
    }
}

impl<W: Write> DerefMut for AlternateScreenTerminal<W> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.terminal
    }
}

fn enter_alternate_screen(output: TerminalOutput) -> io::Result<()> {
    match output {
        TerminalOutput::Stdout => {
            crossterm::execute!(io::stdout(), EnterAlternateScreen, Hide)
        }
        TerminalOutput::Stderr => {
            crossterm::execute!(io::stderr(), EnterAlternateScreen, Hide)
        }
    }
}

fn leave_alternate_screen(output: TerminalOutput) -> io::Result<()> {
    match output {
        TerminalOutput::Stdout => {
            crossterm::execute!(io::stdout(), Show, LeaveAlternateScreen)
        }
        TerminalOutput::Stderr => {
            crossterm::execute!(io::stderr(), Show, LeaveAlternateScreen)
        }
    }
}

#[cfg(unix)]
fn stderr_terminal_window_size() -> WindowSize {
    let mut size = libc::winsize {
        ws_row: 0,
        ws_col: 0,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    // SAFETY: stderr is a process-owned descriptor and `size` is valid writable storage.
    let result = unsafe { libc::ioctl(libc::STDERR_FILENO, libc::TIOCGWINSZ, &mut size) };
    if result == 0 && size.ws_col > 0 && size.ws_row > 0 {
        return WindowSize {
            columns_rows: Size::new(size.ws_col, size.ws_row),
            pixels: Size::new(size.ws_xpixel, size.ws_ypixel),
        };
    }
    fallback_stderr_window_size()
}

#[cfg(not(unix))]
fn stderr_terminal_window_size() -> WindowSize {
    let Ok((width, height)) = crossterm::terminal::size() else {
        return fallback_stderr_window_size();
    };
    if width == 0 || height == 0 {
        return fallback_stderr_window_size();
    }
    WindowSize {
        columns_rows: Size::new(width, height),
        pixels: Size::new(0, 0),
    }
}

fn fallback_stderr_window_size() -> WindowSize {
    WindowSize {
        columns_rows: Size::new(80, 24),
        pixels: Size::new(0, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    thread_local! {
        static CALLS: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
    }

    fn record(call: &'static str) {
        CALLS.with(|calls| calls.borrow_mut().push(call));
    }

    fn fake_enable_raw_mode() -> io::Result<()> {
        record("enable_raw_mode");
        Ok(())
    }

    fn fake_enter_alternate_screen(_: TerminalOutput) -> io::Result<()> {
        record("enter_alternate_screen");
        Err(io::Error::other("injected enter failure"))
    }

    fn fake_leave_alternate_screen(_: TerminalOutput) -> io::Result<()> {
        record("leave_alternate_screen");
        Ok(())
    }

    fn fake_disable_raw_mode() -> io::Result<()> {
        record("disable_raw_mode");
        Ok(())
    }

    #[test]
    fn partial_setup_failure_restores_raw_mode() {
        CALLS.with(|calls| calls.borrow_mut().clear());
        let result = TerminalSessionGuard::enter_with_operations(
            "test terminal",
            TerminalOutput::Stdout,
            TerminalOperations {
                enable_raw_mode: fake_enable_raw_mode,
                enter_alternate_screen: fake_enter_alternate_screen,
                leave_alternate_screen: fake_leave_alternate_screen,
                disable_raw_mode: fake_disable_raw_mode,
            },
        );

        assert!(result.is_err());
        CALLS.with(|calls| {
            assert_eq!(
                calls.borrow().as_slice(),
                [
                    "enable_raw_mode",
                    "enter_alternate_screen",
                    "disable_raw_mode"
                ]
            );
        });
    }
}
