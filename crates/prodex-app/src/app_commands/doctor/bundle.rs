use super::{
    audit_logs_json_value, codex_bin, collect_install_check_rows,
    collect_live_runtime_broker_observations, collect_profile_summaries,
    collect_runtime_doctor_summary_with_tail_bytes, doctor_quota_reports_json_value,
    first_line_of_error, format_binary_resolution, format_runtime_policy_summary, kiro_bin,
    runtime_current_prodex_version, runtime_doctor_json_value,
    runtime_doctor_json_value_with_policy_suggestions, runtime_logs_json_value,
    runtime_policy_json_value, runtime_proxy_latest_log_pointer_path, secret_backend_json_value,
};
use crate::{AppPaths, AppState, DoctorArgs, RuntimeConfig};
use anyhow::{Context, Result};
use chrono::Local;
use codex_config::codex_configured_model_provider;
use prodex_runtime_policy::RuntimePolicySummary;
use redaction::{redaction_key_looks_sensitive, redaction_redacted_body_snippet};
use std::{fs, path::Path};
use terminal_ui::print_stdout_line;

pub(super) fn doctor_runtime_policy_status(
    summary: Option<&RuntimePolicySummary>,
    error: Option<&str>,
) -> String {
    error.map_or_else(
        || format_runtime_policy_summary(summary),
        |error| format!("Invalid ({})", first_line_of_error(error)),
    )
}

pub(super) struct DoctorRedactedBundleContext<'a> {
    pub(super) args: &'a DoctorArgs,
    pub(super) paths: &'a AppPaths,
    pub(super) state: &'a AppState,
    pub(super) codex_home: &'a Path,
    pub(super) policy_summary: Option<&'a RuntimePolicySummary>,
    pub(super) runtime_metrics_targets: &'a [String],
    pub(super) import_auth_journal_count: usize,
    pub(super) repaired_import_auth_journals: Option<usize>,
    pub(super) runtime_config: Option<&'a RuntimeConfig>,
    pub(super) runtime_config_error: Option<&'a str>,
    pub(super) policy_summary_error: Option<&'a str>,
}

pub(super) fn doctor_redacted_bundle_json_value(
    context: DoctorRedactedBundleContext<'_>,
) -> serde_json::Value {
    let runtime_summary = collect_runtime_doctor_summary_with_tail_bytes(context.args.tail_bytes);
    let runtime_json = if context.args.suggest_policy {
        context
            .runtime_config
            .map(|config| {
                runtime_doctor_json_value_with_policy_suggestions(&runtime_summary, config)
            })
            .unwrap_or_else(|| runtime_doctor_json_value(&runtime_summary))
    } else {
        runtime_doctor_json_value(&runtime_summary)
    };
    let profile_summaries = doctor_profile_summaries_json_value(context.state);

    let mut value = serde_json::json!({
        "bundle": {
            "kind": "prodex_doctor",
            "redacted": true,
            "generated_at": Local::now().to_rfc3339(),
        },
        "prodex": {
            "version": runtime_current_prodex_version(),
            "codex_binary": format_binary_resolution(&codex_bin()),
            "kiro_binary": format_binary_resolution(&kiro_bin()),
        },
        "paths": {
            "prodex_root": context.paths.root.display().to_string(),
            "state_file": context.paths.state_file.display().to_string(),
            "state_file_exists": context.paths.state_file.exists(),
            "profiles_root": context.paths.managed_profiles_root.display().to_string(),
            "shared_codex_root": context.paths.shared_codex_root.display().to_string(),
            "default_codex_home": context.codex_home.display().to_string(),
            "default_codex_home_exists": context.codex_home.exists(),
        },
        "config": {
            "runtime_policy": runtime_policy_json_value(context.policy_summary),
            "runtime_policy_error": context.policy_summary_error,
            "runtime_configuration_error": context.runtime_config_error,
            "runtime_logs": runtime_logs_json_value(),
            "runtime_latest_log_pointer": runtime_proxy_latest_log_pointer_path().display().to_string(),
            "audit_logs": audit_logs_json_value(),
            "secret_backend": secret_backend_json_value(),
            "runtime_metrics_targets": context.runtime_metrics_targets,
            "live_brokers": collect_live_runtime_broker_observations(context.paths),
            "live_broker_metrics_targets": context.runtime_metrics_targets,
            "import_auth_journals": import_auth_journals_json_value(
                context.import_auth_journal_count,
                context.repaired_import_auth_journals,
            ),
        },
        "state": {
            "profile_count": context.state.profiles.len(),
            "active_profile": context.state.active_profile.as_deref(),
        },
        "profiles": {
            "count": context.state.profiles.len(),
            "items": profile_summaries,
        },
        "runtime": runtime_json,
    });
    if context.args.quota {
        value["quota_probes"] = doctor_quota_reports_json_value(context.state);
    }
    if context.args.install {
        value["install_checks"] = serde_json::Value::Array(
            collect_install_check_rows(context.paths)
                .into_iter()
                .map(|(name, status)| serde_json::json!({ "name": name, "status": status }))
                .collect(),
        );
    }
    doctor_redact_json_value(&mut value);
    value
}

fn doctor_profile_summaries_json_value(state: &AppState) -> serde_json::Value {
    let profiles = collect_profile_summaries(state)
        .into_iter()
        .map(|profile| {
            let (configured_model_provider, config_error) =
                match codex_configured_model_provider(&profile.codex_home) {
                    Ok(provider) => (provider, None),
                    Err(error) => (None, Some(error.to_string())),
                };
            serde_json::json!({
                "name": profile.name,
                "active": profile.active,
                "managed": profile.managed,
                "provider": {
                    "label": profile.provider.label(),
                    "display_name": profile.provider.display_name(),
                    "runtime_route_policy": profile.provider.capabilities().runtime_route_policy.label(),
                    "quota_shape": profile.provider.capabilities().quota_shape.label(),
                    "uses_openai_client_format": profile.provider.capabilities().uses_openai_client_format,
                    "supports_runtime_rotation": profile.provider.capabilities().supports_runtime_rotation,
                    "supports_remote_compact_affinity": profile.provider.capabilities().supports_remote_compact_affinity,
                    "supports_websocket_reuse": profile.provider.capabilities().supports_websocket_reuse,
                },
                "auth": {
                    "label": profile.auth.label,
                    "quota_compatible": profile.auth.quota_compatible,
                },
                "identity": {
                    "email": profile.email,
                },
                "codex_home": {
                    "path": profile.codex_home.display().to_string(),
                    "exists": profile.codex_home.exists(),
                    "has_config_toml": profile.codex_home.join("config.toml").exists(),
                    "configured_model_provider": configured_model_provider,
                    "config_error": config_error,
                },
            })
        })
        .collect::<Vec<_>>();
    serde_json::Value::Array(profiles)
}

pub(super) fn write_doctor_bundle_json(bundle_path: &Path, json: &str) -> Result<()> {
    if bundle_path == Path::new("-") {
        print_stdout_line(json)?;
        return Ok(());
    }

    fs::write(bundle_path, format!("{json}\n"))
        .with_context(|| format!("failed to write doctor bundle {}", bundle_path.display()))?;
    Ok(())
}

pub(crate) fn doctor_redact_json_value(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(object) => {
            for (key, value) in object.iter_mut() {
                if doctor_json_key_should_be_redacted(key) {
                    *value = serde_json::Value::String("<redacted>".to_string());
                } else {
                    doctor_redact_json_value(value);
                }
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                doctor_redact_json_value(value);
            }
        }
        serde_json::Value::String(text) => {
            *text = doctor_redacted_string(text);
        }
        _ => {}
    }
}

fn doctor_json_key_should_be_redacted(key: &str) -> bool {
    if matches!(key, "secret_backend") {
        return false;
    }
    matches!(key, "raw_value") || redaction_key_looks_sensitive(key)
}

fn doctor_redacted_string(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    redaction_redacted_body_snippet(text.as_bytes(), text.chars().count().saturating_add(64))
}

pub(super) fn format_import_auth_journal_status(
    orphan_count: usize,
    repaired: Option<usize>,
) -> String {
    if let Some(repaired) = repaired {
        if orphan_count == 0 {
            return format!("Repaired {repaired} orphan journal(s).");
        }
        return format!("Repaired {repaired}; {orphan_count} orphan journal(s) remain.");
    }

    if orphan_count > 0 {
        format!(
            "Warning: profile-import-auth-journal contains {orphan_count} orphan journal(s); run `prodex doctor --repair-import-auth-journals`."
        )
    } else {
        "None".to_string()
    }
}

pub(super) fn import_auth_journals_json_value(
    orphan_count: usize,
    repaired: Option<usize>,
) -> serde_json::Value {
    serde_json::json!({
        "orphan_count": orphan_count,
        "repair_performed": repaired.is_some(),
        "repaired": repaired.unwrap_or(0),
        "status": if orphan_count > 0 { "warning" } else { "ok" },
    })
}
