use super::*;

pub(crate) fn handle_doctor(args: DoctorArgs) -> Result<()> {
    let paths = AppPaths::discover()?;
    let state = AppState::load(&paths)?;
    let codex_home = default_codex_home(&paths)?;
    let policy_summary = runtime_policy_summary()?;
    let runtime_metrics_targets = collect_runtime_broker_metrics_targets(&paths);

    if args.runtime && args.json {
        let summary = collect_runtime_doctor_summary();
        let mut value = runtime_doctor_json_value(&summary);
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
            "Secret backend".to_string(),
            format_secret_backend_summary(),
        ),
        ("Runtime logs".to_string(), format_runtime_logs_summary()),
        ("Audit logs".to_string(), format_audit_logs_summary()),
        (
            "Runtime metrics".to_string(),
            format_runtime_broker_metrics_targets(&runtime_metrics_targets),
        ),
        ("Profiles".to_string(), state.profiles.len().to_string()),
        (
            "Active profile".to_string(),
            state.active_profile.as_deref().unwrap_or("-").to_string(),
        ),
    ];
    print_panel("Doctor", &summary_fields);

    if args.runtime {
        print_blank_line();
        print_panel("Runtime Proxy", &runtime_doctor_fields());
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
