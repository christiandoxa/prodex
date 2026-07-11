use prodex_config::{
    ConfigCacheState, ConfigCacheWindow, ConfigCacheWindowError, ConfigCacheWindowErrorStatus,
    ConfigInvalidationError, ConfigInvalidationErrorStatus, ConfigPublicationError,
    ConfigPublicationErrorStatus, ConfigPublicationEventDelivery, ConfigPublicationEventError,
    ConfigPublicationEventErrorStatus, ConfigPublicationEventTarget, ConfigRefreshDecision,
    ConfigRefreshError, ConfigRefreshErrorStatus, ConfigRevision, ConfigSecretReferenceError,
    ConfigSecretReferenceErrorStatus, ConfigSecretSource, config_refresh_error_for_decision,
    evaluate_config_refresh, plan_config_activation, plan_config_cache_window_error_response,
    plan_config_invalidation, plan_config_invalidation_error_response,
    plan_config_publication_error_response, plan_config_publication_event,
    plan_config_publication_event_error_response, plan_config_refresh_error_response,
    plan_config_secret_reference, plan_config_secret_reference_error_response,
    validate_config_publication,
};
use prodex_domain::{PolicyRevisionId, SecretPurpose, SecretRef, TenantId};

fn window() -> ConfigCacheWindow {
    ConfigCacheWindow::new(1_000, 2_000, 3_000).unwrap()
}

#[test]
fn cache_window_rejects_invalid_ordering() {
    assert_eq!(
        ConfigCacheWindow::new(2_001, 2_000, 3_000),
        Err(ConfigCacheWindowError::RefreshAfterStale)
    );
    assert_eq!(
        ConfigCacheWindow::new(1_000, 3_001, 3_000),
        Err(ConfigCacheWindowError::StaleAfterExpiry)
    );
}

#[test]
fn cache_window_error_response_is_stable_and_redacted() {
    let refresh_error = ConfigCacheWindowError::RefreshAfterStale;
    assert_eq!(
        refresh_error.to_string(),
        "configuration cache window is invalid"
    );
    assert!(!refresh_error.to_string().contains("refresh"));
    assert!(!refresh_error.to_string().contains("stale"));
    let refresh_response = plan_config_cache_window_error_response(&refresh_error);

    assert_eq!(
        refresh_response.status,
        ConfigCacheWindowErrorStatus::InvalidConfiguration
    );
    assert_eq!(refresh_response.code, "configuration_cache_window_invalid");
    assert_eq!(
        refresh_response.message,
        "configuration cache window is invalid"
    );
    assert!(!refresh_response.message.contains("refresh"));
    assert!(!refresh_response.message.contains("stale"));

    let stale_error = ConfigCacheWindowError::StaleAfterExpiry;
    assert_eq!(
        stale_error.to_string(),
        "configuration cache window is invalid"
    );
    assert!(!stale_error.to_string().contains("stale"));
    assert!(!stale_error.to_string().contains("expiry"));
    let stale_response = plan_config_cache_window_error_response(&stale_error);

    assert_eq!(stale_response, refresh_response);
    assert!(!stale_response.message.contains("expiry"));
}

#[test]
fn cache_window_debug_output_is_stable_and_redacted() {
    let window = ConfigCacheWindow::new(1_000, 2_000, 3_000).unwrap();

    let rendered = format!("{window:?}");
    for sensitive in ["1000", "2000", "3000"] {
        assert!(
            !rendered.contains(sensitive),
            "config cache window debug output leaked timing {sensitive}: {rendered}"
        );
    }
}

#[test]
fn config_revision_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let revision_id = PolicyRevisionId::new();
    let revision = ConfigRevision {
        tenant_id,
        revision_id,
        published_at_unix_ms: 1_234,
        payload: "raw-config-payload",
    };

    let rendered = format!("{revision:?}");
    for sensitive in [
        tenant_id.to_string(),
        revision_id.to_string(),
        "1234".to_string(),
        "raw-config-payload".to_string(),
    ] {
        assert!(
            !rendered.contains(&sensitive),
            "config revision debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
}

#[test]
fn config_cache_state_debug_output_is_stable_and_redacted() {
    let tenant_id = TenantId::new();
    let active_revision_id = PolicyRevisionId::new();
    let lkg_revision_id = PolicyRevisionId::new();
    let invalidated_revision_id = PolicyRevisionId::new();
    let state = ConfigCacheState {
        tenant_id,
        active_revision_id: Some(active_revision_id),
        last_known_good_revision_id: Some(lkg_revision_id),
        invalidated_revision_id: Some(invalidated_revision_id),
        window: window(),
    };

    let rendered = format!("{state:?}");
    for sensitive in [
        tenant_id.to_string(),
        active_revision_id.to_string(),
        lkg_revision_id.to_string(),
        invalidated_revision_id.to_string(),
        "1000".to_string(),
        "2000".to_string(),
        "3000".to_string(),
    ] {
        assert!(
            !rendered.contains(&sensitive),
            "config cache state debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
}

#[test]
fn config_secret_reference_accepts_secret_ref_without_raw_material() {
    let tenant_id = TenantId::new();
    let reference = SecretRef::new("external-secrets", "providers/openai", Some("v3"));

    let plan = plan_config_secret_reference(
        tenant_id,
        SecretPurpose::ProviderCredential,
        ConfigSecretSource::Reference(reference.clone()),
    )
    .unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.reference, reference);
    assert_eq!(plan.purpose, SecretPurpose::ProviderCredential);
    assert_eq!(plan.reference.to_string(), "<redacted-secret-ref>");

    let rendered = format!("{plan:?}");
    for sensitive in [
        &tenant_id.to_string(),
        "external-secrets",
        "providers/openai",
        "v3",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "config secret reference plan debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
}

#[test]
fn config_secret_source_debug_output_is_stable_and_redacted() {
    let source = ConfigSecretSource::Reference(SecretRef::new(
        "external-secrets",
        "providers/openai",
        Some("v3"),
    ));

    let rendered = format!("{source:?}");
    for sensitive in ["external-secrets", "providers/openai", "v3"] {
        assert!(
            !rendered.contains(sensitive),
            "config secret source debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
}

#[test]
fn config_secret_reference_rejects_raw_secret_material_with_redacted_response() {
    let tenant_id = TenantId::new();
    let error = plan_config_secret_reference(
        tenant_id,
        SecretPurpose::ProviderCredential,
        ConfigSecretSource::RawSecretMaterial,
    )
    .unwrap_err();

    assert_eq!(error, ConfigSecretReferenceError::RawSecretMaterialRejected);
    assert_eq!(
        error.to_string(),
        "configuration secrets must use secret references"
    );
    assert!(!error.to_string().contains("SecretRef"));
    let response = plan_config_secret_reference_error_response(&error);
    assert_eq!(
        response.status,
        ConfigSecretReferenceErrorStatus::InvalidConfiguration
    );
    assert_eq!(response.code, "configuration_secret_reference_required");
    assert_eq!(
        response.message,
        "configuration secrets must use secret references"
    );

    let rendered = format!("{response:?}");
    for sensitive in ["providers/openai", "raw-token", "v3"] {
        assert!(
            !rendered.contains(sensitive),
            "config secret response leaked sensitive token {sensitive}: {rendered}"
        );
    }
}

#[test]
fn config_secret_reference_rejects_malformed_secret_ref_with_redacted_response() {
    let tenant_id = TenantId::new();
    let error = plan_config_secret_reference(
        tenant_id,
        SecretPurpose::ProviderCredential,
        ConfigSecretSource::Reference(SecretRef::new("external secrets", "raw-token", Some("v3"))),
    )
    .unwrap_err();

    assert_eq!(error, ConfigSecretReferenceError::MalformedSecretReference);
    assert_eq!(
        error.to_string(),
        "configuration secrets must use secret references"
    );
    assert!(!error.to_string().contains("SecretRef"));
    let response = plan_config_secret_reference_error_response(&error);
    assert_eq!(
        response.status,
        ConfigSecretReferenceErrorStatus::InvalidConfiguration
    );
    assert_eq!(response.code, "configuration_secret_reference_invalid");
    assert_eq!(
        response.message,
        "configuration secrets must use secret references"
    );

    let rendered = format!("{response:?}");
    for sensitive in ["external secrets", "raw-token", "v3"] {
        assert!(
            !rendered.contains(sensitive),
            "config secret response leaked sensitive token {sensitive}: {rendered}"
        );
    }

    let overlong = "x".repeat(129);
    for reference in [
        SecretRef::new("external-secrets", "providers/openai☃", Some("v3")),
        SecretRef::new("external-secrets", overlong.as_str(), Some("v3")),
    ] {
        assert_eq!(
            plan_config_secret_reference(
                tenant_id,
                SecretPurpose::ProviderCredential,
                ConfigSecretSource::Reference(reference),
            ),
            Err(ConfigSecretReferenceError::MalformedSecretReference)
        );
    }
}

#[test]
fn config_refresh_uses_active_refreshes_async_then_lkg_then_required() {
    let tenant_id = TenantId::new();
    let active_revision_id = PolicyRevisionId::new();
    let lkg_revision_id = PolicyRevisionId::new();
    let state = ConfigCacheState {
        tenant_id,
        active_revision_id: Some(active_revision_id),
        last_known_good_revision_id: Some(lkg_revision_id),
        invalidated_revision_id: None,
        window: window(),
    };

    assert_eq!(
        evaluate_config_refresh(&state, 999),
        ConfigRefreshDecision::UseActive
    );
    assert_eq!(
        evaluate_config_refresh(&state, 1_500),
        ConfigRefreshDecision::RefreshAsync
    );
    assert_eq!(
        evaluate_config_refresh(&state, 2_500),
        ConfigRefreshDecision::UseLastKnownGood
    );
    assert_eq!(
        evaluate_config_refresh(&state, 3_000),
        ConfigRefreshDecision::RefreshRequired
    );
}

#[test]
fn config_refresh_rejects_invalidated_active_without_lkg() {
    let tenant_id = TenantId::new();
    let revision_id = PolicyRevisionId::new();
    let state = ConfigCacheState {
        tenant_id,
        active_revision_id: Some(revision_id),
        last_known_good_revision_id: None,
        invalidated_revision_id: Some(revision_id),
        window: window(),
    };

    assert_eq!(
        evaluate_config_refresh(&state, 500),
        ConfigRefreshDecision::RejectedInvalidated
    );
}

#[test]
fn config_refresh_falls_back_to_lkg_when_active_is_invalidated() {
    let tenant_id = TenantId::new();
    let active_revision_id = PolicyRevisionId::new();
    let lkg_revision_id = PolicyRevisionId::new();
    let state = ConfigCacheState {
        tenant_id,
        active_revision_id: Some(active_revision_id),
        last_known_good_revision_id: Some(lkg_revision_id),
        invalidated_revision_id: Some(active_revision_id),
        window: window(),
    };

    assert_eq!(
        evaluate_config_refresh(&state, 500),
        ConfigRefreshDecision::UseLastKnownGood
    );
}

#[test]
fn config_refresh_does_not_use_invalidated_lkg() {
    let tenant_id = TenantId::new();
    let active_revision_id = PolicyRevisionId::new();
    let lkg_revision_id = PolicyRevisionId::new();
    let state = ConfigCacheState {
        tenant_id,
        active_revision_id: Some(active_revision_id),
        last_known_good_revision_id: Some(lkg_revision_id),
        invalidated_revision_id: Some(lkg_revision_id),
        window: window(),
    };

    assert_eq!(
        evaluate_config_refresh(&state, 2_500),
        ConfigRefreshDecision::RefreshRequired
    );
}

#[test]
fn config_refresh_error_for_decision_only_wraps_unservable_cache_states() {
    assert_eq!(
        config_refresh_error_for_decision(ConfigRefreshDecision::UseActive),
        None
    );
    assert_eq!(
        config_refresh_error_for_decision(ConfigRefreshDecision::RefreshAsync),
        None
    );
    assert_eq!(
        config_refresh_error_for_decision(ConfigRefreshDecision::UseLastKnownGood),
        None
    );
    assert_eq!(
        config_refresh_error_for_decision(ConfigRefreshDecision::RefreshRequired),
        Some(ConfigRefreshError::RefreshRequired)
    );
    assert_eq!(
        config_refresh_error_for_decision(ConfigRefreshDecision::RejectedInvalidated),
        Some(ConfigRefreshError::InvalidatedRevisionRejected)
    );
}

#[test]
fn config_refresh_error_response_is_stable_and_redacted() {
    let required_response =
        plan_config_refresh_error_response(&ConfigRefreshError::RefreshRequired);
    assert_eq!(
        ConfigRefreshError::RefreshRequired.to_string(),
        "configuration is not currently available"
    );
    assert!(
        !ConfigRefreshError::RefreshRequired
            .to_string()
            .contains("refresh")
    );
    assert_eq!(
        required_response.status,
        ConfigRefreshErrorStatus::ServiceUnavailable
    );
    assert_eq!(required_response.code, "configuration_refresh_required");
    assert_eq!(
        required_response.message,
        "configuration is not currently available"
    );

    let invalidated_response =
        plan_config_refresh_error_response(&ConfigRefreshError::InvalidatedRevisionRejected);
    assert_eq!(
        ConfigRefreshError::InvalidatedRevisionRejected.to_string(),
        "configuration is not currently available"
    );
    assert!(
        !ConfigRefreshError::InvalidatedRevisionRejected
            .to_string()
            .contains("invalidated")
    );
    assert!(
        !ConfigRefreshError::InvalidatedRevisionRejected
            .to_string()
            .contains("revision")
    );
    assert_eq!(
        invalidated_response.status,
        ConfigRefreshErrorStatus::ServiceUnavailable
    );
    assert_eq!(
        invalidated_response.code,
        "configuration_revision_unavailable"
    );
    assert_eq!(
        invalidated_response.message,
        "configuration is not currently available"
    );

    let rendered = format!("{required_response:?} {invalidated_response:?}");
    for sensitive in [
        "refresh_after",
        "stale_after",
        "expires_after",
        "last_known_good",
    ] {
        assert!(
            !rendered.contains(sensitive),
            "config refresh response leaked cache internals {sensitive}: {rendered}"
        );
    }
}

#[test]
fn publication_requires_matching_tenant_and_newer_revision() {
    let tenant_id = TenantId::new();
    let current = PolicyRevisionId::new();
    let candidate = ConfigRevision {
        tenant_id,
        revision_id: PolicyRevisionId::new(),
        published_at_unix_ms: 1_000,
        payload: "config-v2",
    };

    assert_eq!(
        validate_config_publication(tenant_id, Some(current), &candidate),
        Ok(())
    );

    let wrong_tenant_id = TenantId::new();
    assert_eq!(
        validate_config_publication(wrong_tenant_id, Some(current), &candidate),
        Err(ConfigPublicationError::TenantMismatch {
            expected: wrong_tenant_id,
            actual: tenant_id,
        })
    );
}

#[test]
fn publication_rejects_same_or_older_revision() {
    let tenant_id = TenantId::new();
    let current = PolicyRevisionId::new();
    let candidate = ConfigRevision {
        tenant_id,
        revision_id: current,
        published_at_unix_ms: 1_000,
        payload: "config-v1",
    };

    assert_eq!(
        validate_config_publication(tenant_id, Some(current), &candidate),
        Err(ConfigPublicationError::RevisionNotNewer {
            current,
            candidate: current,
        })
    );
}

#[test]
fn publication_error_responses_are_stable_and_redacted() {
    let expected = TenantId::new();
    let actual = TenantId::new();
    let tenant_error = ConfigPublicationError::TenantMismatch { expected, actual };
    assert_eq!(tenant_error.to_string(), "configuration request is invalid");
    assert!(!tenant_error.to_string().contains(&expected.to_string()));
    assert!(!tenant_error.to_string().contains(&actual.to_string()));
    assert!(!tenant_error.to_string().contains("tenant mismatch"));
    let tenant_rendered = format!("{tenant_error:?}");
    assert!(!tenant_rendered.contains(&expected.to_string()));
    assert!(!tenant_rendered.contains(&actual.to_string()));
    let tenant_response = plan_config_publication_error_response(&tenant_error);

    assert_eq!(
        tenant_response.status,
        ConfigPublicationErrorStatus::BadRequest
    );
    assert_eq!(tenant_response.code, "configuration_tenant_mismatch");
    assert_eq!(
        tenant_response.message,
        "configuration tenant does not match request tenant"
    );
    assert!(!tenant_response.message.contains(&expected.to_string()));
    assert!(!tenant_response.message.contains(&actual.to_string()));

    let current = PolicyRevisionId::new();
    let candidate = current;
    let revision_error = ConfigPublicationError::RevisionNotNewer { current, candidate };
    assert_eq!(
        revision_error.to_string(),
        "configuration revision is not newer than active revision"
    );
    assert!(!revision_error.to_string().contains(&current.to_string()));
    assert!(!revision_error.to_string().contains(&candidate.to_string()));
    assert!(!revision_error.to_string().contains("config revision"));
    let revision_rendered = format!("{revision_error:?}");
    assert!(!revision_rendered.contains(&current.to_string()));
    assert!(!revision_rendered.contains(&candidate.to_string()));
    let revision_response = plan_config_publication_error_response(&revision_error);

    assert_eq!(
        revision_response.status,
        ConfigPublicationErrorStatus::Conflict
    );
    assert_eq!(revision_response.code, "configuration_revision_not_newer");
    assert_eq!(
        revision_response.message,
        "configuration revision is not newer than active revision"
    );
    assert!(!revision_response.message.contains(&current.to_string()));
    assert!(!revision_response.message.contains(&candidate.to_string()));
}

#[test]
fn config_activation_promotes_candidate_and_preserves_previous_active_as_lkg_when_valid() {
    let tenant_id = TenantId::new();
    let current = PolicyRevisionId::new();
    let older_lkg = PolicyRevisionId::new();
    let candidate = ConfigRevision {
        tenant_id,
        revision_id: PolicyRevisionId::new(),
        published_at_unix_ms: 2_000,
        payload: "config-v2",
    };
    let state = ConfigCacheState {
        tenant_id,
        active_revision_id: Some(current),
        last_known_good_revision_id: Some(older_lkg),
        invalidated_revision_id: None,
        window: window(),
    };

    let plan = plan_config_activation(&state, &candidate).unwrap();

    assert_eq!(plan.previous_active_revision_id, Some(current));
    assert_eq!(plan.activated_revision_id, candidate.revision_id);
    assert_eq!(plan.next_state.tenant_id, tenant_id);
    assert_eq!(
        plan.next_state.active_revision_id,
        Some(candidate.revision_id)
    );
    assert_eq!(plan.next_state.last_known_good_revision_id, Some(current));
    assert_eq!(plan.next_state.invalidated_revision_id, None);
    assert_eq!(plan.next_state.window, state.window);

    let rendered = format!("{plan:?}");
    for sensitive in [
        current.to_string(),
        older_lkg.to_string(),
        candidate.revision_id.to_string(),
        tenant_id.to_string(),
        "1000".to_string(),
        "2000".to_string(),
        "3000".to_string(),
    ] {
        assert!(
            !rendered.contains(&sensitive),
            "config activation plan debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
}

#[test]
fn config_activation_does_not_promote_invalidated_active_as_lkg() {
    let tenant_id = TenantId::new();
    let current = PolicyRevisionId::new();
    let older_lkg = PolicyRevisionId::new();
    let candidate = ConfigRevision {
        tenant_id,
        revision_id: PolicyRevisionId::new(),
        published_at_unix_ms: 2_000,
        payload: "config-v2",
    };
    let state = ConfigCacheState {
        tenant_id,
        active_revision_id: Some(current),
        last_known_good_revision_id: Some(older_lkg),
        invalidated_revision_id: Some(current),
        window: window(),
    };

    let plan = plan_config_activation(&state, &candidate).unwrap();

    assert_eq!(plan.previous_active_revision_id, Some(current));
    assert_eq!(
        plan.next_state.active_revision_id,
        Some(candidate.revision_id)
    );
    assert_eq!(plan.next_state.last_known_good_revision_id, Some(older_lkg));
    assert_eq!(plan.next_state.invalidated_revision_id, None);
}

#[test]
fn config_activation_clears_lkg_when_only_fallback_was_invalidated() {
    let tenant_id = TenantId::new();
    let current = PolicyRevisionId::new();
    let candidate = ConfigRevision {
        tenant_id,
        revision_id: PolicyRevisionId::new(),
        published_at_unix_ms: 2_000,
        payload: "config-v2",
    };
    let state = ConfigCacheState {
        tenant_id,
        active_revision_id: Some(current),
        last_known_good_revision_id: Some(current),
        invalidated_revision_id: Some(current),
        window: window(),
    };

    let plan = plan_config_activation(&state, &candidate).unwrap();

    assert_eq!(
        plan.next_state.active_revision_id,
        Some(candidate.revision_id)
    );
    assert_eq!(plan.next_state.last_known_good_revision_id, None);
    assert_eq!(plan.next_state.invalidated_revision_id, None);
}

#[test]
fn config_activation_rejects_cross_tenant_or_non_newer_candidate() {
    let tenant_id = TenantId::new();
    let current = PolicyRevisionId::new();
    let state = ConfigCacheState {
        tenant_id,
        active_revision_id: Some(current),
        last_known_good_revision_id: None,
        invalidated_revision_id: None,
        window: window(),
    };
    let wrong_tenant_id = TenantId::new();
    let wrong_tenant_candidate = ConfigRevision {
        tenant_id: wrong_tenant_id,
        revision_id: PolicyRevisionId::new(),
        published_at_unix_ms: 2_000,
        payload: "wrong-tenant",
    };
    let stale_candidate = ConfigRevision {
        tenant_id,
        revision_id: current,
        published_at_unix_ms: 2_000,
        payload: "stale",
    };

    assert_eq!(
        plan_config_activation(&state, &wrong_tenant_candidate),
        Err(ConfigPublicationError::TenantMismatch {
            expected: tenant_id,
            actual: wrong_tenant_id,
        })
    );
    assert_eq!(
        plan_config_activation(&state, &stale_candidate),
        Err(ConfigPublicationError::RevisionNotNewer {
            current,
            candidate: current,
        })
    );
}

#[test]
fn config_publication_event_targets_gateway_cache_and_runtime_policy_reload() {
    let tenant_id = TenantId::new();
    let current = PolicyRevisionId::new();
    let candidate = ConfigRevision {
        tenant_id,
        revision_id: PolicyRevisionId::new(),
        published_at_unix_ms: 2_000,
        payload: "config-v2",
    };
    let state = ConfigCacheState {
        tenant_id,
        active_revision_id: Some(current),
        last_known_good_revision_id: None,
        invalidated_revision_id: None,
        window: window(),
    };
    let activation = plan_config_activation(&state, &candidate).unwrap();

    let event = plan_config_publication_event(
        &activation,
        ConfigPublicationEventDelivery {
            gateway_cache_refresh: true,
            runtime_policy_reload: true,
        },
    )
    .unwrap();

    assert_eq!(event.tenant_id, tenant_id);
    assert_eq!(event.activated_revision_id, candidate.revision_id);
    assert_eq!(event.previous_active_revision_id, Some(current));
    assert_eq!(event.last_known_good_revision_id, Some(current));
    assert_eq!(
        event.targets,
        [
            ConfigPublicationEventTarget::GatewayCacheRefresh,
            ConfigPublicationEventTarget::RuntimePolicyReload,
        ]
    );

    let rendered = format!("{event:?}");
    for sensitive in [
        tenant_id.to_string(),
        current.to_string(),
        candidate.revision_id.to_string(),
    ] {
        assert!(
            !rendered.contains(&sensitive),
            "config publication event plan debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
}

#[test]
fn config_publication_event_rejects_missing_delivery_targets_with_redacted_response() {
    let tenant_id = TenantId::new();
    let current = PolicyRevisionId::new();
    let candidate = ConfigRevision {
        tenant_id,
        revision_id: PolicyRevisionId::new(),
        published_at_unix_ms: 2_000,
        payload: "config-v2",
    };
    let state = ConfigCacheState {
        tenant_id,
        active_revision_id: Some(current),
        last_known_good_revision_id: None,
        invalidated_revision_id: None,
        window: window(),
    };
    let activation = plan_config_activation(&state, &candidate).unwrap();

    let cache_error = plan_config_publication_event(
        &activation,
        ConfigPublicationEventDelivery {
            gateway_cache_refresh: false,
            runtime_policy_reload: true,
        },
    )
    .unwrap_err();
    assert_eq!(
        cache_error,
        ConfigPublicationEventError::MissingGatewayCacheRefreshTarget
    );
    assert_eq!(
        cache_error.to_string(),
        "configuration publication event is incomplete"
    );
    assert!(!cache_error.to_string().contains("gateway"));
    assert!(!cache_error.to_string().contains("cache"));

    let reload_error = plan_config_publication_event(
        &activation,
        ConfigPublicationEventDelivery {
            gateway_cache_refresh: true,
            runtime_policy_reload: false,
        },
    )
    .unwrap_err();
    assert_eq!(
        reload_error,
        ConfigPublicationEventError::MissingRuntimePolicyReloadTarget
    );
    assert_eq!(
        reload_error.to_string(),
        "configuration publication event is incomplete"
    );
    assert!(!reload_error.to_string().contains("runtime"));
    assert!(!reload_error.to_string().contains("policy"));

    let response = plan_config_publication_event_error_response(&reload_error);
    assert_eq!(
        response.status,
        ConfigPublicationEventErrorStatus::InvalidConfiguration
    );
    assert_eq!(response.code, "configuration_publication_event_incomplete");
    assert_eq!(
        response.message,
        "configuration publication event is incomplete"
    );
    assert!(!response.message.contains(&tenant_id.to_string()));
    assert!(
        !response
            .message
            .contains(&candidate.revision_id.to_string())
    );
}

#[test]
fn config_invalidation_marks_known_revision_without_mutating_active_or_lkg() {
    let tenant_id = TenantId::new();
    let active_revision_id = PolicyRevisionId::new();
    let lkg_revision_id = PolicyRevisionId::new();
    let state = ConfigCacheState {
        tenant_id,
        active_revision_id: Some(active_revision_id),
        last_known_good_revision_id: Some(lkg_revision_id),
        invalidated_revision_id: None,
        window: window(),
    };

    let plan = plan_config_invalidation(tenant_id, &state, active_revision_id).unwrap();

    assert_eq!(plan.tenant_id, tenant_id);
    assert_eq!(plan.invalidated_revision_id, active_revision_id);
    assert_eq!(plan.next_state.tenant_id, tenant_id);
    assert_eq!(plan.next_state.active_revision_id, Some(active_revision_id));
    assert_eq!(
        plan.next_state.last_known_good_revision_id,
        Some(lkg_revision_id)
    );
    assert_eq!(
        plan.next_state.invalidated_revision_id,
        Some(active_revision_id)
    );
    assert_eq!(
        evaluate_config_refresh(&plan.next_state, 500),
        ConfigRefreshDecision::UseLastKnownGood
    );

    let rendered = format!("{plan:?}");
    for sensitive in [
        tenant_id.to_string(),
        active_revision_id.to_string(),
        lkg_revision_id.to_string(),
        "1000".to_string(),
        "2000".to_string(),
        "3000".to_string(),
    ] {
        assert!(
            !rendered.contains(&sensitive),
            "config invalidation plan debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
}

#[test]
fn config_invalidation_rejects_cross_tenant_or_unknown_revision() {
    let tenant_id = TenantId::new();
    let other_tenant_id = TenantId::new();
    let active_revision_id = PolicyRevisionId::new();
    let unknown_revision_id = PolicyRevisionId::new();
    let state = ConfigCacheState {
        tenant_id,
        active_revision_id: Some(active_revision_id),
        last_known_good_revision_id: None,
        invalidated_revision_id: None,
        window: window(),
    };

    assert_eq!(
        plan_config_invalidation(other_tenant_id, &state, active_revision_id),
        Err(ConfigInvalidationError::TenantMismatch {
            expected: other_tenant_id,
            actual: tenant_id,
        })
    );
    assert_eq!(
        plan_config_invalidation(tenant_id, &state, unknown_revision_id),
        Err(ConfigInvalidationError::UnknownRevision {
            revision_id: unknown_revision_id,
        })
    );
}

#[test]
fn config_invalidation_error_responses_are_stable_and_redacted() {
    let expected = TenantId::new();
    let actual = TenantId::new();
    let tenant_error = ConfigInvalidationError::TenantMismatch { expected, actual };
    assert_eq!(tenant_error.to_string(), "configuration request is invalid");
    assert!(!tenant_error.to_string().contains(&expected.to_string()));
    assert!(!tenant_error.to_string().contains(&actual.to_string()));
    assert!(!tenant_error.to_string().contains("tenant mismatch"));
    let tenant_rendered = format!("{tenant_error:?}");
    assert!(!tenant_rendered.contains(&expected.to_string()));
    assert!(!tenant_rendered.contains(&actual.to_string()));
    let tenant_response = plan_config_invalidation_error_response(&tenant_error);

    assert_eq!(
        tenant_response.status,
        ConfigInvalidationErrorStatus::BadRequest
    );
    assert_eq!(tenant_response.code, "configuration_tenant_mismatch");
    assert_eq!(
        tenant_response.message,
        "configuration tenant does not match request tenant"
    );
    assert!(!tenant_response.message.contains(&expected.to_string()));
    assert!(!tenant_response.message.contains(&actual.to_string()));

    let revision_id = PolicyRevisionId::new();
    let revision_error = ConfigInvalidationError::UnknownRevision { revision_id };
    assert_eq!(
        revision_error.to_string(),
        "configuration revision is not known"
    );
    assert!(
        !revision_error
            .to_string()
            .contains(&revision_id.to_string())
    );
    assert!(!revision_error.to_string().contains("cache"));
    assert!(!format!("{revision_error:?}").contains(&revision_id.to_string()));
    let revision_response = plan_config_invalidation_error_response(&revision_error);

    assert_eq!(
        revision_response.status,
        ConfigInvalidationErrorStatus::Conflict
    );
    assert_eq!(revision_response.code, "configuration_revision_unknown");
    assert_eq!(
        revision_response.message,
        "configuration revision is not known"
    );
    assert!(!revision_response.message.contains(&revision_id.to_string()));
}
