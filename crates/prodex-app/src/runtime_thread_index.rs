use crate::ChildProcessPlan;
use anyhow::{Context, Result, bail};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const THREAD_INDEX_PAGE_LIMIT: u64 = 100;
const THREAD_INDEX_TIMEOUT: Duration = Duration::from_secs(60);
const THREAD_INDEX_CLEANUP_TIMEOUT: Duration = Duration::from_secs(1);

/// Runs Codex's own scan-and-repair listing against the exact child home and environment.
///
/// `useStateDbOnly` stays false deliberately: Codex owns the SQLite schema and its normal
/// listing path is the compatibility layer that repairs rollout/index divergence.
pub(crate) fn reconcile_codex_thread_index(
    codex_binary: &OsStr,
    child: &ChildProcessPlan,
) -> Result<()> {
    let mut command = Command::new(codex_binary);
    command
        .arg("app-server")
        .env("CODEX_HOME", &child.codex_home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for key in &child.removed_env {
        command.env_remove(key);
    }
    for (key, value) in &child.extra_env {
        command.env(key, value);
    }
    crate::configure_child_process_group(&mut command, true);
    let mut process = command.spawn().with_context(|| {
        format!(
            "failed to start {} app-server for thread index reconciliation",
            codex_binary.to_string_lossy()
        )
    })?;
    let stdin = process
        .stdin
        .take()
        .context("failed to capture thread index reconciliation stdin")?;
    let stdout = process
        .stdout
        .take()
        .context("failed to capture thread index reconciliation stdout")?;
    let (completion_tx, completion_rx) = mpsc::channel();
    let worker = thread::Builder::new()
        .name("prodex-thread-index-reconciliation".to_string())
        .spawn(move || {
            let result = reconcile_codex_thread_index_protocol(
                &mut BufReader::new(stdout),
                &mut BufWriter::new(stdin),
            );
            if completion_tx.send(result).is_err() {
                // The caller has already timed out; the process cleanup below still owns
                // termination and reaping.
            }
        })
        .context("failed to start thread index reconciliation worker")?;

    let result = match completion_rx.recv_timeout(THREAD_INDEX_TIMEOUT) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            Err(anyhow::anyhow!("thread index reconciliation timed out"))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(anyhow::anyhow!(
            "thread index reconciliation worker stopped"
        )),
    };
    let _ = crate::terminate_child_process_tree(&mut process, true);
    let _ = process.wait();
    crate::join_thread_with_timeout(
        worker,
        THREAD_INDEX_CLEANUP_TIMEOUT,
        "thread index reconciliation worker",
    )?;
    result
}

pub(crate) fn reconcile_codex_thread_index_protocol(
    reader: &mut impl BufRead,
    writer: &mut impl Write,
) -> Result<()> {
    let mut request_id = 1_u64;
    write_app_server_message(
        writer,
        &serde_json::json!({
            "id": request_id,
            "method": "initialize",
            "params": {
                "clientInfo": {
                    "name": "prodex-thread-index-reconciliation",
                    "version": env!("CARGO_PKG_VERSION"),
                }
            }
        }),
    )?;
    read_app_server_response(reader, request_id)?;
    write_app_server_message(writer, &serde_json::json!({"method": "initialized"}))?;

    for archived in [false, true] {
        let mut cursor = None;
        let mut seen_cursors = HashSet::new();
        loop {
            request_id += 1;
            write_app_server_message(
                writer,
                &serde_json::json!({
                    "id": request_id,
                    "method": "thread/list",
                    "params": {
                        "archived": archived,
                        "cursor": cursor,
                        "limit": THREAD_INDEX_PAGE_LIMIT,
                        "modelProviders": [],
                        "sortKey": "updated_at",
                        "sourceKinds": [],
                        "useStateDbOnly": false,
                    }
                }),
            )?;
            let result = read_app_server_response(reader, request_id)?;
            let next_cursor = match result.get("nextCursor") {
                None | Some(serde_json::Value::Null) => None,
                Some(serde_json::Value::String(cursor)) => Some(cursor.clone()),
                Some(_) => bail!("Codex app-server returned an invalid thread list cursor"),
            };
            let Some(next_cursor) = next_cursor else {
                break;
            };
            if !seen_cursors.insert(next_cursor.clone()) {
                bail!("Codex app-server repeated a thread list cursor");
            }
            cursor = Some(next_cursor);
        }
    }
    Ok(())
}

fn write_app_server_message(writer: &mut impl Write, message: &serde_json::Value) -> Result<()> {
    serde_json::to_writer(&mut *writer, message).context("failed to encode app-server request")?;
    writer.write_all(b"\n")?;
    writer.flush().context("failed to send app-server request")
}

fn read_app_server_response(
    reader: &mut impl BufRead,
    request_id: u64,
) -> Result<serde_json::Value> {
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            bail!("Codex app-server stopped during thread index reconciliation");
        }
        let mut message: serde_json::Value = serde_json::from_str(&line)
            .context("Codex app-server returned invalid JSON during thread index reconciliation")?;
        if message.get("id").and_then(serde_json::Value::as_u64) != Some(request_id) {
            continue;
        }
        if let Some(error) = message.get("error") {
            let detail = error
                .get("message")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown app-server error");
            bail!("Codex thread index reconciliation failed: {detail}");
        }
        return message
            .get_mut("result")
            .map(serde_json::Value::take)
            .context("Codex app-server response is missing its result");
    }
}

#[cfg(test)]
mod tests {
    use super::reconcile_codex_thread_index_protocol;

    #[test]
    fn reconciliation_scans_all_active_and_archived_pages() {
        let responses = concat!(
            "{\"id\":1,\"result\":{}}\n",
            "{\"method\":\"remoteControl/status/changed\",\"params\":{}}\n",
            "{\"id\":2,\"result\":{\"data\":[],\"nextCursor\":\"active-next\"}}\n",
            "{\"id\":3,\"result\":{\"data\":[],\"nextCursor\":null}}\n",
            "{\"id\":4,\"result\":{\"data\":[],\"nextCursor\":null}}\n",
        );
        let mut reader = std::io::Cursor::new(responses.as_bytes());
        let mut written = Vec::new();

        reconcile_codex_thread_index_protocol(&mut reader, &mut written).unwrap();

        let requests = String::from_utf8(written)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(requests.len(), 5);
        assert_eq!(requests[0]["method"], "initialize");
        assert_eq!(requests[1]["method"], "initialized");
        assert_eq!(requests[2]["method"], "thread/list");
        assert_eq!(requests[2]["params"]["archived"], false);
        assert_eq!(requests[2]["params"]["cursor"], serde_json::Value::Null);
        assert_eq!(requests[2]["params"]["useStateDbOnly"], false);
        assert_eq!(
            requests[2]["params"]["modelProviders"],
            serde_json::json!([])
        );
        assert_eq!(requests[3]["params"]["cursor"], "active-next");
        assert_eq!(requests[4]["params"]["archived"], true);
    }

    #[test]
    fn reconciliation_rejects_repeated_cursor() {
        let responses = concat!(
            "{\"id\":1,\"result\":{}}\n",
            "{\"id\":2,\"result\":{\"nextCursor\":\"same\"}}\n",
            "{\"id\":3,\"result\":{\"nextCursor\":\"same\"}}\n",
        );
        let mut reader = std::io::Cursor::new(responses.as_bytes());
        let mut written = Vec::new();

        let error = reconcile_codex_thread_index_protocol(&mut reader, &mut written).unwrap_err();

        assert!(error.to_string().contains("repeated"));
    }
}
