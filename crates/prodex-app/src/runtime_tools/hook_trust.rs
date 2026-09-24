use crate::ChildProcessPlan;
use anyhow::{Context, Result, anyhow, bail};
use serde_json::{Value, json};
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

const HOOK_TRUST_RPC_TIMEOUT: Duration = Duration::from_secs(12);
const METHOD_NOT_FOUND: i64 = -32601;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum HookTrustPreflight {
    Trusted,
    LegacyBypass,
}

#[derive(Debug)]
enum RpcReply {
    Result(Value),
    Error { code: i64, message: String },
}

pub(super) fn trust_super_hooks(
    plan: &ChildProcessPlan,
    workspace: &Path,
) -> Result<HookTrustPreflight> {
    crate::validate_selected_codex_binary(&plan.binary)?;

    let mut command = Command::new(&plan.binary);
    let mut preflight_args = app_server_config_args(&plan.args);
    preflight_args.extend([OsString::from("app-server"), OsString::from("--stdio")]);
    command
        .args(preflight_args)
        .current_dir(workspace)
        .env("CODEX_HOME", &plan.codex_home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    for key in &plan.removed_env {
        command.env_remove(key);
    }
    for (key, value) in &plan.extra_env {
        command.env(key, value);
    }

    let mut child = command.spawn().with_context(|| {
        format!(
            "failed to start Codex hook-trust preflight with {}",
            plan.binary.to_string_lossy()
        )
    })?;
    let mut stdin = child
        .stdin
        .take()
        .context("Codex hook-trust preflight stdin was unavailable")?;
    let stdout = child
        .stdout
        .take()
        .context("Codex hook-trust preflight stdout was unavailable")?;

    let (sender, receiver) = mpsc::channel();
    let reader = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(line) => {
                    if sender.send(Ok(line)).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    let _ = sender.send(Err(error.to_string()));
                    break;
                }
            }
        }
    });

    let result = run_hook_trust_preflight(&mut stdin, &receiver, workspace);

    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
    let _ = reader.join();
    result
}

fn run_hook_trust_preflight(
    stdin: &mut impl Write,
    receiver: &mpsc::Receiver<std::result::Result<String, String>>,
    workspace: &Path,
) -> Result<HookTrustPreflight> {
    expect_rpc_result(
        rpc_request(
            stdin,
            receiver,
            1,
            "initialize",
            json!({
                "clientInfo": {
                    "name": "prodex-hook-trust",
                    "version": env!("CARGO_PKG_VERSION"),
                },
                "capabilities": {
                    "experimentalApi": true,
                },
            }),
        )?,
        "initialize",
    )?;

    let workspace = workspace.to_string_lossy().into_owned();
    let Some(listed) = rpc_result_or_legacy(
        rpc_request(
            stdin,
            receiver,
            2,
            "hooks/list",
            json!({ "cwds": [workspace] }),
        )?,
        "hooks/list",
    )?
    else {
        return Ok(HookTrustPreflight::LegacyBypass);
    };
    let updates = hook_trust_updates(&listed)?;

    if !updates.is_empty() {
        let Some(written) = rpc_result_or_legacy(
            rpc_request(
                stdin,
                receiver,
                3,
                "config/batchWrite",
                json!({
                    "edits": [{
                        "keyPath": "hooks.state",
                        "value": updates,
                        "mergeStrategy": "upsert",
                    }],
                    "filePath": null,
                    "expectedVersion": null,
                    "reloadUserConfig": true,
                }),
            )?,
            "config/batchWrite",
        )?
        else {
            return Ok(HookTrustPreflight::LegacyBypass);
        };
        if !hook_trust_config_write_succeeded(&written) {
            let status = written
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or("<missing>");
            bail!("Codex hook-trust config write did not report a successful status: {status}");
        }
    }

    let verified = expect_rpc_result(
        rpc_request(
            stdin,
            receiver,
            4,
            "hooks/list",
            json!({ "cwds": [workspace] }),
        )?,
        "hooks/list verification",
    )?;
    verify_hooks_trusted(&verified)?;
    Ok(HookTrustPreflight::Trusted)
}

fn rpc_result_or_legacy(reply: RpcReply, operation: &str) -> Result<Option<Value>> {
    match reply {
        RpcReply::Error {
            code: METHOD_NOT_FOUND,
            ..
        } => Ok(None),
        reply => expect_rpc_result(reply, operation).map(Some),
    }
}

fn app_server_config_args(runtime_args: &[OsString]) -> Vec<OsString> {
    let mut args = Vec::new();
    let mut index = 0;
    while index < runtime_args.len() {
        let argument = runtime_args[index].to_string_lossy();
        let next = runtime_args.get(index + 1).cloned();
        match argument.as_ref() {
            "-c" | "--config" | "--enable" | "--disable" | "--code-mode-host" => {
                if let Some(value) = next {
                    args.extend([runtime_args[index].clone(), value]);
                    index += 2;
                    continue;
                }
            }
            "--strict-config" => args.push(runtime_args[index].clone()),
            "-m" | "--model" => {
                if let Some(value) = next.and_then(|value| value.into_string().ok()) {
                    args.extend([
                        OsString::from("-c"),
                        OsString::from(format!(
                            "model={}",
                            crate::runtime_catalog_config::toml_string_literal(&value)
                        )),
                    ]);
                    index += 2;
                    continue;
                }
            }
            value if value.starts_with("--model=") => {
                args.extend([
                    OsString::from("-c"),
                    OsString::from(format!(
                        "model={}",
                        crate::runtime_catalog_config::toml_string_literal(
                            value.trim_start_matches("--model=")
                        )
                    )),
                ]);
            }
            value if value.starts_with("--config=") || value.starts_with("-c=") => {
                args.push(runtime_args[index].clone());
            }
            _ => {}
        }
        index += 1;
    }
    args
}

fn rpc_request(
    stdin: &mut impl Write,
    receiver: &mpsc::Receiver<std::result::Result<String, String>>,
    id: i64,
    method: &str,
    params: Value,
) -> Result<RpcReply> {
    serde_json::to_writer(
        &mut *stdin,
        &json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        }),
    )?;
    stdin.write_all(b"\n")?;
    stdin.flush()?;

    let deadline = Instant::now() + HOOK_TRUST_RPC_TIMEOUT;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            bail!("Codex hook-trust preflight timed out waiting for {method}");
        }
        let line = receiver
            .recv_timeout(remaining)
            .map_err(|error| anyhow!("Codex hook-trust preflight {method} failed: {error}"))?
            .map_err(|error| anyhow!("Codex hook-trust preflight stdout failed: {error}"))?;
        let Ok(frame) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if frame.get("id").and_then(Value::as_i64) != Some(id) {
            continue;
        }
        if let Some(error) = frame.get("error") {
            return Ok(RpcReply::Error {
                code: error
                    .get("code")
                    .and_then(Value::as_i64)
                    .unwrap_or_default(),
                message: error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown JSON-RPC error")
                    .to_string(),
            });
        }
        return Ok(RpcReply::Result(
            frame
                .get("result")
                .cloned()
                .context("Codex hook-trust response omitted result")?,
        ));
    }
}

fn expect_rpc_result(reply: RpcReply, operation: &str) -> Result<Value> {
    match reply {
        RpcReply::Result(value) => Ok(value),
        RpcReply::Error { code, message } => {
            bail!("Codex hook-trust {operation} failed ({code}): {message}")
        }
    }
}

fn hook_trust_config_write_succeeded(result: &Value) -> bool {
    matches!(
        result.get("status").and_then(Value::as_str),
        Some("ok" | "okOverridden")
    )
}

fn hook_trust_updates(listed: &Value) -> Result<serde_json::Map<String, Value>> {
    validate_hook_list_diagnostics(listed)?;
    let mut updates = serde_json::Map::new();
    for entry in hook_list_entries(listed)? {
        let Some(hooks) = entry.get("hooks").and_then(Value::as_array) else {
            bail!("Codex hooks/list entry omitted hooks");
        };
        for hook in hooks {
            if !hook_needs_trust(hook) {
                continue;
            }
            let key = hook
                .get("key")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .context("Codex hooks/list returned a trustable hook without key")?;
            let current_hash = hook
                .get("currentHash")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .context("Codex hooks/list returned a trustable hook without currentHash")?;
            updates.insert(
                key.to_string(),
                json!({
                    "trusted_hash": current_hash,
                }),
            );
        }
    }
    Ok(updates)
}

fn verify_hooks_trusted(listed: &Value) -> Result<()> {
    validate_hook_list_diagnostics(listed)?;
    for entry in hook_list_entries(listed)? {
        let Some(hooks) = entry.get("hooks").and_then(Value::as_array) else {
            bail!("Codex hooks/list entry omitted hooks");
        };
        if let Some(hook) = hooks.iter().find(|hook| hook_needs_trust(hook)) {
            let key = hook
                .get("key")
                .and_then(Value::as_str)
                .unwrap_or("<unknown>");
            bail!("Codex hook remained untrusted after Super preflight: {key}");
        }
    }
    Ok(())
}

fn hook_needs_trust(hook: &Value) -> bool {
    matches!(
        hook.get("trustStatus").and_then(Value::as_str),
        Some("untrusted" | "modified")
    )
}

fn hook_list_entries(listed: &Value) -> Result<&[Value]> {
    listed
        .get("data")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .context("Codex hooks/list response omitted data")
}

fn validate_hook_list_diagnostics(listed: &Value) -> Result<()> {
    for entry in hook_list_entries(listed)? {
        for field in ["warnings", "errors"] {
            let Some(messages) = entry.get(field).and_then(Value::as_array) else {
                continue;
            };
            let rendered = messages
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>();
            if !rendered.is_empty() {
                bail!("Codex hooks/list reported {field}: {}", rendered.join("; "));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_trust_config_write_accepts_codex_success_statuses() {
        assert!(hook_trust_config_write_succeeded(&json!({"status": "ok"})));
        assert!(hook_trust_config_write_succeeded(
            &json!({"status": "okOverridden"})
        ));
        assert!(!hook_trust_config_write_succeeded(
            &json!({"status": "error"})
        ));
        assert!(!hook_trust_config_write_succeeded(&json!({})));
    }

    #[test]
    fn hook_trust_updates_include_only_untrusted_or_modified_hooks() {
        let listed = json!({
            "data": [{
                "cwd": "/repo",
                "hooks": [
                    {"key":"a","currentHash":"sha256:a","trustStatus":"untrusted"},
                    {"key":"b","currentHash":"sha256:b","trustStatus":"modified"},
                    {"key":"c","currentHash":"sha256:c","trustStatus":"trusted"}
                ],
                "warnings": [],
                "errors": []
            }]
        });
        assert_eq!(
            Value::Object(hook_trust_updates(&listed).unwrap()),
            json!({
                "a":{"trusted_hash":"sha256:a"},
                "b":{"trusted_hash":"sha256:b"}
            })
        );
        assert!(verify_hooks_trusted(&listed).is_err());
    }

    #[test]
    fn trusted_hook_list_verifies_without_updates() {
        let listed = json!({
            "data": [{
                "cwd": "/repo",
                "hooks": [
                    {"key":"c","currentHash":"sha256:c","trustStatus":"trusted"}
                ],
                "warnings": [],
                "errors": []
            }]
        });
        assert!(hook_trust_updates(&listed).unwrap().is_empty());
        verify_hooks_trusted(&listed).unwrap();
    }
}
