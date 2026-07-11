use super::*;

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
