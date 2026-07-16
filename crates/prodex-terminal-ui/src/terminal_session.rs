use anyhow::{Context, Result};
use crossterm::cursor::{Hide, Show};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
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
    pub terminal: Terminal<CrosstermBackend<W>>,
    _session: TerminalSessionGuard,
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
        let terminal = Terminal::new(CrosstermBackend::new(writer))
            .with_context(|| format!("failed to initialize {label} terminal"))?;
        Ok(Self {
            terminal,
            _session: session,
        })
    }
}

impl<W: Write> Deref for AlternateScreenTerminal<W> {
    type Target = Terminal<CrosstermBackend<W>>;

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
