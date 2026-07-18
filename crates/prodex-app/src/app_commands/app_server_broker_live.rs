use anyhow::{Context, Result, bail};
use std::io::{BufReader, BufWriter};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use crate::app_server_broker::{AppServerBrokerLiveValidator, app_server_broker_pump_live_stream};

pub(super) fn run_app_server_broker_process(profile: Option<&str>) -> Result<()> {
    let plan = super::super::runtime_launch::codex_app_server_broker_child_plan(profile)?;
    let _session_lock = prodex_shared_codex_fs::lock_codex_sessions_for_child(&plan.codex_home)?;
    let mut command = Command::new(&plan.binary);
    command
        .args(&plan.args)
        .env("CODEX_HOME", &plan.codex_home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    for key in &plan.removed_env {
        command.env_remove(key);
    }
    for (key, value) in &plan.extra_env {
        command.env(key, value);
    }
    let mut child = command.spawn().with_context(|| {
        format!(
            "failed to start {} app-server",
            plan.binary.to_string_lossy()
        )
    })?;
    let child_stdin = child
        .stdin
        .take()
        .context("failed to capture Codex app-server stdin")?;
    let child_stdout = child
        .stdout
        .take()
        .context("failed to capture Codex app-server stdout")?;

    let validator = AppServerBrokerLiveValidator::new();
    let diagnostics = Arc::new(Mutex::new(std::io::stderr()));
    let (completion_tx, completion_rx) = mpsc::channel();

    let input_validator = validator.clone();
    let input_diagnostics = Arc::clone(&diagnostics);
    let input_tx = completion_tx.clone();
    let input_worker = thread::Builder::new()
        .name("prodex-app-server-broker-input".to_string())
        .spawn(move || {
            let result = app_server_broker_pump_live_stream(
                BufReader::new(std::io::stdin()),
                BufWriter::new(child_stdin),
                input_validator,
                input_diagnostics,
                "client_to_server",
            );
            let _ = input_tx.send((
                "client_to_server",
                result.map_err(|error| error.to_string()),
            ));
        })?;

    let output_validator = validator.clone();
    let output_diagnostics = Arc::clone(&diagnostics);
    let output_worker = thread::Builder::new()
        .name("prodex-app-server-broker-output".to_string())
        .spawn(move || {
            let result = app_server_broker_pump_live_stream(
                BufReader::new(child_stdout),
                BufWriter::new(std::io::stdout()),
                output_validator,
                output_diagnostics,
                "server_to_client",
            );
            let _ = completion_tx.send((
                "server_to_client",
                result.map_err(|error| error.to_string()),
            ));
        })?;

    let mut first_error = None;
    let mut input_finished_at = None;
    let status = loop {
        match completion_rx.recv_timeout(Duration::from_millis(50)) {
            Ok((direction, result)) => {
                if direction == "client_to_server" {
                    input_finished_at = Some(Instant::now());
                }
                if let Err(error) = result {
                    first_error.get_or_insert_with(|| format!("{direction}: {error}"));
                    let _ = child.kill();
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {}
        }
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if input_finished_at.is_some_and(|at| at.elapsed() >= Duration::from_secs(5)) {
            first_error.get_or_insert_with(|| {
                "Codex app-server did not stop after client input closed".to_string()
            });
            let _ = child.kill();
            break child.wait()?;
        }
    };

    output_worker
        .join()
        .map_err(|_| anyhow::anyhow!("app-server broker output worker panicked"))?;
    if input_worker.is_finished() {
        input_worker
            .join()
            .map_err(|_| anyhow::anyhow!("app-server broker input worker panicked"))?;
    }
    for (direction, result) in completion_rx.try_iter() {
        if let Err(error) = result {
            first_error.get_or_insert_with(|| format!("{direction}: {error}"));
        }
    }
    {
        let mut diagnostics = diagnostics
            .lock()
            .map_err(|_| anyhow::anyhow!("app-server broker diagnostics lock poisoned"))?;
        validator.finish(&mut *diagnostics)?;
    }
    if let Some(error) = first_error {
        bail!("app-server broker stopped: {error}");
    }
    if !status.success() {
        bail!("Codex app-server exited with status {status}");
    }
    Ok(())
}
