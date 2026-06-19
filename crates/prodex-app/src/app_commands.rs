use super::*;
use std::io::{BufRead, IsTerminal, Write};

mod audit;
mod broker;
mod capability;
mod child_process;
mod cleanup;
mod context;
mod dashboard;
mod doctor;
mod gateway;
mod info;
mod info_handler;
mod log;
mod mem0_memory;
mod memory_mcp;
mod presidio;
mod quota;
mod runtime_launch;
mod selection;
mod session;
mod shared;

pub(crate) use self::audit::*;
pub(crate) use self::broker::*;
pub(crate) use self::capability::*;
pub(crate) use self::child_process::*;
pub(crate) use self::cleanup::*;
pub(crate) use self::context::*;
pub(crate) use self::dashboard::*;
pub(crate) use self::doctor::*;
pub(crate) use self::gateway::*;
pub(crate) use self::info::*;
pub(crate) use self::info_handler::*;
pub(crate) use self::log::*;
pub(crate) use self::mem0_memory::*;
pub(crate) use self::memory_mcp::*;
pub(crate) use self::presidio::*;
pub(crate) use self::quota::*;
pub(crate) use self::selection::*;
pub(crate) use self::session::*;
pub(crate) use self::shared::*;

pub(super) fn handle_run(args: RunArgs) -> Result<()> {
    runtime_launch::handle_run(args)
}

pub(super) fn handle_super(args: SuperArgs) -> Result<()> {
    let use_presidio = match args.presidio_preference() {
        Some(use_presidio) => use_presidio,
        None => prompt_super_presidio_opt_in()?,
    };
    if matches!(args.cli, Some(SuperCliAgent::Gemini | SuperCliAgent::Agy)) {
        if args.mem0 {
            bail!("--mem0 is only supported by the Codex Super overlay, not native Gemini/Agy CLI");
        }
        return crate::runtime_gemini_cli::handle_super_google_cli(args, use_presidio);
    }
    let use_mem0 = match args.mem0_preference() {
        Some(use_mem0) => use_mem0,
        None => prompt_super_mem0_opt_in()?,
    };
    handle_caveman(args.into_caveman_args_with_choices(use_presidio, use_mem0))
}

pub(super) fn prepare_runtime_launch(
    request: RuntimeLaunchRequest<'_>,
) -> Result<PreparedRuntimeLaunch> {
    runtime_launch::prepare_runtime_launch(request)
}

pub(super) fn prepare_runtime_launch_dry_run(
    request: RuntimeLaunchRequest<'_>,
) -> Result<PreparedRuntimeLaunch> {
    runtime_launch::prepare_runtime_launch_dry_run(request)
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn resolve_runtime_launch_profile_name(
    state: &AppState,
    requested: Option<&str>,
) -> Result<String> {
    runtime_launch::resolve_runtime_launch_profile_name(state, requested)
}

fn prompt_super_presidio_opt_in() -> Result<bool> {
    let stdin = io::stdin();
    let stderr = io::stderr();
    prompt_super_presidio_opt_in_from(
        stdin.is_terminal(),
        stderr.is_terminal(),
        stdin.lock(),
        stderr,
    )
}

fn prompt_super_mem0_opt_in() -> Result<bool> {
    let stdin = io::stdin();
    let stderr = io::stderr();
    prompt_super_mem0_opt_in_from(
        stdin.is_terminal(),
        stderr.is_terminal(),
        stdin.lock(),
        stderr,
    )
}

fn prompt_super_presidio_opt_in_from<R, W>(
    stdin_is_terminal: bool,
    stderr_is_terminal: bool,
    mut input: R,
    mut output: W,
) -> Result<bool>
where
    R: BufRead,
    W: Write,
{
    if !stdin_is_terminal || !stderr_is_terminal {
        return Ok(false);
    }

    write!(output, "Use Presidio for data safety? [y/N] ")?;
    output.flush().context("failed to flush prompt")?;
    let mut answer = String::new();
    input
        .read_line(&mut answer)
        .context("failed to read Presidio prompt answer")?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn prompt_super_mem0_opt_in_from<R, W>(
    stdin_is_terminal: bool,
    stderr_is_terminal: bool,
    mut input: R,
    mut output: W,
) -> Result<bool>
where
    R: BufRead,
    W: Write,
{
    if !stdin_is_terminal || !stderr_is_terminal {
        return Ok(false);
    }

    write!(
        output,
        "Enable prodex-memory via managed Mem0 Docker? [y/N] "
    )?;
    output.flush().context("failed to flush prompt")?;
    let mut answer = String::new();
    input
        .read_line(&mut answer)
        .context("failed to read Mem0 memory prompt answer")?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prompt_answer(answer: &str) -> Result<(bool, String)> {
        let mut output = Vec::new();
        let enabled =
            prompt_super_presidio_opt_in_from(true, true, io::Cursor::new(answer), &mut output)?;
        let output = String::from_utf8(output).context("prompt output should be UTF-8")?;
        Ok((enabled, output))
    }

    #[test]
    fn super_presidio_prompt_accepts_y_and_yes() -> Result<()> {
        for answer in ["y\n", "Y\n", "yes\n", "YES\n"] {
            let (enabled, output) = prompt_answer(answer)?;
            assert!(enabled, "{answer:?} should opt in");
            assert_eq!(output, "Use Presidio for data safety? [y/N] ");
        }
        Ok(())
    }

    #[test]
    fn super_presidio_prompt_rejects_n_and_default() -> Result<()> {
        for answer in ["n\n", "N\n", "\n", "no\n"] {
            let (enabled, output) = prompt_answer(answer)?;
            assert!(!enabled, "{answer:?} should not opt in");
            assert_eq!(output, "Use Presidio for data safety? [y/N] ");
        }
        Ok(())
    }

    #[test]
    fn super_presidio_prompt_skips_non_terminal_io() -> Result<()> {
        for (stdin_is_terminal, stderr_is_terminal) in [(false, true), (true, false)] {
            let mut output = Vec::new();
            let enabled = prompt_super_presidio_opt_in_from(
                stdin_is_terminal,
                stderr_is_terminal,
                io::Cursor::new("y\n"),
                &mut output,
            )?;
            assert!(!enabled);
            assert!(output.is_empty());
        }
        Ok(())
    }

    fn mem0_prompt_answer(answer: &str) -> Result<(bool, String)> {
        let mut output = Vec::new();
        let enabled =
            prompt_super_mem0_opt_in_from(true, true, io::Cursor::new(answer), &mut output)?;
        let output = String::from_utf8(output).context("prompt output should be UTF-8")?;
        Ok((enabled, output))
    }

    #[test]
    fn super_mem0_prompt_accepts_y_and_yes() -> Result<()> {
        for answer in ["y\n", "Y\n", "yes\n", "YES\n"] {
            let (enabled, output) = mem0_prompt_answer(answer)?;
            assert!(enabled, "{answer:?} should opt in");
            assert_eq!(
                output,
                "Enable prodex-memory via managed Mem0 Docker? [y/N] "
            );
        }
        Ok(())
    }

    #[test]
    fn super_mem0_prompt_rejects_n_and_default() -> Result<()> {
        for answer in ["n\n", "N\n", "\n", "no\n"] {
            let (enabled, output) = mem0_prompt_answer(answer)?;
            assert!(!enabled, "{answer:?} should not opt in");
            assert_eq!(
                output,
                "Enable prodex-memory via managed Mem0 Docker? [y/N] "
            );
        }
        Ok(())
    }
}
