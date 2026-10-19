use super::*;

#[test]
fn no_ready_preflight_failure_preserves_detail_and_continue_message() {
    let report = prodex_shared_types::RunProfileProbeReport {
        name: "main".to_string(),
        order_index: 0,
        auth: prodex_quota::AuthSummary {
            label: "fixture".to_string(),
            quota_compatible: true,
        },
        result: Err("timeout:\0detail".to_string()),
    };
    assert_eq!(
        no_ready_runtime_profiles_plan(&report, "main", false),
        RuntimeLaunchNoReadyProfilesPlan::ProbeFailed {
            warning_message: "Warning: quota preflight failed for 'main': timeout:\0detail"
                .to_string(),
            continue_message: "Continuing without quota gate.".to_string(),
        }
    );
}

#[test]
fn blocked_preflight_preserves_fixed_profile_alternatives_and_stop_order() {
    let blocked = [prodex_quota::BlockedLimit {
        message: "fixture exhausted".to_string(),
    }];
    assert_eq!(blocked_selected_runtime_profile_plan("main", &blocked, false, &["βeta".to_string(), "alpha".to_string()]),
        RuntimeLaunchBlockedSelectedProfilePlan::Stop {
            blocked_message: "Quota preflight blocked profile 'main': fixture exhausted".to_string(),
            messages: vec!["Other profiles that look ready: βeta, alpha".to_string(),
                "Rerun without `--no-auto-rotate` to allow fallback.".to_string(),
                "Inspect with `prodex quota --profile main` or bypass with `prodex run --skip-quota-check`.".to_string()],
            error_message: "quota preflight blocked profile 'main' with auto-rotate disabled".to_string(),
        });
}

#[test]
fn no_ready_runtime_profiles_plan_formats_blocked_report() {
    let report = prodex_shared_types::RunProfileProbeReport {
        name: "main".to_string(),
        order_index: 0,
        auth: prodex_quota::AuthSummary {
            label: "chatgpt".to_string(),
            quota_compatible: true,
        },
        result: Ok(prodex_quota::UsageResponse {
            email: None,
            plan_type: None,
            rate_limit: Some(prodex_quota::WindowPair {
                allowed: None,
                limit_reached: None,
                extra: std::collections::BTreeMap::new(),
                primary_window: Some(prodex_quota::UsageWindow {
                    used_percent: Some(100),
                    reset_at: Some(1_900_000_000),
                    limit_window_seconds: Some(18_000),
                }),
                secondary_window: None,
            }),
            code_review_rate_limit: None,
            rate_limit_reset_credits: None,
            additional_rate_limits: Vec::new(),
        }),
    };

    let RuntimeLaunchNoReadyProfilesPlan::Blocked {
        blocked_message,
        no_ready_message,
        inspect_hint,
        error_message,
    } = no_ready_runtime_profiles_plan(&report, "main", false)
    else {
        panic!("expected blocked plan");
    };

    assert!(blocked_message.contains("Quota preflight blocked profile 'main'"));
    assert_eq!(no_ready_message, "No ready profile was found.");
    assert!(inspect_hint.contains("prodex quota --profile main"));
    assert!(error_message.contains("no ready profile"));
}

#[test]
fn blocked_selected_runtime_profile_plan_formats_rotation() {
    let blocked = vec![prodex_quota::BlockedLimit {
        message: "5h exhausted until later".to_string(),
    }];
    let plan =
        blocked_selected_runtime_profile_plan("main", &blocked, true, &["backup".to_string()]);

    assert_eq!(
        plan,
        RuntimeLaunchBlockedSelectedProfilePlan::Rotate {
            blocked_message: "Quota preflight blocked profile 'main': 5h exhausted until later"
                .to_string(),
            next_profile: "backup".to_string(),
            rotate_message: "Auto-rotating to profile 'backup'.".to_string(),
        }
    );
}
