//! Post-child terminal output and launch-owned session identity acquisition.
use crate::RuntimeLaunchPlan;
use std::io::{self, IsTerminal, Read, Write};
use uuid::Uuid;

pub(crate) fn print_saved_session_guidance(plan: &RuntimeLaunchPlan) {
    if plan.companion.is_none()
        || !plan.child.reset_terminal_keyboard_enhancement
        || !io::stdin().is_terminal()
        || !io::stdout().is_terminal()
        || !io::stderr().is_terminal()
    {
        return;
    }
    let session_id = saved_session_id(plan);
    // The child has exited and terminal modes are restored. Do not create a
    // Ratatui inline viewport: its blank diff buffer overwrites native output
    // while leaving old glyphs in every unwritten space. CRLF text is appended
    // intact, with no cursor query/movement, width truncation or screen clear.
    let _ = io::stdout().lock().flush();
    let _ = write_saved_session_guidance(&mut io::stderr().lock(), session_id);
}

fn saved_session_id(plan: &RuntimeLaunchPlan) -> Option<Uuid> {
    // The hook's root identity is authoritative, including /new or /resume
    // inside this TUI. Never choose another terminal's newest session file.
    if let Some(path) = plan.saved_session_marker.as_ref()
        && let Ok(file) = std::fs::File::open(path)
    {
        let mut raw = String::new();
        if file.take(128).read_to_string(&mut raw).is_ok()
            && let Ok(id) = Uuid::parse_str(raw.trim())
        {
            return Some(id);
        }
    }
    prodex_runtime_launch::codex_resume_session_id(&plan.child.args)
        .and_then(|id| Uuid::parse_str(id).ok())
}

fn write_saved_session_guidance(
    writer: &mut impl Write,
    session_id: Option<Uuid>,
) -> io::Result<()> {
    let canonical = session_id.map(|id| id.to_string());
    let text = prodex_mojo_core::launch::saved_session_guidance(canonical.as_deref()).map_err(
        |error| io::Error::other(format!("Mojo saved-session guidance failed: {error:?}")),
    )?;
    writer.write_all(text.as_bytes())?;
    writer.flush()
}

#[cfg(test)]
#[path = "child_process_saved_session_tests.rs"]
mod tests;
