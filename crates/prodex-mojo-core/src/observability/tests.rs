use super::*;
#[test]
fn labels_are_bounded_and_reject_unknowns() {
    assert!(label(0, 0).is_ok());
    assert_eq!(label(-1, 0), Err(MojoError::InvalidInput));
    assert_eq!(label(0, -1), Err(MojoError::InvalidInput));
    assert!(label(10_000, 0).is_err());
}

#[test]
fn websocket_executor_labels_are_mojo_owned() {
    assert_eq!(
        runtime_websocket_local_pressure_label(0).unwrap(),
        "dns_resolve_timeout"
    );
    assert_eq!(
        runtime_websocket_local_pressure_label(2).unwrap(),
        "tcp_connect_executor_overflow"
    );
    assert_eq!(runtime_websocket_task_label(0).unwrap(), "tcp_connect");
    assert_eq!(
        runtime_websocket_worker_thread_prefix(1).unwrap(),
        "prodex-ws-dns"
    );
    assert_eq!(
        runtime_websocket_dispatcher_thread_name(0).unwrap(),
        "prodex-ws-connect-dispatch"
    );
    assert_eq!(
        runtime_websocket_overflow_enqueue_event(1).unwrap(),
        "websocket_dns_overflow_enqueue"
    );
    assert_eq!(
        runtime_websocket_overflow_dispatch_event(0).unwrap(),
        "websocket_connect_overflow_dispatch"
    );
    assert_eq!(
        runtime_websocket_overflow_reject_event(1).unwrap(),
        "websocket_dns_overflow_reject"
    );
    assert_eq!(
        runtime_websocket_task_label(2),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn runtime_proxy_observability_labels_are_mojo_owned() {
    assert_eq!(
        runtime_websocket_direct_fallback_reason_label(0).unwrap(),
        "precommit_budget_exhausted"
    );
    assert_eq!(runtime_http_error_class_label(4).unwrap(), "transient_5xx");
    assert_eq!(
        runtime_http_error_action_label(1).unwrap(),
        "rotate_profile"
    );
    assert_eq!(
        runtime_precommit_quota_block_reason_label(2).unwrap(),
        "quota_windows_unavailable_after_reprobe"
    );
    assert_eq!(
        runtime_quota_pressure_band_reason_label(3).unwrap(),
        "quota_exhausted"
    );
    assert_eq!(
        runtime_quota_window_status_reason_label(2).unwrap(),
        "critical"
    );
    assert_eq!(runtime_quota_source_label(1).unwrap(), "persisted_snapshot");
    assert_eq!(
        runtime_http_error_class_label(6),
        Err(MojoError::InvalidInput)
    );
    assert_eq!(runtime_quota_source_label(2), Err(MojoError::InvalidInput));
}

#[test]
fn operational_event_source_labels_are_mojo_owned() {
    assert_eq!(operational_event_source_label(0).unwrap(), None);
    assert_eq!(operational_event_source_label(1).unwrap(), Some("request"));
    assert_eq!(operational_event_source_label(2).unwrap(), Some("mcp"));
    assert_eq!(operational_event_source_label(19).unwrap(), Some("event"));
    assert_eq!(
        operational_event_source_label(20),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn operational_detail_specs_are_mojo_owned() {
    let specs = (0..62)
        .map(operational_detail_spec)
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(specs.len(), 62);
    assert_eq!(specs[0].key, "profile");
    assert_eq!(specs[0].label, "profile");
    assert_eq!(specs[0].format, OperationalDetailFormat::Plain);
    assert_eq!(specs[35].key, "five_hour_remaining");
    assert_eq!(specs[35].label, "5h");
    assert_eq!(specs[35].format, OperationalDetailFormat::Percent);
    assert_eq!(specs[15].key, "path");
    assert_eq!(specs[15].label, "path");
    assert_eq!(specs[15].format, OperationalDetailFormat::Endpoint);
    assert_eq!(operational_detail_spec(62), Err(MojoError::InvalidInput));
}

#[test]
fn plan_label_metadata_is_bounded_and_stable() {
    assert_eq!(
        plan_label_spec(68, 0),
        Ok(ObservabilityPlanLabelSpec {
            key: 132,
            kind: 124
        })
    );
    assert_eq!(
        plan_label_spec(63, 4),
        Ok(ObservabilityPlanLabelSpec { key: 82, kind: 118 })
    );
    assert_eq!(
        plan_label_spec(76, 0),
        Ok(ObservabilityPlanLabelSpec { key: 94, kind: 64 })
    );
    assert!(plan_label_spec(68, 9).is_err());
    assert!(plan_label_spec(10_000, 0).is_err());
}
