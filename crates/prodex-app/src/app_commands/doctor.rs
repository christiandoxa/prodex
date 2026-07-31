use super::*;
use redaction::redaction_redact_secret_like_text;

mod bundle;
mod render;

#[cfg(test)]
pub(crate) use bundle::doctor_redact_json_value;
use bundle::{
    DoctorRedactedBundleContext, doctor_redacted_bundle_json_value, doctor_runtime_policy_status,
    format_import_auth_journal_status, import_auth_journals_json_value, write_doctor_bundle_json,
};
use render::{doctor_quota_error_summary, print_doctor_output};

#[derive(Debug, Clone)]
struct DoctorPanel {
    title: String,
    fields: Vec<(String, String)>,
}

struct DoctorContext<'a> {
    args: &'a DoctorArgs,
    paths: &'a AppPaths,
    state: &'a AppState,
    codex_home: &'a std::path::Path,
    policy_summary: Option<&'a RuntimePolicySummary>,
    policy_summary_error: Option<&'a str>,
    runtime_metrics_targets: &'a [String],
    import_auth_journal_count: usize,
    repaired_import_auth_journals: Option<usize>,
    runtime_config: Option<&'a RuntimeConfig>,
    runtime_config_error: Option<&'a str>,
}

pub(crate) fn handle_doctor(args: DoctorArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let runtime_config = RuntimeConfig::from_env_policy_and_cli(&paths);
    let runtime_config_error = runtime_config
        .as_ref()
        .err()
        .map(|error| redaction_redact_secret_like_text(&error.to_string()));
    let runtime_config = runtime_config.ok();
    let mut state = AppState::load(&paths)?;
    let repaired_import_auth_journals = if args.repair_import_auth_journals {
        let repaired = repair_profile_import_auth_journals(&paths, &mut state)?;
        audit_log_event(
            "profile",
            "repair_import_auth_journals",
            "success",
            serde_json::json!({ "repaired": repaired }),
        )?;
        Some(repaired)
    } else {
        None
    };
    let import_auth_journal_count = count_profile_import_auth_journals(&paths)?;
    let codex_home = default_codex_home(&paths)?;
    let policy_summary = runtime_policy_summary();
    let policy_summary_error = policy_summary
        .as_ref()
        .err()
        .map(|error| redaction_redact_secret_like_text(&format!("{error:#}")));
    let policy_summary = policy_summary.ok().flatten();
    let runtime_metrics_targets = collect_runtime_broker_metrics_targets(&paths);

    let context = DoctorContext {
        args: &args,
        paths: &paths,
        state: &state,
        codex_home: &codex_home,
        policy_summary: policy_summary.as_ref(),
        policy_summary_error: policy_summary_error.as_deref(),
        runtime_metrics_targets: &runtime_metrics_targets,
        import_auth_journal_count,
        repaired_import_auth_journals,
        runtime_config: runtime_config.as_ref(),
        runtime_config_error: runtime_config_error.as_deref(),
    };
    if handle_doctor_bundle(&context)? || handle_doctor_runtime_json(&context)? {
        return Ok(());
    }
    render_human_doctor(context)
}

fn handle_doctor_bundle(context: &DoctorContext<'_>) -> Result<bool> {
    let Some(bundle_path) = context.args.bundle.as_ref() else {
        return Ok(false);
    };
    if !context.args.redacted {
        bail!("doctor --bundle requires --redacted");
    }
    let bundle = doctor_redacted_bundle_json_value(DoctorRedactedBundleContext {
        args: context.args,
        paths: context.paths,
        state: context.state,
        codex_home: context.codex_home,
        policy_summary: context.policy_summary,
        runtime_metrics_targets: context.runtime_metrics_targets,
        import_auth_journal_count: context.import_auth_journal_count,
        repaired_import_auth_journals: context.repaired_import_auth_journals,
        runtime_config: context.runtime_config,
        runtime_config_error: context.runtime_config_error,
        policy_summary_error: context.policy_summary_error,
    });
    let json = serde_json::to_string_pretty(&bundle)
        .context("failed to serialize redacted doctor bundle")?;
    write_doctor_bundle_json(bundle_path, &json)?;
    Ok(true)
}

fn handle_doctor_runtime_json(context: &DoctorContext<'_>) -> Result<bool> {
    if !context.args.runtime || !context.args.json {
        return Ok(false);
    }
    let summary = collect_runtime_doctor_summary_with_tail_bytes(context.args.tail_bytes);
    let mut value = if context.args.suggest_policy {
        context
            .runtime_config
            .map(|config| runtime_doctor_json_value_with_policy_suggestions(&summary, config))
            .unwrap_or_else(|| runtime_doctor_json_value(&summary))
    } else {
        runtime_doctor_json_value(&summary)
    };
    if let Some(object) = value.as_object_mut() {
        append_doctor_runtime_json_fields(object, context);
    }
    let json = serde_json::to_string_pretty(&value)
        .context("failed to serialize runtime doctor summary")?;
    print_stdout_line(&json)?;
    Ok(true)
}

fn append_doctor_runtime_json_fields(
    object: &mut serde_json::Map<String, serde_json::Value>,
    context: &DoctorContext<'_>,
) {
    object.insert(
        "runtime_policy".to_string(),
        runtime_policy_json_value(context.policy_summary),
    );
    if let Some(error) = context.policy_summary_error {
        object.insert(
            "runtime_policy_error".to_string(),
            serde_json::Value::String(error.to_string()),
        );
    }
    if let Some(error) = context.runtime_config_error {
        object.insert(
            "runtime_configuration_error".to_string(),
            serde_json::Value::String(error.to_string()),
        );
    }
    object.insert("secret_backend".to_string(), secret_backend_json_value());
    object.insert("runtime_logs".to_string(), runtime_logs_json_value());
    object.insert("audit_logs".to_string(), audit_logs_json_value());
    object.insert(
        "live_brokers".to_string(),
        serde_json::to_value(collect_live_runtime_broker_observations(context.paths))
            .unwrap_or_else(|_| serde_json::Value::Array(Vec::new())),
    );
    object.insert(
        "live_broker_metrics_targets".to_string(),
        serde_json::to_value(context.runtime_metrics_targets)
            .unwrap_or_else(|_| serde_json::Value::Array(Vec::new())),
    );
    object.insert(
        "import_auth_journals".to_string(),
        import_auth_journals_json_value(
            context.import_auth_journal_count,
            context.repaired_import_auth_journals,
        ),
    );
    if context.args.install {
        object.insert(
            "install_checks".to_string(),
            serde_json::Value::Array(
                collect_install_check_rows(context.paths)
                    .into_iter()
                    .map(|(name, status)| serde_json::json!({ "name": name, "status": status }))
                    .collect(),
            ),
        );
    }
    if context.args.quota {
        object.insert(
            "quota_probes".to_string(),
            doctor_quota_reports_json_value(context.state),
        );
    }
}

fn render_human_doctor(context: DoctorContext<'_>) -> Result<()> {
    let DoctorContext {
        args,
        paths,
        state,
        codex_home,
        policy_summary,
        policy_summary_error,
        runtime_metrics_targets,
        import_auth_journal_count,
        repaired_import_auth_journals,
        runtime_config,
        ..
    } = context;
    let summary_fields = vec![
        ("Prodex root".to_string(), paths.root.display().to_string()),
        (
            "State file".to_string(),
            format!(
                "{} ({})",
                paths.state_file.display(),
                if paths.state_file.exists() {
                    "exists"
                } else {
                    "missing"
                }
            ),
        ),
        (
            "Profiles root".to_string(),
            paths.managed_profiles_root.display().to_string(),
        ),
        (
            "Default CODEX_HOME".to_string(),
            format!(
                "{} ({})",
                codex_home.display(),
                if codex_home.exists() {
                    "exists"
                } else {
                    "missing"
                }
            ),
        ),
        (
            "Codex binary".to_string(),
            format_binary_resolution(&codex_bin()),
        ),
        (
            "Kiro binary".to_string(),
            format_binary_resolution(&kiro_bin()),
        ),
        (
            "Quota endpoint".to_string(),
            usage_url(&quota_base_url(None)?),
        ),
        (
            "Runtime policy".to_string(),
            doctor_runtime_policy_status(policy_summary, policy_summary_error),
        ),
        (
            "Runtime proxy contract".to_string(),
            format_runtime_proxy_contract_summary(),
        ),
        (
            "Secret backend".to_string(),
            format_secret_backend_summary(),
        ),
        ("Runtime logs".to_string(), format_runtime_logs_summary()),
        ("Audit logs".to_string(), format_audit_logs_summary()),
        (
            "Runtime metrics".to_string(),
            format_runtime_broker_metrics_targets(runtime_metrics_targets),
        ),
        (
            "Import auth journals".to_string(),
            format_import_auth_journal_status(
                import_auth_journal_count,
                repaired_import_auth_journals,
            ),
        ),
        ("Profiles".to_string(), state.profiles.len().to_string()),
        (
            "Active profile".to_string(),
            state.active_profile.as_deref().unwrap_or("-").to_string(),
        ),
    ];
    let mut panels = vec![DoctorPanel {
        title: "Doctor".to_string(),
        fields: summary_fields,
    }];
    let mut suggestion_lines = Vec::new();

    if args.install {
        panels.push(DoctorPanel {
            title: "Install Checks".to_string(),
            fields: collect_install_check_rows(paths),
        });
    }

    if args.runtime {
        let summary = collect_runtime_doctor_summary_with_tail_bytes(args.tail_bytes);
        let fields =
            runtime_doctor_fields_for_summary(&summary, &runtime_proxy_latest_log_pointer_path());
        panels.push(DoctorPanel {
            title: "Runtime Proxy".to_string(),
            fields,
        });
        if args.suggest_policy
            && let Some(runtime_config) = runtime_config.as_ref()
        {
            let suggestions = runtime_doctor_policy_suggestions(&summary, runtime_config);
            suggestion_lines = runtime_doctor_policy_suggestion_lines(&suggestions);
        }
    }

    if state.profiles.is_empty() {
        print_doctor_output(&panels, &suggestion_lines)?;
        return Ok(());
    }

    for report in collect_doctor_profile_reports(state, args.quota) {
        panels.push(doctor_profile_panel(report));
    }

    print_doctor_output(&panels, &suggestion_lines)?;
    Ok(())
}

fn doctor_profile_panel(report: DoctorProfileReport) -> DoctorPanel {
    let summary = report.summary;
    let kind = if summary.managed {
        "managed"
    } else {
        "external"
    };
    let mut fields = vec![
        (
            "Current".to_string(),
            if summary.active { "Yes" } else { "No" }.to_string(),
        ),
        ("Kind".to_string(), kind.to_string()),
        (
            "Provider".to_string(),
            summary.provider.display_name().to_string(),
        ),
        (
            "Runtime route".to_string(),
            summary
                .provider
                .capabilities()
                .runtime_route_policy
                .label()
                .to_string(),
        ),
        (
            "Quota shape".to_string(),
            summary
                .provider
                .capabilities()
                .quota_shape
                .label()
                .to_string(),
        ),
        ("Auth".to_string(), summary.auth.label),
        (
            "Identity".to_string(),
            summary.email.as_deref().unwrap_or("-").to_string(),
        ),
        ("Path".to_string(), summary.codex_home.display().to_string()),
        (
            "Exists".to_string(),
            if summary.codex_home.exists() {
                "Yes"
            } else {
                "No"
            }
            .to_string(),
        ),
    ];
    if let Some(quota) = report.quota {
        append_doctor_quota_fields(&mut fields, quota);
    }
    DoctorPanel {
        title: format!("Profile {}", summary.name),
        fields,
    }
}

fn append_doctor_quota_fields(
    fields: &mut Vec<(String, String)>,
    quota: std::result::Result<ProviderQuotaSnapshot, String>,
) {
    match quota {
        Ok(ProviderQuotaSnapshot::OpenAi(usage)) => {
            let blocked = collect_blocked_limits(&usage, false);
            fields.push((
                "Quota".to_string(),
                if blocked.is_empty() {
                    "Ready".to_string()
                } else {
                    format!("Blocked ({})", format_blocked_limits(&blocked))
                },
            ));
            fields.push(("Main".to_string(), format_main_windows(&usage)));
        }
        Ok(ProviderQuotaSnapshot::Copilot(info)) => {
            fields.push(("Quota".to_string(), format_copilot_quota_status(&info)));
            fields.push(("Main".to_string(), format_copilot_main_quota(&info)));
            if let Some(reset) = format_copilot_reset_summary(&info) {
                fields.push(("Reset".to_string(), reset));
            }
        }
        Ok(ProviderQuotaSnapshot::Gemini(info)) => {
            fields.push(("Quota".to_string(), format_gemini_quota_status(&info)));
            fields.push(("Main".to_string(), format_gemini_main_quota(&info)));
            if let Some(reset) = format_gemini_reset_summary(&info) {
                fields.push(("Reset".to_string(), reset));
            }
        }
        Ok(ProviderQuotaSnapshot::External(info)) => {
            fields.push(("Quota".to_string(), info.status));
            fields.push(("Main".to_string(), info.main));
            if let Some(reset) = info.reset {
                fields.push(("Reset".to_string(), reset));
            }
        }
        Err(err) => fields.push(("Quota".to_string(), doctor_quota_error_summary(&err))),
    }
}

fn doctor_quota_reports_json_value(state: &AppState) -> serde_json::Value {
    serde_json::Value::Array(
        collect_doctor_profile_reports(state, true)
            .into_iter()
            .map(|report| {
                let quota = report.quota.map(|quota| match quota {
                    Ok(ProviderQuotaSnapshot::OpenAi(usage)) => serde_json::json!({
                        "status": if collect_blocked_limits(&usage, false).is_empty() { "ready" } else { "blocked" },
                        "main": format_main_windows(&usage),
                    }),
                    Ok(ProviderQuotaSnapshot::Copilot(info)) => serde_json::json!({
                        "status": format_copilot_quota_status(&info),
                        "main": format_copilot_main_quota(&info),
                        "reset": format_copilot_reset_summary(&info),
                    }),
                    Ok(ProviderQuotaSnapshot::Gemini(info)) => serde_json::json!({
                        "status": format_gemini_quota_status(&info),
                        "main": format_gemini_main_quota(&info),
                        "reset": format_gemini_reset_summary(&info),
                    }),
                    Ok(ProviderQuotaSnapshot::External(info)) => serde_json::json!({
                        "status": info.status,
                        "main": info.main,
                        "reset": info.reset,
                    }),
                    Err(error) => serde_json::json!({
                        "error": doctor_quota_error_summary(&error),
                    }),
                });
                serde_json::json!({
                    "profile": report.summary.name,
                    "provider": report.summary.provider.label(),
                    "quota": quota,
                })
            })
            .collect(),
    )
}
