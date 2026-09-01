use super::super::super_expose::{
    ExposeEngineRequest, ExposeLifecycleEvent, run_super_expose_engine,
};
use super::support::copy_public_url_to_clipboard;
use super::{ExposeTuiAction, ExposeTuiPhase, ExposeTuiState};
use anyhow::{Context, Result};
use crossterm::event;
use prodex_cli::SuperArgs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};

pub(super) fn redraw_if_needed(
    terminal: &mut super::ExposeTuiTerminal,
    state: &mut super::ExposeTuiState,
) -> Result<()> {
    if !state.redraw_needed {
        return Ok(());
    }
    terminal
        .autoresize()
        .context("failed to resize Super expose TUI")?;
    super::draw(terminal, state)?;
    state.redraw_needed = false;
    Ok(())
}

pub(super) fn reap_finished_worker(
    state: &mut ExposeTuiState,
    worker: &mut Option<JoinHandle<Result<()>>>,
) -> Result<()> {
    if !worker.as_ref().is_some_and(JoinHandle::is_finished) {
        return Ok(());
    }
    let Some(worker_handle) = worker.take() else {
        return Ok(());
    };
    let finished = worker_handle
        .join()
        .map_err(|_| anyhow::anyhow!("expose lifecycle worker panicked"))?;
    if let Err(error) = finished
        && !matches!(
            state.phase,
            ExposeTuiPhase::Failed | ExposeTuiPhase::Stopped
        )
    {
        state.apply_engine_event(ExposeLifecycleEvent::Failed(
            redaction::redaction_redact_secret_like_text(&format!("{error:#}")),
        ));
    }
    Ok(())
}

pub(super) fn handle_signal(
    state: &mut ExposeTuiState,
    cancel: &AtomicBool,
    worker: &Option<JoinHandle<Result<()>>>,
    stopping: &mut bool,
) -> bool {
    if !signal_requested() || *stopping {
        return false;
    }
    if worker.is_none() {
        return true;
    }
    cancel.store(true, Ordering::SeqCst);
    state.set_stopping();
    *stopping = true;
    false
}

fn signal_requested() -> bool {
    #[cfg(unix)]
    {
        crate::InteractiveSigintGuard::count() > 0
    }
    #[cfg(not(unix))]
    {
        false
    }
}

pub(super) fn should_finish(
    state: &ExposeTuiState,
    worker: &Option<JoinHandle<Result<()>>>,
    stopping: bool,
) -> bool {
    worker.is_none()
        && (stopping
            || matches!(
                state.phase,
                ExposeTuiPhase::Stopped | ExposeTuiPhase::Failed
            ))
}

pub(super) fn handle_input(
    state: &mut ExposeTuiState,
    event_tx: &mpsc::SyncSender<ExposeLifecycleEvent>,
    cancel: &Arc<AtomicBool>,
    launch: &mut Option<(crate::ExposeArgs, SuperArgs, PathBuf, String, String)>,
    worker: &mut Option<JoinHandle<Result<()>>>,
    stopping: &mut bool,
) -> Result<bool> {
    let action =
        state.handle_event(event::read().context("failed to read Super expose TUI input")?);
    handle_action(state, event_tx, cancel, launch, worker, stopping, action)
}

pub(super) fn handle_action(
    state: &mut ExposeTuiState,
    event_tx: &mpsc::SyncSender<ExposeLifecycleEvent>,
    cancel: &Arc<AtomicBool>,
    launch: &mut Option<(crate::ExposeArgs, SuperArgs, PathBuf, String, String)>,
    worker: &mut Option<JoinHandle<Result<()>>>,
    stopping: &mut bool,
    action: super::ExposeTuiAction,
) -> Result<bool> {
    match action {
        ExposeTuiAction::None => {}
        ExposeTuiAction::CopyUrl => {
            if let Some(ready) = state.ready.as_ref()
                && let Some(public_url) = ready.public_url.as_ref()
            {
                match copy_public_url_to_clipboard(public_url) {
                    Ok(()) => state.set_status("MCP URL copied to clipboard"),
                    Err(_) => state.set_status("clipboard is unavailable"),
                }
            }
        }
        ExposeTuiAction::Start {
            endpoint,
            existing,
            openai_credentials,
        } => {
            let (args, super_args, workspace_root, workspace_name, display_name) =
                launch.take().context("expose endpoint selected twice")?;
            let mut args = args;
            if let Some(selection) = existing.map(|selection| *selection) {
                args.cloudflare_config = selection.config_path;
                args.cloudflare_token_file = selection.token_file;
                args.cloudflare_tunnel = selection.tunnel;
                args.cloudflare_hostname = Some(selection.hostname);
                args.cloudflare_origin_port = Some(selection.origin_port);
            }
            let event_tx = event_tx.clone();
            let cancel = Arc::clone(cancel);
            *worker = Some(thread::spawn(move || {
                run_super_expose_engine(
                    ExposeEngineRequest {
                        args,
                        super_args,
                        workspace_root,
                        workspace_name,
                        display_name,
                        endpoint,
                        openai_credentials,
                    },
                    Some(event_tx),
                    cancel,
                )
            }));
        }
        ExposeTuiAction::Stop => {
            if worker.is_none() {
                return Ok(true);
            }
            cancel.store(true, Ordering::SeqCst);
            state.set_stopping();
            *stopping = true;
        }
    }
    Ok(false)
}
