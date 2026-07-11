use prodex_domain::{
    AuditAction, AuditActionError, AuditChainError, AuditDigest, AuditDigestError, AuditEnvelope,
    AuditErrorStatus, AuditEvent, AuditEventId, AuditExportFormat, AuditExportFormatError,
    AuditExportPlan, AuditOutcome, AuditOutcomeError, AuditPageLimit, AuditPageLimitError,
    AuditQueryCursor, AuditQueryCursorError, AuditQueryPageError, AuditQueryPlan,
    AuditQueryPlanError, AuditQueryScope, AuditQueryScopeError, AuditReasonCode,
    AuditReasonCodeError, AuditResource, AuditResourceId, AuditResourceIdError,
    AuditResourceKindError, AuditRetentionBatchLimit, AuditRetentionBatchLimitError,
    AuditRetentionDecision, AuditRetentionDecisionError, AuditRetentionHold,
    AuditRetentionHoldError, AuditRetentionPageError, AuditRetentionPlan, AuditRetentionPlanError,
    AuditRetentionPolicy, AuditRetentionPolicyError, AuditRetentionPurgeBatch,
    AuditRetentionPurgeBatchError, AuditRetentionPurgeKey, AuditSortOrder, AuditSortOrderError,
    AuditTimeRange, AuditTimeRangeError, AuditTimestamp, AuditTimestampError, CredentialScope,
    Cursor, Principal, PrincipalId, PrincipalKind, Role, TenantContext, TenantId,
    plan_audit_action_error_response, plan_audit_chain_error_response,
    plan_audit_digest_error_response, plan_audit_export_format_error_response,
    plan_audit_outcome_error_response, plan_audit_page_limit_error_response,
    plan_audit_query_cursor_error_response, plan_audit_query_page_error_response,
    plan_audit_query_plan_error_response, plan_audit_query_scope_error_response,
    plan_audit_reason_code_error_response, plan_audit_resource_id_error_response,
    plan_audit_resource_kind_error_response, plan_audit_retention_batch_limit_error_response,
    plan_audit_retention_decision_error_response, plan_audit_retention_hold_error_response,
    plan_audit_retention_page_error_response, plan_audit_retention_plan_error_response,
    plan_audit_retention_policy_error_response, plan_audit_retention_purge_batch_error_response,
    plan_audit_sort_order_error_response, plan_audit_time_range_error_response,
    plan_audit_timestamp_error_response,
};

#[test]
fn audit_event_uses_canonical_principal_and_tenant_context() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::User,
        Role::Admin,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };

    let event = AuditEvent::new(
        1_725_000_000_000,
        tenant,
        &principal,
        AuditAction::new("tenant.update"),
        AuditResource::new("tenant", Some(tenant_id.to_string()), Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );

    assert_eq!(event.tenant_id, tenant_id);
    assert_eq!(event.principal_id, principal.id);
    assert_eq!(event.action.as_str(), "tenant.update");
    assert_eq!(event.resource.tenant_id, Some(tenant_id));
}

#[test]
fn audit_events_have_distinct_immutable_ids() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };

    let first = AuditEvent::new(
        1,
        tenant,
        &principal,
        AuditAction::new("policy.activate"),
        AuditResource::new("policy", None::<String>, Some(tenant_id)),
        AuditOutcome::Denied,
        Some("insufficient_role"),
    );
    let second = AuditEvent::new(
        2,
        tenant,
        &principal,
        AuditAction::new("policy.activate"),
        AuditResource::new("policy", None::<String>, Some(tenant_id)),
        AuditOutcome::Denied,
        Some("insufficient_role"),
    );

    assert_ne!(first.immutable_key(), second.immutable_key());
    assert_eq!(first.reason_code.as_deref(), Some("insufficient_role"));
}

#[test]
fn audit_event_serializes_without_raw_secret_fields() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::VirtualKey,
        Role::Viewer,
        CredentialScope::DataPlane,
    );
    let tenant = TenantContext { tenant_id };

    let event = AuditEvent::new(
        3,
        tenant,
        &principal,
        AuditAction::new("inference.denied"),
        AuditResource::new("virtual_key", Some("vk-redacted"), Some(tenant_id)),
        AuditOutcome::Denied,
        Some("budget_exceeded"),
    );
    let encoded = serde_json::to_string(&event).unwrap();

    assert!(encoded.contains("budget_exceeded"));
    assert!(!encoded.contains("Authorization"));
    assert!(!encoded.contains("Bearer"));
    assert!(!encoded.contains("secret"));
}

#[test]
fn audit_query_scope_allows_same_tenant_events() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let event = AuditEvent::new(
        1_725_000_000_000,
        tenant,
        &principal,
        AuditAction::new("audit.query"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let scope = AuditQueryScope::tenant(tenant);

    assert_eq!(scope.authorize_event(&event), Ok(()));
}

#[test]
fn audit_query_scope_rejects_cross_tenant_events() {
    let query_tenant_id = TenantId::new();
    let event_tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(event_tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let event_tenant = TenantContext {
        tenant_id: event_tenant_id,
    };
    let event = AuditEvent::new(
        1_725_000_000_000,
        event_tenant,
        &principal,
        AuditAction::new("audit.query"),
        AuditResource::new("audit_log", None::<String>, Some(event_tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let scope = AuditQueryScope::tenant(TenantContext {
        tenant_id: query_tenant_id,
    });

    assert_eq!(
        scope.authorize_event(&event),
        Err(AuditQueryScopeError::CrossTenantEvent)
    );
}

#[test]
fn audit_query_scope_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let scope = AuditQueryScope::tenant(TenantContext { tenant_id });
    let plan = AuditQueryPlan::new(
        scope,
        AuditTimeRange::new(None, None).unwrap(),
        AuditPageLimit::new(Some(10)).unwrap(),
        AuditSortOrder::OccurredAtAsc,
    );

    let rendered = format!("{scope:?} {plan:?}");

    assert!(
        !rendered.contains(&tenant_id.to_string()),
        "audit query scope debug output leaked tenant ID: {rendered}"
    );
}

#[test]
fn audit_query_scope_error_debug_output_is_stable_and_redacted() {
    let error = AuditQueryScopeError::CrossTenantEvent;

    assert_eq!(error.to_string(), "audit query scope is invalid");
    assert_eq!(
        AuditQueryPlanError::Scope(error.clone()).to_string(),
        "audit query scope is invalid"
    );
    assert!(!error.to_string().contains("tenant"));
    assert_eq!(format!("{error:?}"), "CrossTenantEvent");
}

#[test]
fn audit_query_and_export_plan_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let plan = AuditQueryPlan::new(
        AuditQueryScope::tenant(TenantContext { tenant_id }),
        AuditTimeRange::new(
            Some(AuditTimestamp::new(1_725_000_000_000).unwrap()),
            Some(AuditTimestamp::new(1_725_000_001_000).unwrap()),
        )
        .unwrap(),
        AuditPageLimit::new(Some(50)).unwrap(),
        AuditSortOrder::OccurredAtDesc,
    );
    let export = AuditExportPlan::new(plan, AuditExportFormat::Jsonl);

    let rendered = format!("{plan:?} {export:?}");

    for sensitive in [
        &tenant_id.to_string(),
        "1725000000000",
        "1725000001000",
        "50",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "audit query/export debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
}

#[test]
fn audit_error_response_plan_debug_output_is_stable_and_redacted() {
    let response = plan_audit_outcome_error_response(&AuditOutcomeError::Unknown);

    assert_eq!(
        format!("{response:?}"),
        "AuditErrorResponsePlan { status: InvalidRequest, code: \"audit_outcome_invalid\", message: \"audit outcome is invalid\" }"
    );
}

#[test]
fn audit_query_plan_matches_tenant_and_time_range() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let event = AuditEvent::new(
        1_725_000_000_500,
        tenant,
        &principal,
        AuditAction::new("audit.query"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let plan = AuditQueryPlan::new(
        AuditQueryScope::tenant(tenant),
        AuditTimeRange::new(
            Some(AuditTimestamp::new(1_725_000_000_000).unwrap()),
            Some(AuditTimestamp::new(1_725_000_001_000).unwrap()),
        )
        .unwrap(),
        AuditPageLimit::new(Some(50)).unwrap(),
        AuditSortOrder::OccurredAtDesc,
    );

    assert_eq!(plan.matches_event(&event), Ok(true));
    assert_eq!(plan.page_limit.get(), 50);
    assert_eq!(plan.sort_order, AuditSortOrder::OccurredAtDesc);
}

#[test]
fn audit_query_plan_excludes_out_of_range_events() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let event = AuditEvent::new(
        1_725_000_002_000,
        tenant,
        &principal,
        AuditAction::new("audit.query"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let plan = AuditQueryPlan::new(
        AuditQueryScope::tenant(tenant),
        AuditTimeRange::new(
            Some(AuditTimestamp::new(1_725_000_000_000).unwrap()),
            Some(AuditTimestamp::new(1_725_000_001_000).unwrap()),
        )
        .unwrap(),
        AuditPageLimit::new(None).unwrap(),
        AuditSortOrder::OccurredAtDesc,
    );

    assert_eq!(plan.matches_event(&event), Ok(false));
}

#[test]
fn audit_query_plan_fails_closed_for_cross_tenant_or_invalid_timestamp_events() {
    let query_tenant_id = TenantId::new();
    let event_tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(event_tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let cross_tenant = AuditEvent::new(
        1_725_000_000_500,
        TenantContext {
            tenant_id: event_tenant_id,
        },
        &principal,
        AuditAction::new("audit.query"),
        AuditResource::new("audit_log", None::<String>, Some(event_tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let invalid_timestamp = AuditEvent::new(
        0,
        TenantContext {
            tenant_id: query_tenant_id,
        },
        &principal,
        AuditAction::new("audit.query"),
        AuditResource::new("audit_log", None::<String>, Some(query_tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let plan = AuditQueryPlan::new(
        AuditQueryScope::tenant(TenantContext {
            tenant_id: query_tenant_id,
        }),
        AuditTimeRange::new(None, None).unwrap(),
        AuditPageLimit::new(None).unwrap(),
        AuditSortOrder::OccurredAtDesc,
    );

    assert_eq!(
        plan.matches_event(&cross_tenant),
        Err(AuditQueryPlanError::Scope(
            AuditQueryScopeError::CrossTenantEvent
        ))
    );
    assert_eq!(
        plan.matches_event(&invalid_timestamp),
        Err(AuditQueryPlanError::Timestamp(AuditTimestampError::Zero))
    );
}

#[test]
fn audit_query_plan_error_debug_output_is_stable_and_redacted() {
    let rejected = AuditEvent::MAX_UNIX_MS + 1;
    let error =
        AuditQueryPlanError::Timestamp(AuditTimestampError::TooFarFuture { unix_ms: rejected });

    assert_eq!(error.to_string(), "audit query cursor is invalid");

    let rendered = format!("{error:?}");
    assert!(
        !rendered.contains(&rejected.to_string()),
        "audit query plan error debug output leaked sensitive value: {rendered}"
    );
}

#[test]
fn audit_query_plan_selects_sorted_page_of_matching_events() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let oldest = AuditEvent::new(
        1_725_000_000_100,
        tenant,
        &principal,
        AuditAction::new("audit.query"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let newest = AuditEvent::new(
        1_725_000_000_300,
        tenant,
        &principal,
        AuditAction::new("audit.query"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let middle = AuditEvent::new(
        1_725_000_000_200,
        tenant,
        &principal,
        AuditAction::new("audit.query"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let plan = AuditQueryPlan::new(
        AuditQueryScope::tenant(tenant),
        AuditTimeRange::new(None, None).unwrap(),
        AuditPageLimit::new(Some(2)).unwrap(),
        AuditSortOrder::OccurredAtDesc,
    );

    let selected = plan
        .select_events([&oldest, &newest, &middle])
        .unwrap()
        .into_iter()
        .map(|event| event.occurred_at_unix_ms)
        .collect::<Vec<_>>();

    assert_eq!(selected, vec![1_725_000_000_300, 1_725_000_000_200]);
}

#[test]
fn audit_query_plan_selects_page_after_cursor_position() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let oldest = AuditEvent::new(
        1_725_000_000_100,
        tenant,
        &principal,
        AuditAction::new("audit.query"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let newest = AuditEvent::new(
        1_725_000_000_300,
        tenant,
        &principal,
        AuditAction::new("audit.query"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let middle = AuditEvent::new(
        1_725_000_000_200,
        tenant,
        &principal,
        AuditAction::new("audit.query"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let plan = AuditQueryPlan::new(
        AuditQueryScope::tenant(tenant),
        AuditTimeRange::new(None, None).unwrap(),
        AuditPageLimit::new(Some(2)).unwrap(),
        AuditSortOrder::OccurredAtDesc,
    );
    let after = AuditQueryCursor::from_event(&newest, AuditSortOrder::OccurredAtDesc).unwrap();

    let selected = plan
        .select_events_after([&oldest, &newest, &middle], Some(after))
        .unwrap()
        .into_iter()
        .map(|event| event.occurred_at_unix_ms)
        .collect::<Vec<_>>();

    assert_eq!(selected, vec![1_725_000_000_200, 1_725_000_000_100]);
}

#[test]
fn audit_query_plan_pages_events_with_next_cursor() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let first = AuditEvent::new(
        1_725_000_000_100,
        tenant,
        &principal,
        AuditAction::new("audit.query"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let second = AuditEvent::new(
        1_725_000_000_200,
        tenant,
        &principal,
        AuditAction::new("audit.query"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let third = AuditEvent::new(
        1_725_000_000_300,
        tenant,
        &principal,
        AuditAction::new("audit.query"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let plan = AuditQueryPlan::new(
        AuditQueryScope::tenant(tenant),
        AuditTimeRange::new(None, None).unwrap(),
        AuditPageLimit::new(Some(2)).unwrap(),
        AuditSortOrder::OccurredAtAsc,
    );

    let page = plan.page_events([&third, &first, &second], None).unwrap();
    let next = AuditQueryCursor::parse(page.next_cursor.as_ref().unwrap()).unwrap();

    assert_eq!(
        page.items
            .iter()
            .map(|event| event.occurred_at_unix_ms)
            .collect::<Vec<_>>(),
        vec![1_725_000_000_100, 1_725_000_000_200]
    );
    assert_eq!(next.event_id, second.immutable_key());
    assert_eq!(next.sort_order, AuditSortOrder::OccurredAtAsc);
}

#[test]
fn audit_query_plan_page_omits_next_cursor_on_final_page() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let first = AuditEvent::new(
        1_725_000_000_100,
        tenant,
        &principal,
        AuditAction::new("audit.query"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let second = AuditEvent::new(
        1_725_000_000_200,
        tenant,
        &principal,
        AuditAction::new("audit.query"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let plan = AuditQueryPlan::new(
        AuditQueryScope::tenant(tenant),
        AuditTimeRange::new(None, None).unwrap(),
        AuditPageLimit::new(Some(2)).unwrap(),
        AuditSortOrder::OccurredAtAsc,
    );

    let page = plan.page_events([&first, &second], None).unwrap();

    assert_eq!(page.items.len(), 2);
    assert_eq!(page.next_cursor, None);
}

#[test]
fn audit_query_page_error_debug_output_is_stable_and_redacted() {
    let rejected = AuditEvent::MAX_UNIX_MS + 1;
    let error =
        AuditQueryPageError::Timestamp(AuditTimestampError::TooFarFuture { unix_ms: rejected });

    assert_eq!(error.to_string(), "audit query cursor is invalid");

    let rendered = format!("{error:?}");
    assert!(
        !rendered.contains(&rejected.to_string()),
        "audit query page error debug output leaked sensitive value: {rendered}"
    );
}

#[test]
fn audit_query_plan_rejects_cursor_sort_order_mismatch() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let event = AuditEvent::new(
        1_725_000_000_100,
        tenant,
        &principal,
        AuditAction::new("audit.query"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let plan = AuditQueryPlan::new(
        AuditQueryScope::tenant(tenant),
        AuditTimeRange::new(None, None).unwrap(),
        AuditPageLimit::new(Some(2)).unwrap(),
        AuditSortOrder::OccurredAtAsc,
    );
    let after = AuditQueryCursor::from_event(&event, AuditSortOrder::OccurredAtDesc).unwrap();

    assert_eq!(
        plan.select_events_after([&event], Some(after)),
        Err(AuditQueryPlanError::CursorSortOrderMismatch)
    );
}

#[test]
fn audit_query_plan_selection_fails_closed_for_unscoped_or_invalid_events() {
    let query_tenant_id = TenantId::new();
    let event_tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(query_tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let valid_event = AuditEvent::new(
        1_725_000_000_100,
        TenantContext {
            tenant_id: query_tenant_id,
        },
        &principal,
        AuditAction::new("audit.query"),
        AuditResource::new("audit_log", None::<String>, Some(query_tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let cross_tenant = AuditEvent::new(
        1_725_000_000_200,
        TenantContext {
            tenant_id: event_tenant_id,
        },
        &principal,
        AuditAction::new("audit.query"),
        AuditResource::new("audit_log", None::<String>, Some(event_tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let invalid_timestamp = AuditEvent::new(
        0,
        TenantContext {
            tenant_id: query_tenant_id,
        },
        &principal,
        AuditAction::new("audit.query"),
        AuditResource::new("audit_log", None::<String>, Some(query_tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let plan = AuditQueryPlan::new(
        AuditQueryScope::tenant(TenantContext {
            tenant_id: query_tenant_id,
        }),
        AuditTimeRange::new(None, None).unwrap(),
        AuditPageLimit::new(Some(10)).unwrap(),
        AuditSortOrder::OccurredAtAsc,
    );

    assert_eq!(
        plan.select_events([&valid_event, &cross_tenant]),
        Err(AuditQueryPlanError::Scope(
            AuditQueryScopeError::CrossTenantEvent
        ))
    );
    assert_eq!(
        plan.select_events([&valid_event, &invalid_timestamp]),
        Err(AuditQueryPlanError::Timestamp(AuditTimestampError::Zero))
    );
}

#[test]
fn audit_query_cursor_round_trips_stable_position() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let event = AuditEvent::new(
        1_725_000_000_100,
        tenant,
        &principal,
        AuditAction::new("audit.query"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let position = AuditQueryCursor::from_event(&event, AuditSortOrder::OccurredAtDesc).unwrap();
    let cursor = position.to_cursor().unwrap();
    let parsed = AuditQueryCursor::parse(&cursor).unwrap();

    assert_eq!(parsed, position);
    assert_eq!(parsed.occurred_at.unix_ms(), 1_725_000_000_100);
    assert_eq!(parsed.event_id, event.immutable_key());
    assert_eq!(parsed.sort_order, AuditSortOrder::OccurredAtDesc);
    assert!(cursor.as_str().starts_with("audit:v1:occurred_at_desc:"));
}

#[test]
fn audit_query_cursor_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let event = AuditEvent::new(
        1_725_000_000_100,
        TenantContext { tenant_id },
        &principal,
        AuditAction::new("audit.query"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let position = AuditQueryCursor::from_event(&event, AuditSortOrder::OccurredAtDesc).unwrap();

    assert_eq!(position.occurred_at.unix_ms(), event.occurred_at_unix_ms);
    assert_eq!(position.event_id, event.immutable_key());

    let rendered = format!("{position:?}");
    for sensitive in [
        event.occurred_at_unix_ms.to_string(),
        event.immutable_key().to_string(),
    ] {
        assert!(
            !rendered.contains(&sensitive),
            "audit query cursor debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
}

#[test]
fn audit_query_cursor_rejects_empty_malformed_or_incompatible_values() {
    assert!(matches!(
        AuditQueryCursor::try_parse(" "),
        Err(AuditQueryCursorError::Cursor(_))
    ));
    assert_eq!(
        AuditQueryCursor::try_parse("audit:v1:occurred_at_desc"),
        Err(AuditQueryCursorError::Malformed)
    );
    assert_eq!(
        AuditQueryCursor::try_parse("audit:v2:occurred_at_desc:1725000000100:not-a-uuid"),
        Err(AuditQueryCursorError::UnsupportedVersion)
    );
    assert_eq!(
        AuditQueryCursor::try_parse("audit:v1:created_at_desc:1725000000100:not-a-uuid"),
        Err(AuditQueryCursorError::SortOrder(
            AuditSortOrderError::Unknown
        ))
    );
    assert_eq!(
        AuditQueryCursor::try_parse("audit:v1:occurred_at_desc:0:not-a-uuid"),
        Err(AuditQueryCursorError::Timestamp(AuditTimestampError::Zero))
    );
    assert!(matches!(
        AuditQueryCursor::try_parse("audit:v1:occurred_at_desc:1725000000100:not-a-uuid"),
        Err(AuditQueryCursorError::EventId(_))
    ));
}

#[test]
fn audit_query_cursor_parse_accepts_generic_cursor_boundary() {
    let event_id = AuditEventId::new();
    let cursor = Cursor::new(format!("audit:v1:occurred_at_asc:1725000000100:{event_id}")).unwrap();
    let parsed = AuditQueryCursor::parse(&cursor).unwrap();

    assert_eq!(parsed.event_id, event_id);
    assert_eq!(parsed.sort_order, AuditSortOrder::OccurredAtAsc);
}

#[test]
fn audit_query_cursor_error_debug_output_is_stable_and_redacted() {
    let rejected = AuditEvent::MAX_UNIX_MS + 1;
    let error =
        AuditQueryCursorError::Timestamp(AuditTimestampError::TooFarFuture { unix_ms: rejected });

    assert_eq!(error.to_string(), "audit query cursor is invalid");

    let rendered = format!("{error:?}");
    assert!(
        !rendered.contains(&rejected.to_string()),
        "audit query cursor error debug output leaked sensitive value: {rendered}"
    );
}

#[test]
fn audit_digest_rejects_empty_values() {
    assert_eq!(AuditDigest::new(""), Err(AuditDigestError::Empty));
    assert_eq!(
        AuditDigest::new(" "),
        Err(AuditDigestError::InvalidCharacter)
    );
    assert_eq!(
        AuditDigest::new("a".repeat(129)),
        Err(AuditDigestError::TooLong { length: 129 })
    );
    assert_eq!(
        AuditDigest::new("sha256:Secret/Token"),
        Err(AuditDigestError::InvalidCharacter)
    );
    assert_eq!(
        AuditDigest::new(" sha256:previous"),
        Err(AuditDigestError::InvalidCharacter)
    );
}

#[test]
fn audit_digest_error_debug_output_is_stable_and_redacted() {
    let error = AuditDigestError::TooLong { length: 129 };

    assert_eq!(error.to_string(), "audit digest is invalid");

    let rendered = format!("{error:?}");
    assert!(
        !rendered.contains("129"),
        "audit digest error debug output leaked sensitive length: {rendered}"
    );
}

#[test]
fn audit_outcome_parse_accepts_stable_machine_readable_values() {
    assert_eq!(AuditOutcome::parse("success"), Ok(AuditOutcome::Success));
    assert_eq!(AuditOutcome::parse("denied"), Ok(AuditOutcome::Denied));
    assert_eq!(AuditOutcome::parse("failed"), Ok(AuditOutcome::Failed));
    assert_eq!(AuditOutcome::Denied.as_str(), "denied");
}

#[test]
fn audit_outcome_parse_rejects_empty_unknown_or_secret_like_values() {
    assert_eq!(AuditOutcome::parse(""), Err(AuditOutcomeError::Empty));
    assert_eq!(AuditOutcome::parse(" "), Err(AuditOutcomeError::Unknown));
    assert_eq!(
        AuditOutcome::parse("SUCCESS"),
        Err(AuditOutcomeError::Unknown)
    );
    assert_eq!(
        AuditOutcome::parse(" success"),
        Err(AuditOutcomeError::Unknown)
    );
    assert_eq!(
        AuditOutcome::parse("Authorization: Bearer secret-token"),
        Err(AuditOutcomeError::Unknown)
    );
}

#[test]
fn audit_outcome_error_debug_output_is_stable_and_redacted() {
    let error = AuditOutcomeError::Unknown;

    assert_eq!(
        AuditOutcomeError::Empty.to_string(),
        "audit outcome is invalid"
    );
    assert_eq!(error.to_string(), "audit outcome is invalid");
    assert!(!error.to_string().contains("unknown"));
    assert_eq!(format!("{error:?}"), "Unknown");
}

#[test]
fn audit_export_format_parse_accepts_stable_formats() {
    assert_eq!(
        AuditExportFormat::parse("jsonl"),
        Ok(AuditExportFormat::Jsonl)
    );
    assert_eq!(AuditExportFormat::parse("csv"), Ok(AuditExportFormat::Csv));
    assert_eq!(
        AuditExportFormat::Jsonl.content_type(),
        "application/x-ndjson"
    );
    assert_eq!(AuditExportFormat::Csv.file_extension(), "csv");
}

#[test]
fn audit_export_format_parse_rejects_empty_unknown_or_secret_like_values() {
    assert_eq!(
        AuditExportFormat::parse(""),
        Err(AuditExportFormatError::Empty)
    );
    assert_eq!(
        AuditExportFormat::parse(" "),
        Err(AuditExportFormatError::Unknown)
    );
    assert_eq!(
        AuditExportFormat::parse("JSON"),
        Err(AuditExportFormatError::Unknown)
    );
    assert_eq!(
        AuditExportFormat::parse(" jsonl"),
        Err(AuditExportFormatError::Unknown)
    );
    assert_eq!(
        AuditExportFormat::parse("Authorization: Bearer secret-token"),
        Err(AuditExportFormatError::Unknown)
    );
}

#[test]
fn audit_export_format_error_debug_output_is_stable_and_redacted() {
    let error = AuditExportFormatError::Unknown;

    assert_eq!(
        AuditExportFormatError::Empty.to_string(),
        "audit export format is invalid"
    );
    assert_eq!(error.to_string(), "audit export format is invalid");
    assert!(!error.to_string().contains("unknown"));
    assert_eq!(format!("{error:?}"), "Unknown");
}

#[test]
fn audit_export_plan_uses_query_scope_and_format_metadata() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let event = AuditEvent::new(
        1_725_000_000_500,
        tenant,
        &principal,
        AuditAction::new("audit.export"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let query = AuditQueryPlan::new(
        AuditQueryScope::tenant(tenant),
        AuditTimeRange::new(
            Some(AuditTimestamp::new(1_725_000_000_000).unwrap()),
            Some(AuditTimestamp::new(1_725_000_001_000).unwrap()),
        )
        .unwrap(),
        AuditPageLimit::new(Some(100)).unwrap(),
        AuditSortOrder::OccurredAtAsc,
    );
    let plan = AuditExportPlan::new(query, AuditExportFormat::Jsonl);

    assert_eq!(plan.content_type(), "application/x-ndjson");
    assert_eq!(plan.file_extension(), "jsonl");
    assert_eq!(plan.matches_event(&event), Ok(true));
}

#[test]
fn audit_export_plan_fails_closed_for_cross_tenant_events() {
    let query_tenant_id = TenantId::new();
    let event_tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(event_tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let event = AuditEvent::new(
        1_725_000_000_500,
        TenantContext {
            tenant_id: event_tenant_id,
        },
        &principal,
        AuditAction::new("audit.export"),
        AuditResource::new("audit_log", None::<String>, Some(event_tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let query = AuditQueryPlan::new(
        AuditQueryScope::tenant(TenantContext {
            tenant_id: query_tenant_id,
        }),
        AuditTimeRange::new(None, None).unwrap(),
        AuditPageLimit::new(None).unwrap(),
        AuditSortOrder::OccurredAtDesc,
    );
    let plan = AuditExportPlan::new(query, AuditExportFormat::Csv);

    assert_eq!(plan.content_type(), "text/csv");
    assert_eq!(plan.file_extension(), "csv");
    assert_eq!(
        plan.matches_event(&event),
        Err(AuditQueryPlanError::Scope(
            AuditQueryScopeError::CrossTenantEvent
        ))
    );
}

#[test]
fn audit_export_plan_selects_events_with_query_plan_rules() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let inside = AuditEvent::new(
        1_725_000_000_500,
        tenant,
        &principal,
        AuditAction::new("audit.export"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let outside = AuditEvent::new(
        1_725_000_002_000,
        tenant,
        &principal,
        AuditAction::new("audit.export"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let query = AuditQueryPlan::new(
        AuditQueryScope::tenant(tenant),
        AuditTimeRange::new(
            Some(AuditTimestamp::new(1_725_000_000_000).unwrap()),
            Some(AuditTimestamp::new(1_725_000_001_000).unwrap()),
        )
        .unwrap(),
        AuditPageLimit::new(Some(10)).unwrap(),
        AuditSortOrder::OccurredAtAsc,
    );
    let plan = AuditExportPlan::new(query, AuditExportFormat::Jsonl);

    let selected = plan.select_events([&outside, &inside]).unwrap();

    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].immutable_key(), inside.immutable_key());
}

#[test]
fn audit_export_plan_selects_events_after_cursor_with_query_rules() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let first = AuditEvent::new(
        1_725_000_000_100,
        tenant,
        &principal,
        AuditAction::new("audit.export"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let second = AuditEvent::new(
        1_725_000_000_200,
        tenant,
        &principal,
        AuditAction::new("audit.export"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let query = AuditQueryPlan::new(
        AuditQueryScope::tenant(tenant),
        AuditTimeRange::new(None, None).unwrap(),
        AuditPageLimit::new(Some(10)).unwrap(),
        AuditSortOrder::OccurredAtAsc,
    );
    let after = AuditQueryCursor::from_event(&first, AuditSortOrder::OccurredAtAsc).unwrap();
    let plan = AuditExportPlan::new(query, AuditExportFormat::Jsonl);

    let selected = plan
        .select_events_after([&first, &second], Some(after))
        .unwrap();

    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].immutable_key(), second.immutable_key());
}

#[test]
fn audit_export_plan_pages_events_with_query_rules() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let first = AuditEvent::new(
        1_725_000_000_100,
        tenant,
        &principal,
        AuditAction::new("audit.export"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let second = AuditEvent::new(
        1_725_000_000_200,
        tenant,
        &principal,
        AuditAction::new("audit.export"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let query = AuditQueryPlan::new(
        AuditQueryScope::tenant(tenant),
        AuditTimeRange::new(None, None).unwrap(),
        AuditPageLimit::new(Some(1)).unwrap(),
        AuditSortOrder::OccurredAtAsc,
    );
    let plan = AuditExportPlan::new(query, AuditExportFormat::Jsonl);

    let page = plan.page_events([&second, &first], None).unwrap();

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].immutable_key(), first.immutable_key());
    assert!(page.next_cursor.is_some());
}

#[test]
fn audit_sort_order_parse_accepts_stable_values() {
    assert_eq!(
        AuditSortOrder::parse("occurred_at_asc"),
        Ok(AuditSortOrder::OccurredAtAsc)
    );
    assert_eq!(
        AuditSortOrder::parse("occurred_at_desc"),
        Ok(AuditSortOrder::OccurredAtDesc)
    );
    assert_eq!(AuditSortOrder::OccurredAtDesc.as_str(), "occurred_at_desc");
}

#[test]
fn audit_sort_order_parse_rejects_empty_unknown_or_secret_like_values() {
    assert_eq!(AuditSortOrder::parse(""), Err(AuditSortOrderError::Empty));
    assert_eq!(
        AuditSortOrder::parse(" "),
        Err(AuditSortOrderError::Unknown)
    );
    assert_eq!(
        AuditSortOrder::parse("created_at_desc"),
        Err(AuditSortOrderError::Unknown)
    );
    assert_eq!(
        AuditSortOrder::parse(" occurred_at_desc"),
        Err(AuditSortOrderError::Unknown)
    );
    assert_eq!(
        AuditSortOrder::parse("Authorization: Bearer secret-token"),
        Err(AuditSortOrderError::Unknown)
    );
}

#[test]
fn audit_sort_order_error_debug_output_is_stable_and_redacted() {
    let error = AuditSortOrderError::Unknown;

    assert_eq!(
        AuditSortOrderError::Empty.to_string(),
        "audit sort order is invalid"
    );
    assert_eq!(error.to_string(), "audit sort order is invalid");
    assert!(!error.to_string().contains("unknown"));
    assert_eq!(format!("{error:?}"), "Unknown");
}

#[test]
fn audit_timestamp_accepts_bounded_unix_milliseconds() {
    let timestamp = AuditTimestamp::new(1_725_000_000_000).unwrap();

    assert_eq!(timestamp.unix_ms(), 1_725_000_000_000);
}

#[test]
fn audit_timestamp_debug_output_is_stable_and_redacted() {
    let timestamp = AuditTimestamp::new(1_725_000_000_000).unwrap();

    assert_eq!(timestamp.unix_ms(), 1_725_000_000_000);

    let rendered = format!("{timestamp:?}");
    assert!(
        !rendered.contains("1725000000000"),
        "audit timestamp debug output leaked sensitive value: {rendered}"
    );
}

#[test]
fn audit_timestamp_rejects_zero_pre_unix_ms_and_far_future_values() {
    assert_eq!(AuditTimestamp::new(0), Err(AuditTimestampError::Zero));
    assert_eq!(
        AuditTimestamp::new(999),
        Err(AuditTimestampError::BeforeUnixMilliseconds)
    );
    assert_eq!(
        AuditTimestamp::new(AuditEvent::MAX_UNIX_MS + 1),
        Err(AuditTimestampError::TooFarFuture {
            unix_ms: AuditEvent::MAX_UNIX_MS + 1
        })
    );
}

#[test]
fn audit_timestamp_error_debug_output_is_stable_and_redacted() {
    let rejected = AuditEvent::MAX_UNIX_MS + 1;
    let error = AuditTimestampError::TooFarFuture { unix_ms: rejected };

    assert_eq!(error.to_string(), "audit timestamp is invalid");
    assert!(!error.to_string().contains("future"));

    let rendered = format!("{error:?}");
    assert!(
        !rendered.contains(&rejected.to_string()),
        "audit timestamp error debug output leaked sensitive value: {rendered}"
    );
}

#[test]
fn audit_time_range_accepts_ordered_bounds_and_checks_membership() {
    let start = AuditTimestamp::new(1_725_000_000_000).unwrap();
    let end = AuditTimestamp::new(1_725_000_001_000).unwrap();
    let inside = AuditTimestamp::new(1_725_000_000_500).unwrap();
    let outside = AuditTimestamp::new(1_725_000_001_001).unwrap();
    let range = AuditTimeRange::new(Some(start), Some(end)).unwrap();

    assert!(range.contains(inside));
    assert!(!range.contains(outside));
}

#[test]
fn audit_time_range_debug_output_is_stable_and_redacted() {
    let start = AuditTimestamp::new(1_725_000_000_000).unwrap();
    let end = AuditTimestamp::new(1_725_000_001_000).unwrap();
    let range = AuditTimeRange::new(Some(start), Some(end)).unwrap();

    assert_eq!(range.start, Some(start));
    assert_eq!(range.end, Some(end));

    let rendered = format!("{range:?}");
    for sensitive in [start.unix_ms().to_string(), end.unix_ms().to_string()] {
        assert!(
            !rendered.contains(&sensitive),
            "audit time range debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
}

#[test]
fn audit_time_range_rejects_start_after_end() {
    let start = AuditTimestamp::new(1_725_000_001_000).unwrap();
    let end = AuditTimestamp::new(1_725_000_000_000).unwrap();

    assert_eq!(
        AuditTimeRange::new(Some(start), Some(end)),
        Err(AuditTimeRangeError::StartAfterEnd)
    );
}

#[test]
fn audit_time_range_error_debug_output_is_stable_and_redacted() {
    let error = AuditTimeRangeError::StartAfterEnd;

    assert_eq!(error.to_string(), "audit time range is invalid");
    assert_eq!(format!("{error:?}"), "StartAfterEnd");
}

#[test]
fn audit_page_limit_defaults_and_accepts_bounded_values() {
    assert_eq!(
        AuditPageLimit::new(None).unwrap().get(),
        AuditPageLimit::DEFAULT
    );
    assert_eq!(
        AuditPageLimit::new(Some(AuditPageLimit::MAX))
            .unwrap()
            .get(),
        AuditPageLimit::MAX
    );
}

#[test]
fn audit_page_limit_debug_output_is_stable_and_redacted() {
    let limit = AuditPageLimit::new(Some(73)).unwrap();

    assert_eq!(limit.get(), 73);

    let rendered = format!("{limit:?}");
    assert!(
        !rendered.contains("73"),
        "audit page limit debug output leaked sensitive value: {rendered}"
    );
}

#[test]
fn audit_page_limit_rejects_zero_and_oversized_values() {
    assert_eq!(AuditPageLimit::new(Some(0)), Err(AuditPageLimitError::Zero));
    assert_eq!(
        AuditPageLimit::new(Some(AuditPageLimit::MAX + 1)),
        Err(AuditPageLimitError::TooLarge {
            value: AuditPageLimit::MAX + 1
        })
    );
}

#[test]
fn audit_page_limit_error_debug_output_is_stable_and_redacted() {
    let rejected = AuditPageLimit::MAX + 1;
    let error = AuditPageLimitError::TooLarge { value: rejected };

    assert_eq!(error.to_string(), "audit page limit is invalid");
    assert!(!error.to_string().contains("too large"));

    let rendered = format!("{error:?}");
    assert!(
        !rendered.contains(&rejected.to_string()),
        "audit page limit error debug output leaked sensitive value: {rendered}"
    );
}

#[test]
fn audit_retention_policy_defaults_and_accepts_bounded_days() {
    assert_eq!(
        AuditRetentionPolicy::new(None).unwrap().days(),
        AuditRetentionPolicy::DEFAULT_DAYS
    );
    assert_eq!(
        AuditRetentionPolicy::new(Some(AuditRetentionPolicy::MIN_DAYS))
            .unwrap()
            .days(),
        AuditRetentionPolicy::MIN_DAYS
    );
    assert_eq!(
        AuditRetentionPolicy::new(Some(AuditRetentionPolicy::MAX_DAYS))
            .unwrap()
            .days(),
        AuditRetentionPolicy::MAX_DAYS
    );
}

#[test]
fn audit_retention_policy_debug_output_is_stable_and_redacted() {
    let policy = AuditRetentionPolicy::new(Some(731)).unwrap();

    assert_eq!(policy.days(), 731);

    let rendered = format!("{policy:?}");
    assert!(
        !rendered.contains("731"),
        "audit retention policy debug output leaked sensitive value: {rendered}"
    );
}

#[test]
fn audit_retention_policy_rejects_too_short_or_too_long_values() {
    assert_eq!(
        AuditRetentionPolicy::new(Some(AuditRetentionPolicy::MIN_DAYS - 1)),
        Err(AuditRetentionPolicyError::TooShort {
            days: AuditRetentionPolicy::MIN_DAYS - 1
        })
    );
    assert_eq!(
        AuditRetentionPolicy::new(Some(AuditRetentionPolicy::MAX_DAYS + 1)),
        Err(AuditRetentionPolicyError::TooLong {
            days: AuditRetentionPolicy::MAX_DAYS + 1
        })
    );
}

#[test]
fn audit_retention_policy_error_debug_output_is_stable_and_redacted() {
    let rejected_short = AuditRetentionPolicy::MIN_DAYS - 1;
    let rejected_long = AuditRetentionPolicy::MAX_DAYS + 1;

    let cases = [
        (
            AuditRetentionPolicyError::TooShort {
                days: rejected_short,
            },
            rejected_short,
        ),
        (
            AuditRetentionPolicyError::TooLong {
                days: rejected_long,
            },
            rejected_long,
        ),
    ];

    for (error, rejected) in cases {
        assert_eq!(error.to_string(), "audit retention policy is invalid");

        let rendered = format!("{error:?}");
        assert!(
            !rendered.contains(&rejected.to_string()),
            "audit retention policy error debug output leaked sensitive value: {rendered}"
        );
    }
}

#[test]
fn audit_retention_batch_limit_defaults_and_accepts_bounded_values() {
    assert_eq!(
        AuditRetentionBatchLimit::new(None).unwrap().get(),
        AuditRetentionBatchLimit::DEFAULT
    );
    assert_eq!(
        AuditRetentionBatchLimit::new(Some(AuditRetentionBatchLimit::MAX))
            .unwrap()
            .get(),
        AuditRetentionBatchLimit::MAX
    );
}

#[test]
fn audit_retention_batch_limit_debug_output_is_stable_and_redacted() {
    let limit = AuditRetentionBatchLimit::new(Some(47)).unwrap();

    assert_eq!(limit.get(), 47);

    let rendered = format!("{limit:?}");
    assert!(
        !rendered.contains("47"),
        "audit retention batch limit debug output leaked sensitive value: {rendered}"
    );
}

#[test]
fn audit_retention_batch_limit_rejects_zero_and_oversized_values() {
    assert_eq!(
        AuditRetentionBatchLimit::new(Some(0)),
        Err(AuditRetentionBatchLimitError::Zero)
    );
    assert_eq!(
        AuditRetentionBatchLimit::new(Some(AuditRetentionBatchLimit::MAX + 1)),
        Err(AuditRetentionBatchLimitError::TooLarge {
            value: AuditRetentionBatchLimit::MAX + 1
        })
    );
}

#[test]
fn audit_retention_batch_limit_error_debug_output_is_stable_and_redacted() {
    let rejected = AuditRetentionBatchLimit::MAX + 1;
    let error = AuditRetentionBatchLimitError::TooLarge { value: rejected };

    assert_eq!(
        error.to_string(),
        "audit retention batch limit is too large"
    );

    let rendered = format!("{error:?}");
    assert!(
        !rendered.contains(&rejected.to_string()),
        "audit retention batch limit error debug output leaked sensitive value: {rendered}"
    );
}

#[test]
fn audit_retention_plan_marks_only_in_scope_expired_events() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let now = AuditTimestamp::new(1_800_000_000_000).unwrap();
    let old_event = AuditEvent::new(
        now.unix_ms() - (31 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let recent_event = AuditEvent::new(
        now.unix_ms() - (29 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let plan = AuditRetentionPlan::new(
        AuditQueryScope::tenant(tenant),
        AuditRetentionPolicy::new(Some(30)).unwrap(),
        now,
    );

    assert_eq!(plan.cutoff().unix_ms(), now.unix_ms() - (30 * 86_400_000));
    assert_eq!(plan.is_event_expired(&old_event), Ok(true));
    assert_eq!(plan.is_event_expired(&recent_event), Ok(false));
}

#[test]
fn audit_retention_plan_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let now = AuditTimestamp::new(1_800_000_000_000).unwrap();
    let plan = AuditRetentionPlan::new(
        AuditQueryScope::tenant(TenantContext { tenant_id }),
        AuditRetentionPolicy::new(Some(30)).unwrap(),
        now,
    );

    let rendered = format!("{plan:?}");

    for sensitive in [&tenant_id.to_string(), "1800000000000", "30"] {
        assert!(
            !rendered.contains(sensitive),
            "audit retention plan debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
}

#[test]
fn audit_retention_plan_selects_bounded_purge_candidates() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let now = AuditTimestamp::new(1_800_000_000_000).unwrap();
    let oldest = AuditEvent::new(
        now.unix_ms() - (33 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let old = AuditEvent::new(
        now.unix_ms() - (32 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let recent = AuditEvent::new(
        now.unix_ms() - (29 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let plan = AuditRetentionPlan::new(
        AuditQueryScope::tenant(tenant),
        AuditRetentionPolicy::new(Some(30)).unwrap(),
        now,
    );
    let candidates = plan
        .purge_candidates(
            [&recent, &old, &oldest],
            AuditRetentionBatchLimit::new(Some(1)).unwrap(),
        )
        .unwrap();

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].immutable_key(), oldest.immutable_key());
}

#[test]
fn audit_retention_plan_pages_purge_candidates_with_next_cursor() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let now = AuditTimestamp::new(1_800_000_000_000).unwrap();
    let first = AuditEvent::new(
        now.unix_ms() - (33 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let second = AuditEvent::new(
        now.unix_ms() - (32 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let third = AuditEvent::new(
        now.unix_ms() - (31 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let plan = AuditRetentionPlan::new(
        AuditQueryScope::tenant(tenant),
        AuditRetentionPolicy::new(Some(30)).unwrap(),
        now,
    );

    let page = plan
        .purge_candidate_page(
            [&third, &first, &second],
            AuditRetentionBatchLimit::new(Some(2)).unwrap(),
            None,
        )
        .unwrap();
    let next = AuditQueryCursor::parse(page.next_cursor.as_ref().unwrap()).unwrap();

    assert_eq!(page.items.len(), 2);
    assert_eq!(page.items[0].immutable_key(), first.immutable_key());
    assert_eq!(page.items[1].immutable_key(), second.immutable_key());
    assert_eq!(next.event_id, second.immutable_key());
    assert_eq!(next.sort_order, AuditSortOrder::OccurredAtAsc);
}

#[test]
fn audit_retention_plan_resumes_purge_candidates_after_cursor() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let now = AuditTimestamp::new(1_800_000_000_000).unwrap();
    let first = AuditEvent::new(
        now.unix_ms() - (33 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let second = AuditEvent::new(
        now.unix_ms() - (32 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let plan = AuditRetentionPlan::new(
        AuditQueryScope::tenant(tenant),
        AuditRetentionPolicy::new(Some(30)).unwrap(),
        now,
    );
    let after = AuditQueryCursor::from_event(&first, AuditSortOrder::OccurredAtAsc).unwrap();

    let page = plan
        .purge_candidate_page(
            [&first, &second],
            AuditRetentionBatchLimit::new(Some(10)).unwrap(),
            Some(after),
        )
        .unwrap();

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].immutable_key(), second.immutable_key());
    assert_eq!(page.next_cursor, None);
}

#[test]
fn audit_retention_plan_rejects_descending_purge_cursor() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let now = AuditTimestamp::new(1_800_000_000_000).unwrap();
    let event = AuditEvent::new(
        now.unix_ms() - (33 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let plan = AuditRetentionPlan::new(
        AuditQueryScope::tenant(tenant),
        AuditRetentionPolicy::new(Some(30)).unwrap(),
        now,
    );
    let after = AuditQueryCursor::from_event(&event, AuditSortOrder::OccurredAtDesc).unwrap();

    assert_eq!(
        plan.purge_candidate_page(
            [&event],
            AuditRetentionBatchLimit::new(Some(10)).unwrap(),
            Some(after)
        ),
        Err(AuditRetentionPageError::CursorSortOrderMismatch)
    );
}

#[test]
fn audit_retention_hold_protects_matching_event_while_active() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let event = AuditEvent::new(
        1_725_000_000_000,
        tenant,
        &principal,
        AuditAction::new("audit.retention_hold"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let hold = AuditRetentionHold::new(
        tenant,
        event.immutable_key(),
        AuditReasonCode::new("legal_hold.investigation").unwrap(),
        Some(AuditTimestamp::new(1_800_000_000_000).unwrap()),
    );

    assert_eq!(
        hold.protects_event(&event, AuditTimestamp::new(1_750_000_000_000).unwrap()),
        Ok(true)
    );
    assert_eq!(hold.reason_code.as_str(), "legal_hold.investigation");
}

#[test]
fn audit_retention_hold_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let event_id = AuditEventId::new();
    let hold = AuditRetentionHold::new(
        TenantContext { tenant_id },
        event_id,
        AuditReasonCode::new("legal_hold.investigation").unwrap(),
        Some(AuditTimestamp::new(1_800_000_000_000).unwrap()),
    );

    assert_eq!(hold.tenant_id, tenant_id);
    assert_eq!(hold.event_id, event_id);
    assert_eq!(hold.reason_code.as_str(), "legal_hold.investigation");

    let rendered = format!("{hold:?}");
    for sensitive in [
        &tenant_id.to_string(),
        &event_id.to_string(),
        "legal_hold.investigation",
        "1800000000000",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "audit retention hold debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
}

#[test]
fn audit_retention_hold_does_not_protect_non_matching_or_expired_events() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let held_event = AuditEvent::new(
        1_725_000_000_000,
        tenant,
        &principal,
        AuditAction::new("audit.retention_hold"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let other_event = AuditEvent::new(
        1_725_000_000_001,
        tenant,
        &principal,
        AuditAction::new("audit.retention_hold"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let hold = AuditRetentionHold::new(
        tenant,
        held_event.immutable_key(),
        AuditReasonCode::new("legal_hold.investigation").unwrap(),
        Some(AuditTimestamp::new(1_750_000_000_000).unwrap()),
    );

    assert_eq!(
        hold.protects_event(
            &other_event,
            AuditTimestamp::new(1_740_000_000_000).unwrap()
        ),
        Ok(false)
    );
    assert_eq!(
        hold.protects_event(&held_event, AuditTimestamp::new(1_760_000_000_000).unwrap()),
        Ok(false)
    );
}

#[test]
fn audit_retention_hold_fails_closed_for_cross_tenant_or_invalid_timestamp_events() {
    let hold_tenant_id = TenantId::new();
    let event_tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(hold_tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let cross_tenant = AuditEvent::new(
        1_725_000_000_000,
        TenantContext {
            tenant_id: event_tenant_id,
        },
        &principal,
        AuditAction::new("audit.retention_hold"),
        AuditResource::new("audit_log", None::<String>, Some(event_tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let invalid_timestamp = AuditEvent::new(
        0,
        TenantContext {
            tenant_id: hold_tenant_id,
        },
        &principal,
        AuditAction::new("audit.retention_hold"),
        AuditResource::new("audit_log", None::<String>, Some(hold_tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let hold = AuditRetentionHold::new(
        TenantContext {
            tenant_id: hold_tenant_id,
        },
        invalid_timestamp.immutable_key(),
        AuditReasonCode::new("legal_hold.investigation").unwrap(),
        None,
    );

    assert_eq!(
        hold.protects_event(
            &cross_tenant,
            AuditTimestamp::new(1_750_000_000_000).unwrap()
        ),
        Err(AuditRetentionHoldError::Scope(
            AuditQueryScopeError::CrossTenantEvent
        ))
    );
    assert_eq!(
        hold.protects_event(
            &invalid_timestamp,
            AuditTimestamp::new(1_750_000_000_000).unwrap()
        ),
        Err(AuditRetentionHoldError::Timestamp(
            AuditTimestampError::Zero
        ))
    );
}

#[test]
fn audit_retention_hold_error_debug_output_is_stable_and_redacted() {
    let rejected = AuditEvent::MAX_UNIX_MS + 1;
    let error =
        AuditRetentionHoldError::Timestamp(AuditTimestampError::TooFarFuture { unix_ms: rejected });

    assert_eq!(
        error.to_string(),
        "audit timestamp is too far in the future"
    );

    let rendered = format!("{error:?}");
    assert!(
        !rendered.contains(&rejected.to_string()),
        "audit retention hold error debug output leaked sensitive value: {rendered}"
    );
}

#[test]
fn audit_retention_decision_retains_recent_events_and_purges_expired_unheld_events() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let now = AuditTimestamp::new(1_800_000_000_000).unwrap();
    let recent = AuditEvent::new(
        now.unix_ms() - (29 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let expired = AuditEvent::new(
        now.unix_ms() - (31 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let plan = AuditRetentionPlan::new(
        AuditQueryScope::tenant(tenant),
        AuditRetentionPolicy::new(Some(30)).unwrap(),
        now,
    );

    assert_eq!(
        plan.purge_decision(&recent, std::iter::empty::<&AuditRetentionHold>()),
        Ok(AuditRetentionDecision::Retain)
    );
    assert_eq!(
        plan.purge_decision(&expired, std::iter::empty::<&AuditRetentionHold>()),
        Ok(AuditRetentionDecision::Purge)
    );
}

#[test]
fn audit_retention_decision_respects_active_holds() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let now = AuditTimestamp::new(1_800_000_000_000).unwrap();
    let expired = AuditEvent::new(
        now.unix_ms() - (31 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let hold = AuditRetentionHold::new(
        tenant,
        expired.immutable_key(),
        AuditReasonCode::new("legal_hold.investigation").unwrap(),
        Some(now),
    );
    let plan = AuditRetentionPlan::new(
        AuditQueryScope::tenant(tenant),
        AuditRetentionPolicy::new(Some(30)).unwrap(),
        now,
    );

    assert_eq!(
        plan.purge_decision(&expired, [&hold]),
        Ok(AuditRetentionDecision::ProtectedByHold)
    );
}

#[test]
fn audit_retention_decision_error_debug_output_is_stable_and_redacted() {
    let rejected = AuditEvent::MAX_UNIX_MS + 1;
    let error = AuditRetentionDecisionError::Hold(AuditRetentionHoldError::Timestamp(
        AuditTimestampError::TooFarFuture { unix_ms: rejected },
    ));

    assert_eq!(
        error.to_string(),
        "audit timestamp is too far in the future"
    );

    let rendered = format!("{error:?}");
    assert!(
        !rendered.contains(&rejected.to_string()),
        "audit retention decision error debug output leaked sensitive value: {rendered}"
    );
}

#[test]
fn audit_retention_plan_selects_only_unheld_purgeable_candidates() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let now = AuditTimestamp::new(1_800_000_000_000).unwrap();
    let held = AuditEvent::new(
        now.unix_ms() - (33 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let purgeable = AuditEvent::new(
        now.unix_ms() - (32 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let hold = AuditRetentionHold::new(
        tenant,
        held.immutable_key(),
        AuditReasonCode::new("legal_hold.investigation").unwrap(),
        None,
    );
    let plan = AuditRetentionPlan::new(
        AuditQueryScope::tenant(tenant),
        AuditRetentionPolicy::new(Some(30)).unwrap(),
        now,
    );

    let candidates = plan
        .purgeable_candidates(
            [&held, &purgeable],
            [&hold],
            AuditRetentionBatchLimit::new(Some(10)).unwrap(),
        )
        .unwrap();

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].immutable_key(), purgeable.immutable_key());
}

#[test]
fn audit_retention_plan_pages_only_unheld_purgeable_candidates_with_next_cursor() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let now = AuditTimestamp::new(1_800_000_000_000).unwrap();
    let first = AuditEvent::new(
        now.unix_ms() - (34 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let held = AuditEvent::new(
        now.unix_ms() - (33 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let second = AuditEvent::new(
        now.unix_ms() - (32 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let third = AuditEvent::new(
        now.unix_ms() - (31 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let hold = AuditRetentionHold::new(
        tenant,
        held.immutable_key(),
        AuditReasonCode::new("legal_hold.investigation").unwrap(),
        None,
    );
    let plan = AuditRetentionPlan::new(
        AuditQueryScope::tenant(tenant),
        AuditRetentionPolicy::new(Some(30)).unwrap(),
        now,
    );

    let page = plan
        .purgeable_candidate_page(
            [&third, &held, &first, &second],
            [&hold],
            AuditRetentionBatchLimit::new(Some(2)).unwrap(),
            None,
        )
        .unwrap();
    let next = AuditQueryCursor::parse(page.next_cursor.as_ref().unwrap()).unwrap();

    assert_eq!(page.items.len(), 2);
    assert_eq!(page.items[0].immutable_key(), first.immutable_key());
    assert_eq!(page.items[1].immutable_key(), second.immutable_key());
    assert_eq!(next.event_id, second.immutable_key());
    assert_eq!(next.sort_order, AuditSortOrder::OccurredAtAsc);
}

#[test]
fn audit_retention_plan_resumes_purgeable_candidates_after_cursor() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let now = AuditTimestamp::new(1_800_000_000_000).unwrap();
    let first = AuditEvent::new(
        now.unix_ms() - (33 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let second = AuditEvent::new(
        now.unix_ms() - (32 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let plan = AuditRetentionPlan::new(
        AuditQueryScope::tenant(tenant),
        AuditRetentionPolicy::new(Some(30)).unwrap(),
        now,
    );
    let after = AuditQueryCursor::from_event(&first, AuditSortOrder::OccurredAtAsc).unwrap();

    let page = plan
        .purgeable_candidate_page(
            [&first, &second],
            std::iter::empty::<&AuditRetentionHold>(),
            AuditRetentionBatchLimit::new(Some(10)).unwrap(),
            Some(after),
        )
        .unwrap();

    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].immutable_key(), second.immutable_key());
    assert_eq!(page.next_cursor, None);
}

#[test]
fn audit_retention_plan_purgeable_page_fails_closed_for_hold_scope() {
    let tenant_id = TenantId::new();
    let other_tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let now = AuditTimestamp::new(1_800_000_000_000).unwrap();
    let expired = AuditEvent::new(
        now.unix_ms() - (33 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let bad_hold = AuditRetentionHold::new(
        TenantContext {
            tenant_id: other_tenant_id,
        },
        expired.immutable_key(),
        AuditReasonCode::new("legal_hold.investigation").unwrap(),
        None,
    );
    let plan = AuditRetentionPlan::new(
        AuditQueryScope::tenant(tenant),
        AuditRetentionPolicy::new(Some(30)).unwrap(),
        now,
    );

    assert_eq!(
        plan.purgeable_candidate_page(
            [&expired],
            [&bad_hold],
            AuditRetentionBatchLimit::new(Some(10)).unwrap(),
            None
        ),
        Err(AuditRetentionPageError::Decision(
            AuditRetentionDecisionError::Hold(AuditRetentionHoldError::Scope(
                AuditQueryScopeError::CrossTenantEvent
            ))
        ))
    );
}

#[test]
fn audit_retention_page_error_debug_output_is_stable_and_redacted() {
    let rejected = AuditEvent::MAX_UNIX_MS + 1;
    let error =
        AuditRetentionPageError::Timestamp(AuditTimestampError::TooFarFuture { unix_ms: rejected });

    assert_eq!(
        error.to_string(),
        "audit timestamp is too far in the future"
    );

    let rendered = format!("{error:?}");
    assert!(
        !rendered.contains(&rejected.to_string()),
        "audit retention page error debug output leaked sensitive value: {rendered}"
    );
}

#[test]
fn audit_retention_plan_pages_tenant_scoped_purge_keys() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let now = AuditTimestamp::new(1_800_000_000_000).unwrap();
    let held = AuditEvent::new(
        now.unix_ms() - (34 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let first = AuditEvent::new(
        now.unix_ms() - (33 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let second = AuditEvent::new(
        now.unix_ms() - (32 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let hold = AuditRetentionHold::new(
        tenant,
        held.immutable_key(),
        AuditReasonCode::new("legal_hold.investigation").unwrap(),
        None,
    );
    let plan = AuditRetentionPlan::new(
        AuditQueryScope::tenant(tenant),
        AuditRetentionPolicy::new(Some(30)).unwrap(),
        now,
    );

    let page = plan
        .purgeable_candidate_key_page(
            [&second, &held, &first],
            [&hold],
            AuditRetentionBatchLimit::new(Some(1)).unwrap(),
            None,
        )
        .unwrap();
    let next = AuditQueryCursor::parse(page.next_cursor.as_ref().unwrap()).unwrap();

    assert_eq!(
        page.items,
        vec![AuditRetentionPurgeKey {
            tenant_id,
            event_id: first.immutable_key()
        }]
    );
    assert_eq!(next.event_id, first.immutable_key());
    assert_eq!(next.sort_order, AuditSortOrder::OccurredAtAsc);
}

#[test]
fn audit_retention_purge_key_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let key = AuditRetentionPurgeKey {
        tenant_id,
        event_id: AuditEventId::new(),
    };

    let rendered = format!("{key:?}");
    for sensitive in [&key.tenant_id.to_string(), &key.event_id.to_string()] {
        assert!(
            !rendered.contains(sensitive),
            "audit retention purge key debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
}

#[test]
fn audit_retention_purge_key_rejects_cross_tenant_event() {
    let tenant_id = TenantId::new();
    let other_tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(other_tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let event = AuditEvent::new(
        1_725_000_000_000,
        TenantContext {
            tenant_id: other_tenant_id,
        },
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(other_tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );

    assert_eq!(
        AuditRetentionPurgeKey::from_event(
            AuditQueryScope::tenant(TenantContext { tenant_id }),
            &event
        ),
        Err(AuditRetentionPlanError::Scope(
            AuditQueryScopeError::CrossTenantEvent
        ))
    );
}

#[test]
fn audit_retention_purge_batch_accepts_tenant_scoped_keys() {
    let tenant_id = TenantId::new();
    let tenant = TenantContext { tenant_id };
    let first = AuditRetentionPurgeKey {
        tenant_id,
        event_id: AuditEventId::new(),
    };
    let second = AuditRetentionPurgeKey {
        tenant_id,
        event_id: AuditEventId::new(),
    };

    let batch = AuditRetentionPurgeBatch::new(
        AuditQueryScope::tenant(tenant),
        [first, second],
        AuditRetentionBatchLimit::new(Some(2)).unwrap(),
    )
    .unwrap();

    assert_eq!(batch.tenant_id, tenant_id);
    assert_eq!(batch.len(), 2);
    assert!(!batch.is_empty());
    assert_eq!(
        batch.event_ids().collect::<Vec<_>>(),
        vec![first.event_id, second.event_id]
    );
}

#[test]
fn audit_retention_purge_batch_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let first = AuditRetentionPurgeKey {
        tenant_id,
        event_id: AuditEventId::new(),
    };
    let second = AuditRetentionPurgeKey {
        tenant_id,
        event_id: AuditEventId::new(),
    };
    let batch = AuditRetentionPurgeBatch::new(
        AuditQueryScope::tenant(TenantContext { tenant_id }),
        [first, second],
        AuditRetentionBatchLimit::new(Some(2)).unwrap(),
    )
    .unwrap();

    let rendered = format!("{batch:?}");
    for sensitive in [
        &tenant_id.to_string(),
        &first.event_id.to_string(),
        &second.event_id.to_string(),
    ] {
        assert!(
            !rendered.contains(sensitive),
            "audit retention purge batch debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
}

#[test]
fn audit_retention_purge_batch_rejects_cross_tenant_key() {
    let tenant_id = TenantId::new();
    let other_tenant_id = TenantId::new();
    let tenant = TenantContext { tenant_id };
    let cross_tenant = AuditRetentionPurgeKey {
        tenant_id: other_tenant_id,
        event_id: AuditEventId::new(),
    };

    assert_eq!(
        AuditRetentionPurgeBatch::new(
            AuditQueryScope::tenant(tenant),
            [cross_tenant],
            AuditRetentionBatchLimit::new(Some(10)).unwrap(),
        ),
        Err(AuditRetentionPurgeBatchError::CrossTenantKey)
    );
}

#[test]
fn audit_retention_purge_batch_rejects_oversized_keys() {
    let tenant_id = TenantId::new();
    let tenant = TenantContext { tenant_id };
    let first = AuditRetentionPurgeKey {
        tenant_id,
        event_id: AuditEventId::new(),
    };
    let second = AuditRetentionPurgeKey {
        tenant_id,
        event_id: AuditEventId::new(),
    };

    assert_eq!(
        AuditRetentionPurgeBatch::new(
            AuditQueryScope::tenant(tenant),
            [first, second],
            AuditRetentionBatchLimit::new(Some(1)).unwrap(),
        ),
        Err(AuditRetentionPurgeBatchError::TooManyKeys { count: 2, limit: 1 })
    );
}

#[test]
fn audit_retention_purge_batch_error_debug_output_is_stable_and_redacted() {
    let error = AuditRetentionPurgeBatchError::TooManyKeys { count: 2, limit: 1 };

    assert_eq!(error.to_string(), "audit retention purge batch is invalid");

    let rendered = format!("{error:?}");
    for sensitive in ["2", "1"] {
        assert!(
            !rendered.contains(sensitive),
            "audit retention purge batch error debug output leaked sensitive value {sensitive}: {rendered}"
        );
    }
}

#[test]
fn audit_retention_plan_binds_purgeable_candidates_to_hold_scope() {
    let tenant_id = TenantId::new();
    let other_tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let now = AuditTimestamp::new(1_800_000_000_000).unwrap();
    let expired = AuditEvent::new(
        now.unix_ms() - (33 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let bad_hold = AuditRetentionHold::new(
        TenantContext {
            tenant_id: other_tenant_id,
        },
        expired.immutable_key(),
        AuditReasonCode::new("legal_hold.investigation").unwrap(),
        None,
    );
    let plan = AuditRetentionPlan::new(
        AuditQueryScope::tenant(tenant),
        AuditRetentionPolicy::new(Some(30)).unwrap(),
        now,
    );

    assert_eq!(
        plan.purgeable_candidates(
            [&expired],
            [&bad_hold],
            AuditRetentionBatchLimit::new(Some(10)).unwrap()
        ),
        Err(AuditRetentionDecisionError::Hold(
            AuditRetentionHoldError::Scope(AuditQueryScopeError::CrossTenantEvent)
        ))
    );
}

#[test]
fn audit_retention_decision_fails_closed_for_invalid_hold_scope() {
    let tenant_id = TenantId::new();
    let other_tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let now = AuditTimestamp::new(1_800_000_000_000).unwrap();
    let expired = AuditEvent::new(
        now.unix_ms() - (31 * 86_400_000),
        tenant,
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let cross_tenant_hold = AuditRetentionHold::new(
        TenantContext {
            tenant_id: other_tenant_id,
        },
        expired.immutable_key(),
        AuditReasonCode::new("legal_hold.investigation").unwrap(),
        None,
    );
    let plan = AuditRetentionPlan::new(
        AuditQueryScope::tenant(tenant),
        AuditRetentionPolicy::new(Some(30)).unwrap(),
        now,
    );

    assert_eq!(
        plan.purge_decision(&expired, [&cross_tenant_hold]),
        Err(AuditRetentionDecisionError::Hold(
            AuditRetentionHoldError::Scope(AuditQueryScopeError::CrossTenantEvent)
        ))
    );
}

#[test]
fn audit_retention_plan_fails_closed_for_cross_tenant_or_invalid_timestamp_events() {
    let query_tenant_id = TenantId::new();
    let event_tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(query_tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let cross_tenant = AuditEvent::new(
        1_725_000_000_000,
        TenantContext {
            tenant_id: event_tenant_id,
        },
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(event_tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let invalid_timestamp = AuditEvent::new(
        0,
        TenantContext {
            tenant_id: query_tenant_id,
        },
        &principal,
        AuditAction::new("audit.retention"),
        AuditResource::new("audit_log", None::<String>, Some(query_tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let plan = AuditRetentionPlan::new(
        AuditQueryScope::tenant(TenantContext {
            tenant_id: query_tenant_id,
        }),
        AuditRetentionPolicy::new(Some(30)).unwrap(),
        AuditTimestamp::new(1_800_000_000_000).unwrap(),
    );

    assert_eq!(
        plan.is_event_expired(&cross_tenant),
        Err(AuditRetentionPlanError::Scope(
            AuditQueryScopeError::CrossTenantEvent
        ))
    );
    assert_eq!(
        plan.is_event_expired(&invalid_timestamp),
        Err(AuditRetentionPlanError::Timestamp(
            AuditTimestampError::Zero
        ))
    );
}

#[test]
fn audit_retention_plan_error_debug_output_is_stable_and_redacted() {
    let rejected = AuditEvent::MAX_UNIX_MS + 1;
    let error =
        AuditRetentionPlanError::Timestamp(AuditTimestampError::TooFarFuture { unix_ms: rejected });

    assert_eq!(
        error.to_string(),
        "audit timestamp is too far in the future"
    );

    let rendered = format!("{error:?}");
    assert!(
        !rendered.contains(&rejected.to_string()),
        "audit retention plan error debug output leaked sensitive value: {rendered}"
    );
}

#[test]
fn audit_event_can_use_validated_timestamp() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::User,
        Role::Admin,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let timestamp = AuditTimestamp::new(1_725_000_000_000).unwrap();

    let event = AuditEvent::new_at(
        timestamp,
        tenant,
        &principal,
        AuditAction::new("tenant.update"),
        AuditResource::new("tenant", Some(tenant_id.to_string()), Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );

    assert_eq!(event.occurred_at_unix_ms, 1_725_000_000_000);
}

#[test]
fn audit_action_try_new_accepts_stable_machine_readable_actions() {
    let action = AuditAction::try_new("tenant.policy_activate_v2").unwrap();

    assert_eq!(action.as_str(), "tenant.policy_activate_v2");
    assert_eq!(action.validate(), Ok(()));
}

#[test]
fn audit_action_debug_output_is_stable_and_redacted() {
    let action = AuditAction::try_new("tenant.policy_activate_v2").unwrap();

    assert_eq!(action.as_str(), "tenant.policy_activate_v2");

    let rendered = format!("{action:?}");
    assert!(
        !rendered.contains("tenant.policy_activate_v2"),
        "audit action debug output leaked sensitive value: {rendered}"
    );
}

#[test]
fn audit_action_try_new_rejects_unstable_or_path_like_actions() {
    assert_eq!(AuditAction::try_new(""), Err(AuditActionError::Empty));
    assert_eq!(
        AuditAction::try_new(" "),
        Err(AuditActionError::InvalidCharacter)
    );
    assert_eq!(
        AuditAction::try_new("tenant..policy_activate"),
        Err(AuditActionError::EmptySegment)
    );
    assert_eq!(
        AuditAction::try_new("Tenant.PolicyActivate"),
        Err(AuditActionError::InvalidCharacter)
    );
    assert_eq!(
        AuditAction::try_new("tenant/policy-activate"),
        Err(AuditActionError::InvalidCharacter)
    );
    assert_eq!(
        AuditAction::try_new(" tenant.policy_activate"),
        Err(AuditActionError::InvalidCharacter)
    );
    assert_eq!(
        AuditAction::try_new("a".repeat(129)),
        Err(AuditActionError::TooLong { length: 129 })
    );
}

#[test]
fn audit_action_error_debug_output_is_stable_and_redacted() {
    let error = AuditActionError::TooLong { length: 129 };

    assert_eq!(
        AuditActionError::Empty.to_string(),
        "audit action is invalid"
    );
    assert_eq!(error.to_string(), "audit action is invalid");
    assert!(!error.to_string().contains("too long"));

    let rendered = format!("{error:?}");
    assert!(
        !rendered.contains("129"),
        "audit action error debug output leaked sensitive length: {rendered}"
    );
}

#[test]
fn audit_resource_try_new_accepts_stable_machine_readable_kinds() {
    let tenant_id = TenantId::new();
    let resource = AuditResource::try_new(
        "control_plane.virtual_key",
        Some("vk-redacted"),
        Some(tenant_id),
    )
    .unwrap();

    assert_eq!(resource.kind, "control_plane.virtual_key");
    assert_eq!(resource.id.as_deref(), Some("vk-redacted"));
    assert_eq!(resource.tenant_id, Some(tenant_id));
    assert_eq!(resource.validate_kind(), Ok(()));
}

#[test]
fn audit_resource_try_new_rejects_unstable_or_path_like_kinds() {
    assert_eq!(
        AuditResource::try_new("", None::<String>, None),
        Err(AuditResourceKindError::Empty)
    );
    assert_eq!(
        AuditResource::try_new(" ", None::<String>, None),
        Err(AuditResourceKindError::InvalidCharacter)
    );
    assert_eq!(
        AuditResource::try_new("control_plane..virtual_key", None::<String>, None),
        Err(AuditResourceKindError::EmptySegment)
    );
    assert_eq!(
        AuditResource::try_new("ControlPlane.VirtualKey", None::<String>, None),
        Err(AuditResourceKindError::InvalidCharacter)
    );
    assert_eq!(
        AuditResource::try_new("control-plane/virtual-key", None::<String>, None),
        Err(AuditResourceKindError::InvalidCharacter)
    );
    assert_eq!(
        AuditResource::try_new(" control_plane.virtual_key", None::<String>, None),
        Err(AuditResourceKindError::InvalidCharacter)
    );
    assert_eq!(
        AuditResource::try_new("a".repeat(97), None::<String>, None),
        Err(AuditResourceKindError::TooLong { length: 97 })
    );
}

#[test]
fn audit_resource_kind_error_debug_output_is_stable_and_redacted() {
    let error = AuditResourceKindError::TooLong { length: 97 };

    assert_eq!(
        AuditResourceKindError::Empty.to_string(),
        "audit resource kind is invalid"
    );
    assert_eq!(error.to_string(), "audit resource kind is invalid");
    assert!(!error.to_string().contains("too long"));

    let rendered = format!("{error:?}");
    assert!(
        !rendered.contains("97"),
        "audit resource kind error debug output leaked sensitive length: {rendered}"
    );
}

#[test]
fn audit_resource_new_replaces_unvalidated_raw_kind() {
    let tenant_id = TenantId::new();
    let resource = AuditResource::new(
        "Authorization: Bearer secret-token",
        Some("virtual_key:01hxy3-redacted"),
        Some(tenant_id),
    );

    assert_eq!(resource.kind, "audit.invalid_resource");
    assert_eq!(resource.id.as_deref(), Some("virtual_key:01hxy3-redacted"));
    assert_eq!(resource.tenant_id, Some(tenant_id));
}

#[test]
fn audit_resource_id_new_accepts_stable_machine_readable_values() {
    let id = AuditResourceId::new("virtual_key:01hxy3-redacted").unwrap();

    assert_eq!(id.as_str(), "virtual_key:01hxy3-redacted");
}

#[test]
fn audit_resource_id_new_rejects_unstable_or_secret_like_values() {
    assert_eq!(AuditResourceId::new(""), Err(AuditResourceIdError::Empty));
    assert_eq!(
        AuditResourceId::new(" "),
        Err(AuditResourceIdError::InvalidCharacter)
    );
    assert_eq!(
        AuditResourceId::new("a".repeat(257)),
        Err(AuditResourceIdError::TooLong { length: 257 })
    );
    assert_eq!(
        AuditResourceId::new("VirtualKey/SecretToken"),
        Err(AuditResourceIdError::InvalidCharacter)
    );
    assert_eq!(
        AuditResourceId::new("virtual key secret token"),
        Err(AuditResourceIdError::InvalidCharacter)
    );
    assert_eq!(
        AuditResourceId::new(" virtual_key:01hxy3-redacted"),
        Err(AuditResourceIdError::InvalidCharacter)
    );
}

#[test]
fn audit_resource_id_error_debug_output_is_stable_and_redacted() {
    let error = AuditResourceIdError::TooLong { length: 257 };

    assert_eq!(
        AuditResourceIdError::Empty.to_string(),
        "audit resource id is invalid"
    );
    assert_eq!(error.to_string(), "audit resource id is invalid");
    assert!(!error.to_string().contains("too long"));

    let rendered = format!("{error:?}");
    assert!(
        !rendered.contains("257"),
        "audit resource id error debug output leaked sensitive length: {rendered}"
    );
}

#[test]
fn audit_resource_can_use_validated_resource_id() {
    let tenant_id = TenantId::new();
    let id = AuditResourceId::new("virtual_key:01hxy3-redacted").unwrap();
    let resource =
        AuditResource::new_with_resource_id("virtual_key", Some(id), Some(tenant_id)).unwrap();

    assert_eq!(resource.kind, "virtual_key");
    assert_eq!(resource.id.as_deref(), Some("virtual_key:01hxy3-redacted"));
    assert_eq!(resource.tenant_id, Some(tenant_id));
}

#[test]
fn audit_resource_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let id = AuditResourceId::new("virtual_key:01hxy3-redacted").unwrap();
    let resource =
        AuditResource::new_with_resource_id("virtual_key", Some(id.clone()), Some(tenant_id))
            .unwrap();

    assert_eq!(resource.kind, "virtual_key");
    assert_eq!(resource.id.as_deref(), Some("virtual_key:01hxy3-redacted"));
    assert_eq!(resource.tenant_id, Some(tenant_id));

    let rendered = format!("{id:?} {resource:?}");
    for sensitive in [&tenant_id.to_string(), "virtual_key:01hxy3-redacted"] {
        assert!(
            !rendered.contains(sensitive),
            "audit resource debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
}

#[test]
fn audit_event_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let principal_id = PrincipalId::new();
    let principal = Principal::new(
        principal_id,
        Some(tenant_id),
        PrincipalKind::User,
        Role::Admin,
        CredentialScope::ControlPlane,
    );
    let event = AuditEvent::new_with_reason_code(
        1_725_000_000_100,
        TenantContext { tenant_id },
        &principal,
        AuditAction::try_new("policy.activate").unwrap(),
        AuditResource::new(
            "virtual_key",
            Some("virtual_key:01hxy3-redacted"),
            Some(tenant_id),
        ),
        AuditOutcome::Denied,
        Some(AuditReasonCode::new("authorization.insufficient_role").unwrap()),
    );

    assert_eq!(event.tenant_id, tenant_id);
    assert_eq!(event.principal_id, principal_id);
    assert_eq!(
        event.resource.id.as_deref(),
        Some("virtual_key:01hxy3-redacted")
    );
    assert_eq!(
        event.reason_code.as_deref(),
        Some("authorization.insufficient_role")
    );

    let rendered = format!("{event:?}");
    for sensitive in [
        &event.id.to_string(),
        &tenant_id.to_string(),
        &principal_id.to_string(),
        "1725000000100",
        "virtual_key:01hxy3-redacted",
        "authorization.insufficient_role",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "audit event debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
}

#[test]
fn audit_resource_drops_unvalidated_raw_resource_id() {
    let tenant_id = TenantId::new();
    let resource = AuditResource::new(
        "virtual_key",
        Some("Authorization: Bearer secret-token"),
        Some(tenant_id),
    );

    assert_eq!(resource.kind, "virtual_key");
    assert_eq!(resource.id, None);
    assert_eq!(resource.tenant_id, Some(tenant_id));
}

#[test]
fn audit_event_replaces_unvalidated_raw_action() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };

    let event = AuditEvent::new(
        5,
        tenant,
        &principal,
        AuditAction::new("Authorization: Bearer secret-token"),
        AuditResource::new("policy", None::<String>, Some(tenant_id)),
        AuditOutcome::Denied,
        None::<String>,
    );

    assert_eq!(event.action.as_str(), "audit.invalid_action");
}

#[test]
fn audit_reason_code_new_accepts_stable_machine_readable_values() {
    let reason = AuditReasonCode::new("authorization.insufficient_role").unwrap();

    assert_eq!(reason.as_str(), "authorization.insufficient_role");
}

#[test]
fn audit_reason_code_debug_output_is_stable_and_redacted() {
    let reason = AuditReasonCode::new("authorization.insufficient_role").unwrap();

    assert_eq!(reason.as_str(), "authorization.insufficient_role");

    let rendered = format!("{reason:?}");
    assert!(
        !rendered.contains("authorization.insufficient_role"),
        "audit reason code debug output leaked sensitive value: {rendered}"
    );
}

#[test]
fn audit_reason_code_new_rejects_unstable_or_path_like_values() {
    assert_eq!(AuditReasonCode::new(""), Err(AuditReasonCodeError::Empty));
    assert_eq!(
        AuditReasonCode::new(" "),
        Err(AuditReasonCodeError::InvalidCharacter)
    );
    assert_eq!(
        AuditReasonCode::new("authorization..insufficient_role"),
        Err(AuditReasonCodeError::EmptySegment)
    );
    assert_eq!(
        AuditReasonCode::new("Authorization.InsufficientRole"),
        Err(AuditReasonCodeError::InvalidCharacter)
    );
    assert_eq!(
        AuditReasonCode::new("authorization/insufficient-role"),
        Err(AuditReasonCodeError::InvalidCharacter)
    );
    assert_eq!(
        AuditReasonCode::new(" authorization.insufficient_role"),
        Err(AuditReasonCodeError::InvalidCharacter)
    );
    assert_eq!(
        AuditReasonCode::new("a".repeat(129)),
        Err(AuditReasonCodeError::TooLong { length: 129 })
    );
}

#[test]
fn audit_reason_code_error_debug_output_is_stable_and_redacted() {
    let error = AuditReasonCodeError::TooLong { length: 129 };

    assert_eq!(error.to_string(), "audit reason code is invalid");

    let rendered = format!("{error:?}");
    assert!(
        !rendered.contains("129"),
        "audit reason code error debug output leaked sensitive length: {rendered}"
    );
}

#[test]
fn audit_event_can_use_validated_reason_code() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let reason = AuditReasonCode::new("authorization.insufficient_role").unwrap();

    let event = AuditEvent::new_with_reason_code(
        5,
        tenant,
        &principal,
        AuditAction::new("policy.activate"),
        AuditResource::new("policy", None::<String>, Some(tenant_id)),
        AuditOutcome::Denied,
        Some(reason),
    );

    assert_eq!(
        event.reason_code.as_deref(),
        Some("authorization.insufficient_role")
    );
}

#[test]
fn audit_event_drops_unvalidated_raw_reason_code() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };

    let event = AuditEvent::new(
        5,
        tenant,
        &principal,
        AuditAction::new("policy.activate"),
        AuditResource::new("policy", None::<String>, Some(tenant_id)),
        AuditOutcome::Denied,
        Some("Authorization: Bearer secret-token"),
    );

    assert_eq!(event.reason_code, None);
}

#[test]
fn audit_envelope_verifies_append_only_chain_link() {
    let tenant_id = TenantId::new();
    let principal = Principal::new(
        PrincipalId::new(),
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let tenant = TenantContext { tenant_id };
    let event = AuditEvent::new(
        4,
        tenant,
        &principal,
        AuditAction::new("key.rotate"),
        AuditResource::new("virtual_key", Some("vk-redacted"), Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let previous = AuditDigest::new("sha256:previous").unwrap();
    let current = AuditDigest::new("sha256:current").unwrap();
    let envelope = AuditEnvelope::new(event, Some(previous.clone()), current);

    assert_eq!(envelope.verify_chain_link(Some(&previous)), Ok(()));
    assert_eq!(
        envelope.verify_chain_link(Some(&AuditDigest::new("sha256:other").unwrap())),
        Err(AuditChainError::PreviousDigestMismatch)
    );
    assert_eq!(
        envelope.verify_chain_link(None),
        Err(AuditChainError::PreviousDigestMismatch)
    );
}

#[test]
fn audit_chain_error_debug_output_is_stable_and_redacted() {
    let error = AuditChainError::PreviousDigestMismatch;

    assert_eq!(format!("{error:?}"), "PreviousDigestMismatch");
}

#[test]
fn audit_envelope_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let principal_id = PrincipalId::new();
    let principal = Principal::new(
        principal_id,
        Some(tenant_id),
        PrincipalKind::ServiceAccount,
        Role::Operator,
        CredentialScope::ControlPlane,
    );
    let event = AuditEvent::new(
        4,
        TenantContext { tenant_id },
        &principal,
        AuditAction::new("key.rotate"),
        AuditResource::new("virtual_key", Some("vk-redacted"), Some(tenant_id)),
        AuditOutcome::Success,
        None::<String>,
    );
    let previous = AuditDigest::new("sha256:previous").unwrap();
    let current = AuditDigest::new("sha256:current").unwrap();
    let envelope = AuditEnvelope::new(event.clone(), Some(previous.clone()), current.clone());

    assert_eq!(previous.as_str(), "sha256:previous");
    assert_eq!(current.as_str(), "sha256:current");
    assert_eq!(envelope.event.id, event.id);

    let rendered = format!("{previous:?} {current:?} {envelope:?}");
    for sensitive in [
        &event.id.to_string(),
        &tenant_id.to_string(),
        &principal_id.to_string(),
        "sha256:previous",
        "sha256:current",
        "vk-redacted",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "audit envelope debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
}

#[test]
fn audit_error_responses_are_stable_and_redacted() {
    let action_error = AuditActionError::TooLong { length: 1_000 };
    assert!(!action_error.to_string().contains("1000"));
    let action = plan_audit_action_error_response(&action_error);
    assert_eq!(action.status, AuditErrorStatus::InvalidRequest);
    assert_eq!(action.code, "audit_action_invalid");
    assert_eq!(action.message, "audit action is invalid");

    let resource_kind_error = AuditResourceKindError::TooLong { length: 2_000 };
    assert!(!resource_kind_error.to_string().contains("2000"));
    let resource_kind = plan_audit_resource_kind_error_response(&resource_kind_error);
    assert_eq!(resource_kind.status, AuditErrorStatus::InvalidRequest);
    assert_eq!(resource_kind.code, "audit_resource_kind_invalid");
    assert_eq!(resource_kind.message, "audit resource kind is invalid");

    let resource_id_error = AuditResourceIdError::TooLong { length: 4_000 };
    assert!(!resource_id_error.to_string().contains("4000"));
    let resource_id = plan_audit_resource_id_error_response(&resource_id_error);
    assert_eq!(resource_id.status, AuditErrorStatus::InvalidRequest);
    assert_eq!(resource_id.code, "audit_resource_id_invalid");
    assert_eq!(resource_id.message, "audit resource id is invalid");

    let reason_code_error = AuditReasonCodeError::TooLong { length: 3_000 };
    assert!(!reason_code_error.to_string().contains("3000"));
    let reason_code = plan_audit_reason_code_error_response(&reason_code_error);
    assert_eq!(reason_code.status, AuditErrorStatus::InvalidRequest);
    assert_eq!(reason_code.code, "audit_reason_code_invalid");
    assert_eq!(reason_code.message, "audit reason code is invalid");

    let digest = plan_audit_digest_error_response(&AuditDigestError::Empty);
    assert_eq!(digest.status, AuditErrorStatus::InvalidRequest);
    assert_eq!(digest.code, "audit_digest_required");
    assert_eq!(digest.message, "audit digest is invalid");

    let invalid_digest =
        plan_audit_digest_error_response(&AuditDigestError::TooLong { length: 5_000 });
    assert_eq!(invalid_digest.status, AuditErrorStatus::InvalidRequest);
    assert_eq!(invalid_digest.code, "audit_digest_invalid");
    assert_eq!(invalid_digest.message, "audit digest is invalid");

    let timestamp_error = AuditTimestampError::TooFarFuture {
        unix_ms: AuditEvent::MAX_UNIX_MS + 10,
    };
    assert_eq!(timestamp_error.to_string(), "audit timestamp is invalid");
    assert!(!timestamp_error.to_string().contains("32503680000010"));
    let timestamp = plan_audit_timestamp_error_response(&timestamp_error);
    assert_eq!(timestamp.status, AuditErrorStatus::InvalidRequest);
    assert_eq!(timestamp.code, "audit_timestamp_invalid");
    assert_eq!(timestamp.message, "audit timestamp is invalid");

    let time_range = plan_audit_time_range_error_response(&AuditTimeRangeError::StartAfterEnd);
    assert_eq!(time_range.status, AuditErrorStatus::InvalidRequest);
    assert_eq!(time_range.code, "audit_time_range_invalid");
    assert_eq!(time_range.message, "audit time range is invalid");

    let page_limit =
        plan_audit_page_limit_error_response(&AuditPageLimitError::TooLarge { value: 65_535 });
    assert_eq!(
        AuditPageLimitError::TooLarge { value: 65_535 }.to_string(),
        "audit page limit is invalid"
    );
    assert!(
        !AuditPageLimitError::TooLarge { value: 65_535 }
            .to_string()
            .contains("65535")
    );
    assert_eq!(page_limit.status, AuditErrorStatus::InvalidRequest);
    assert_eq!(page_limit.code, "audit_page_limit_invalid");
    assert_eq!(page_limit.message, "audit page limit is invalid");

    let retention_policy_error = AuditRetentionPolicyError::TooLong { days: 65_535 };
    assert!(!retention_policy_error.to_string().contains("65535"));
    let retention_policy = plan_audit_retention_policy_error_response(&retention_policy_error);
    assert_eq!(retention_policy.status, AuditErrorStatus::InvalidRequest);
    assert_eq!(retention_policy.code, "audit_retention_policy_invalid");
    assert_eq!(
        retention_policy.message,
        "audit retention policy is invalid"
    );

    let retention_batch_error = AuditRetentionBatchLimitError::TooLarge { value: 65_535 };
    assert_eq!(
        retention_batch_error.to_string(),
        "audit retention batch limit is too large"
    );
    assert!(!retention_batch_error.to_string().contains("65535"));
    let retention_batch = plan_audit_retention_batch_limit_error_response(&retention_batch_error);
    assert_eq!(retention_batch.status, AuditErrorStatus::InvalidRequest);
    assert_eq!(retention_batch.code, "audit_retention_batch_limit_invalid");
    assert_eq!(
        retention_batch.message,
        "audit retention batch limit is invalid"
    );

    let retention_plan = plan_audit_retention_plan_error_response(
        &AuditRetentionPlanError::Timestamp(AuditTimestampError::TooFarFuture {
            unix_ms: AuditEvent::MAX_UNIX_MS + 12,
        }),
    );
    assert_eq!(retention_plan.status, AuditErrorStatus::InvalidRequest);
    assert_eq!(retention_plan.code, "audit_timestamp_invalid");
    assert_eq!(retention_plan.message, "audit timestamp is invalid");

    let retention_page =
        plan_audit_retention_page_error_response(&AuditRetentionPageError::CursorSortOrderMismatch);
    assert_eq!(
        AuditRetentionPageError::CursorSortOrderMismatch.to_string(),
        "audit query cursor is invalid"
    );
    assert_eq!(retention_page.status, AuditErrorStatus::InvalidRequest);
    assert_eq!(retention_page.code, "audit_query_cursor_invalid");
    assert_eq!(retention_page.message, "audit query cursor is invalid");

    let retention_page_decision = plan_audit_retention_page_error_response(
        &AuditRetentionPageError::Decision(AuditRetentionDecisionError::Hold(
            AuditRetentionHoldError::Scope(AuditQueryScopeError::CrossTenantEvent),
        )),
    );
    assert_eq!(retention_page_decision.status, AuditErrorStatus::Forbidden);
    assert_eq!(retention_page_decision.code, "audit_query_scope_denied");
    assert_eq!(
        retention_page_decision.message,
        "audit query scope is invalid"
    );

    let retention_purge_batch = plan_audit_retention_purge_batch_error_response(
        &AuditRetentionPurgeBatchError::CrossTenantKey,
    );
    assert_eq!(retention_purge_batch.status, AuditErrorStatus::Forbidden);
    assert_eq!(
        retention_purge_batch.code,
        "audit_retention_purge_batch_scope_denied"
    );
    assert_eq!(
        retention_purge_batch.message,
        "audit retention purge batch is invalid"
    );

    let retention_hold = plan_audit_retention_hold_error_response(
        &AuditRetentionHoldError::Timestamp(AuditTimestampError::TooFarFuture {
            unix_ms: AuditEvent::MAX_UNIX_MS + 13,
        }),
    );
    assert_eq!(retention_hold.status, AuditErrorStatus::InvalidRequest);
    assert_eq!(retention_hold.code, "audit_timestamp_invalid");
    assert_eq!(retention_hold.message, "audit timestamp is invalid");

    let retention_decision =
        plan_audit_retention_decision_error_response(&AuditRetentionDecisionError::Retention(
            AuditRetentionPlanError::Timestamp(AuditTimestampError::TooFarFuture {
                unix_ms: AuditEvent::MAX_UNIX_MS + 14,
            }),
        ));
    assert_eq!(retention_decision.status, AuditErrorStatus::InvalidRequest);
    assert_eq!(retention_decision.code, "audit_timestamp_invalid");
    assert_eq!(retention_decision.message, "audit timestamp is invalid");

    let query_scope =
        plan_audit_query_scope_error_response(&AuditQueryScopeError::CrossTenantEvent);
    assert_eq!(query_scope.status, AuditErrorStatus::Forbidden);
    assert_eq!(query_scope.code, "audit_query_scope_denied");
    assert_eq!(query_scope.message, "audit query scope is invalid");

    let query_plan = plan_audit_query_plan_error_response(&AuditQueryPlanError::Timestamp(
        AuditTimestampError::TooFarFuture {
            unix_ms: AuditEvent::MAX_UNIX_MS + 11,
        },
    ));
    assert_eq!(query_plan.status, AuditErrorStatus::InvalidRequest);
    assert_eq!(query_plan.code, "audit_timestamp_invalid");
    assert_eq!(query_plan.message, "audit timestamp is invalid");

    let query_plan_cursor =
        plan_audit_query_plan_error_response(&AuditQueryPlanError::CursorSortOrderMismatch);
    assert_eq!(
        AuditQueryPlanError::CursorSortOrderMismatch.to_string(),
        "audit query cursor is invalid"
    );
    assert_eq!(query_plan_cursor.status, AuditErrorStatus::InvalidRequest);
    assert_eq!(query_plan_cursor.code, "audit_query_cursor_invalid");
    assert_eq!(query_plan_cursor.message, "audit query cursor is invalid");

    let query_page = plan_audit_query_page_error_response(&AuditQueryPageError::Query(
        AuditQueryPlanError::CursorSortOrderMismatch,
    ));
    assert_eq!(query_page.status, AuditErrorStatus::InvalidRequest);
    assert_eq!(query_page.code, "audit_query_cursor_invalid");
    assert_eq!(query_page.message, "audit query cursor is invalid");

    let query_cursor =
        plan_audit_query_cursor_error_response(&AuditQueryCursorError::UnsupportedVersion);
    assert_eq!(
        AuditQueryCursorError::UnsupportedVersion.to_string(),
        "audit query cursor is invalid"
    );
    assert_eq!(query_cursor.status, AuditErrorStatus::InvalidRequest);
    assert_eq!(query_cursor.code, "audit_query_cursor_invalid");
    assert_eq!(query_cursor.message, "audit query cursor is invalid");

    let outcome = plan_audit_outcome_error_response(&AuditOutcomeError::Unknown);
    assert_eq!(outcome.status, AuditErrorStatus::InvalidRequest);
    assert_eq!(outcome.code, "audit_outcome_invalid");
    assert_eq!(outcome.message, "audit outcome is invalid");

    let export_format = plan_audit_export_format_error_response(&AuditExportFormatError::Unknown);
    assert_eq!(export_format.status, AuditErrorStatus::InvalidRequest);
    assert_eq!(export_format.code, "audit_export_format_invalid");
    assert_eq!(export_format.message, "audit export format is invalid");

    let sort_order = plan_audit_sort_order_error_response(&AuditSortOrderError::Unknown);
    assert_eq!(sort_order.status, AuditErrorStatus::InvalidRequest);
    assert_eq!(sort_order.code, "audit_sort_order_invalid");
    assert_eq!(sort_order.message, "audit sort order is invalid");

    let chain = plan_audit_chain_error_response(&AuditChainError::PreviousDigestMismatch);
    assert_eq!(chain.status, AuditErrorStatus::Conflict);
    assert_eq!(chain.code, "audit_chain_conflict");
    assert_eq!(chain.message, "audit chain verification failed");

    let rendered = format!(
        "{action:?} {resource_kind:?} {resource_id:?} {reason_code:?} {digest:?} {invalid_digest:?} {timestamp:?} {time_range:?} {page_limit:?} {retention_policy:?} {retention_batch:?} {retention_plan:?} {retention_page:?} {retention_hold:?} {retention_decision:?} {query_scope:?} {query_plan:?} {query_plan_cursor:?} {query_page:?} {query_cursor:?} {outcome:?} {export_format:?} {sort_order:?} {chain:?}"
    );
    for sensitive in [
        "1000",
        "2000",
        "3000",
        "4000",
        "5000",
        "65535",
        "4102444800010",
        "4102444800012",
        "4102444800013",
        "4102444800014",
        "1725000000000",
        "tenant.policy_activate_v2",
        "Tenant.PolicyActivate",
        "tenant/policy-activate",
        "control_plane.virtual_key",
        "ControlPlane.VirtualKey",
        "control-plane/virtual-key",
        "virtual_key:01hxy3-redacted",
        "VirtualKey/SecretToken",
        "virtual key secret token",
        "authorization.insufficient_role",
        "Authorization.InsufficientRole",
        "authorization/insufficient-role",
        "sha256:previous",
        "sha256:current",
        "sha256:Secret/Token",
        "SUCCESS",
        "JSON",
        "created_at_desc",
        "audit:v1:occurred_at_desc:1725000000100:not-a-uuid",
        "not-a-uuid",
        "tenant.update",
        "audit.query",
        "audit_log",
        "policy.activate",
        "virtual_key",
        "Bearer",
        "secret-token",
        "principal",
        "tenant_id",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "audit response leaked sensitive token {sensitive}: {rendered}"
        );
    }
}
