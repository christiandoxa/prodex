use anyhow::{Context, Result, bail};
use std::io::{BufReader, BufWriter};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use super::super::{configure_child_process_group, terminate_child_process_tree};
use crate::app_server_broker::{AppServerBrokerLiveValidator, app_server_broker_pump_live_stream};

pub(super) fn run_app_server_broker_process(profile: Option<&str>) -> Result<()> {
    let (plan, runtime_proxy) =
        super::super::runtime_launch::codex_app_server_broker_launch(profile)?;
    let plan = &plan;
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
    configure_child_process_group(&mut command, true);
    let mut child = command.spawn().with_context(|| {
        format!(
            "failed to start {} app-server",
            plan.binary.to_string_lossy()
        )
    })?;
    let _runtime_proxy_lease = match runtime_proxy.as_ref() {
        Some(proxy) => match proxy.create_child_lease(child.id()) {
            Ok(lease) => Some(lease),
            Err(err) => {
                let _ = terminate_child_process_tree(&mut child, true);
                let _ = child.wait();
                return Err(err);
            }
        },
        None => None,
    };
    let child_stdin = child
        .stdin
        .take()
        .context("failed to capture Codex app-server stdin")?;
    let child_stdout = child
        .stdout
        .take()
        .context("failed to capture Codex app-server stdout")?;

    let validator = AppServerBrokerLiveValidator::new()?;
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

    let (status, mut first_error) = wait_for_app_server_broker(&mut child, &completion_rx)?;

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

fn wait_for_app_server_broker(
    child: &mut Child,
    completion_rx: &mpsc::Receiver<(&'static str, std::result::Result<(), String>)>,
) -> Result<(ExitStatus, Option<String>)> {
    let mut first_error = None;
    let mut input_finished_at = None;
    loop {
        match completion_rx.recv_timeout(Duration::from_millis(50)) {
            Ok((direction, result)) => {
                if direction == "client_to_server" {
                    input_finished_at = Some(Instant::now());
                }
                if let Err(error) = result {
                    first_error.get_or_insert_with(|| format!("{direction}: {error}"));
                    let _ = terminate_child_process_tree(child, true);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) | Err(mpsc::RecvTimeoutError::Disconnected) => {}
        }
        if let Some(status) = child.try_wait()? {
            let _ = terminate_child_process_tree(child, true);
            return Ok((status, first_error));
        }
        if input_finished_at.is_some_and(|at| at.elapsed() >= Duration::from_secs(5)) {
            first_error.get_or_insert_with(|| {
                "Codex app-server did not stop after client input closed".to_string()
            });
            let _ = terminate_child_process_tree(child, true);
            return Ok((child.wait()?, first_error));
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::wait_for_app_server_broker;
    use crate::{
        configure_child_process_group, join_thread_with_timeout, terminate_child_process_tree,
    };
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, Instant};

    #[test]
    fn exited_app_server_parent_cleans_descendant_held_pipes() {
        let mut command = Command::new("sh");
        command
            .args(["-c", "sleep 30 & exit 0"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        configure_child_process_group(&mut command, true);
        let mut child = command.spawn().unwrap();
        let mut stdout = child.stdout.take().unwrap();
        let output = thread::spawn(move || {
            let mut bytes = Vec::new();
            stdout.read_to_end(&mut bytes).unwrap();
        });
        let (_completion_tx, completion_rx) = mpsc::channel();
        let started = Instant::now();

        let (status, first_error) = wait_for_app_server_broker(&mut child, &completion_rx).unwrap();
        let output_finished = {
            let deadline = Instant::now() + Duration::from_secs(2);
            while !output.is_finished() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(10));
            }
            output.is_finished()
        };
        let _ = terminate_child_process_tree(&mut child, true);
        let _ = child.wait();
        join_thread_with_timeout(
            output,
            Duration::from_secs(2),
            "app-server broker output worker",
        )
        .unwrap();

        assert!(
            output_finished,
            "output worker did not drain after child exit"
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(status.success());
        assert!(first_error.is_none());
    }
}
