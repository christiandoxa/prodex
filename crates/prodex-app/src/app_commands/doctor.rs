use super::*;
use redaction::{redaction_key_looks_sensitive, redaction_redacted_body_snippet};

pub(crate) fn handle_doctor(args: DoctorArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
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
    let policy_summary = runtime_policy_summary()?;
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
            runtime_doctor_json_value_with_policy_suggestions(&summary)
        } else {
            runtime_doctor_json_value(&summary)
        };
        if let Some(object) = value.as_object_mut() {
            object.insert(
                "runtime_policy".to_string(),
                runtime_policy_json_value(policy_summary.as_ref()),
            );
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
        print_stdout_line(&json);
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
            "Quota endpoint".to_string(),
            usage_url(&quota_base_url(None)),
        ),
        (
            "Runtime policy".to_string(),
            format_runtime_policy_summary(policy_summary.as_ref()),
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
    print_panel("Doctor", &summary_fields);

    if args.install {
        print_blank_line();
        print_panel("Install Checks", &collect_install_check_rows(&paths));
    }

    if args.runtime {
        print_blank_line();
        let summary = collect_runtime_doctor_summary_with_tail_bytes(args.tail_bytes);
        let fields =
            runtime_doctor_fields_for_summary(&summary, &runtime_proxy_latest_log_pointer_path());
        print_panel("Runtime Proxy", &fields);
        if args.suggest_policy {
            print_blank_line();
            let suggestions = runtime_doctor_policy_suggestions(&summary);
            for line in runtime_doctor_policy_suggestion_lines(&suggestions) {
                print_stdout_line(&line);
            }
        }
    }

    if state.profiles.is_empty() {
        return Ok(());
    }

    for report in collect_doctor_profile_reports(&state, args.quota) {
        let kind = if report.summary.managed {
            "managed"
        } else {
            "external"
        };

        print_blank_line();
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
                    fields.push((
                        "Quota".to_string(),
                        format!("Error ({})", first_line_of_error(&err.to_string())),
                    ));
                }
            }
        }
        print_panel(&format!("Profile {}", report.summary.name), &fields);
    }

    Ok(())
}

struct DoctorRedactedBundleContext<'a> {
    args: &'a DoctorArgs,
    paths: &'a AppPaths,
    state: &'a AppState,
    codex_home: &'a Path,
    policy_summary: Option<&'a RuntimePolicySummary>,
    runtime_metrics_targets: &'a [String],
    import_auth_journal_count: usize,
    repaired_import_auth_journals: Option<usize>,
}

fn doctor_redacted_bundle_json_value(
    context: DoctorRedactedBundleContext<'_>,
) -> serde_json::Value {
    let runtime_summary = collect_runtime_doctor_summary_with_tail_bytes(context.args.tail_bytes);
    let runtime_json = if context.args.suggest_policy {
        runtime_doctor_json_value_with_policy_suggestions(&runtime_summary)
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
    doctor_redact_json_value(&mut value);
    value
}

fn doctor_profile_summaries_json_value(state: &AppState) -> serde_json::Value {
    let profiles = collect_profile_summaries(state)
        .into_iter()
        .map(|profile| {
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
                    "configured_model_provider": codex_configured_model_provider(&profile.codex_home),
                },
            })
        })
        .collect::<Vec<_>>();
    serde_json::Value::Array(profiles)
}

fn write_doctor_bundle_json(bundle_path: &Path, json: &str) -> Result<()> {
    if bundle_path == Path::new("-") {
        print_stdout_line(json);
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

fn format_import_auth_journal_status(orphan_count: usize, repaired: Option<usize>) -> String {
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

fn import_auth_journals_json_value(
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
