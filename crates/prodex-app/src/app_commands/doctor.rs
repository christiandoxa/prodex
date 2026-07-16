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
        if repaired > 0 {
            state
                .save(&paths)
                .context("failed to save repaired import auth rollback state")?;
        }
        audit_log_event_best_effort(
            "profile",
            "repair_import_auth_journals",
            "success",
            serde_json::json!({ "repaired": repaired }),
        );
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

    if args.bundle.is_some() {
        if !args.redacted {
            bail!("doctor --bundle requires --redacted");
        }
        let bundle = doctor_redacted_bundle_json_value(DoctorRedactedBundleContext {
            args: &args,
            paths: &paths,
            state: &state,
            codex_home: &codex_home,
            policy_summary: policy_summary.as_ref(),
            runtime_metrics_targets: &runtime_metrics_targets,
            import_auth_journal_count,
            repaired_import_auth_journals,
            runtime_config: runtime_config.as_ref(),
            runtime_config_error: runtime_config_error.as_deref(),
            policy_summary_error: policy_summary_error.as_deref(),
        });
        let json = serde_json::to_string_pretty(&bundle)
            .context("failed to serialize redacted doctor bundle")?;
        if let Some(bundle_path) = args.bundle.as_ref() {
            write_doctor_bundle_json(bundle_path, &json)?;
        }
        return Ok(());
    }

    if args.runtime && args.json {
        let summary = collect_runtime_doctor_summary_with_tail_bytes(args.tail_bytes);
        let mut value = if args.suggest_policy {
            runtime_config
                .as_ref()
                .map(|config| runtime_doctor_json_value_with_policy_suggestions(&summary, config))
                .unwrap_or_else(|| runtime_doctor_json_value(&summary))
        } else {
            runtime_doctor_json_value(&summary)
        };
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "runtime_policy".to_string(),
                runtime_policy_json_value(policy_summary.as_ref()),
            );
            if let Some(error) = policy_summary_error.as_deref() {
                object.insert(
                    "runtime_policy_error".to_string(),
                    serde_json::Value::String(error.to_string()),
                );
            }
            if let Some(error) = runtime_config_error.as_deref() {
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
                serde_json::to_value(collect_live_runtime_broker_observations(&paths))
                    .unwrap_or_else(|_| serde_json::Value::Array(Vec::new())),
            );
            object.insert(
                "live_broker_metrics_targets".to_string(),
                serde_json::to_value(&runtime_metrics_targets)
                    .unwrap_or_else(|_| serde_json::Value::Array(Vec::new())),
            );
            object.insert(
                "import_auth_journals".to_string(),
                import_auth_journals_json_value(
                    import_auth_journal_count,
                    repaired_import_auth_journals,
                ),
            );
            if args.install {
                object.insert(
                    "install_checks".to_string(),
                    serde_json::Value::Array(
                        collect_install_check_rows(&paths)
                            .into_iter()
                            .map(|(name, status)| {
                                serde_json::json!({ "name": name, "status": status })
                            })
                            .collect(),
                    ),
                );
            }
        }
        let json = serde_json::to_string_pretty(&value)
            .context("failed to serialize runtime doctor summary")?;
        print_stdout_line(&json)?;
        return Ok(());
    }

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
            doctor_runtime_policy_status(policy_summary.as_ref(), policy_summary_error.as_deref()),
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
            format_runtime_broker_metrics_targets(&runtime_metrics_targets),
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
            fields: collect_install_check_rows(&paths),
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

    for report in collect_doctor_profile_reports(&state, args.quota) {
        let kind = if report.summary.managed {
            "managed"
        } else {
            "external"
        };

        let mut fields = vec![
            (
                "Current".to_string(),
                if report.summary.active {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
            ),
            ("Kind".to_string(), kind.to_string()),
            (
                "Provider".to_string(),
                report.summary.provider.display_name().to_string(),
            ),
            (
                "Runtime route".to_string(),
                report
                    .summary
                    .provider
                    .capabilities()
                    .runtime_route_policy
                    .label()
                    .to_string(),
            ),
            (
                "Quota shape".to_string(),
                report
                    .summary
                    .provider
                    .capabilities()
                    .quota_shape
                    .label()
                    .to_string(),
            ),
            ("Auth".to_string(), report.summary.auth.label),
            (
                "Identity".to_string(),
                report.summary.email.as_deref().unwrap_or("-").to_string(),
            ),
            (
                "Path".to_string(),
                report.summary.codex_home.display().to_string(),
            ),
            (
                "Exists".to_string(),
                if report.summary.codex_home.exists() {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                },
            ),
        ];

        if let Some(quota) = report.quota {
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
                Err(err) => {
                    fields.push(("Quota".to_string(), doctor_quota_error_summary(&err)));
                }
            }
        }
        panels.push(DoctorPanel {
            title: format!("Profile {}", report.summary.name),
            fields,
        });
    }

    print_doctor_output(&panels, &suggestion_lines)?;
    Ok(())
}
