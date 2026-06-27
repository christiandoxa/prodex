use crate::audit_log::append_audit_event;
use anyhow::Result;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use std::cell::RefCell;
use std::env;
use std::io::{self, IsTerminal};
use terminal_ui::draw_status_panel_terminal;

pub(crate) use prodex_core::{absolutize, default_codex_home};
#[cfg(test)]
pub(crate) use prodex_core::{same_path, select_default_codex_home};

pub(crate) fn audit_log_event_best_effort(
    component: &str,
    action: &str,
    outcome: &str,
    details: serde_json::Value,
) {
    let _ = append_audit_event(component, action, outcome, details);
}

pub(crate) fn print_launch_status(message: &str) {
    LAUNCH_STATUS_TUI.with(|state| {
        let mut state = state.borrow_mut();
        match state.render(message) {
            Ok(()) => {}
            Err(_) => eprintln!("Prodex launch: {message}"),
        }
    });
}

pub(crate) fn try_inline_stdout_terminal(
    height: u16,
) -> Option<Terminal<CrosstermBackend<io::Stdout>>> {
    if !inline_tui_allowed() || !io::stdout().is_terminal() {
        return None;
    }
    Terminal::with_options(
        CrosstermBackend::new(io::stdout()),
        ratatui::TerminalOptions {
            viewport: ratatui::Viewport::Inline(height),
        },
    )
    .ok()
}

pub(crate) fn try_inline_stderr_terminal(
    height: u16,
) -> Option<Terminal<CrosstermBackend<io::Stderr>>> {
    if !inline_tui_allowed() || !io::stderr().is_terminal() {
        return None;
    }
    Terminal::with_options(
        CrosstermBackend::new(io::stderr()),
        ratatui::TerminalOptions {
            viewport: ratatui::Viewport::Inline(height),
        },
    )
    .ok()
}

fn inline_tui_allowed() -> bool {
    if env::var_os("PRODEX_FORCE_TUI").is_some() {
        return true;
    }
    env::var_os("CODEX_CI").is_none()
        && env::var("CI")
            .map(|value| {
                !matches!(
                    value.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes"
                )
            })
            .unwrap_or(true)
}

thread_local! {
    static LAUNCH_STATUS_TUI: RefCell<LaunchStatusTui> = RefCell::new(LaunchStatusTui::default());
}

#[derive(Default)]
struct LaunchStatusTui {
    terminal: Option<Terminal<CrosstermBackend<io::Stderr>>>,
    disabled: bool,
}

impl LaunchStatusTui {
    fn render(&mut self, message: &str) -> Result<()> {
        if self.disabled {
            anyhow::bail!("launch status TUI disabled");
        }
        if self.terminal.is_none() {
            self.terminal = try_inline_stderr_terminal(5);
        }
        let Some(terminal) = self.terminal.as_mut() else {
            self.disabled = true;
            anyhow::bail!("stderr is not an inline-capable terminal");
        };
        if let Err(err) = render_launch_status_tui(terminal, message) {
            self.disabled = true;
            anyhow::bail!(err);
        }
        Ok(())
    }
}

fn render_launch_status_tui(
    terminal: &mut Terminal<CrosstermBackend<io::Stderr>>,
    message: &str,
) -> Result<()> {
    draw_status_panel_terminal(terminal, "Prodex Launch", "preflight", "Status", message)
}
