use super::*;

fn policy(name: &str) -> GatewayVirtualKeyPolicy {
    GatewayVirtualKeyPolicy {
        name: name.to_string(),
        tenant_id: None,
        team_id: None,
        project_id: None,
        user_id: None,
        budget_id: None,
        allowed_models: Vec::new(),
        budget_microusd: None,
        request_budget: None,
        rpm_limit: None,
        tpm_limit: None,
    }
}

fn request(
    policy: GatewayVirtualKeyPolicy,
    usage: GatewayVirtualKeyUsage,
) -> GatewayVirtualKeyAdmissionRequest {
    GatewayVirtualKeyAdmissionRequest {
        policy,
        usage,
        grouped_usage: Vec::new(),
        model: Some("gpt-5.4".to_string()),
        input_tokens: 5,
        reserved_tokens: 22,
        estimated_cost_microusd: Some(39),
        minute_epoch: 10,
        reservation: None,
        distributed_rate_limit: false,
        now_unix_ms: 600_001,
    }
}

fn reservation(
    tenant_id: TenantId,
    virtual_key_id: VirtualKeyId,
    durable_store: Option<DurableStoreKind>,
) -> GatewayVirtualKeyReservationContext {
    GatewayVirtualKeyReservationContext {
        tenant_id,
        virtual_key_id: Some(virtual_key_id),
        call_id: CallId::new(),
        reservation_id: ReservationId::new(),
        durable_store,
        created_at_unix_ms: 1_000,
        ttl_ms: 60_000,
    }
}

#[test]
fn admission_limit_parity_table() {
    struct Case {
        name: &'static str,
        policy: GatewayVirtualKeyPolicy,
        usage: GatewayVirtualKeyUsage,
        minute_epoch: u64,
        expected: Result<(), GatewayVirtualKeyAdmissionError>,
    }

    let mut model = policy("model");
    model.allowed_models = vec![" other-model ".to_string()];
    let mut request_budget = policy("request");
    request_budget.request_budget = Some(2);
    let mut spend = policy("spend");
    spend.budget_microusd = Some(100);
    let mut rpm = policy("rpm");
    rpm.rpm_limit = Some(1);
    let mut tpm = policy("tpm");
    tpm.tpm_limit = Some(25);
    let cases = [
        Case {
            name: "model allowlist",
            policy: model,
            usage: GatewayVirtualKeyUsage::default(),
            minute_epoch: 10,
            expected: Err(GatewayVirtualKeyAdmissionError::ModelNotAllowed),
        },
        Case {
            name: "request budget equality rejects",
            policy: request_budget,
            usage: GatewayVirtualKeyUsage {
                requests_total: 2,
                ..GatewayVirtualKeyUsage::default()
            },
            minute_epoch: 10,
            expected: Err(GatewayVirtualKeyAdmissionError::RequestBudgetExceeded),
        },
        Case {
            name: "spend over limit rejects",
            policy: spend,
            usage: GatewayVirtualKeyUsage {
                spend_microusd: 62,
                ..GatewayVirtualKeyUsage::default()
            },
            minute_epoch: 10,
            expected: Err(GatewayVirtualKeyAdmissionError::BudgetExceeded),
        },
        Case {
            name: "rpm equality plus request rejects",
            policy: rpm.clone(),
            usage: GatewayVirtualKeyUsage {
                minute_epoch: 10,
                requests_this_minute: 1,
                ..GatewayVirtualKeyUsage::default()
            },
            minute_epoch: 10,
            expected: Err(GatewayVirtualKeyAdmissionError::RpmLimitExceeded),
        },
        Case {
            name: "tpm over limit rejects",
            policy: tpm,
            usage: GatewayVirtualKeyUsage {
                minute_epoch: 10,
                tokens_this_minute: 4,
                ..GatewayVirtualKeyUsage::default()
            },
            minute_epoch: 10,
            expected: Err(GatewayVirtualKeyAdmissionError::TpmLimitExceeded),
        },
        Case {
            name: "minute rollover resets rpm",
            policy: rpm,
            usage: GatewayVirtualKeyUsage {
                minute_epoch: 9,
                requests_this_minute: u64::MAX,
                ..GatewayVirtualKeyUsage::default()
            },
            minute_epoch: 10,
            expected: Ok(()),
        },
    ];

    for case in cases {
        let mut request = request(case.policy, case.usage);
        request.minute_epoch = case.minute_epoch;
        let actual = plan_gateway_virtual_key_admission(request).map(|_| ());
        assert_eq!(actual, case.expected, "{}", case.name);
    }
}

#[test]
fn allowed_models_trim_policy_entries_and_ignore_ascii_case() {
    let mut selected = policy("selected");
    selected.allowed_models = vec![" GPT-5.4 ".to_string()];
    plan_gateway_virtual_key_admission(request(selected, GatewayVirtualKeyUsage::default()))
        .expect("trimmed case-insensitive allowlist entry should match");
}

#[test]
fn grouped_budget_uses_exact_full_scope_and_precedes_model_check() {
    let mut selected = policy("selected");
    selected.tenant_id = Some("tenant-a".to_string());
    selected.team_id = Some("team-a".to_string());
    selected.budget_id = Some("shared".to_string());
    selected.request_budget = Some(2);
    selected.allowed_models = vec!["different-model".to_string()];
    let mut matching = selected.clone();
    matching.name = "matching".to_string();
    let mut other_tenant = selected.clone();
    other_tenant.name = "other-tenant".to_string();
    other_tenant.tenant_id = Some("tenant-b".to_string());
    let mut untrimmed = selected.clone();
    untrimmed.name = "untrimmed".to_string();
    untrimmed.budget_id = Some(" shared ".to_string());
    let mut admission = request(selected, GatewayVirtualKeyUsage::default());
    admission.grouped_usage = vec![
        GatewayVirtualKeyUsageEntry {
            policy: matching,
            usage: GatewayVirtualKeyUsage {
                requests_total: 2,
                ..GatewayVirtualKeyUsage::default()
            },
        },
        GatewayVirtualKeyUsageEntry {
            policy: other_tenant,
            usage: GatewayVirtualKeyUsage {
                requests_total: u64::MAX,
                ..GatewayVirtualKeyUsage::default()
            },
        },
        GatewayVirtualKeyUsageEntry {
            policy: untrimmed,
            usage: GatewayVirtualKeyUsage {
                requests_total: u64::MAX,
                ..GatewayVirtualKeyUsage::default()
            },
        },
    ];

    assert_eq!(
        plan_gateway_virtual_key_admission(admission),
        Err(GatewayVirtualKeyAdmissionError::RequestBudgetExceeded)
    );
}

#[test]
fn durable_projection_parity_table() {
    let tenant_id = TenantId::new();
    let tenant = tenant_id.to_string();
    let virtual_key_id = VirtualKeyId::new();
    let stale = GatewayVirtualKeyUsage {
        requests_total: 1,
        spend_microusd: 100,
        ..GatewayVirtualKeyUsage::default()
    };

    for (name, store, budget_id, expected) in [
        ("postgres", DurableStoreKind::Postgres, None, Ok(())),
        ("sqlite", DurableStoreKind::Sqlite, None, Ok(())),
        (
            "sqlite grouped remains local",
            DurableStoreKind::Sqlite,
            Some("shared"),
            Err(GatewayVirtualKeyAdmissionError::RequestBudgetExceeded),
        ),
    ] {
        let mut selected = policy(name);
        selected.tenant_id = Some(tenant.clone());
        selected.budget_id = budget_id.map(str::to_string);
        selected.request_budget = Some(1);
        selected.budget_microusd = Some(100);
        let mut admission = request(selected, stale.clone());
        admission.reservation = Some(reservation(tenant_id, virtual_key_id, Some(store)));
        assert_eq!(
            plan_gateway_virtual_key_admission(admission).map(|_| ()),
            expected,
            "{name}"
        );
    }
}

#[test]
fn budget_group_storage_scope_is_stable_and_tenant_scoped() {
    let tenant_id = TenantId::new();
    let mut selected = policy("alpha");
    selected.tenant_id = Some(tenant_id.to_string());
    selected.budget_id = Some("shared".to_string());
    let mut alpha = request(selected.clone(), GatewayVirtualKeyUsage::default());
    alpha.reservation = Some(reservation(
        tenant_id,
        VirtualKeyId::new(),
        Some(DurableStoreKind::Postgres),
    ));
    let alpha = plan_gateway_virtual_key_admission(alpha)
        .unwrap()
        .reservation
        .unwrap()
        .storage_key;

    selected.name = "beta".to_string();
    let mut beta = request(selected.clone(), GatewayVirtualKeyUsage::default());
    beta.reservation = Some(reservation(
        tenant_id,
        VirtualKeyId::new(),
        Some(DurableStoreKind::Postgres),
    ));
    let beta = plan_gateway_virtual_key_admission(beta)
        .unwrap()
        .reservation
        .unwrap()
        .storage_key;
    assert_eq!(alpha.budget_scope, beta.budget_scope);
    assert_ne!(alpha.virtual_key_id, beta.virtual_key_id);

    let other_tenant = TenantId::new();
    selected.tenant_id = Some(other_tenant.to_string());
    let mut other = request(selected, GatewayVirtualKeyUsage::default());
    other.reservation = Some(reservation(
        other_tenant,
        VirtualKeyId::new(),
        Some(DurableStoreKind::Postgres),
    ));
    let other = plan_gateway_virtual_key_admission(other)
        .unwrap()
        .reservation
        .unwrap()
        .storage_key;
    assert_ne!(alpha.budget_scope, other.budget_scope);
}

#[test]
fn tenant_mismatch_fails_before_reservation_planning() {
    let policy_tenant = TenantId::new();
    let mut selected = policy("mismatch");
    selected.tenant_id = Some(policy_tenant.to_string());
    let mut admission = request(selected, GatewayVirtualKeyUsage::default());
    admission.reservation = Some(reservation(
        TenantId::new(),
        VirtualKeyId::new(),
        Some(DurableStoreKind::Postgres),
    ));

    assert_eq!(
        plan_gateway_virtual_key_admission(admission),
        Err(GatewayVirtualKeyAdmissionError::PolicyStateUnavailable)
    );
}

#[test]
fn redis_plan_is_additional_to_local_rate_limits() {
    let tenant_id = TenantId::new();
    let mut selected = policy("rate-limited");
    selected.tenant_id = Some(tenant_id.to_string());
    selected.rpm_limit = Some(1);
    let context = reservation(tenant_id, VirtualKeyId::new(), None);

    let mut rejected = request(
        selected.clone(),
        GatewayVirtualKeyUsage {
            minute_epoch: 10,
            requests_this_minute: 1,
            ..GatewayVirtualKeyUsage::default()
        },
    );
    rejected.reservation = Some(context);
    rejected.distributed_rate_limit = true;
    assert_eq!(
        plan_gateway_virtual_key_admission(rejected),
        Err(GatewayVirtualKeyAdmissionError::RpmLimitExceeded)
    );

    let mut allowed = request(selected, GatewayVirtualKeyUsage::default());
    allowed.reservation = Some(context);
    allowed.distributed_rate_limit = true;
    assert!(
        plan_gateway_virtual_key_admission(allowed)
            .unwrap()
            .distributed_rate_limit
            .is_some()
    );
}

#[test]
fn usage_update_applies_to_fresh_state_after_lock_reacquisition() {
    let update = GatewayVirtualKeyUsageUpdate {
        minute_epoch: 10,
        reserved_tokens: 22,
        estimated_cost_microusd: Some(39),
    };
    let mut current = GatewayVirtualKeyUsage {
        minute_epoch: 10,
        requests_this_minute: 3,
        tokens_this_minute: 30,
        requests_total: 7,
        spend_microusd: 100,
    };
    apply_gateway_virtual_key_usage_update(&mut current, update);
    assert_eq!(current.requests_this_minute, 4);
    assert_eq!(current.tokens_this_minute, 52);
    assert_eq!(current.requests_total, 8);
    assert_eq!(current.spend_microusd, 139);

    current.minute_epoch = 9;
    current.requests_this_minute = u64::MAX;
    current.tokens_this_minute = u64::MAX;
    apply_gateway_virtual_key_usage_update(&mut current, update);
    assert_eq!(current.requests_this_minute, 1);
    assert_eq!(current.tokens_this_minute, 22);
    assert_eq!(current.requests_total, 9);
}
