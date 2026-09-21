use std::cell::RefCell;
use std::io;

use anyhow::Result;
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use terminal_ui::draw_status_panel_terminal;

use crate::{print_stderr_line, try_inline_stderr_terminal};

thread_local! {
    static PROFILE_IMPORT_PROGRESS_TUI: RefCell<ProfileImportProgressTui> =
        RefCell::new(ProfileImportProgressTui::default());
}

pub(super) fn print_profile_import_progress(message: &str) -> Result<()> {
    PROFILE_IMPORT_PROGRESS_TUI.with(|state| {
        let mut state = state.borrow_mut();
        match state.render(message) {
            Ok(()) => Ok(()),
            Err(_) => print_stderr_line(message).map_err(Into::into),
        }
    })
}

#[derive(Default)]
struct ProfileImportProgressTui {
    terminal: Option<Terminal<CrosstermBackend<io::Stderr>>>,
    disabled: bool,
}

impl ProfileImportProgressTui {
    fn render(&mut self, message: &str) -> Result<()> {
        if self.disabled {
            anyhow::bail!("profile import progress TUI disabled");
        }
        if self.terminal.is_none() {
            self.terminal = try_inline_stderr_terminal(3);
        }
        let Some(terminal) = self.terminal.as_mut() else {
            self.disabled = true;
            anyhow::bail!("stderr is not an inline-capable terminal");
        };
        if let Err(err) = render_profile_import_progress_tui(terminal, message) {
            self.disabled = true;
            anyhow::bail!(err);
        }
        let _ = terminal.show_cursor();
        Ok(())
    }
}

fn render_profile_import_progress_tui(
    terminal: &mut Terminal<CrosstermBackend<io::Stderr>>,
    message: &str,
) -> Result<()> {
    draw_status_panel_terminal(terminal, "Profile Import", "progress", "Status", message)
}
