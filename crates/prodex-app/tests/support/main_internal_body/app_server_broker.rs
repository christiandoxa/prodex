use super::*;

#[derive(serde::Deserialize)]
struct AppServerBrokerDiagnosticFixtureCase {
    name: String,
    payload: serde_json::Value,
    expect: AppServerBrokerDiagnosticFixtureExpect,
}

#[derive(serde::Deserialize)]
struct AppServerBrokerDiagnosticFixtureExpect {
    valid_jsonrpc: bool,
    frame_kind: String,
    id: Option<serde_json::Value>,
    method: Option<String>,
    method_kind: String,
    is_lifecycle_method: bool,
    lifecycle_stage: Option<String>,
    continuation_affinity: AppServerBrokerContinuationAffinityExpect,
    continuation_decision: String,
    policy_hint: AppServerBrokerPolicyHintExpect,
    primary_affinity: Option<AppServerBrokerDiagnosticAffinityKeyExpect>,
    affinity_keys: Vec<AppServerBrokerDiagnosticAffinityKeyExpect>,
    invalid_reason: Option<String>,
    session_id: Option<String>,
    thread_id: Option<String>,
    turn_id: Option<String>,
    item_id: Option<String>,
}

#[derive(serde::Deserialize)]
struct AppServerBrokerDiagnosticAffinityKeyExpect {
    kind: String,
    value: String,
}

#[derive(serde::Deserialize)]
struct AppServerBrokerContinuationAffinityExpect {
    stage: Option<String>,
    owner_kind: String,
    owner: Option<AppServerBrokerDiagnosticAffinityKeyExpect>,
    primary: Option<AppServerBrokerDiagnosticAffinityKeyExpect>,
    key_count: u64,
    has_turn: bool,
    has_thread: bool,
    has_session: bool,
}

#[derive(serde::Deserialize)]
struct AppServerBrokerPolicyHintExpect {
    mode: String,
    routing_hint: String,
    commit_boundary: String,
    rotation_window: String,
    turn_committed: bool,
    affinity_required: bool,
    rotation_allowed: bool,
}

#[derive(serde::Deserialize)]
struct AppServerBrokerRequestFixtureCase {
    name: String,
    payload: serde_json::Value,
    expect: AppServerBrokerRequestFixtureExpect,
}

#[derive(serde::Deserialize)]
struct AppServerBrokerRequestFixtureExpect {
    id: Option<serde_json::Value>,
    raw_method: String,
    method: String,
    method_kind: String,
    lifecycle_stage: Option<String>,
    continuation_affinity: AppServerBrokerContinuationAffinityExpect,
    continuation_decision: String,
    policy_hint: AppServerBrokerPolicyHintExpect,
    primary_affinity: Option<AppServerBrokerDiagnosticAffinityKeyExpect>,
    affinity_keys: Vec<AppServerBrokerDiagnosticAffinityKeyExpect>,
    session_id: Option<String>,
    thread_id: Option<String>,
    turn_id: Option<String>,
    item_id: Option<String>,
}

#[derive(serde::Deserialize)]
struct AppServerBrokerFrameFixtureCase {
    name: String,
    payload: serde_json::Value,
    expect: AppServerBrokerFrameFixtureExpect,
}

#[derive(serde::Deserialize)]
struct AppServerBrokerFrameFixtureExpect {
    frame_kind: String,
    method: Option<String>,
    valid_jsonrpc: bool,
    id: Option<serde_json::Value>,
    has_result: bool,
    has_error: bool,
    error_code: Option<serde_json::Value>,
    error_message: Option<serde_json::Value>,
    continuation_affinity: AppServerBrokerContinuationAffinityExpect,
    continuation_decision: String,
    policy_hint: AppServerBrokerPolicyHintExpect,
    primary_affinity: Option<AppServerBrokerDiagnosticAffinityKeyExpect>,
    affinity_keys: Vec<AppServerBrokerDiagnosticAffinityKeyExpect>,
    session_id: Option<String>,
    thread_id: Option<String>,
    turn_id: Option<String>,
    item_id: Option<String>,
}

#[derive(serde::Deserialize)]
struct AppServerBrokerProtocolSurfaceFixture {
    jsonrpc_version: String,
    wire_omits_jsonrpc_header: bool,
    canonical_lifecycle_methods: Vec<String>,
    accepted_lifecycle_aliases: Vec<String>,
    example_other_methods: Vec<String>,
    frame_kinds: Vec<String>,
    method_kinds: Vec<String>,
    invalid_reasons: Vec<String>,
    affinity: AppServerBrokerProtocolAffinityFixture,
}

#[derive(serde::Deserialize)]
struct AppServerBrokerProtocolAffinityFixture {
    thread_session_owner_required: bool,
    continuation_affinity_wins: bool,
    rotate_only_before_turn_commit: bool,
    decision_kinds: Vec<String>,
    policy_modes: Vec<String>,
    routing_hints: Vec<String>,
    commit_boundaries: Vec<String>,
    rotation_windows: Vec<String>,
}

#[derive(serde::Deserialize)]
struct AppServerBrokerOutputSurfaceFixture {
    preview_event: AppServerBrokerOutputObjectFixture,
    preview_report: AppServerBrokerOutputReportFixture,
}

#[derive(serde::Deserialize)]
struct AppServerBrokerOutputObjectFixture {
    object: String,
    required_fields: Vec<String>,
    preview_required_fields: Vec<String>,
    summary_required_fields: Vec<String>,
    policy_hint_required_fields: Vec<String>,
    metadata_required_fields: Vec<String>,
    error_required_fields: Vec<String>,
}

#[derive(serde::Deserialize)]
struct AppServerBrokerOutputReportFixture {
    object: String,
    required_fields: Vec<String>,
    report_required_fields: Vec<String>,
    frame_kind_count_fields: Vec<String>,
    method_kind_count_fields: Vec<String>,
    continuation_decision_count_fields: Vec<String>,
    policy_mode_count_fields: Vec<String>,
    commit_boundary_count_fields: Vec<String>,
    rotation_window_count_fields: Vec<String>,
    routing_hint_count_fields: Vec<String>,
    policy_flag_count_fields: Vec<String>,
    owner_kind_count_fields: Vec<String>,
    invalid_reason_count_fields: Vec<String>,
}

#[derive(serde::Deserialize)]
struct AppServerBrokerLogSurfaceFixture {
    observe_event: AppServerBrokerLogEventFixture,
    summary_event: AppServerBrokerLogEventFixture,
    audit_event: AppServerBrokerAuditEventFixture,
}

#[derive(serde::Deserialize)]
struct AppServerBrokerLogEventFixture {
    event: String,
    required_fields: Vec<String>,
    parsed_frame_required_fields: Option<Vec<String>>,
    parse_error_required_fields: Option<Vec<String>>,
}

#[derive(serde::Deserialize)]
struct AppServerBrokerAuditEventFixture {
    component: String,
    action: String,
    outcome: String,
    modes: Vec<String>,
    required_fields: Vec<String>,
}

#[derive(serde::Deserialize)]
struct AppServerBrokerMetadataSurfaceFixture {
    fields: Vec<String>,
    cases: Vec<AppServerBrokerMetadataSurfaceCase>,
}

#[derive(serde::Deserialize)]
struct AppServerBrokerUpstreamLifecycleSchemaManifest {
    canonical_methods: Vec<String>,
    accepted_aliases: Vec<String>,
    lifecycle_schema_files: Vec<AppServerBrokerUpstreamLifecycleSchemaFile>,
}

#[derive(serde::Deserialize)]
struct AppServerBrokerUpstreamLifecycleSchemaFile {
    method: String,
    file: String,
    title: String,
    required: Option<Vec<String>>,
}

#[derive(serde::Deserialize)]
struct AppServerBrokerUpstreamSchemaCase {
    name: String,
    schema_file: String,
    body_field: String,
    payload: serde_json::Value,
}

#[derive(serde::Deserialize)]
struct AppServerBrokerExpectedRuntimeLogFixture {
    observe: Vec<std::collections::BTreeMap<String, String>>,
    summary: std::collections::BTreeMap<String, String>,
}

#[derive(serde::Deserialize)]
struct AppServerBrokerMetadataSurfaceCase {
    name: String,
    payload: serde_json::Value,
    expect: AppServerBrokerMetadataSurfaceExpect,
}

#[derive(serde::Deserialize)]
struct AppServerBrokerMetadataSurfaceExpect {
    session_id: Option<String>,
    thread_id: Option<String>,
    turn_id: Option<String>,
    item_id: Option<String>,
}

fn app_server_broker_fixture_cases() -> Vec<AppServerBrokerDiagnosticFixtureCase> {
    serde_json::from_str(include_str!(
        "../../fixtures/compat_replay/app_server_broker_diagnostic_cases.json"
    ))
    .expect("broker diagnostic fixture cases should parse")
}

fn app_server_broker_request_fixture_cases() -> Vec<AppServerBrokerRequestFixtureCase> {
    serde_json::from_str(include_str!(
        "../../fixtures/compat_replay/app_server_broker_request_cases.json"
    ))
    .expect("broker request fixture cases should parse")
}

fn app_server_broker_frame_fixture_cases() -> Vec<AppServerBrokerFrameFixtureCase> {
    serde_json::from_str(include_str!(
        "../../fixtures/compat_replay/app_server_broker_frame_cases.json"
    ))
    .expect("broker frame fixture cases should parse")
}

fn app_server_broker_protocol_surface_fixture() -> AppServerBrokerProtocolSurfaceFixture {
    serde_json::from_str(include_str!(
        "../../fixtures/compat_replay/app_server_broker_protocol_surface.json"
    ))
    .expect("broker protocol surface fixture should parse")
}

fn app_server_broker_output_surface_fixture() -> AppServerBrokerOutputSurfaceFixture {
    serde_json::from_str(include_str!(
        "../../fixtures/compat_replay/app_server_broker_output_surface.json"
    ))
    .expect("broker output surface fixture should parse")
}

fn app_server_broker_log_surface_fixture() -> AppServerBrokerLogSurfaceFixture {
    serde_json::from_str(include_str!(
        "../../fixtures/compat_replay/app_server_broker_log_surface.json"
    ))
    .expect("broker log surface fixture should parse")
}

fn app_server_broker_metadata_surface_fixture() -> AppServerBrokerMetadataSurfaceFixture {
    serde_json::from_str(include_str!(
        "../../fixtures/compat_replay/app_server_broker_metadata_surface.json"
    ))
    .expect("broker metadata surface fixture should parse")
}

fn app_server_broker_upstream_thread_start_schema_fixture() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../fixtures/compat_replay/upstream_codex_schema/ThreadStartParams.json"
    ))
    .expect("upstream ThreadStartParams schema fixture should parse")
}

fn app_server_broker_upstream_turn_start_schema_fixture() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../fixtures/compat_replay/upstream_codex_schema/TurnStartParams.json"
    ))
    .expect("upstream TurnStartParams schema fixture should parse")
}

fn app_server_broker_upstream_lifecycle_schema_manifest_fixture(
) -> AppServerBrokerUpstreamLifecycleSchemaManifest {
    serde_json::from_str(include_str!(
        "../../fixtures/compat_replay/app_server_broker_upstream_lifecycle_schema_manifest.json"
    ))
    .expect("upstream lifecycle schema manifest fixture should parse")
}

fn app_server_broker_upstream_schema_cases() -> Vec<AppServerBrokerUpstreamSchemaCase> {
    serde_json::from_str(include_str!(
        "../../fixtures/compat_replay/app_server_broker_upstream_schema_cases.json"
    ))
    .expect("upstream schema replay cases should parse")
}

fn app_server_broker_upstream_schema_replay_fixture() -> &'static str {
    include_str!("../../fixtures/compat_replay/app_server_broker_upstream_schema_replay.txt")
}

fn app_server_broker_upstream_schema_expected_report_fixture() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../fixtures/compat_replay/app_server_broker_upstream_schema_expected_report.json"
    ))
    .expect("broker upstream schema stdio preview expected report fixture should parse")
}

fn app_server_broker_upstream_schema_expected_stream_fixture() -> &'static str {
    include_str!(
        "../../fixtures/compat_replay/app_server_broker_upstream_schema_expected_stream.jsonl"
    )
}

fn app_server_broker_upstream_schema_passthrough_expected_stdout_fixture() -> &'static str {
    include_str!(
        "../../fixtures/compat_replay/app_server_broker_upstream_schema_passthrough_expected_stdout.txt"
    )
}

fn app_server_broker_upstream_schema_passthrough_expected_stderr_fixture() -> &'static str {
    include_str!(
        "../../fixtures/compat_replay/app_server_broker_upstream_schema_passthrough_expected_stderr.jsonl"
    )
}

fn app_server_broker_upstream_schema_expected_runtime_log_fixture(
) -> AppServerBrokerExpectedRuntimeLogFixture {
    serde_json::from_str(include_str!(
        "../../fixtures/compat_replay/app_server_broker_upstream_schema_expected_runtime_log.json"
    ))
    .expect("broker upstream schema runtime log fixture should parse")
}

fn assert_object_has_fields(value: &serde_json::Value, fields: &[String], context: &str) {
    let object = value.as_object().unwrap_or_else(|| panic!("{context}: object expected"));
    for field in fields {
        assert!(object.contains_key(field), "{context}: missing `{field}`");
    }
}

fn expected_affinity_json(
    expect: &AppServerBrokerContinuationAffinityExpect,
) -> serde_json::Value {
    serde_json::json!({
        "stage": expect.stage,
        "owner_kind": expect.owner_kind,
        "owner": expect.owner.as_ref().map(|key| serde_json::json!({
            "kind": key.kind,
            "value": key.value,
        })),
        "primary": expect.primary.as_ref().map(|key| serde_json::json!({
            "kind": key.kind,
            "value": key.value,
        })),
        "key_count": expect.key_count,
        "has_turn": expect.has_turn,
        "has_thread": expect.has_thread,
        "has_session": expect.has_session,
    })
}

fn expected_policy_hint_json(expect: &AppServerBrokerPolicyHintExpect) -> serde_json::Value {
    let preserved_owner_kind = match expect.routing_hint.as_str() {
        "fresh-select-ok" => None,
        "preserve-session-owner" => Some("session"),
        "preserve-thread-owner" => Some("thread"),
        "preserve-turn-owner" => Some("turn"),
        other => panic!("unexpected routing hint `{other}` in fixture policy hint"),
    };
    serde_json::json!({
        "mode": expect.mode,
        "routing_hint": expect.routing_hint,
        "preserved_owner_kind": preserved_owner_kind,
        "commit_boundary": expect.commit_boundary,
        "rotation_window": expect.rotation_window,
        "turn_committed": expect.turn_committed,
        "affinity_required": expect.affinity_required,
        "rotation_allowed": expect.rotation_allowed,
        "preserves_owner": preserved_owner_kind.is_some(),
    })
}

fn expected_policy_hint_struct(
    expect: &AppServerBrokerPolicyHintExpect,
) -> AppServerBrokerPolicyHint {
    AppServerBrokerPolicyHint {
        mode: match expect.mode.as_str() {
            "fresh-selection-ok" => AppServerBrokerPolicyHintMode::FreshSelectionOk,
            "preserve-session-affinity" => AppServerBrokerPolicyHintMode::PreserveSessionAffinity,
            "preserve-thread-affinity" => AppServerBrokerPolicyHintMode::PreserveThreadAffinity,
            "preserve-turn-affinity" => AppServerBrokerPolicyHintMode::PreserveTurnAffinity,
            other => panic!("unexpected policy mode {other}"),
        },
        routing_hint: match expect.routing_hint.as_str() {
            "fresh-select-ok" => AppServerBrokerRoutingHint::FreshSelectOk,
            "preserve-session-owner" => AppServerBrokerRoutingHint::PreserveSessionOwner,
            "preserve-thread-owner" => AppServerBrokerRoutingHint::PreserveThreadOwner,
            "preserve-turn-owner" => AppServerBrokerRoutingHint::PreserveTurnOwner,
            other => panic!("unexpected routing hint {other}"),
        },
        commit_boundary: match expect.commit_boundary.as_str() {
            "precommit" => AppServerBrokerCommitBoundary::Precommit,
            "turn-committed" => AppServerBrokerCommitBoundary::TurnCommitted,
            other => panic!("unexpected commit boundary {other}"),
        },
        rotation_window: match expect.rotation_window.as_str() {
            "open" => AppServerBrokerRotationWindow::Open,
            "closed" => AppServerBrokerRotationWindow::Closed,
            other => panic!("unexpected rotation window {other}"),
        },
    }
}

fn assert_policy_hint_struct_matches_expect(
    actual: AppServerBrokerPolicyHint,
    expect: &AppServerBrokerPolicyHintExpect,
    context: &str,
) {
    assert_eq!(actual, expected_policy_hint_struct(expect), "{context}");
    assert_eq!(actual.turn_committed(), expect.turn_committed, "{context}");
    assert_eq!(actual.affinity_required(), expect.affinity_required, "{context}");
    assert_eq!(actual.rotation_allowed(), expect.rotation_allowed, "{context}");
    let expected_owner_kind = match expect.routing_hint.as_str() {
        "fresh-select-ok" => None,
        "preserve-session-owner" => Some(AppServerBrokerContinuationOwnerKind::Session),
        "preserve-thread-owner" => Some(AppServerBrokerContinuationOwnerKind::Thread),
        "preserve-turn-owner" => Some(AppServerBrokerContinuationOwnerKind::Turn),
        other => panic!("unexpected routing hint {other}"),
    };
    assert_eq!(
        actual.preserved_owner_kind(),
        expected_owner_kind,
        "{context}"
    );
    assert_eq!(actual.preserves_owner(), expected_owner_kind.is_some(), "{context}");
}

fn assert_log_fields_have_keys(
    fields: &std::collections::BTreeMap<String, String>,
    required_fields: &[String],
    context: &str,
) {
    for field in required_fields {
        assert!(fields.contains_key(field), "{context}: missing `{field}`");
    }
}

fn runtime_log_message_body(line: &str) -> &str {
    line.split_once("] ").map(|(_, message)| message).unwrap_or(line)
}

fn resolve_upstream_schema_node<'a>(
    schema: &'a serde_json::Value,
    root_schema: &'a serde_json::Value,
) -> &'a serde_json::Value {
    if let Some(reference) = schema.get("$ref").and_then(serde_json::Value::as_str)
        && let Some(definition) = reference.strip_prefix("#/definitions/")
    {
        return root_schema
            .get("definitions")
            .and_then(serde_json::Value::as_object)
            .and_then(|definitions| definitions.get(definition))
            .unwrap_or_else(|| panic!("missing definition `{definition}`"));
    }
    if let Some(all_of) = schema.get("allOf").and_then(serde_json::Value::as_array)
        && let Some(first) = all_of.first()
    {
        return resolve_upstream_schema_node(first, root_schema);
    }
    schema
}

fn validate_upstream_schema_subset(
    value: &serde_json::Value,
    schema: &serde_json::Value,
    root_schema: &serde_json::Value,
    context: &str,
) -> Result<(), String> {
    let schema = resolve_upstream_schema_node(schema, root_schema);
    if let Some(any_of) = schema.get("anyOf").and_then(serde_json::Value::as_array) {
        let mut errors = Vec::new();
        for candidate in any_of {
            match validate_upstream_schema_subset(value, candidate, root_schema, context) {
                Ok(()) => return Ok(()),
                Err(error) => errors.push(error),
            }
        }
        return Err(format!(
            "{context}: no anyOf branch matched ({})",
            errors.join("; ")
        ));
    }
    if let Some(one_of) = schema.get("oneOf").and_then(serde_json::Value::as_array) {
        let mut errors = Vec::new();
        for candidate in one_of {
            match validate_upstream_schema_subset(value, candidate, root_schema, context) {
                Ok(()) => return Ok(()),
                Err(error) => errors.push(error),
            }
        }
        return Err(format!(
            "{context}: no oneOf branch matched ({})",
            errors.join("; ")
        ));
    }
    if let Some(enum_values) = schema.get("enum").and_then(serde_json::Value::as_array) {
        if enum_values.iter().any(|candidate| candidate == value) {
            return Ok(());
        }
        return Err(format!("{context}: value `{value}` not present in enum"));
    }
    if let Some(expected_type) = schema.get("type").and_then(serde_json::Value::as_str) {
        let valid_type = match expected_type {
            "object" => value.is_object(),
            "array" => value.is_array(),
            "string" => value.is_string(),
            "boolean" => value.is_boolean(),
            "integer" => value.as_i64().is_some() || value.as_u64().is_some(),
            "number" => value.is_number(),
            "null" => value.is_null(),
            _ => true,
        };
        if !valid_type {
            return Err(format!("{context}: expected `{expected_type}`, got `{value}`"));
        }
    }
    if let Some(const_value) = schema.get("const")
        && const_value != value
    {
        return Err(format!("{context}: expected const `{const_value}`, got `{value}`"));
    }
    if let Some(items_schema) = schema.get("items") {
        let array = value
            .as_array()
            .ok_or_else(|| format!("{context}: array expected"))?;
        for (index, item) in array.iter().enumerate() {
            validate_upstream_schema_subset(
                item,
                items_schema,
                root_schema,
                &format!("{context}[{index}]"),
            )?;
        }
        return Ok(());
    }
    let Some(object) = value.as_object() else {
        return Ok(());
    };
    if let Some(required) = schema.get("required").and_then(serde_json::Value::as_array) {
        for field in required {
            let field = field
                .as_str()
                .ok_or_else(|| format!("{context}: schema required entries should be strings"))?;
            if !object.contains_key(field) {
                return Err(format!("{context}: missing `{field}`"));
            }
        }
    }
    let properties = schema
        .get("properties")
        .and_then(serde_json::Value::as_object);
    for (field, field_value) in object {
        let property_schema = properties
            .and_then(|properties| properties.get(field))
            .ok_or_else(|| format!("{context}: unexpected field `{field}`"))?;
        validate_upstream_schema_subset(
            field_value,
            property_schema,
            root_schema,
            &format!("{context}.{field}"),
        )?;
    }
    Ok(())
}

fn assert_upstream_schema_subset(
    value: &serde_json::Value,
    schema: &serde_json::Value,
    root_schema: &serde_json::Value,
    context: &str,
) {
    if let Err(error) = validate_upstream_schema_subset(value, schema, root_schema, context) {
        panic!("{error}");
    }
}

fn assert_upstream_schema_object_subset(
    value: &serde_json::Value,
    schema: &serde_json::Value,
    root_schema: &serde_json::Value,
    context: &str,
) {
    let object = value
        .as_object()
        .unwrap_or_else(|| panic!("{context}: object expected"));
    assert_upstream_schema_subset(value, schema, root_schema, context);
    if let Some(additional_properties) = schema.get("additionalProperties")
        && additional_properties == &serde_json::Value::Bool(false)
    {
        let properties = schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .unwrap_or_else(|| panic!("{context}: schema properties should be an object"));
        for field in object.keys() {
            assert!(properties.contains_key(field), "{context}: unexpected `{field}`");
        }
    }
}

fn assert_payload_body_matches_upstream_schema_case(
    payload: &serde_json::Value,
    schema_file: &str,
    body_field: &str,
    context: &str,
) {
    let root_schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!(
            "{}/tests/fixtures/compat_replay/upstream_codex_schema/{schema_file}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap_or_else(|_| panic!("{context}: missing upstream schema fixture `{schema_file}`")),
    )
    .unwrap_or_else(|_| panic!("{context}: invalid upstream schema fixture `{schema_file}`"));
    let schema = resolve_upstream_schema_node(&root_schema, &root_schema);
    let body = payload
        .get(body_field)
        .unwrap_or_else(|| panic!("{context}: missing `{body_field}` body"));
    assert_upstream_schema_subset(body, schema, &root_schema, context);
}

fn assert_preview_event_matches_surface(
    value: &serde_json::Value,
    fixture: &AppServerBrokerOutputSurfaceFixture,
    context: &str,
) {
    assert_object_has_fields(value, &fixture.preview_event.required_fields, context);
    assert_eq!(value["object"], fixture.preview_event.object, "{context}");
    assert!(value["line"].is_u64(), "{context}: line should be u64");
    let preview = &value["preview"];
    assert_object_has_fields(
        preview,
        &fixture.preview_event.preview_required_fields,
        &format!("{context}.preview"),
    );
    assert!(preview["parse_ok"].is_boolean(), "{context}: parse_ok should be bool");
    if preview["parse_ok"] == serde_json::Value::Bool(true) {
        let summary = &preview["summary"];
        assert_object_has_fields(
            summary,
            &fixture.preview_event.summary_required_fields,
            &format!("{context}.summary"),
        );
        assert_object_has_fields(
            &summary["policy_hint"],
            &fixture.preview_event.policy_hint_required_fields,
            &format!("{context}.summary.policy_hint"),
        );
        let metadata = &summary["metadata"];
        assert_object_has_fields(
            metadata,
            &fixture.preview_event.metadata_required_fields,
            &format!("{context}.summary.metadata"),
        );
    } else {
        assert_object_has_fields(
            preview,
            &fixture.preview_event.error_required_fields,
            &format!("{context}.preview"),
        );
    }
}

fn assert_preview_report_matches_surface(
    value: &serde_json::Value,
    fixture: &AppServerBrokerOutputSurfaceFixture,
    context: &str,
) {
    assert_object_has_fields(value, &fixture.preview_report.required_fields, context);
    assert_eq!(value["object"], fixture.preview_report.object, "{context}");
    assert_preview_report_body_matches_surface(&value["report"], fixture, &format!("{context}.report"));
}

fn assert_preview_report_body_matches_surface(
    report: &serde_json::Value,
    fixture: &AppServerBrokerOutputSurfaceFixture,
    context: &str,
) {
    assert_object_has_fields(
        report,
        &fixture.preview_report.report_required_fields,
        context,
    );
    assert!(report["line_count"].is_u64(), "{context}: line_count should be u64");
    assert!(report["parsed_count"].is_u64(), "{context}: parsed_count should be u64");
    assert!(report["error_count"].is_u64(), "{context}: error_count should be u64");
    assert_object_has_fields(
        &report["frame_kind_counts"],
        &fixture.preview_report.frame_kind_count_fields,
        &format!("{context}.frame_kind_counts"),
    );
    assert_object_has_fields(
        &report["method_kind_counts"],
        &fixture.preview_report.method_kind_count_fields,
        &format!("{context}.method_kind_counts"),
    );
    assert_object_has_fields(
        &report["continuation_decision_counts"],
        &fixture.preview_report.continuation_decision_count_fields,
        &format!("{context}.continuation_decision_counts"),
    );
    assert_object_has_fields(
        &report["policy_mode_counts"],
        &fixture.preview_report.policy_mode_count_fields,
        &format!("{context}.policy_mode_counts"),
    );
    assert_object_has_fields(
        &report["commit_boundary_counts"],
        &fixture.preview_report.commit_boundary_count_fields,
        &format!("{context}.commit_boundary_counts"),
    );
    assert_object_has_fields(
        &report["rotation_window_counts"],
        &fixture.preview_report.rotation_window_count_fields,
        &format!("{context}.rotation_window_counts"),
    );
    assert_object_has_fields(
        &report["routing_hint_counts"],
        &fixture.preview_report.routing_hint_count_fields,
        &format!("{context}.routing_hint_counts"),
    );
    assert_object_has_fields(
        &report["policy_flag_counts"],
        &fixture.preview_report.policy_flag_count_fields,
        &format!("{context}.policy_flag_counts"),
    );
    assert_object_has_fields(
        &report["owner_kind_counts"],
        &fixture.preview_report.owner_kind_count_fields,
        &format!("{context}.owner_kind_counts"),
    );
    assert_object_has_fields(
        &report["invalid_reason_counts"],
        &fixture.preview_report.invalid_reason_count_fields,
        &format!("{context}.invalid_reason_counts"),
    );
    let previews = report["previews"]
        .as_array()
        .unwrap_or_else(|| panic!("{context}: previews should be an array"));
    for (index, preview) in previews.iter().enumerate() {
        assert_preview_event_matches_surface(
            preview,
            fixture,
            &format!("{context}.report.previews[{index}]"),
        );
    }
}

fn assert_preview_report_counts_match_previews(report: &serde_json::Value, context: &str) {
    let previews = report["previews"]
        .as_array()
        .unwrap_or_else(|| panic!("{context}: previews should be an array"));
    let parsed_count = previews
        .iter()
        .filter(|preview| preview["preview"]["parse_ok"] == serde_json::Value::Bool(true))
        .count() as u64;
    let error_count = (previews.len() as u64).saturating_sub(parsed_count);

    let mut request = 0u64;
    let mut notification = 0u64;
    let mut response = 0u64;
    let mut invalid = 0u64;
    let mut lifecycle = 0u64;
    let mut other = 0u64;
    let mut absent = 0u64;
    let mut fresh = 0u64;
    let mut continue_session = 0u64;
    let mut continue_thread = 0u64;
    let mut continue_turn = 0u64;
    let mut fresh_selection_ok = 0u64;
    let mut preserve_session_affinity = 0u64;
    let mut preserve_thread_affinity = 0u64;
    let mut preserve_turn_affinity = 0u64;
    let mut precommit = 0u64;
    let mut turn_committed = 0u64;
    let mut rotation_open = 0u64;
    let mut rotation_closed = 0u64;
    let mut fresh_select_ok = 0u64;
    let mut preserve_session_owner = 0u64;
    let mut preserve_thread_owner = 0u64;
    let mut preserve_turn_owner = 0u64;
    let mut affinity_required = 0u64;
    let mut rotation_allowed = 0u64;
    let mut preserves_owner = 0u64;
    let mut owner_none = 0u64;
    let mut owner_session = 0u64;
    let mut owner_thread = 0u64;
    let mut owner_turn = 0u64;
    let mut non_jsonrpc_version = 0u64;
    let mut batch_frame_unsupported = 0u64;
    let mut non_object_frame = 0u64;
    let mut non_scalar_id = 0u64;
    let mut non_container_params = 0u64;
    let mut non_object_error = 0u64;
    let mut non_integer_error_code = 0u64;
    let mut non_string_error_message = 0u64;
    let mut non_string_method = 0u64;
    let mut invalid_method_name = 0u64;
    let mut result_with_error = 0u64;
    let mut missing_response_id = 0u64;
    let mut method_with_result_or_error = 0u64;
    let mut missing_method_and_response_payload = 0u64;

    for preview in previews {
        match preview["preview"]["summary"]["frame_kind"].as_str() {
            Some("request") => request += 1,
            Some("notification") => notification += 1,
            Some("response") => response += 1,
            Some("invalid") => invalid += 1,
            _ => {}
        }
        match preview["preview"]["summary"]["method_kind"].as_str() {
            Some("lifecycle") => lifecycle += 1,
            Some("other") => other += 1,
            Some("absent") => absent += 1,
            _ => {}
        }
        match preview["preview"]["summary"]["continuation_decision"].as_str() {
            Some("fresh") => fresh += 1,
            Some("continue-session") => continue_session += 1,
            Some("continue-thread") => continue_thread += 1,
            Some("continue-turn") => continue_turn += 1,
            _ => {}
        }
        match preview["preview"]["summary"]["policy_hint"]["mode"].as_str() {
            Some("fresh-selection-ok") => fresh_selection_ok += 1,
            Some("preserve-session-affinity") => preserve_session_affinity += 1,
            Some("preserve-thread-affinity") => preserve_thread_affinity += 1,
            Some("preserve-turn-affinity") => preserve_turn_affinity += 1,
            _ => {}
        }
        match preview["preview"]["summary"]["policy_hint"]["commit_boundary"].as_str() {
            Some("precommit") => precommit += 1,
            Some("turn-committed") => turn_committed += 1,
            _ => {}
        }
        match preview["preview"]["summary"]["policy_hint"]["rotation_window"].as_str() {
            Some("open") => rotation_open += 1,
            Some("closed") => rotation_closed += 1,
            _ => {}
        }
        match preview["preview"]["summary"]["policy_hint"]["routing_hint"].as_str() {
            Some("fresh-select-ok") => fresh_select_ok += 1,
            Some("preserve-session-owner") => preserve_session_owner += 1,
            Some("preserve-thread-owner") => preserve_thread_owner += 1,
            Some("preserve-turn-owner") => preserve_turn_owner += 1,
            _ => {}
        }
        if preview["preview"]["summary"]["policy_hint"]["affinity_required"]
            == serde_json::Value::Bool(true)
        {
            affinity_required += 1;
        }
        if preview["preview"]["summary"]["policy_hint"]["rotation_allowed"]
            == serde_json::Value::Bool(true)
        {
            rotation_allowed += 1;
        }
        if preview["preview"]["summary"]["policy_hint"]["preserves_owner"]
            == serde_json::Value::Bool(true)
        {
            preserves_owner += 1;
        }
        match preview["preview"]["summary"]["continuation_affinity"]["owner_kind"].as_str() {
            Some("none") => owner_none += 1,
            Some("session") => owner_session += 1,
            Some("thread") => owner_thread += 1,
            Some("turn") => owner_turn += 1,
            _ => {}
        }
        match preview["preview"]["summary"]["invalid_reason"].as_str() {
            Some("non_jsonrpc_version") => non_jsonrpc_version += 1,
            Some("batch_frame_unsupported") => batch_frame_unsupported += 1,
            Some("non_object_frame") => non_object_frame += 1,
            Some("non_scalar_id") => non_scalar_id += 1,
            Some("non_container_params") => non_container_params += 1,
            Some("non_object_error") => non_object_error += 1,
            Some("non_integer_error_code") => non_integer_error_code += 1,
            Some("non_string_error_message") => non_string_error_message += 1,
            Some("non_string_method") => non_string_method += 1,
            Some("invalid_method_name") => invalid_method_name += 1,
            Some("result_with_error") => result_with_error += 1,
            Some("missing_response_id") => missing_response_id += 1,
            Some("method_with_result_or_error") => method_with_result_or_error += 1,
            Some("missing_method_and_response_payload") => missing_method_and_response_payload += 1,
            _ => {}
        }
    }

    assert_eq!(report["line_count"].as_u64(), Some(previews.len() as u64), "{context}");
    assert_eq!(report["parsed_count"].as_u64(), Some(parsed_count), "{context}");
    assert_eq!(report["error_count"].as_u64(), Some(error_count), "{context}");
    assert_eq!(report["frame_kind_counts"]["request"].as_u64(), Some(request), "{context}");
    assert_eq!(
        report["frame_kind_counts"]["notification"].as_u64(),
        Some(notification),
        "{context}"
    );
    assert_eq!(report["frame_kind_counts"]["response"].as_u64(), Some(response), "{context}");
    assert_eq!(report["frame_kind_counts"]["invalid"].as_u64(), Some(invalid), "{context}");
    assert_eq!(
        report["method_kind_counts"]["lifecycle"].as_u64(),
        Some(lifecycle),
        "{context}"
    );
    assert_eq!(report["method_kind_counts"]["other"].as_u64(), Some(other), "{context}");
    assert_eq!(report["method_kind_counts"]["absent"].as_u64(), Some(absent), "{context}");
    assert_eq!(report["continuation_decision_counts"]["fresh"].as_u64(), Some(fresh), "{context}");
    assert_eq!(
        report["continuation_decision_counts"]["continue-session"].as_u64(),
        Some(continue_session),
        "{context}"
    );
    assert_eq!(
        report["continuation_decision_counts"]["continue-thread"].as_u64(),
        Some(continue_thread),
        "{context}"
    );
    assert_eq!(
        report["continuation_decision_counts"]["continue-turn"].as_u64(),
        Some(continue_turn),
        "{context}"
    );
    assert_eq!(
        report["policy_mode_counts"]["fresh-selection-ok"].as_u64(),
        Some(fresh_selection_ok),
        "{context}"
    );
    assert_eq!(
        report["policy_mode_counts"]["preserve-session-affinity"].as_u64(),
        Some(preserve_session_affinity),
        "{context}"
    );
    assert_eq!(
        report["policy_mode_counts"]["preserve-thread-affinity"].as_u64(),
        Some(preserve_thread_affinity),
        "{context}"
    );
    assert_eq!(
        report["policy_mode_counts"]["preserve-turn-affinity"].as_u64(),
        Some(preserve_turn_affinity),
        "{context}"
    );
    assert_eq!(
        report["commit_boundary_counts"]["precommit"].as_u64(),
        Some(precommit),
        "{context}"
    );
    assert_eq!(
        report["commit_boundary_counts"]["turn-committed"].as_u64(),
        Some(turn_committed),
        "{context}"
    );
    assert_eq!(
        report["rotation_window_counts"]["open"].as_u64(),
        Some(rotation_open),
        "{context}"
    );
    assert_eq!(
        report["rotation_window_counts"]["closed"].as_u64(),
        Some(rotation_closed),
        "{context}"
    );
    assert_eq!(
        report["routing_hint_counts"]["fresh-select-ok"].as_u64(),
        Some(fresh_select_ok),
        "{context}"
    );
    assert_eq!(
        report["routing_hint_counts"]["preserve-session-owner"].as_u64(),
        Some(preserve_session_owner),
        "{context}"
    );
    assert_eq!(
        report["routing_hint_counts"]["preserve-thread-owner"].as_u64(),
        Some(preserve_thread_owner),
        "{context}"
    );
    assert_eq!(
        report["routing_hint_counts"]["preserve-turn-owner"].as_u64(),
        Some(preserve_turn_owner),
        "{context}"
    );
    assert_eq!(
        report["policy_flag_counts"]["affinity_required"].as_u64(),
        Some(affinity_required),
        "{context}"
    );
    assert_eq!(
        report["policy_flag_counts"]["rotation_allowed"].as_u64(),
        Some(rotation_allowed),
        "{context}"
    );
    assert_eq!(
        report["policy_flag_counts"]["preserves_owner"].as_u64(),
        Some(preserves_owner),
        "{context}"
    );
    assert_eq!(report["owner_kind_counts"]["none"].as_u64(), Some(owner_none), "{context}");
    assert_eq!(
        report["owner_kind_counts"]["session"].as_u64(),
        Some(owner_session),
        "{context}"
    );
    assert_eq!(
        report["owner_kind_counts"]["thread"].as_u64(),
        Some(owner_thread),
        "{context}"
    );
    assert_eq!(report["owner_kind_counts"]["turn"].as_u64(), Some(owner_turn), "{context}");
    assert_eq!(
        report["invalid_reason_counts"]["non_jsonrpc_version"].as_u64(),
        Some(non_jsonrpc_version),
        "{context}"
    );
    assert_eq!(
        report["invalid_reason_counts"]["non_object_frame"].as_u64(),
        Some(non_object_frame),
        "{context}"
    );
    assert_eq!(
        report["invalid_reason_counts"]["batch_frame_unsupported"].as_u64(),
        Some(batch_frame_unsupported),
        "{context}"
    );
    assert_eq!(
        report["invalid_reason_counts"]["non_string_method"].as_u64(),
        Some(non_string_method),
        "{context}"
    );
    assert_eq!(
        report["invalid_reason_counts"]["invalid_method_name"].as_u64(),
        Some(invalid_method_name),
        "{context}"
    );
    assert_eq!(
        report["invalid_reason_counts"]["non_scalar_id"].as_u64(),
        Some(non_scalar_id),
        "{context}"
    );
    assert_eq!(
        report["invalid_reason_counts"]["non_container_params"].as_u64(),
        Some(non_container_params),
        "{context}"
    );
    assert_eq!(
        report["invalid_reason_counts"]["non_object_error"].as_u64(),
        Some(non_object_error),
        "{context}"
    );
    assert_eq!(
        report["invalid_reason_counts"]["non_integer_error_code"].as_u64(),
        Some(non_integer_error_code),
        "{context}"
    );
    assert_eq!(
        report["invalid_reason_counts"]["non_string_error_message"].as_u64(),
        Some(non_string_error_message),
        "{context}"
    );
    assert_eq!(
        report["invalid_reason_counts"]["result_with_error"].as_u64(),
        Some(result_with_error),
        "{context}"
    );
    assert_eq!(
        report["invalid_reason_counts"]["missing_response_id"].as_u64(),
        Some(missing_response_id),
        "{context}"
    );
    assert_eq!(
        report["invalid_reason_counts"]["method_with_result_or_error"].as_u64(),
        Some(method_with_result_or_error),
        "{context}"
    );
    assert_eq!(
        report["invalid_reason_counts"]["missing_method_and_response_payload"].as_u64(),
        Some(missing_method_and_response_payload),
        "{context}"
    );
}

fn app_server_broker_stdio_preview_replay_fixture() -> &'static str {
    include_str!("../../fixtures/compat_replay/app_server_broker_stdio_preview_replay.txt")
}

fn app_server_broker_stdio_preview_expected_report_fixture() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../fixtures/compat_replay/app_server_broker_stdio_preview_expected_report.json"
    ))
    .expect("broker stdio preview expected report fixture should parse")
}

fn app_server_broker_stdio_preview_expected_stream_fixture() -> &'static str {
    include_str!("../../fixtures/compat_replay/app_server_broker_stdio_preview_expected_stream.jsonl")
}

fn app_server_broker_stdio_preview_malformed_replay_fixture() -> &'static str {
    include_str!(
        "../../fixtures/compat_replay/app_server_broker_stdio_preview_malformed_replay.txt"
    )
}

fn app_server_broker_stdio_preview_malformed_expected_report_fixture() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../fixtures/compat_replay/app_server_broker_stdio_preview_malformed_expected_report.json"
    ))
    .expect("broker malformed stdio preview expected report fixture should parse")
}

fn app_server_broker_stdio_preview_malformed_expected_stream_fixture() -> &'static str {
    include_str!(
        "../../fixtures/compat_replay/app_server_broker_stdio_preview_malformed_expected_stream.jsonl"
    )
}

fn app_server_broker_stdio_preview_wire_replay_fixture() -> &'static str {
    include_str!("../../fixtures/compat_replay/app_server_broker_stdio_preview_wire_replay.txt")
}

fn app_server_broker_stdio_preview_wire_expected_report_fixture() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../fixtures/compat_replay/app_server_broker_stdio_preview_wire_expected_report.json"
    ))
    .expect("broker wire-format stdio preview expected report fixture should parse")
}

fn app_server_broker_stdio_preview_wire_expected_stream_fixture() -> &'static str {
    include_str!("../../fixtures/compat_replay/app_server_broker_stdio_preview_wire_expected_stream.jsonl")
}

fn app_server_broker_stdio_preview_lifecycle_replay_fixture() -> &'static str {
    include_str!("../../fixtures/compat_replay/app_server_broker_stdio_preview_lifecycle_replay.txt")
}

fn app_server_broker_stdio_preview_lifecycle_expected_report_fixture() -> serde_json::Value {
    serde_json::from_str(include_str!(
        "../../fixtures/compat_replay/app_server_broker_stdio_preview_lifecycle_expected_report.json"
    ))
    .expect("broker lifecycle stdio preview expected report fixture should parse")
}

fn app_server_broker_stdio_preview_lifecycle_expected_stream_fixture() -> &'static str {
    include_str!(
        "../../fixtures/compat_replay/app_server_broker_stdio_preview_lifecycle_expected_stream.jsonl"
    )
}

fn app_server_broker_stdio_preview_lifecycle_passthrough_expected_stdout_fixture() -> &'static str {
    include_str!(
        "../../fixtures/compat_replay/app_server_broker_stdio_preview_lifecycle_passthrough_expected_stdout.txt"
    )
}

fn app_server_broker_stdio_preview_lifecycle_passthrough_expected_stderr_fixture() -> &'static str {
    include_str!(
        "../../fixtures/compat_replay/app_server_broker_stdio_preview_lifecycle_passthrough_expected_stderr.jsonl"
    )
}

struct AppServerBrokerRuntimeLogTestDir {
    path: std::path::PathBuf,
}

impl AppServerBrokerRuntimeLogTestDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "prodex-app-server-broker-log-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).expect("broker runtime log test dir should exist");
        Self { path }
    }
}

impl Drop for AppServerBrokerRuntimeLogTestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn assert_broker_preview_audit_event(
    audit_dir: &std::path::Path,
    mode: &str,
    line_count: u64,
    parsed_count: u64,
    error_count: u64,
) {
    let fixture = app_server_broker_log_surface_fixture();
    let audit = &fixture.audit_event;
    let path = audit_dir.join("prodex-audit.log");
    let result = prodex_audit_log::read_recent_audit_events_with_scope(
        &path,
        &prodex_audit_log::AuditLogQuery {
            tail: 10,
            component: Some(audit.component.clone()),
            action: Some(audit.action.clone()),
            outcome: Some(audit.outcome.clone()),
        },
    )
    .expect("broker audit log should be readable");
    let event = result
        .events
        .last()
        .expect("broker preview audit event should exist");
    assert_eq!(event.component, audit.component);
    assert_eq!(event.action, audit.action);
    assert_eq!(event.outcome, audit.outcome);
    assert_object_has_fields(&event.details, &audit.required_fields, "broker preview audit event");
    assert!(
        audit.modes.iter().any(|candidate| candidate == mode),
        "unexpected broker preview audit mode {mode}"
    );
    assert_eq!(event.details["mode"], serde_json::Value::String(mode.to_string()));
    assert_eq!(event.details["line_count"], serde_json::json!(line_count));
    assert_eq!(event.details["parsed_count"], serde_json::json!(parsed_count));
    assert_eq!(event.details["error_count"], serde_json::json!(error_count));
    assert!(event.details["frame_kind_counts"].is_object());
    assert!(event.details["continuation_decision_counts"].is_object());
    assert!(event.details["policy_mode_counts"].is_object());
    assert!(event.details["commit_boundary_counts"].is_object());
    assert!(event.details["rotation_window_counts"].is_object());
    assert!(event.details["routing_hint_counts"].is_object());
    assert!(event.details["provider_switch_counts"].is_object());
    assert!(event.details["provider_switch_counts"]["allowed"].is_u64());
    assert!(
        event.details["provider_switch_counts"]["requires_override"].is_u64()
    );
    assert!(event.details["policy_flag_counts"].is_object());
}

#[test]
fn app_server_broker_parses_jsonrpc_lifecycle_methods() {
    for (method, expected) in [
        ("initialize", AppServerBrokerMethod::Initialize),
        ("notifications/initialized", AppServerBrokerMethod::Initialized),
        ("thread/start", AppServerBrokerMethod::ThreadStart),
        ("thread/resume", AppServerBrokerMethod::ThreadResume),
        ("thread/fork", AppServerBrokerMethod::ThreadFork),
        ("turn/start", AppServerBrokerMethod::TurnStart),
        ("turn/interrupt", AppServerBrokerMethod::TurnInterrupt),
        ("turn/cancel", AppServerBrokerMethod::TurnInterrupt),
        ("unknown/method", AppServerBrokerMethod::Other),
    ] {
        let request = parse_app_server_broker_request(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": method,
            "params": {}
        }))
        .expect("valid JSON-RPC request should parse");

        assert_eq!(request.method, expected);
        assert_eq!(request.method.label(), expected.label());
        assert_eq!(request.raw_method, method);
        assert!(request.metadata.is_empty());
    }

    assert!(
        parse_app_server_broker_request(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "turn/start",
            "result": {"ok": true}
        }))
        .is_none()
    );
}

#[test]
fn app_server_broker_protocol_surface_fixture_matches_helper_taxonomy() {
    let fixture = app_server_broker_protocol_surface_fixture();
    assert_eq!(fixture.jsonrpc_version, "2.0");
    assert!(fixture.wire_omits_jsonrpc_header);
    assert_eq!(
        fixture.canonical_lifecycle_methods,
        app_server_broker_lifecycle_methods()
            .iter()
            .map(|method| method.to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        fixture.accepted_lifecycle_aliases,
        vec![
            "notifications/initialized".to_string(),
            "turn/cancel".to_string()
        ]
    );
    assert_eq!(
        fixture.frame_kinds,
        vec![
            AppServerBrokerFrameKind::Request.label().to_string(),
            AppServerBrokerFrameKind::Notification.label().to_string(),
            AppServerBrokerFrameKind::Response.label().to_string(),
            AppServerBrokerFrameKind::Invalid.label().to_string(),
        ]
    );
    assert_eq!(
        fixture.method_kinds,
        vec![
            AppServerBrokerMethodKind::Lifecycle.label().to_string(),
            AppServerBrokerMethodKind::Other.label().to_string(),
            AppServerBrokerMethodKind::Absent.label().to_string(),
        ]
    );
    assert_eq!(
        fixture.invalid_reasons,
        vec![
            "non_jsonrpc_version".to_string(),
            "batch_frame_unsupported".to_string(),
            "non_object_frame".to_string(),
            "non_scalar_id".to_string(),
            "non_container_params".to_string(),
            "non_object_error".to_string(),
            "non_integer_error_code".to_string(),
            "non_string_error_message".to_string(),
            "non_string_method".to_string(),
            "invalid_method_name".to_string(),
            "result_with_error".to_string(),
            "missing_response_id".to_string(),
            "method_with_result_or_error".to_string(),
            "missing_method_and_response_payload".to_string(),
        ]
    );
    assert!(fixture.affinity.thread_session_owner_required);
    assert!(fixture.affinity.continuation_affinity_wins);
    assert!(fixture.affinity.rotate_only_before_turn_commit);
    assert_eq!(
        fixture.affinity.decision_kinds,
        app_server_broker_continuation_decision_kinds()
            .iter()
            .map(|label| label.to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        fixture.affinity.policy_modes,
        app_server_broker_policy_modes()
            .iter()
            .map(|label| label.to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        fixture.affinity.routing_hints,
        app_server_broker_routing_hints()
            .iter()
            .map(|label| label.to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        fixture.affinity.commit_boundaries,
        app_server_broker_commit_boundaries()
            .iter()
            .map(|label| label.to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        fixture.affinity.rotation_windows,
        app_server_broker_rotation_windows()
            .iter()
            .map(|label| label.to_string())
            .collect::<Vec<_>>()
    );
    for method in fixture
        .canonical_lifecycle_methods
        .iter()
        .chain(fixture.accepted_lifecycle_aliases.iter())
    {
        assert!(app_server_broker_is_lifecycle_method(method), "{method}");
    }
    for method in &fixture.example_other_methods {
        assert!(!app_server_broker_is_lifecycle_method(method), "{method}");
        assert_eq!(
            app_server_broker_method_kind(Some(method)).label(),
            AppServerBrokerMethodKind::Other.label(),
            "{method}"
        );
    }
}

#[test]
fn app_server_broker_contract_matches_protocol_surface_fixture() {
    let fixture = app_server_broker_protocol_surface_fixture();
    let log_surface = app_server_broker_log_surface_fixture();
    let contract = app_server_broker_contract_json();
    assert_eq!(contract["jsonrpc"], fixture.jsonrpc_version);
    assert_eq!(
        contract["wire_omits_jsonrpc_header"],
        serde_json::Value::Bool(fixture.wire_omits_jsonrpc_header)
    );
    assert_eq!(
        contract["lifecycle_methods"],
        serde_json::to_value(&fixture.canonical_lifecycle_methods).unwrap()
    );
    assert_eq!(
        contract["accepted_lifecycle_aliases"],
        serde_json::to_value(&fixture.accepted_lifecycle_aliases).unwrap()
    );
    assert_eq!(
        contract["diagnostics"]["frame_kinds"],
        serde_json::to_value(&fixture.frame_kinds).unwrap()
    );
    assert_eq!(
        contract["diagnostics"]["method_kinds"],
        serde_json::to_value(&fixture.method_kinds).unwrap()
    );
    assert_eq!(
        contract["diagnostics"]["invalid_reasons"],
        serde_json::to_value(&fixture.invalid_reasons).unwrap()
    );
    assert_eq!(
        contract["diagnostics"]["max_preview_line_bytes"].as_u64(),
        Some(crate::app_server_broker::APP_SERVER_BROKER_MAX_PREVIEW_LINE_BYTES as u64)
    );
    assert_eq!(
        contract["diagnostics"]["turn_committed_hint"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["diagnostics"]["affinity_required_hint"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["diagnostics"]["rotation_allowed_hint"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["diagnostics"]["provider_switch_policy_hint"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["diagnostics"]["preserved_owner_kind_hint"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["diagnostics"]["preserves_owner_hint"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["diagnostics"]["policy_flag_counts"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["diagnostics"]["audit_preview_summary"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["diagnostics"]["stdio_validate_passthrough_fail_closed"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["diagnostics"]["stdio_validate_passthrough_preserves_valid_input"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["diagnostics"]["stdio_validate_fail_closed"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["diagnostics"]["request_response_id_validation"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["diagnostics"]["request_response_validation_reasons"],
        serde_json::json!([
            "request_missing_id",
            "response_missing_id",
            "response_without_request",
            "duplicate_pending_request_id",
            "pending_request_without_response",
            "lifecycle_response_missing_thread_id",
            "lifecycle_response_missing_thread_status",
            "lifecycle_response_invalid_thread_status",
            "lifecycle_response_missing_thread_context",
            "lifecycle_response_invalid_thread_context",
            "lifecycle_response_missing_thread_object_context",
            "lifecycle_response_missing_turn_id",
            "lifecycle_response_missing_turn_items",
            "lifecycle_response_missing_turn_status",
            "lifecycle_response_invalid_turn_status"
        ])
    );
    assert_eq!(
        contract["diagnostics"]["lifecycle_payload_validation"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["diagnostics"]["lifecycle_payload_validation_reasons"],
        serde_json::json!([
            "lifecycle_missing_thread_id",
            "lifecycle_missing_thread_object_id",
            "lifecycle_missing_thread_context",
            "lifecycle_missing_thread_status",
            "lifecycle_invalid_thread_status",
            "lifecycle_missing_turn_input",
            "lifecycle_missing_turn_items",
            "lifecycle_invalid_turn_status",
            "lifecycle_missing_turn_status"
        ])
    );
    assert_eq!(
        contract["diagnostics"]["lifecycle_consistency_validation"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["diagnostics"]["lifecycle_validation_reasons"],
        serde_json::json!([
            "turn_started_missing_turn_id",
            "turn_started_missing_thread_id",
            "turn_completed_missing_turn_id",
            "turn_completed_missing_thread_id",
            "turn_interrupt_missing_turn_id",
            "turn_interrupt_missing_thread_id",
            "turn_completed_without_turn_started",
            "turn_started_after_completed",
            "thread_active_turn_conflict",
            "turn_completed_not_active",
            "turn_interrupt_active_turn_conflict",
            "duplicate_turn_started",
            "duplicate_turn_completed"
        ])
    );
    assert_eq!(
        contract["cli"]["experimental_stdio_validate_passthrough"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["cli"]["experimental_stdio_live"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["cli"]["experimental_stdio_validate"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["audit"]["preview_summary"]["component"],
        serde_json::Value::String(log_surface.audit_event.component.clone())
    );
    assert_eq!(
        contract["audit"]["preview_summary"]["action"],
        serde_json::Value::String(log_surface.audit_event.action.clone())
    );
    assert_eq!(
        contract["audit"]["preview_summary"]["outcome"],
        serde_json::Value::String(log_surface.audit_event.outcome.clone())
    );
    assert_eq!(
        contract["audit"]["preview_summary"]["modes"],
        serde_json::to_value(&log_surface.audit_event.modes).unwrap()
    );
    assert_eq!(
        contract["audit"]["preview_summary"]["counts_only"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["audit"]["preview_summary"]["required_fields"],
        serde_json::to_value(&log_surface.audit_event.required_fields).unwrap()
    );
    assert_eq!(
        contract["affinity"]["thread_session_owner_required"],
        serde_json::Value::Bool(fixture.affinity.thread_session_owner_required)
    );
    assert_eq!(
        contract["affinity"]["continuation_affinity_wins"],
        serde_json::Value::Bool(fixture.affinity.continuation_affinity_wins)
    );
    assert_eq!(
        contract["affinity"]["rotate_only_before_turn_commit"],
        serde_json::Value::Bool(fixture.affinity.rotate_only_before_turn_commit)
    );
    assert_eq!(
        contract["affinity"]["decision_kinds"],
        serde_json::to_value(&fixture.affinity.decision_kinds).unwrap()
    );
    assert_eq!(
        contract["affinity"]["policy_modes"],
        serde_json::to_value(&fixture.affinity.policy_modes).unwrap()
    );
    assert_eq!(
        contract["affinity"]["routing_hints"],
        serde_json::to_value(&fixture.affinity.routing_hints).unwrap()
    );
    assert_eq!(
        contract["affinity"]["commit_boundaries"],
        serde_json::to_value(&fixture.affinity.commit_boundaries).unwrap()
    );
    assert_eq!(
        contract["affinity"]["rotation_windows"],
        serde_json::to_value(&fixture.affinity.rotation_windows).unwrap()
    );
    assert_eq!(
        contract["affinity"]["policy_modes"],
        serde_json::to_value(&fixture.affinity.policy_modes).unwrap()
    );
    assert_eq!(
        contract["affinity"]["routing_hints"],
        serde_json::to_value(&fixture.affinity.routing_hints).unwrap()
    );
    assert_eq!(
        contract["affinity"]["commit_boundaries"],
        serde_json::to_value(&fixture.affinity.commit_boundaries).unwrap()
    );
    assert_eq!(
        contract["affinity"]["rotation_windows"],
        serde_json::to_value(&fixture.affinity.rotation_windows).unwrap()
    );
    assert_eq!(
        contract["schema_validation"]["protocol_surface_fixture"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["fixture_drift_tests"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["helper_consistency_tests"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["stream_helper_consistency_tests"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["cross_surface_consistency_tests"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["report_aggregation_consistency_tests"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["metadata_surface_fixture"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["metadata_drift_tests"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["output_surface_fixture"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["output_drift_tests"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["runtime_log_surface_fixture"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["runtime_log_drift_tests"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["upstream_codex_schema_imported"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["lifecycle_response_schema_hints"],
        serde_json::Value::Bool(true)
    );
}

#[test]
fn app_server_broker_imported_upstream_codex_schema_fixtures_match_lifecycle_surface() {
    let thread_start = app_server_broker_upstream_thread_start_schema_fixture();
    let turn_start = app_server_broker_upstream_turn_start_schema_fixture();

    assert_eq!(thread_start["title"], "ThreadStartParams");
    assert_eq!(thread_start["type"], "object");
    assert_eq!(
        thread_start["properties"]
            .as_object()
            .expect("thread/start schema properties should be an object")
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from([
            "approvalPolicy",
            "approvalsReviewer",
            "baseInstructions",
            "config",
            "cwd",
            "developerInstructions",
            "ephemeral",
            "model",
            "modelProvider",
            "personality",
            "sandbox",
            "serviceName",
            "serviceTier",
            "sessionStartSource",
            "threadSource",
        ])
    );

    assert_eq!(turn_start["title"], "TurnStartParams");
    assert_eq!(turn_start["type"], "object");
    assert_eq!(
        turn_start["required"],
        serde_json::json!(["input", "threadId"])
    );
    assert_eq!(
        turn_start["properties"]
            .as_object()
            .expect("turn/start schema properties should be an object")
            .keys()
            .map(String::as_str)
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from([
            "approvalPolicy",
            "approvalsReviewer",
            "clientUserMessageId",
            "cwd",
            "effort",
            "input",
            "model",
            "outputSchema",
            "personality",
            "sandboxPolicy",
            "serviceTier",
            "summary",
            "threadId",
        ])
    );

    let lifecycle_methods = app_server_broker_lifecycle_methods();
    assert!(lifecycle_methods.contains(&"thread/start"));
    assert!(lifecycle_methods.contains(&"thread/started"));
    assert!(lifecycle_methods.contains(&"turn/start"));
    assert!(lifecycle_methods.contains(&"turn/started"));
    assert!(lifecycle_methods.contains(&"turn/completed"));
    assert!(lifecycle_methods.contains(&"turn/interrupt"));
}

#[test]
fn app_server_broker_upstream_lifecycle_schema_manifest_matches_imported_fixtures() {
    let fixture = app_server_broker_upstream_lifecycle_schema_manifest_fixture();
    assert_eq!(
        fixture.canonical_methods,
        app_server_broker_lifecycle_methods()
            .iter()
            .map(|method| method.to_string())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        fixture.accepted_aliases,
        vec![
            "notifications/initialized".to_string(),
            "turn/cancel".to_string()
        ]
    );

    for schema in fixture.lifecycle_schema_files {
        let contents = std::fs::read_to_string(format!(
            "{}/tests/fixtures/compat_replay/upstream_codex_schema/{}",
            env!("CARGO_MANIFEST_DIR"),
            schema.file
        ))
        .unwrap_or_else(|_| panic!("missing imported upstream schema fixture {}", schema.file));
        let value: serde_json::Value =
            serde_json::from_str(&contents).unwrap_or_else(|_| panic!("invalid JSON in {}", schema.file));
        assert_eq!(value["title"], schema.title, "{}", schema.file);
        assert_eq!(value["type"], "object", "{}", schema.file);
        assert_eq!(
            value.get("required").cloned(),
            schema.required.map(serde_json::Value::from),
            "{}",
            schema.file
        );
        assert!(
            app_server_broker_is_lifecycle_method(&schema.method),
            "{}",
            schema.method
        );
    }
}

#[test]
fn app_server_broker_lifecycle_schema_hints_match_imported_codex_fixtures() {
    let manifest = app_server_broker_upstream_lifecycle_schema_manifest_fixture();
    let files = manifest
        .lifecycle_schema_files
        .iter()
        .map(|schema| schema.file.as_str())
        .collect::<std::collections::BTreeSet<_>>();

    for (method, frame_kind, expected_file) in [
        (
            "thread/start",
            AppServerBrokerFrameKind::Request,
            "ThreadStartParams.json",
        ),
        (
            "thread/started",
            AppServerBrokerFrameKind::Notification,
            "ThreadStartedNotification.json",
        ),
        (
            "thread/resume",
            AppServerBrokerFrameKind::Request,
            "ThreadResumeParams.json",
        ),
        (
            "thread/fork",
            AppServerBrokerFrameKind::Request,
            "ThreadForkParams.json",
        ),
        (
            "turn/start",
            AppServerBrokerFrameKind::Request,
            "TurnStartParams.json",
        ),
        (
            "turn/started",
            AppServerBrokerFrameKind::Notification,
            "TurnStartedNotification.json",
        ),
        (
            "turn/completed",
            AppServerBrokerFrameKind::Notification,
            "TurnCompletedNotification.json",
        ),
        (
            "turn/interrupt",
            AppServerBrokerFrameKind::Request,
            "TurnInterruptParams.json",
        ),
        (
            "turn/cancel",
            AppServerBrokerFrameKind::Request,
            "TurnInterruptParams.json",
        ),
    ] {
        assert_eq!(
            app_server_broker_lifecycle_schema_file(Some(method), frame_kind),
            Some(expected_file),
            "{method}"
        );
        assert!(files.contains(expected_file), "{expected_file}");
        let mut payload = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
        });
        if matches!(frame_kind, AppServerBrokerFrameKind::Request) {
            payload["id"] = serde_json::json!(format!("req-{method}"));
        }
        assert_eq!(
            app_server_broker_diagnostic_summary_json(&payload)["lifecycle_schema_file"],
            serde_json::Value::String(expected_file.to_string()),
            "{method}"
        );
    }

    assert_eq!(
        app_server_broker_lifecycle_schema_file(
            Some("notifications/initialized"),
            AppServerBrokerFrameKind::Notification
        ),
        None
    );
    assert_eq!(
        app_server_broker_lifecycle_schema_file(Some("turn/start"), AppServerBrokerFrameKind::Response),
        None
    );
}

#[test]
fn app_server_broker_lifecycle_schema_hint_skips_response_error_frames() {
    let replay = "{\"jsonrpc\":\"2.0\",\"id\":\"req-thread-start\",\"method\":\"thread/start\",\"params\":{\"cwd\":\"/workspace\",\"model\":\"gpt-5\",\"modelProvider\":\"openai\",\"approvalPolicy\":\"never\",\"approvalsReviewer\":\"user\",\"ephemeral\":false}}\n\
{\"jsonrpc\":\"2.0\",\"id\":\"req-thread-start\",\"error\":{\"code\":-32000,\"message\":\"failed\"}}\n";
    let previews = app_server_broker_preview_lines(replay);
    assert_eq!(previews.len(), 2);
    assert_eq!(previews[0]["preview"]["summary"]["frame_kind"], "request");
    assert_eq!(previews[0]["preview"]["parse_ok"], serde_json::Value::Bool(true));
    assert_eq!(
        previews[0]["preview"]["summary"]["lifecycle_schema_file"],
        serde_json::Value::String("ThreadStartParams.json".to_string())
    );
    assert_eq!(previews[1]["preview"]["summary"]["frame_kind"], "response");
    assert_eq!(previews[1]["preview"]["parse_ok"], serde_json::Value::Bool(true));
    assert_eq!(previews[1]["preview"]["summary"]["lifecycle_schema_file"], serde_json::Value::Null);
}

#[test]
fn app_server_broker_upstream_schema_cases_match_imported_schema_required_fields() {
    for case in app_server_broker_upstream_schema_cases() {
        assert_payload_body_matches_upstream_schema_case(
            &case.payload,
            &case.schema_file,
            &case.body_field,
            &case.name,
        );
    }
}

#[test]
fn app_server_broker_upstream_schema_manifest_entries_have_replay_cases() {
    let manifest = app_server_broker_upstream_lifecycle_schema_manifest_fixture();
    let cases = app_server_broker_upstream_schema_cases();

    for schema in manifest.lifecycle_schema_files {
        assert!(
            cases.iter().any(|case| case.schema_file == schema.file),
            "missing replay case for imported upstream schema fixture {}",
            schema.file
        );
    }
}

#[test]
fn app_server_broker_upstream_schema_subset_rejects_invalid_enum_and_type_values() {
    let mut cases = app_server_broker_upstream_schema_cases();
    let thread_start = cases
        .iter_mut()
        .find(|case| case.name == "thread_start_request_minimal_wire")
        .expect("thread start schema case should exist");
    thread_start.payload["params"]["approvalPolicy"] = serde_json::json!(true);

    let failure = std::panic::catch_unwind(|| {
        assert_payload_body_matches_upstream_schema_case(
            &thread_start.payload,
            &thread_start.schema_file,
            &thread_start.body_field,
            &thread_start.name,
        );
    });
    assert!(failure.is_err(), "invalid schema case should fail validation");

    let mut turn_cases = app_server_broker_upstream_schema_cases();
    let turn_completed = turn_cases
        .iter_mut()
        .find(|case| case.name == "turn_completed_notification_minimal_wire")
        .expect("turn completed schema case should exist");
    turn_completed.payload["params"]["turn"]["status"] = serde_json::json!("bogus");

    let failure = std::panic::catch_unwind(|| {
        assert_payload_body_matches_upstream_schema_case(
            &turn_completed.payload,
            &turn_completed.schema_file,
            &turn_completed.body_field,
            &turn_completed.name,
        );
    });
    assert!(failure.is_err(), "invalid enum value should fail validation");
}

#[test]
fn app_server_broker_upstream_schema_replay_matches_preview_and_stream_helpers() {
    let replay = app_server_broker_upstream_schema_replay_fixture();
    let cases = app_server_broker_upstream_schema_cases();
    let previews = app_server_broker_preview_lines(replay);
    assert_eq!(previews.len(), cases.len());

    for (index, (preview, case)) in previews.iter().zip(cases.iter()).enumerate() {
        assert_eq!(
            preview["object"],
            serde_json::Value::String("app_server_broker.preview_event".to_string())
        );
        assert_eq!(preview["line"].as_u64(), Some((index + 1) as u64));
        assert_eq!(preview["preview"]["parse_ok"], serde_json::Value::Bool(true));
        let payload = serde_json::from_str::<serde_json::Value>(
            replay
                .lines()
                .nth(index)
                .unwrap_or_else(|| panic!("missing replay line {index}")),
        )
        .unwrap_or_else(|_| panic!("replay line {index} should parse"));
        assert_eq!(payload, case.payload, "replay case ordering should stay stable");
        assert_payload_body_matches_upstream_schema_case(
            &payload,
            &case.schema_file,
            &case.body_field,
            &format!("replay_case_{}", case.name),
        );
    }

    let report = app_server_broker_preview_report_json(replay);
    assert_eq!(report["report"]["line_count"].as_u64(), Some(cases.len() as u64));
    assert_eq!(report["report"]["parsed_count"].as_u64(), Some(cases.len() as u64));
    assert_eq!(report["report"]["error_count"].as_u64(), Some(0));
    assert_eq!(report["report"]["frame_kind_counts"]["request"].as_u64(), Some(5));
    assert_eq!(
        report["report"]["frame_kind_counts"]["notification"].as_u64(),
        Some(3)
    );
    assert_eq!(report["report"]["frame_kind_counts"]["response"].as_u64(), Some(5));
    assert_eq!(report["report"]["frame_kind_counts"]["invalid"].as_u64(), Some(0));
    assert_eq!(
        report["report"]["method_kind_counts"]["lifecycle"].as_u64(),
        Some(8)
    );
    assert_eq!(report["report"]["method_kind_counts"]["other"].as_u64(), Some(0));
    assert_eq!(report["report"]["method_kind_counts"]["absent"].as_u64(), Some(5));
    assert_eq!(
        report["report"]["invalid_reason_counts"]["non_jsonrpc_version"].as_u64(),
        Some(0)
    );
    assert_eq!(
        report["report"]["invalid_reason_counts"]["non_object_frame"].as_u64(),
        Some(0)
    );
    assert_eq!(
        report["report"]["invalid_reason_counts"]["batch_frame_unsupported"].as_u64(),
        Some(0)
    );
    assert_eq!(
        report["report"]["invalid_reason_counts"]["non_string_method"].as_u64(),
        Some(0)
    );
    assert_eq!(
        report["report"]["invalid_reason_counts"]["invalid_method_name"].as_u64(),
        Some(0)
    );
    assert_eq!(
        report["report"]["invalid_reason_counts"]["non_scalar_id"].as_u64(),
        Some(0)
    );
    assert_eq!(
        report["report"]["invalid_reason_counts"]["non_container_params"].as_u64(),
        Some(0)
    );
    assert_eq!(
        report["report"]["invalid_reason_counts"]["non_object_error"].as_u64(),
        Some(0)
    );
    assert_eq!(
        report["report"]["invalid_reason_counts"]["non_integer_error_code"].as_u64(),
        Some(0)
    );
    assert_eq!(
        report["report"]["invalid_reason_counts"]["non_string_error_message"].as_u64(),
        Some(0)
    );
    assert_eq!(
        report["report"]["invalid_reason_counts"]["result_with_error"].as_u64(),
        Some(0)
    );
    assert_eq!(
        report["report"]["invalid_reason_counts"]["missing_response_id"].as_u64(),
        Some(0)
    );
    assert_eq!(
        report["report"]["invalid_reason_counts"]["method_with_result_or_error"].as_u64(),
        Some(0)
    );
    assert_eq!(
        report["report"]["invalid_reason_counts"]["missing_method_and_response_payload"].as_u64(),
        Some(0)
    );

    let mut output = Vec::new();
    app_server_broker_write_stdio_preview_stream(
        std::io::Cursor::new(replay.as_bytes()),
        &mut output,
    )
    .expect("schema replay stream should render");
    let streamed = String::from_utf8(output).expect("schema replay stream should be utf-8");
    let streamed_lines = streamed.lines().filter(|line| !line.trim().is_empty()).count();
    assert_eq!(streamed_lines, cases.len() + 1, "preview stream should end with one report");
}

#[test]
fn app_server_broker_render_stdio_preview_matches_upstream_schema_replay_fixture() {
    let rendered =
        app_server_broker_render_stdio_preview(app_server_broker_upstream_schema_replay_fixture())
            .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(parsed["object"], "app_server_broker.preview_report");
    assert_eq!(
        parsed["report"],
        app_server_broker_upstream_schema_expected_report_fixture()
    );
}

#[test]
fn app_server_broker_write_stdio_preview_stream_matches_upstream_schema_stream_fixture() {
    let replay = app_server_broker_upstream_schema_replay_fixture();
    let mut output = Vec::new();

    app_server_broker_write_stdio_preview_stream(std::io::Cursor::new(replay), &mut output)
        .unwrap();

    let rendered = String::from_utf8(output).unwrap();
    assert_eq!(rendered, app_server_broker_upstream_schema_expected_stream_fixture());
}

#[test]
fn app_server_broker_write_stdio_passthrough_preview_stream_matches_upstream_schema_fixtures() {
    let replay = app_server_broker_upstream_schema_replay_fixture();
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_passthrough_preview_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .unwrap();

    assert_eq!(
        String::from_utf8(passthrough).unwrap(),
        app_server_broker_upstream_schema_passthrough_expected_stdout_fixture()
    );
    assert_eq!(
        String::from_utf8(diagnostics).unwrap(),
        app_server_broker_upstream_schema_passthrough_expected_stderr_fixture()
    );
}

#[test]
fn app_server_broker_write_stdio_passthrough_preview_stream_matches_malformed_fixtures() {
    let replay = app_server_broker_stdio_preview_malformed_replay_fixture();
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_passthrough_preview_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .unwrap();

    assert_eq!(String::from_utf8(passthrough).unwrap(), replay);
    assert_eq!(
        String::from_utf8(diagnostics).unwrap(),
        app_server_broker_stdio_preview_malformed_expected_stream_fixture()
    );
}

#[test]
fn app_server_broker_write_stdio_validate_stream_does_not_hint_response_errors() {
    let replay = "{\"jsonrpc\":\"2.0\",\"id\":\"req-thread-start\",\"method\":\"thread/start\",\"params\":{\"cwd\":\"/workspace\",\"model\":\"gpt-5\",\"modelProvider\":\"openai\",\"approvalPolicy\":\"never\",\"approvalsReviewer\":\"user\",\"ephemeral\":false}}\n\
{\"jsonrpc\":\"2.0\",\"id\":\"req-thread-start\",\"error\":{\"code\":-32000,\"message\":\"failed\"}}\n";
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
        .expect("response-error replay should still validate");

    let diagnostics_text = String::from_utf8(diagnostics).unwrap();
    let lines: Vec<&str> = diagnostics_text.lines().collect();
    assert_eq!(lines.len(), 3);
    let request_preview: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    let response_preview: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    let report: serde_json::Value = serde_json::from_str(lines[2]).unwrap();

    assert_eq!(
        request_preview["preview"]["summary"]["lifecycle_schema_file"],
        serde_json::Value::String("ThreadStartParams.json".to_string())
    );
    assert_eq!(response_preview["preview"]["summary"]["frame_kind"], "response");
    assert_eq!(
        response_preview["preview"]["summary"]["lifecycle_schema_file"],
        serde_json::Value::Null
    );
    assert_eq!(report["report"]["error_count"].as_u64(), Some(0));
}

#[test]
fn app_server_broker_write_stdio_validate_stream_accepts_valid_schema_replay() {
    let replay = app_server_broker_upstream_schema_replay_fixture();
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
        .expect("valid schema replay should validate");

    let rendered = String::from_utf8(diagnostics).unwrap();
    assert_eq!(
        rendered,
        app_server_broker_upstream_schema_expected_stream_fixture()
    );
}

#[test]
fn app_server_broker_write_stdio_validate_stream_rejects_malformed_replay() {
    let replay = app_server_broker_stdio_preview_malformed_replay_fixture();
    let mut diagnostics = Vec::new();

    let err =
        app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
            .expect_err("malformed replay should fail closed");

    assert!(
        err.to_string()
            .contains("app-server broker validation failed"),
        "{err}"
    );
    assert_eq!(
        String::from_utf8(diagnostics).unwrap(),
        app_server_broker_stdio_preview_malformed_expected_stream_fixture()
    );
}

#[test]
fn app_server_broker_write_stdio_validate_passthrough_stream_accepts_valid_schema_replay() {
    let replay = app_server_broker_upstream_schema_replay_fixture();
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect("valid schema replay should validate before passthrough");

    assert_eq!(
        String::from_utf8(passthrough).unwrap(),
        app_server_broker_upstream_schema_passthrough_expected_stdout_fixture()
    );
    assert_eq!(
        String::from_utf8(diagnostics).unwrap(),
        app_server_broker_upstream_schema_passthrough_expected_stderr_fixture()
    );
}

#[test]
fn app_server_broker_write_stdio_validate_passthrough_stream_blocks_malformed_replay() {
    let replay = "\
{\"jsonrpc\":\"2.0\",\"id\":\"req-1\",\"method\":\"custom/ping\",\"params\":{}}\n\
\n\
{\"jsonrpc\":\"2.0\"\n\
{\"jsonrpc\":\"2.0\",\"id\":\"resp-1\",\"result\":{\"ok\":true}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    let err = app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect_err("malformed replay should fail before passthrough");

    assert!(
        err.chain().any(|cause| cause
            .to_string()
            .contains("app-server broker validation failed before passthrough")),
        "{err}"
    );
    assert_eq!(
        String::from_utf8(passthrough).unwrap(),
        "{\"jsonrpc\":\"2.0\",\"id\":\"req-1\",\"method\":\"custom/ping\",\"params\":{}}\n\n"
    );
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"error\":\"invalid_json\""));
    assert!(rendered.contains("\"line\":3"));
    assert!(!rendered.contains("resp-1"));
}

#[test]
fn app_server_broker_write_stdio_validate_passthrough_stream_blocks_invalid_frame() {
    let replay = "{\"jsonrpc\":\"2.0\",\"id\":\"req-1\",\"method\":\"turn/start\",\"result\":{}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    let err = app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect_err("invalid JSON-RPC frame should fail before passthrough");

    assert!(err.to_string().contains("invalid_frame_count=1"), "{err}");
    assert!(passthrough.is_empty());
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"frame_kind\":\"invalid\""));
    assert!(rendered.contains("\"method_with_result_or_error\""));
}

#[test]
fn app_server_broker_write_stdio_validate_stream_rejects_response_without_request() {
    let replay = "{\"id\":\"orphan-response\",\"result\":{\"ok\":true}}\n";
    let mut diagnostics = Vec::new();

    let err =
        app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
            .expect_err("response without observed request id should fail validation");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker request/response validation failed"),
        "{message}"
    );
    assert!(message.contains("response_without_request"), "{message}");
    assert!(message.contains("orphan-response"), "{message}");
}

#[test]
fn app_server_broker_write_stdio_validate_passthrough_stream_blocks_response_without_request() {
    let replay = "{\"id\":\"orphan-response\",\"result\":{\"ok\":true}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    let err = app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect_err("response without observed request id should fail before passthrough");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker request/response validation failed before passthrough"),
        "{message}"
    );
    assert!(message.contains("response_without_request"), "{message}");
    assert!(passthrough.is_empty());
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"frame_kind\":\"response\""));
}

#[test]
fn app_server_broker_write_stdio_validate_stream_rejects_duplicate_pending_request_id() {
    let replay = "\
{\"id\":\"req-1\",\"method\":\"thread/start\",\"params\":{}}\n\
{\"id\":\"req-1\",\"method\":\"thread/resume\",\"params\":{\"threadId\":\"thr_1\"}}\n";
    let mut diagnostics = Vec::new();

    let err =
        app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
            .expect_err("duplicate pending request id should fail validation");

    let message = err.to_string();
    assert!(message.contains("duplicate_pending_request_id"), "{message}");
    assert!(message.contains("req-1"), "{message}");
}

#[test]
fn app_server_broker_write_stdio_validate_stream_allows_request_id_reuse_after_response() {
    let replay = "\
{\"id\":\"req-1\",\"method\":\"custom/ping\",\"params\":{}}\n\
{\"id\":\"req-1\",\"result\":{\"ok\":true}}\n\
{\"id\":\"req-1\",\"method\":\"custom/ping\",\"params\":{}}\n\
{\"id\":\"req-1\",\"result\":{\"ok\":true}}\n";
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
        .expect("request id can be reused after its response closes the pending entry");

    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"line_count\":4"));
}

#[test]
fn app_server_broker_write_stdio_validate_stream_rejects_pending_request_at_eof() {
    let replay = "{\"id\":\"req-pending\",\"method\":\"custom/ping\",\"params\":{}}\n";
    let mut diagnostics = Vec::new();

    let err =
        app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
            .expect_err("pending request without response should fail at EOF");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker request/response validation failed"),
        "{message}"
    );
    assert!(message.contains("pending_request_without_response"), "{message}");
    assert!(message.contains("req-pending"), "{message}");
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"line_count\":1"));
}

#[test]
fn app_server_broker_write_stdio_validate_stream_rejects_thread_started_without_thread_object_id() {
    let replay = "{\"method\":\"thread/started\",\"params\":{\"threadId\":\"thr_1\"}}\n";
    let mut diagnostics = Vec::new();

    let err =
        app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
            .expect_err("thread/started without params.thread.id should fail payload validation");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker lifecycle payload validation failed"),
        "{message}"
    );
    assert!(
        message.contains("lifecycle_missing_thread_object_id"),
        "{message}"
    );
    assert!(message.contains("thread_started_notification"), "{message}");
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"lifecycle_stage\":\"thread_started_notification\""));
}

#[test]
fn app_server_broker_write_stdio_validate_passthrough_stream_blocks_thread_started_without_thread_object_id()
{
    let replay = "{\"method\":\"thread/started\",\"params\":{\"threadId\":\"thr_1\"}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    let err = app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect_err("thread/started without params.thread.id should fail before passthrough");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker lifecycle payload validation failed before passthrough"),
        "{message}"
    );
    assert!(
        message.contains("lifecycle_missing_thread_object_id"),
        "{message}"
    );
    assert!(passthrough.is_empty());
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"lifecycle_stage\":\"thread_started_notification\""));
}

#[test]
fn app_server_broker_write_stdio_validate_stream_rejects_thread_started_with_invalid_thread_status()
{
    let replay =
        "{\"method\":\"thread/started\",\"params\":{\"thread\":{\"id\":\"thr_1\",\"status\":{\"type\":\"bogus\"}}}}\n";
    let mut diagnostics = Vec::new();

    let err =
        app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
            .expect_err("thread/started with invalid thread status should fail payload validation");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker lifecycle payload validation failed"),
        "{message}"
    );
    assert!(
        message.contains("lifecycle_invalid_thread_status"),
        "{message}"
    );
    assert!(message.contains("thread_started_notification"), "{message}");
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"lifecycle_stage\":\"thread_started_notification\""));
}

#[test]
fn app_server_broker_write_stdio_validate_stream_rejects_active_thread_started_without_active_flags()
{
    let replay =
        "{\"method\":\"thread/started\",\"params\":{\"thread\":{\"id\":\"thr_1\",\"status\":{\"type\":\"active\"}}}}\n";
    let mut diagnostics = Vec::new();

    let err =
        app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
            .expect_err("active thread status without activeFlags should fail payload validation");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker lifecycle payload validation failed"),
        "{message}"
    );
    assert!(
        message.contains("lifecycle_invalid_thread_status"),
        "{message}"
    );
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"lifecycle_stage\":\"thread_started_notification\""));
}

#[test]
fn app_server_broker_write_stdio_validate_stream_rejects_thread_started_without_thread_context() {
    let replay =
        "{\"method\":\"thread/started\",\"params\":{\"thread\":{\"id\":\"thr_1\",\"status\":{\"type\":\"idle\"}}}}\n";
    let mut diagnostics = Vec::new();

    let err =
        app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
            .expect_err("thread/started without required thread context should fail validation");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker lifecycle payload validation failed"),
        "{message}"
    );
    assert!(
        message.contains("lifecycle_missing_thread_context"),
        "{message}"
    );
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"lifecycle_stage\":\"thread_started_notification\""));
}

#[test]
fn app_server_broker_write_stdio_validate_stream_allows_active_thread_status_with_active_flags() {
    let replay =
        "{\"method\":\"thread/started\",\"params\":{\"thread\":{\"cliVersion\":\"0.0.0-test\",\"createdAt\":1,\"cwd\":\"/workspace\",\"ephemeral\":false,\"id\":\"thr_1\",\"modelProvider\":\"openai\",\"preview\":\"\",\"sessionId\":\"sess_1\",\"source\":\"cli\",\"status\":{\"type\":\"active\",\"activeFlags\":[\"waitingOnApproval\"]},\"turns\":[],\"updatedAt\":1}}}\n";
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
        .expect("active thread status with activeFlags should pass payload validation");

    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"line_count\":1"));
}

#[test]
fn app_server_broker_write_stdio_validate_stream_allows_thread_started_with_custom_source() {
    let replay =
        "{\"method\":\"thread/started\",\"params\":{\"thread\":{\"cliVersion\":\"0.0.0-test\",\"createdAt\":1,\"cwd\":\"/workspace\",\"ephemeral\":false,\"id\":\"thr_1\",\"modelProvider\":\"openai\",\"preview\":\"\",\"sessionId\":\"sess_1\",\"source\":{\"custom\":\"integration\"},\"status\":{\"type\":\"idle\"},\"turns\":[],\"updatedAt\":1}}}\n";
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
        .expect("custom session source object should pass thread payload validation");

    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"line_count\":1"));
}

#[test]
fn app_server_broker_write_stdio_validate_passthrough_stream_fails_pending_request_at_eof() {
    let replay = "{\"id\":\"req-pending\",\"method\":\"custom/ping\",\"params\":{}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    let err = app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect_err("pending request without response should fail at EOF");

    let message = err.to_string();
    assert!(message.contains("request/response validation failed at EOF"), "{message}");
    assert!(message.contains("pending_request_without_response"), "{message}");
    assert_eq!(String::from_utf8(passthrough).unwrap(), replay);
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"line_count\":1"));
}

#[test]
fn app_server_broker_write_stdio_validate_stream_rejects_lifecycle_response_without_thread_id() {
    let replay = "\
{\"id\":\"req-thread-start\",\"method\":\"thread/start\",\"params\":{}}\n\
{\"id\":\"req-thread-start\",\"result\":{\"thread\":{}}}\n";
    let mut diagnostics = Vec::new();

    let err =
        app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
            .expect_err("thread lifecycle response without thread id should fail validation");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker request/response validation failed"),
        "{message}"
    );
    assert!(
        message.contains("lifecycle_response_missing_thread_id"),
        "{message}"
    );
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"line_count\":2"));
}

#[test]
fn app_server_broker_write_stdio_validate_passthrough_stream_blocks_lifecycle_response_without_thread_status()
{
    let replay = "\
{\"id\":\"req-thread-start\",\"method\":\"thread/start\",\"params\":{}}\n\
{\"id\":\"req-thread-start\",\"result\":{\"thread\":{\"id\":\"thr_1\"}}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    let err = app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect_err("thread lifecycle response without thread status should fail before passthrough");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker request/response validation failed before passthrough"),
        "{message}"
    );
    assert!(
        message.contains("lifecycle_response_missing_thread_status"),
        "{message}"
    );
    let forwarded = String::from_utf8(passthrough).unwrap();
    assert!(forwarded.contains("req-thread-start"));
    assert!(!forwarded.contains("\"id\":\"thr_1\""));
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"frame_kind\":\"response\""));
}

#[test]
fn app_server_broker_write_stdio_validate_passthrough_stream_blocks_active_lifecycle_response_without_active_flags()
{
    let replay = "\
{\"id\":\"req-thread-start\",\"method\":\"thread/start\",\"params\":{}}\n\
{\"id\":\"req-thread-start\",\"result\":{\"thread\":{\"id\":\"thr_1\",\"status\":{\"type\":\"active\"}}}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    let err = app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect_err("active thread response without activeFlags should fail before passthrough");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker request/response validation failed before passthrough"),
        "{message}"
    );
    assert!(
        message.contains("lifecycle_response_invalid_thread_status"),
        "{message}"
    );
    let forwarded = String::from_utf8(passthrough).unwrap();
    assert!(forwarded.contains("req-thread-start"));
    assert!(!forwarded.contains("\"type\":\"active\""));
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"frame_kind\":\"response\""));
}

#[test]
fn app_server_broker_write_stdio_validate_passthrough_stream_blocks_active_lifecycle_response_with_invalid_active_flag()
{
    let replay = "\
{\"id\":\"req-thread-start\",\"method\":\"thread/start\",\"params\":{}}\n\
{\"id\":\"req-thread-start\",\"result\":{\"thread\":{\"id\":\"thr_1\",\"status\":{\"type\":\"active\",\"activeFlags\":[\"bogus\"]}}}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    let err = app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect_err("active thread response with invalid active flag should fail before passthrough");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker request/response validation failed before passthrough"),
        "{message}"
    );
    assert!(
        message.contains("lifecycle_response_invalid_thread_status"),
        "{message}"
    );
    let forwarded = String::from_utf8(passthrough).unwrap();
    assert!(forwarded.contains("req-thread-start"));
    assert!(!forwarded.contains("bogus"));
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"frame_kind\":\"response\""));
}

#[test]
fn app_server_broker_write_stdio_validate_passthrough_stream_blocks_lifecycle_response_without_thread_context()
{
    let replay = "\
{\"id\":\"req-thread-start\",\"method\":\"thread/start\",\"params\":{}}\n\
{\"id\":\"req-thread-start\",\"result\":{\"thread\":{\"id\":\"thr_1\",\"status\":{\"type\":\"idle\"}}}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    let err = app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect_err("thread lifecycle response without top-level context should fail before passthrough");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker request/response validation failed before passthrough"),
        "{message}"
    );
    assert!(
        message.contains("lifecycle_response_missing_thread_context"),
        "{message}"
    );
    let forwarded = String::from_utf8(passthrough).unwrap();
    assert!(forwarded.contains("req-thread-start"));
    assert!(!forwarded.contains("\"id\":\"thr_1\""));
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"frame_kind\":\"response\""));
}

#[test]
fn app_server_broker_write_stdio_validate_passthrough_stream_blocks_lifecycle_response_with_invalid_thread_context()
{
    let replay = "\
{\"id\":\"req-thread-start\",\"method\":\"thread/start\",\"params\":{}}\n\
{\"id\":\"req-thread-start\",\"result\":{\"approvalPolicy\":\"never\",\"approvalsReviewer\":\"user\",\"cwd\":\"/workspace\",\"model\":\"gpt-5\",\"modelProvider\":\"openai\",\"sandbox\":{\"type\":\"bogus\"},\"thread\":{\"id\":\"thr_1\",\"status\":{\"type\":\"idle\"}}}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    let err = app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect_err("thread lifecycle response with invalid context should fail before passthrough");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker request/response validation failed before passthrough"),
        "{message}"
    );
    assert!(
        message.contains("lifecycle_response_invalid_thread_context"),
        "{message}"
    );
    let forwarded = String::from_utf8(passthrough).unwrap();
    assert!(forwarded.contains("req-thread-start"));
    assert!(!forwarded.contains("\"type\":\"bogus\""));
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"frame_kind\":\"response\""));
}

#[test]
fn app_server_broker_write_stdio_validate_passthrough_stream_blocks_lifecycle_response_without_thread_object_context()
{
    let replay = "\
{\"id\":\"req-thread-start\",\"method\":\"thread/start\",\"params\":{}}\n\
{\"id\":\"req-thread-start\",\"result\":{\"approvalPolicy\":\"never\",\"approvalsReviewer\":\"user\",\"cwd\":\"/workspace\",\"model\":\"gpt-5\",\"modelProvider\":\"openai\",\"sandbox\":{\"type\":\"dangerFullAccess\"},\"thread\":{\"id\":\"thr_1\",\"status\":{\"type\":\"idle\"}}}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    let err = app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect_err("thread lifecycle response without required thread object context should fail before passthrough");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker request/response validation failed before passthrough"),
        "{message}"
    );
    assert!(
        message.contains("lifecycle_response_missing_thread_object_context"),
        "{message}"
    );
    let forwarded = String::from_utf8(passthrough).unwrap();
    assert!(forwarded.contains("req-thread-start"));
    assert!(!forwarded.contains("\"id\":\"thr_1\""));
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"frame_kind\":\"response\""));
}

#[test]
fn app_server_broker_write_stdio_validate_passthrough_stream_blocks_lifecycle_response_without_turn_id()
{
    let replay = "\
{\"id\":\"req-turn-start\",\"method\":\"turn/start\",\"params\":{\"threadId\":\"thr_1\",\"input\":[]}}\n\
{\"id\":\"req-turn-start\",\"result\":{\"turn\":{}}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    let err = app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect_err("turn lifecycle response without turn id should fail before passthrough");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker request/response validation failed before passthrough"),
        "{message}"
    );
    assert!(message.contains("lifecycle_response_missing_turn_id"), "{message}");
    let forwarded = String::from_utf8(passthrough).unwrap();
    assert!(forwarded.contains("req-turn-start"));
    assert!(!forwarded.contains("\"turn\":{}"));
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"frame_kind\":\"response\""));
}

#[test]
fn app_server_broker_write_stdio_validate_stream_rejects_lifecycle_response_without_turn_status() {
    let replay = "\
{\"id\":\"req-turn-start\",\"method\":\"turn/start\",\"params\":{\"threadId\":\"thr_1\",\"input\":[]}}\n\
{\"id\":\"req-turn-start\",\"result\":{\"turn\":{\"id\":\"turn_1\"}}}\n";
    let mut diagnostics = Vec::new();

    let err =
        app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
            .expect_err("turn lifecycle response without turn status should fail validation");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker request/response validation failed"),
        "{message}"
    );
    assert!(
        message.contains("lifecycle_response_missing_turn_status"),
        "{message}"
    );
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"frame_kind\":\"response\""));
}

#[test]
fn app_server_broker_write_stdio_validate_passthrough_stream_blocks_lifecycle_response_with_invalid_turn_status()
{
    let replay = "\
{\"id\":\"req-turn-start\",\"method\":\"turn/start\",\"params\":{\"threadId\":\"thr_1\",\"input\":[]}}\n\
{\"id\":\"req-turn-start\",\"result\":{\"turn\":{\"id\":\"turn_1\",\"status\":\"bogus\"}}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    let err = app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect_err("turn lifecycle response with invalid turn status should fail before passthrough");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker request/response validation failed before passthrough"),
        "{message}"
    );
    assert!(
        message.contains("lifecycle_response_invalid_turn_status"),
        "{message}"
    );
    let forwarded = String::from_utf8(passthrough).unwrap();
    assert!(forwarded.contains("req-turn-start"));
    assert!(!forwarded.contains("bogus"));
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"frame_kind\":\"response\""));
}

#[test]
fn app_server_broker_write_stdio_validate_passthrough_stream_blocks_lifecycle_response_without_turn_items()
{
    let replay = "\
{\"id\":\"req-turn-start\",\"method\":\"turn/start\",\"params\":{\"threadId\":\"thr_1\",\"input\":[]}}\n\
{\"id\":\"req-turn-start\",\"result\":{\"turn\":{\"id\":\"turn_1\",\"status\":\"inProgress\"}}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    let err = app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect_err("turn lifecycle response without turn items should fail before passthrough");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker request/response validation failed before passthrough"),
        "{message}"
    );
    assert!(
        message.contains("lifecycle_response_missing_turn_items"),
        "{message}"
    );
    let forwarded = String::from_utf8(passthrough).unwrap();
    assert!(forwarded.contains("req-turn-start"));
    assert!(!forwarded.contains("\"turn_1\""));
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"frame_kind\":\"response\""));
}

#[test]
fn app_server_broker_write_stdio_validate_stream_rejects_turn_start_without_thread_id() {
    let replay =
        "{\"id\":\"req-turn-start\",\"method\":\"turn/start\",\"params\":{\"input\":[]}}\n";
    let mut diagnostics = Vec::new();

    let err =
        app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
            .expect_err("turn/start without thread id should fail lifecycle payload validation");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker lifecycle payload validation failed"),
        "{message}"
    );
    assert!(message.contains("lifecycle_missing_thread_id"), "{message}");
    assert!(message.contains("turn_start_request"), "{message}");
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"lifecycle_stage\":\"turn_start_request\""));
}

#[test]
fn app_server_broker_write_stdio_validate_passthrough_stream_blocks_turn_start_without_thread_id()
{
    let replay =
        "{\"id\":\"req-turn-start\",\"method\":\"turn/start\",\"params\":{\"input\":[]}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    let err = app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect_err("turn/start without thread id should fail before passthrough");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker lifecycle payload validation failed before passthrough"),
        "{message}"
    );
    assert!(message.contains("lifecycle_missing_thread_id"), "{message}");
    assert!(passthrough.is_empty());
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"lifecycle_stage\":\"turn_start_request\""));
}

#[test]
fn app_server_broker_write_stdio_validate_stream_rejects_turn_start_without_input() {
    let replay =
        "{\"id\":\"req-turn-start\",\"method\":\"turn/start\",\"params\":{\"threadId\":\"thr_1\"}}\n";
    let mut diagnostics = Vec::new();

    let err =
        app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
            .expect_err("turn/start without input should fail lifecycle payload validation");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker lifecycle payload validation failed"),
        "{message}"
    );
    assert!(message.contains("lifecycle_missing_turn_input"), "{message}");
    assert!(message.contains("turn_start_request"), "{message}");
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"lifecycle_stage\":\"turn_start_request\""));
}

#[test]
fn app_server_broker_write_stdio_validate_passthrough_stream_blocks_turn_start_without_input() {
    let replay =
        "{\"id\":\"req-turn-start\",\"method\":\"turn/start\",\"params\":{\"threadId\":\"thr_1\"}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    let err = app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect_err("turn/start without input should fail before passthrough");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker lifecycle payload validation failed before passthrough"),
        "{message}"
    );
    assert!(message.contains("lifecycle_missing_turn_input"), "{message}");
    assert!(passthrough.is_empty());
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"lifecycle_stage\":\"turn_start_request\""));
}

#[test]
fn app_server_broker_write_stdio_validate_stream_rejects_turn_started_without_status() {
    let replay =
        "{\"method\":\"turn/started\",\"params\":{\"threadId\":\"thr_1\",\"turn\":{\"id\":\"turn_1\",\"items\":[]}}}\n";
    let mut diagnostics = Vec::new();

    let err =
        app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
            .expect_err("turn/started without status should fail lifecycle payload validation");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker lifecycle payload validation failed"),
        "{message}"
    );
    assert!(message.contains("lifecycle_missing_turn_status"), "{message}");
    assert!(message.contains("turn_started_notification"), "{message}");
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"lifecycle_stage\":\"turn_started_notification\""));
}

#[test]
fn app_server_broker_write_stdio_validate_passthrough_stream_blocks_turn_completed_without_status()
{
    let replay = "\
{\"method\":\"turn/started\",\"params\":{\"threadId\":\"thr_1\",\"turn\":{\"id\":\"turn_1\",\"items\":[],\"status\":\"inProgress\"}}}\n\
{\"method\":\"turn/completed\",\"params\":{\"threadId\":\"thr_1\",\"turn\":{\"id\":\"turn_1\",\"items\":[]}}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    let err = app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect_err("turn/completed without status should fail before passthrough");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker lifecycle payload validation failed before passthrough"),
        "{message}"
    );
    assert!(message.contains("lifecycle_missing_turn_status"), "{message}");
    let forwarded = String::from_utf8(passthrough).unwrap();
    assert!(forwarded.contains("turn/started"));
    assert!(!forwarded.contains("turn/completed"));
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"lifecycle_stage\":\"turn_completed_notification\""));
}

#[test]
fn app_server_broker_write_stdio_validate_stream_rejects_turn_started_without_items() {
    let replay =
        "{\"method\":\"turn/started\",\"params\":{\"threadId\":\"thr_1\",\"turn\":{\"id\":\"turn_1\",\"status\":\"inProgress\"}}}\n";
    let mut diagnostics = Vec::new();

    let err =
        app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
            .expect_err("turn/started without items should fail lifecycle payload validation");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker lifecycle payload validation failed"),
        "{message}"
    );
    assert!(message.contains("lifecycle_missing_turn_items"), "{message}");
    assert!(message.contains("turn_started_notification"), "{message}");
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"lifecycle_stage\":\"turn_started_notification\""));
}

#[test]
fn app_server_broker_write_stdio_validate_stream_rejects_invalid_turn_status() {
    let replay =
        "{\"method\":\"turn/started\",\"params\":{\"threadId\":\"thr_1\",\"turn\":{\"id\":\"turn_1\",\"items\":[],\"status\":\"bogus\"}}}\n";
    let mut diagnostics = Vec::new();

    let err =
        app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
            .expect_err("turn notification with invalid status should fail lifecycle payload validation");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker lifecycle payload validation failed"),
        "{message}"
    );
    assert!(message.contains("lifecycle_invalid_turn_status"), "{message}");
    assert!(message.contains("turn_started_notification"), "{message}");
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"lifecycle_stage\":\"turn_started_notification\""));
}

#[test]
fn app_server_broker_write_stdio_validate_passthrough_stream_blocks_invalid_turn_status() {
    let replay = "\
{\"method\":\"turn/started\",\"params\":{\"threadId\":\"thr_1\",\"turn\":{\"id\":\"turn_1\",\"items\":[],\"status\":\"inProgress\"}}}\n\
{\"method\":\"turn/completed\",\"params\":{\"threadId\":\"thr_1\",\"turn\":{\"id\":\"turn_1\",\"items\":[],\"status\":\"bogus\"}}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    let err = app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect_err("turn notification with invalid status should fail before passthrough");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker lifecycle payload validation failed before passthrough"),
        "{message}"
    );
    assert!(message.contains("lifecycle_invalid_turn_status"), "{message}");
    let forwarded = String::from_utf8(passthrough).unwrap();
    assert!(forwarded.contains("turn/started"));
    assert!(!forwarded.contains("bogus"));
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"lifecycle_stage\":\"turn_completed_notification\""));
}

#[test]
fn app_server_broker_write_stdio_validate_stream_rejects_turn_completed_before_started() {
    let replay =
        "{\"method\":\"turn/completed\",\"params\":{\"threadId\":\"thr_1\",\"turn\":{\"id\":\"turn_1\",\"items\":[],\"status\":\"completed\"}}}\n";
    let mut diagnostics = Vec::new();

    let err =
        app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
            .expect_err("completed turn without started notification should fail lifecycle validation");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker lifecycle validation failed"),
        "{message}"
    );
    assert!(
        message.contains("turn_completed_without_turn_started"),
        "{message}"
    );
    assert!(message.contains("turn_id=turn_1"), "{message}");
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"lifecycle_stage\":\"turn_completed_notification\""));
}

#[test]
fn app_server_broker_write_stdio_validate_passthrough_stream_blocks_turn_completed_before_started()
{
    let replay =
        "{\"method\":\"turn/completed\",\"params\":{\"threadId\":\"thr_1\",\"turn\":{\"id\":\"turn_1\",\"items\":[],\"status\":\"completed\"}}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    let err = app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect_err("completed turn without started notification should fail before passthrough");

    let message = err.to_string();
    assert!(
        message.contains("app-server broker lifecycle validation failed before passthrough"),
        "{message}"
    );
    assert!(
        message.contains("turn_completed_without_turn_started"),
        "{message}"
    );
    assert!(passthrough.is_empty());
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"lifecycle_stage\":\"turn_completed_notification\""));
}

#[test]
fn app_server_broker_write_stdio_validate_stream_rejects_second_active_turn_on_same_thread() {
    let replay = "\
{\"method\":\"turn/started\",\"params\":{\"threadId\":\"thr_1\",\"turn\":{\"id\":\"turn_1\",\"items\":[],\"status\":\"inProgress\"}}}\n\
{\"method\":\"turn/started\",\"params\":{\"threadId\":\"thr_1\",\"turn\":{\"id\":\"turn_2\",\"items\":[],\"status\":\"inProgress\"}}}\n";
    let mut diagnostics = Vec::new();

    let err =
        app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
            .expect_err("second active turn on the same thread should fail lifecycle validation");

    let message = err.to_string();
    assert!(message.contains("thread_active_turn_conflict"), "{message}");
    assert!(message.contains("thread_id=thr_1"), "{message}");
    assert!(message.contains("active_turn_id=turn_1"), "{message}");
    assert!(message.contains("turn_id=turn_2"), "{message}");
}

#[test]
fn app_server_broker_write_stdio_validate_passthrough_stream_blocks_second_active_turn_before_passthrough()
{
    let replay = "\
{\"method\":\"turn/started\",\"params\":{\"threadId\":\"thr_1\",\"turn\":{\"id\":\"turn_1\",\"items\":[],\"status\":\"inProgress\"}}}\n\
{\"method\":\"turn/started\",\"params\":{\"threadId\":\"thr_1\",\"turn\":{\"id\":\"turn_2\",\"items\":[],\"status\":\"inProgress\"}}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    let err = app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect_err("second active turn on the same thread should fail before passthrough");

    let message = err.to_string();
    assert!(message.contains("thread_active_turn_conflict"), "{message}");
    let forwarded = String::from_utf8(passthrough).unwrap();
    assert!(forwarded.contains("turn_1"));
    assert!(!forwarded.contains("turn_2"));
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"turn_id\":\"turn_2\""));
}

#[test]
fn app_server_broker_write_stdio_validate_stream_allows_next_turn_after_completion() {
    let replay = "\
{\"method\":\"turn/started\",\"params\":{\"threadId\":\"thr_1\",\"turn\":{\"id\":\"turn_1\",\"items\":[],\"status\":\"inProgress\"}}}\n\
{\"method\":\"turn/completed\",\"params\":{\"threadId\":\"thr_1\",\"turn\":{\"id\":\"turn_1\",\"items\":[],\"status\":\"completed\"}}}\n\
{\"method\":\"turn/started\",\"params\":{\"threadId\":\"thr_1\",\"turn\":{\"id\":\"turn_2\",\"items\":[],\"status\":\"inProgress\"}}}\n\
{\"method\":\"turn/completed\",\"params\":{\"threadId\":\"thr_1\",\"turn\":{\"id\":\"turn_2\",\"items\":[],\"status\":\"completed\"}}}\n";
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
        .expect("completed turn should release active-turn validation state");

    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"line_count\":4"));
}

#[test]
fn app_server_broker_write_stdio_validate_stream_allows_next_turn_after_interrupt() {
    let replay = "\
{\"method\":\"turn/started\",\"params\":{\"threadId\":\"thr_1\",\"turn\":{\"id\":\"turn_1\",\"items\":[],\"status\":\"inProgress\"}}}\n\
{\"id\":\"interrupt-1\",\"method\":\"turn/interrupt\",\"params\":{\"threadId\":\"thr_1\",\"turnId\":\"turn_1\"}}\n\
{\"id\":\"interrupt-1\",\"result\":{}}\n\
{\"method\":\"turn/started\",\"params\":{\"threadId\":\"thr_1\",\"turn\":{\"id\":\"turn_2\",\"items\":[],\"status\":\"inProgress\"}}}\n";
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
        .expect("interrupt should release active-turn validation state");

    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"line_count\":4"));
}

#[test]
fn app_server_broker_write_stdio_validate_stream_rejects_interrupt_for_non_active_turn() {
    let replay = "\
{\"method\":\"turn/started\",\"params\":{\"threadId\":\"thr_1\",\"turn\":{\"id\":\"turn_1\",\"items\":[],\"status\":\"inProgress\"}}}\n\
{\"id\":\"interrupt-2\",\"method\":\"turn/interrupt\",\"params\":{\"threadId\":\"thr_1\",\"turnId\":\"turn_2\"}}\n";
    let mut diagnostics = Vec::new();

    let err =
        app_server_broker_write_stdio_validate_stream(std::io::Cursor::new(replay), &mut diagnostics)
            .expect_err("interrupt for a different active turn should fail lifecycle validation");

    let message = err.to_string();
    assert!(
        message.contains("turn_interrupt_active_turn_conflict"),
        "{message}"
    );
    assert!(message.contains("active_turn_id=turn_1"), "{message}");
    assert!(message.contains("turn_id=turn_2"), "{message}");
}

#[test]
fn app_server_broker_write_stdio_validate_passthrough_stream_blocks_interrupt_for_non_active_turn()
{
    let replay = "\
{\"method\":\"turn/started\",\"params\":{\"threadId\":\"thr_1\",\"turn\":{\"id\":\"turn_1\",\"items\":[],\"status\":\"inProgress\"}}}\n\
{\"id\":\"interrupt-2\",\"method\":\"turn/interrupt\",\"params\":{\"threadId\":\"thr_1\",\"turnId\":\"turn_2\"}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    let err = app_server_broker_write_stdio_validate_passthrough_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect_err("interrupt for a different active turn should fail before passthrough");

    let message = err.to_string();
    assert!(
        message.contains("turn_interrupt_active_turn_conflict"),
        "{message}"
    );
    let forwarded = String::from_utf8(passthrough).unwrap();
    assert!(forwarded.contains("turn_1"));
    assert!(!forwarded.contains("turn_2"));
    let rendered = String::from_utf8(diagnostics).unwrap();
    assert!(rendered.contains("\"lifecycle_stage\":\"turn_interrupt_request\""));
}

#[test]
fn app_server_broker_write_stdio_passthrough_preview_stream_matches_wire_fixtures() {
    let replay = app_server_broker_stdio_preview_wire_replay_fixture();
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_passthrough_preview_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .unwrap();

    assert_eq!(String::from_utf8(passthrough).unwrap(), replay);
    assert_eq!(
        String::from_utf8(diagnostics).unwrap(),
        app_server_broker_stdio_preview_wire_expected_stream_fixture()
    );
}

#[test]
fn app_server_broker_command_stdio_passthrough_preview_matches_upstream_schema_fixtures() {
    let replay = app_server_broker_upstream_schema_replay_fixture();
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    run_app_server_broker_stdio_passthrough_preview(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .unwrap();

    assert_eq!(
        String::from_utf8(passthrough).unwrap(),
        app_server_broker_upstream_schema_passthrough_expected_stdout_fixture()
    );
    assert_eq!(
        String::from_utf8(diagnostics).unwrap(),
        app_server_broker_upstream_schema_passthrough_expected_stderr_fixture()
    );
}

#[test]
fn app_server_broker_command_stdio_validate_passthrough_matches_upstream_schema_fixtures() {
    let replay = app_server_broker_upstream_schema_replay_fixture();
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    run_app_server_broker_stdio_validate_passthrough(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .unwrap();

    assert_eq!(
        String::from_utf8(passthrough).unwrap(),
        app_server_broker_upstream_schema_passthrough_expected_stdout_fixture()
    );
    assert_eq!(
        String::from_utf8(diagnostics).unwrap(),
        app_server_broker_upstream_schema_passthrough_expected_stderr_fixture()
    );
}

#[test]
fn app_server_broker_command_stdio_validate_passthrough_wraps_validation_failure() {
    let replay = app_server_broker_stdio_preview_malformed_replay_fixture();
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    let err = run_app_server_broker_stdio_validate_passthrough(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect_err("malformed replay should fail before passthrough");

    assert!(
        err.to_string()
            .contains("failed to validate app-server broker stdio passthrough stream"),
        "{err}"
    );
    assert!(!String::from_utf8(passthrough).unwrap().contains("resp-1"));
}

#[test]
fn app_server_broker_command_stdio_passthrough_preview_matches_malformed_fixtures() {
    let replay = app_server_broker_stdio_preview_malformed_replay_fixture();
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    run_app_server_broker_stdio_passthrough_preview(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .unwrap();

    assert_eq!(String::from_utf8(passthrough).unwrap(), replay);
    assert_eq!(
        String::from_utf8(diagnostics).unwrap(),
        app_server_broker_stdio_preview_malformed_expected_stream_fixture()
    );
}

#[test]
fn app_server_broker_command_stdio_passthrough_preview_matches_wire_fixtures() {
    let replay = app_server_broker_stdio_preview_wire_replay_fixture();
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    run_app_server_broker_stdio_passthrough_preview(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .unwrap();

    assert_eq!(String::from_utf8(passthrough).unwrap(), replay);
    assert_eq!(
        String::from_utf8(diagnostics).unwrap(),
        app_server_broker_stdio_preview_wire_expected_stream_fixture()
    );
}

#[test]
fn app_server_broker_write_stdio_passthrough_preview_stream_matches_lifecycle_fixtures() {
    let replay = app_server_broker_stdio_preview_lifecycle_replay_fixture();
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_passthrough_preview_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .unwrap();

    assert_eq!(
        String::from_utf8(passthrough).unwrap(),
        app_server_broker_stdio_preview_lifecycle_passthrough_expected_stdout_fixture()
    );
    assert_eq!(
        String::from_utf8(diagnostics).unwrap(),
        app_server_broker_stdio_preview_lifecycle_passthrough_expected_stderr_fixture()
    );
}

#[test]
fn app_server_broker_command_stdio_passthrough_preview_matches_lifecycle_fixtures() {
    let replay = app_server_broker_stdio_preview_lifecycle_replay_fixture();
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    run_app_server_broker_stdio_passthrough_preview(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .unwrap();

    assert_eq!(
        String::from_utf8(passthrough).unwrap(),
        app_server_broker_stdio_preview_lifecycle_passthrough_expected_stdout_fixture()
    );
    assert_eq!(
        String::from_utf8(diagnostics).unwrap(),
        app_server_broker_stdio_preview_lifecycle_passthrough_expected_stderr_fixture()
    );
}

#[test]
fn app_server_broker_render_stdio_preview_matches_lifecycle_replay_fixture() {
    let rendered = app_server_broker_render_stdio_preview(
        app_server_broker_stdio_preview_lifecycle_replay_fixture(),
    )
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(parsed["object"], "app_server_broker.preview_report");
    assert_eq!(
        parsed["report"],
        app_server_broker_stdio_preview_lifecycle_expected_report_fixture()
    );
}

#[test]
fn app_server_broker_write_stdio_preview_stream_matches_lifecycle_stream_fixture() {
    let replay = app_server_broker_stdio_preview_lifecycle_replay_fixture();
    let mut output = Vec::new();

    app_server_broker_write_stdio_preview_stream(std::io::Cursor::new(replay), &mut output)
        .unwrap();

    let rendered = String::from_utf8(output).unwrap();
    assert_eq!(
        rendered,
        app_server_broker_stdio_preview_lifecycle_expected_stream_fixture()
    );
}

#[test]
fn app_server_broker_upstream_schema_passthrough_runtime_log_matches_fixture() {
    let _runtime_lock = acquire_test_runtime_lock();
    let _env_lock = TestEnvVarGuard::lock();
    let log_dir = AppServerBrokerRuntimeLogTestDir::new();
    let _log_dir = TestEnvVarGuard::set(
        "PRODEX_RUNTIME_LOG_DIR",
        log_dir.path.to_str().expect("log dir should be utf-8"),
    );
    let replay = app_server_broker_upstream_schema_replay_fixture();
    let fixture = app_server_broker_upstream_schema_expected_runtime_log_fixture();
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_passthrough_preview_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect("passthrough preview should succeed");

    let pointer = std::fs::read_to_string(runtime_proxy_latest_log_pointer_path())
        .expect("latest runtime log pointer should exist");
    let log_path = std::path::PathBuf::from(pointer.trim());
    runtime_proxy_flush_logs_for_path(&log_path).expect("runtime log should flush");
    let log = std::fs::read_to_string(&log_path).expect("runtime log should be readable");

    let observe_lines = log
        .lines()
        .filter(|line| {
            runtime_proxy_crate::runtime_proxy_log_event(runtime_log_message_body(line))
                == Some("app_server_broker_observe")
        })
        .collect::<Vec<_>>();
    assert_eq!(observe_lines.len(), fixture.observe.len(), "log contents:\n{log}");

    for (index, expected_fields) in fixture.observe.iter().enumerate() {
        let fields = runtime_proxy_crate::runtime_proxy_log_fields(runtime_log_message_body(
            observe_lines[index],
        ));
        for (key, expected) in expected_fields {
            assert_eq!(fields.get(key), Some(expected), "observe[{index}] {key}");
        }
    }

    let summary_line = log
        .lines()
        .find(|line| {
            runtime_proxy_crate::runtime_proxy_log_event(runtime_log_message_body(line))
                == Some("app_server_broker_summary")
        })
        .expect("summary log line should exist");
    let summary_fields =
        runtime_proxy_crate::runtime_proxy_log_fields(runtime_log_message_body(summary_line));
    for (key, expected) in &fixture.summary {
        assert_eq!(summary_fields.get(key), Some(expected), "summary {key}");
    }
}

#[test]
fn app_server_broker_output_surface_fixture_matches_golden_outputs() {
    let fixture = app_server_broker_output_surface_fixture();
    assert_eq!(fixture.preview_event.object, "app_server_broker.preview_event");
    assert_eq!(fixture.preview_report.object, "app_server_broker.preview_report");

    let valid_report = app_server_broker_stdio_preview_expected_report_fixture();
    assert_preview_report_body_matches_surface(&valid_report, &fixture, "valid_report_fixture");
    assert_preview_report_counts_match_previews(&valid_report, "valid_report_fixture");

    let malformed_report = app_server_broker_stdio_preview_malformed_expected_report_fixture();
    assert_preview_report_body_matches_surface(
        &malformed_report,
        &fixture,
        "malformed_report_fixture",
    );
    assert_preview_report_counts_match_previews(&malformed_report, "malformed_report_fixture");

    for (name, stream) in [
        (
            "valid_stream_fixture",
            app_server_broker_stdio_preview_expected_stream_fixture(),
        ),
        (
            "malformed_stream_fixture",
            app_server_broker_stdio_preview_malformed_expected_stream_fixture(),
        ),
    ] {
        let lines = stream.lines().filter(|line| !line.trim().is_empty());
        for (index, line) in lines.enumerate() {
            let value: serde_json::Value =
                serde_json::from_str(line).unwrap_or_else(|_| panic!("{name}: line should parse"));
            if index + 1 == stream.lines().filter(|line| !line.trim().is_empty()).count() {
                assert_preview_report_matches_surface(&value, &fixture, name);
                assert_preview_report_counts_match_previews(&value["report"], name);
            } else {
                assert_preview_event_matches_surface(
                    &value,
                    &fixture,
                    &format!("{name}[{index}]"),
                );
            }
        }
    }
}

#[test]
fn app_server_broker_log_surface_fixture_matches_passthrough_log_output() {
    let _runtime_lock = acquire_test_runtime_lock();
    let _env_lock = TestEnvVarGuard::lock();
    let _format = TestEnvVarGuard::set("PRODEX_RUNTIME_LOG_FORMAT", "text");
    let fixture = app_server_broker_log_surface_fixture();
    let log_dir = AppServerBrokerRuntimeLogTestDir::new();
    let _log_dir = TestEnvVarGuard::set(
        "PRODEX_RUNTIME_LOG_DIR",
        log_dir.path.to_str().expect("log dir should be utf-8"),
    );
    let input = concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":\"req-1\",\"method\":\"turn/start\",\"params\":{\"session_id\":\"sess-1\",\"thread_id\":\"thread-1\",\"turn_id\":\"turn-1\"}}\n",
        "{\"jsonrpc\":\"2.0\"\n"
    );
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_passthrough_preview_stream(
        std::io::Cursor::new(input),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect("passthrough preview should succeed");
    assert_eq!(String::from_utf8(passthrough).unwrap(), input);

    let pointer = std::fs::read_to_string(runtime_proxy_latest_log_pointer_path())
        .expect("latest runtime log pointer should exist");
    let log_path = std::path::PathBuf::from(pointer.trim());
    runtime_proxy_flush_logs_for_path(&log_path).expect("runtime log should flush");
    let log = std::fs::read_to_string(&log_path).expect("runtime log should be readable");

    let observe_lines = log
        .lines()
        .filter(|line| {
            runtime_proxy_crate::runtime_proxy_log_event(runtime_log_message_body(line))
                == Some("app_server_broker_observe")
        })
        .collect::<Vec<_>>();
    assert_eq!(observe_lines.len(), 2, "log contents:\n{log}");

    let first_fields =
        runtime_proxy_crate::runtime_proxy_log_fields(runtime_log_message_body(observe_lines[0]));
    assert_log_fields_have_keys(
        &first_fields,
        &fixture.observe_event.required_fields,
        "observe_line[0]",
    );
    assert_log_fields_have_keys(
        &first_fields,
        fixture
            .observe_event
            .parsed_frame_required_fields
            .as_ref()
            .expect("parsed frame fields should exist"),
        "observe_line[0]",
    );
    assert_eq!(
        runtime_proxy_crate::runtime_proxy_log_event(runtime_log_message_body(observe_lines[0])),
        Some(fixture.observe_event.event.as_str())
    );
    assert_eq!(first_fields.get("frame_kind").map(String::as_str), Some("request"));
    assert_eq!(first_fields.get("method").map(String::as_str), Some("turn/start"));
    assert_eq!(
        first_fields.get("lifecycle_stage").map(String::as_str),
        Some("turn_start_request")
    );
    assert_eq!(
        first_fields.get("continuation_decision").map(String::as_str),
        Some("continue-turn")
    );
    assert_eq!(
        first_fields.get("policy_mode").map(String::as_str),
        Some("preserve-turn-affinity")
    );
    assert_eq!(
        first_fields.get("commit_boundary").map(String::as_str),
        Some("precommit")
    );
    assert_eq!(
        first_fields.get("rotation_window").map(String::as_str),
        Some("closed")
    );
    assert_eq!(
        first_fields.get("routing_hint").map(String::as_str),
        Some("preserve-turn-owner")
    );
    assert_eq!(
        first_fields.get("preserved_owner_kind").map(String::as_str),
        Some("turn")
    );
    assert_eq!(
        first_fields.get("affinity_required").map(String::as_str),
        Some("true")
    );
    assert_eq!(
        first_fields.get("rotation_allowed").map(String::as_str),
        Some("false")
    );
    assert_eq!(
        first_fields.get("provider_switch_allowed").map(String::as_str),
        Some("false")
    );
    assert_eq!(
        first_fields
            .get("provider_switch_requires_override")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        first_fields.get("turn_committed").map(String::as_str),
        Some("false")
    );
    assert_eq!(
        first_fields.get("preserves_owner").map(String::as_str),
        Some("true")
    );
    assert_eq!(first_fields.get("owner_kind").map(String::as_str), Some("turn"));
    assert_eq!(
        first_fields.get("affinity_key_count").map(String::as_str),
        Some("3")
    );
    assert_eq!(first_fields.get("session_id").map(String::as_str), Some("sess-1"));
    assert_eq!(first_fields.get("thread_id").map(String::as_str), Some("thread-1"));
    assert_eq!(first_fields.get("turn_id").map(String::as_str), Some("turn-1"));

    let second_fields =
        runtime_proxy_crate::runtime_proxy_log_fields(runtime_log_message_body(observe_lines[1]));
    assert_log_fields_have_keys(
        &second_fields,
        &fixture.observe_event.required_fields,
        "observe_line[1]",
    );
    assert_log_fields_have_keys(
        &second_fields,
        fixture
            .observe_event
            .parse_error_required_fields
            .as_ref()
            .expect("parse error fields should exist"),
        "observe_line[1]",
    );
    assert_eq!(
        second_fields.get("parse_ok").map(String::as_str),
        Some("false")
    );
    assert_eq!(
        second_fields.get("error").map(String::as_str),
        Some("invalid_json")
    );

    let summary_line = log
        .lines()
        .find(|line| {
            runtime_proxy_crate::runtime_proxy_log_event(runtime_log_message_body(line))
                == Some("app_server_broker_summary")
        })
        .expect("summary log line should exist");
    let summary_fields =
        runtime_proxy_crate::runtime_proxy_log_fields(runtime_log_message_body(summary_line));
    assert_eq!(
        runtime_proxy_crate::runtime_proxy_log_event(runtime_log_message_body(summary_line)),
        Some(fixture.summary_event.event.as_str())
    );
    assert_log_fields_have_keys(
        &summary_fields,
        &fixture.summary_event.required_fields,
        "summary_line",
    );
    assert_eq!(summary_fields.get("line_count").map(String::as_str), Some("2"));
    assert_eq!(summary_fields.get("parsed_count").map(String::as_str), Some("1"));
    assert_eq!(summary_fields.get("error_count").map(String::as_str), Some("1"));
    assert_eq!(
        summary_fields.get("fresh_selection_ok_count").map(String::as_str),
        Some("0")
    );
    assert_eq!(
        summary_fields
            .get("preserve_session_affinity_count")
            .map(String::as_str),
        Some("0")
    );
    assert_eq!(
        summary_fields
            .get("preserve_thread_affinity_count")
            .map(String::as_str),
        Some("0")
    );
    assert_eq!(
        summary_fields
            .get("preserve_turn_affinity_count")
            .map(String::as_str),
        Some("1")
    );
    assert_eq!(
        summary_fields.get("precommit_count").map(String::as_str),
        Some("1")
    );
    assert_eq!(
        summary_fields.get("turn_committed_count").map(String::as_str),
        Some("0")
    );
    assert_eq!(
        summary_fields.get("rotation_open_count").map(String::as_str),
        Some("0")
    );
    assert_eq!(
        summary_fields.get("rotation_closed_count").map(String::as_str),
        Some("1")
    );
    assert_eq!(
        summary_fields.get("fresh_select_ok_count").map(String::as_str),
        Some("0")
    );
    assert_eq!(
        summary_fields
            .get("preserve_session_owner_count")
            .map(String::as_str),
        Some("0")
    );
    assert_eq!(
        summary_fields
            .get("preserve_thread_owner_count")
            .map(String::as_str),
        Some("0")
    );
    assert_eq!(
        summary_fields
            .get("preserve_turn_owner_count")
            .map(String::as_str),
        Some("1")
    );
    assert_eq!(
        summary_fields
            .get("affinity_required_count")
            .map(String::as_str),
        Some("1")
    );
    assert_eq!(
        summary_fields
            .get("rotation_allowed_count")
            .map(String::as_str),
        Some("0")
    );
    assert_eq!(
        summary_fields
            .get("preserves_owner_count")
            .map(String::as_str),
        Some("1")
    );
}

#[test]
fn app_server_broker_stdio_preview_writes_audit_summary_event() {
    let _runtime_lock = acquire_test_runtime_lock();
    let _env_lock = TestEnvVarGuard::lock();
    let audit_dir = AppServerBrokerRuntimeLogTestDir::new();
    let _audit_dir = TestEnvVarGuard::set(
        "PRODEX_AUDIT_LOG_DIR",
        audit_dir.path.to_str().expect("audit dir should be utf-8"),
    );
    let input = concat!(
        r#"{"jsonrpc":"2.0","id":"req-1","method":"turn/start","params":{"session_id":"sess-1"}}"#,
        "
",
        r#"{"jsonrpc":"2.0""#,
        "
"
    );
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_preview_stream(std::io::Cursor::new(input), &mut diagnostics)
        .expect("stdio preview should succeed");

    assert_broker_preview_audit_event(&audit_dir.path, "stdio-preview", 2, 1, 1);
}

#[test]
fn app_server_broker_passthrough_preview_writes_audit_summary_event() {
    let _runtime_lock = acquire_test_runtime_lock();
    let _env_lock = TestEnvVarGuard::lock();
    let audit_dir = AppServerBrokerRuntimeLogTestDir::new();
    let _audit_dir = TestEnvVarGuard::set(
        "PRODEX_AUDIT_LOG_DIR",
        audit_dir.path.to_str().expect("audit dir should be utf-8"),
    );
    let input = concat!(
        r#"{"jsonrpc":"2.0","id":"req-1","method":"turn/start","params":{"session_id":"sess-1"}}"#,
        "
",
        r#"{"jsonrpc":"2.0""#,
        "
"
    );
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_passthrough_preview_stream(
        std::io::Cursor::new(input),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect("passthrough preview should succeed");

    assert_broker_preview_audit_event(&audit_dir.path, "stdio-passthrough-preview", 2, 1, 1);
}

#[test]
fn app_server_broker_passthrough_preview_runtime_log_summary_matches_diagnostics_summary() {
    let _runtime_lock = acquire_test_runtime_lock();
    let _env_lock = TestEnvVarGuard::lock();
    let _format = TestEnvVarGuard::set("PRODEX_RUNTIME_LOG_FORMAT", "text");
    let log_dir = AppServerBrokerRuntimeLogTestDir::new();
    let _log_dir = TestEnvVarGuard::set(
        "PRODEX_RUNTIME_LOG_DIR",
        log_dir.path.to_str().expect("log dir should be utf-8"),
    );
    let input = concat!(
        "{\"jsonrpc\":\"2.0\",\"id\":\"req-1\",\"method\":\"turn/start\",\"params\":{\"session_id\":\"sess-1\"}}\n",
        "{\"jsonrpc\":\"2.0\",\"id\":\"req-2\",\"method\":\"custom/ping\",\"params\":{\"context\":{\"turnId\":\"turn-2\"}}}\n",
        "{\"jsonrpc\":\"2.0\"\n"
    );
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_passthrough_preview_stream(
        std::io::Cursor::new(input),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect("passthrough preview should succeed");

    let rendered = String::from_utf8(diagnostics).unwrap();
    let lines: Vec<&str> = rendered.lines().collect();
    let diagnostics_summary: serde_json::Value =
        serde_json::from_str(lines.last().expect("summary line should exist")).unwrap();
    let report = &diagnostics_summary["report"];

    let pointer = std::fs::read_to_string(runtime_proxy_latest_log_pointer_path())
        .expect("latest runtime log pointer should exist");
    let log_path = std::path::PathBuf::from(pointer.trim());
    runtime_proxy_flush_logs_for_path(&log_path).expect("runtime log should flush");
    let log = std::fs::read_to_string(&log_path).expect("runtime log should be readable");

    let observe_lines = log
        .lines()
        .filter(|line| {
            runtime_proxy_crate::runtime_proxy_log_event(runtime_log_message_body(line))
                == Some("app_server_broker_observe")
        })
        .collect::<Vec<_>>();
    let summary_line = log
        .lines()
        .find(|line| {
            runtime_proxy_crate::runtime_proxy_log_event(runtime_log_message_body(line))
                == Some("app_server_broker_summary")
        })
        .expect("summary log line should exist");
    let summary_fields =
        runtime_proxy_crate::runtime_proxy_log_fields(runtime_log_message_body(summary_line));
    let line_count = report["line_count"].as_u64().unwrap().to_string();
    let parsed_count = report["parsed_count"].as_u64().unwrap().to_string();
    let error_count = report["error_count"].as_u64().unwrap().to_string();
    let fresh_selection_ok_count = report["policy_mode_counts"]["fresh-selection-ok"]
        .as_u64()
        .unwrap()
        .to_string();
    let preserve_session_affinity_count = report["policy_mode_counts"]
        ["preserve-session-affinity"]
        .as_u64()
        .unwrap()
        .to_string();
    let preserve_thread_affinity_count = report["policy_mode_counts"]
        ["preserve-thread-affinity"]
        .as_u64()
        .unwrap()
        .to_string();
    let preserve_turn_affinity_count = report["policy_mode_counts"]["preserve-turn-affinity"]
        .as_u64()
        .unwrap()
        .to_string();
    let precommit_count = report["commit_boundary_counts"]["precommit"]
        .as_u64()
        .unwrap()
        .to_string();
    let turn_committed_count = report["commit_boundary_counts"]["turn-committed"]
        .as_u64()
        .unwrap()
        .to_string();
    let rotation_open_count = report["rotation_window_counts"]["open"]
        .as_u64()
        .unwrap()
        .to_string();
    let rotation_closed_count = report["rotation_window_counts"]["closed"]
        .as_u64()
        .unwrap()
        .to_string();
    let fresh_select_ok_count = report["routing_hint_counts"]["fresh-select-ok"]
        .as_u64()
        .unwrap()
        .to_string();
    let preserve_session_owner_count = report["routing_hint_counts"]["preserve-session-owner"]
        .as_u64()
        .unwrap()
        .to_string();
    let preserve_thread_owner_count = report["routing_hint_counts"]["preserve-thread-owner"]
        .as_u64()
        .unwrap()
        .to_string();
    let preserve_turn_owner_count = report["routing_hint_counts"]["preserve-turn-owner"]
        .as_u64()
        .unwrap()
        .to_string();
    let affinity_required_count = report["policy_flag_counts"]["affinity_required"]
        .as_u64()
        .unwrap()
        .to_string();
    let rotation_allowed_count = report["policy_flag_counts"]["rotation_allowed"]
        .as_u64()
        .unwrap()
        .to_string();
    let preserves_owner_count = report["policy_flag_counts"]["preserves_owner"]
        .as_u64()
        .unwrap()
        .to_string();

    assert_eq!(
        observe_lines.len(),
        report["previews"].as_array().unwrap().len(),
        "observe lines should match diagnostics previews"
    );
    assert_eq!(
        summary_fields.get("line_count"),
        Some(&line_count)
    );
    assert_eq!(
        summary_fields.get("parsed_count"),
        Some(&parsed_count)
    );
    assert_eq!(
        summary_fields.get("error_count"),
        Some(&error_count)
    );
    assert_eq!(
        summary_fields.get("fresh_selection_ok_count"),
        Some(&fresh_selection_ok_count)
    );
    assert_eq!(
        summary_fields.get("preserve_session_affinity_count"),
        Some(&preserve_session_affinity_count)
    );
    assert_eq!(
        summary_fields.get("preserve_thread_affinity_count"),
        Some(&preserve_thread_affinity_count)
    );
    assert_eq!(
        summary_fields.get("preserve_turn_affinity_count"),
        Some(&preserve_turn_affinity_count)
    );
    assert_eq!(summary_fields.get("precommit_count"), Some(&precommit_count));
    assert_eq!(
        summary_fields.get("turn_committed_count"),
        Some(&turn_committed_count)
    );
    assert_eq!(
        summary_fields.get("rotation_open_count"),
        Some(&rotation_open_count)
    );
    assert_eq!(
        summary_fields.get("rotation_closed_count"),
        Some(&rotation_closed_count)
    );
    assert_eq!(
        summary_fields.get("fresh_select_ok_count"),
        Some(&fresh_select_ok_count)
    );
    assert_eq!(
        summary_fields.get("preserve_session_owner_count"),
        Some(&preserve_session_owner_count)
    );
    assert_eq!(
        summary_fields.get("preserve_thread_owner_count"),
        Some(&preserve_thread_owner_count)
    );
    assert_eq!(
        summary_fields.get("preserve_turn_owner_count"),
        Some(&preserve_turn_owner_count)
    );
    assert_eq!(
        summary_fields.get("affinity_required_count"),
        Some(&affinity_required_count)
    );
    assert_eq!(
        summary_fields.get("rotation_allowed_count"),
        Some(&rotation_allowed_count)
    );
    assert_eq!(
        summary_fields.get("preserves_owner_count"),
        Some(&preserves_owner_count)
    );
}

#[test]
fn app_server_broker_preview_report_counts_match_previews_for_live_rendered_reports() {
    for (name, input) in [
        ("valid_replay", app_server_broker_stdio_preview_replay_fixture()),
        (
            "malformed_replay",
            app_server_broker_stdio_preview_malformed_replay_fixture(),
        ),
    ] {
        let report = app_server_broker_preview_report(input);
        assert_preview_report_counts_match_previews(&report, name);
    }
}

#[test]
fn app_server_broker_metadata_surface_fixture_matches_helper_behavior() {
    let fixture = app_server_broker_metadata_surface_fixture();
    assert_eq!(
        fixture.fields,
        vec![
            "session_id".to_string(),
            "thread_id".to_string(),
            "turn_id".to_string(),
            "item_id".to_string()
        ]
    );
    for case in fixture.cases {
        let metadata = app_server_broker_metadata_from_value(&case.payload);
        assert_eq!(metadata.session_id, case.expect.session_id, "{}", case.name);
        assert_eq!(metadata.thread_id, case.expect.thread_id, "{}", case.name);
        assert_eq!(metadata.turn_id, case.expect.turn_id, "{}", case.name);
        assert_eq!(metadata.item_id, case.expect.item_id, "{}", case.name);
    }
}

#[test]
fn app_server_broker_protocol_surface_fixture_covers_checked_in_fixture_methods() {
    let fixture = app_server_broker_protocol_surface_fixture();
    let allowed_lifecycle_methods = fixture
        .canonical_lifecycle_methods
        .iter()
        .chain(fixture.accepted_lifecycle_aliases.iter())
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let allowed_other_methods = fixture
        .example_other_methods
        .iter()
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();

    for case in app_server_broker_fixture_cases() {
        if let Some(method) = case.payload.get("method").and_then(serde_json::Value::as_str) {
            assert!(
                allowed_lifecycle_methods.contains(method) || allowed_other_methods.contains(method),
                "{}",
                case.name
            );
        }
        assert_eq!(
            case.expect.valid_jsonrpc,
            case.payload.get("jsonrpc").and_then(serde_json::Value::as_str) == Some("2.0"),
            "{}",
            case.name
        );
    }

    for case in app_server_broker_request_fixture_cases() {
        let method = case
            .payload
            .get("method")
            .and_then(serde_json::Value::as_str)
            .expect("request fixture should include method");
        assert!(
            allowed_lifecycle_methods.contains(method) || allowed_other_methods.contains(method),
            "{}",
            case.name
        );
    }

    for case in app_server_broker_frame_fixture_cases() {
        if let Some(method) = case.payload.get("method").and_then(serde_json::Value::as_str) {
            assert!(
                allowed_lifecycle_methods.contains(method) || allowed_other_methods.contains(method),
                "{}",
                case.name
            );
        }
    }

    for (name, replay) in [
        (
            "valid_replay",
            app_server_broker_stdio_preview_replay_fixture(),
        ),
        (
            "malformed_replay",
            app_server_broker_stdio_preview_malformed_replay_fixture(),
        ),
    ] {
        for line in replay.lines().map(str::trim).filter(|line| !line.is_empty()) {
            let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if let Some(method) = value.get("method").and_then(serde_json::Value::as_str) {
                assert!(
                    allowed_lifecycle_methods.contains(method)
                        || allowed_other_methods.contains(method),
                    "{name}: {method}"
                );
            }
            if let Some(jsonrpc) = value.get("jsonrpc").and_then(serde_json::Value::as_str) {
                assert!(
                    jsonrpc == fixture.jsonrpc_version || jsonrpc == "1.0",
                    "{name}: {jsonrpc}"
                );
            }
        }
    }
}

#[test]
fn app_server_broker_fixture_corpus_covers_lifecycle_methods_and_aliases() {
    let mut seen = std::collections::BTreeSet::new();

    for case in app_server_broker_request_fixture_cases() {
        if let Some(method) = case.payload.get("method").and_then(serde_json::Value::as_str) {
            seen.insert(method.to_string());
        }
    }
    for case in app_server_broker_fixture_cases() {
        if let Some(method) = case.payload.get("method").and_then(serde_json::Value::as_str) {
            seen.insert(method.to_string());
        }
    }

    for required in [
        &["initialize"][..],
        &["initialized", "notifications/initialized"][..],
        &["thread/start"][..],
        &["thread/started"][..],
        &["thread/resume"][..],
        &["thread/fork"][..],
        &["turn/start"][..],
        &["turn/started"][..],
        &["turn/completed"][..],
        &["turn/interrupt", "turn/cancel"][..],
    ] {
        assert!(
            required.iter().any(|method| seen.contains(*method)),
            "missing lifecycle fixture for any of {:?}",
            required
        );
    }
}

#[test]
fn app_server_broker_stdio_replay_corpus_covers_lifecycle_methods_and_aliases() {
    let mut seen = std::collections::BTreeSet::new();

    for replay in [
        app_server_broker_stdio_preview_replay_fixture(),
        app_server_broker_stdio_preview_wire_replay_fixture(),
        app_server_broker_upstream_schema_replay_fixture(),
        app_server_broker_stdio_preview_lifecycle_replay_fixture(),
    ] {
        for line in replay.lines().map(str::trim).filter(|line| !line.is_empty()) {
            let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if let Some(method) = value.get("method").and_then(serde_json::Value::as_str) {
                seen.insert(method.to_string());
            }
        }
    }

    for required in [
        &["initialize"][..],
        &["initialized", "notifications/initialized"][..],
        &["thread/start"][..],
        &["thread/started"][..],
        &["thread/resume"][..],
        &["thread/fork"][..],
        &["turn/start"][..],
        &["turn/started"][..],
        &["turn/completed"][..],
        &["turn/interrupt", "turn/cancel"][..],
    ] {
        assert!(
            required.iter().any(|method| seen.contains(*method)),
            "missing stdio replay coverage for any of {:?}",
            required
        );
    }
}

#[test]
fn app_server_broker_accepts_wire_omitted_jsonrpc_request_shapes() {
    let payload = serde_json::json!({
        "id": "req-wire",
        "method": "turn/start",
        "params": {
            "sessionId": "sess-wire",
            "thread": { "id": "thread-wire" },
            "turn": { "id": "turn-wire" }
        }
    });
    let request = parse_app_server_broker_request(&payload).expect("wire-format request should parse");
    assert_eq!(request.id, Some(serde_json::Value::String("req-wire".to_string())));
    assert_eq!(request.raw_method, "turn/start");
    assert_eq!(request.method.label(), "turn/start");
    assert_eq!(request.metadata.session_id.as_deref(), Some("sess-wire"));
    assert_eq!(request.metadata.thread_id.as_deref(), Some("thread-wire"));
    assert_eq!(request.metadata.turn_id.as_deref(), Some("turn-wire"));
}

#[test]
fn app_server_broker_treats_wire_omitted_jsonrpc_as_valid_diagnostics() {
    for (name, payload, expected_frame_kind) in [
        (
            "request",
            serde_json::json!({"id":"req-1","method":"thread/start","params":{"sessionId":"sess-1"}}),
            "request",
        ),
        (
            "notification",
            serde_json::json!({"method":"notifications/initialized","params":{}}),
            "notification",
        ),
        (
            "response",
            serde_json::json!({"id":"resp-1","result":{"ok":true}}),
            "response",
        ),
    ] {
        let summary = app_server_broker_diagnostic_summary_json(&payload);
        assert_eq!(summary["valid_jsonrpc"], serde_json::Value::Bool(true), "{name}");
        assert_eq!(summary["frame_kind"], expected_frame_kind, "{name}");
        assert_eq!(summary["invalid_reason"], serde_json::Value::Null, "{name}");
    }
}

#[test]
fn app_server_broker_non_jsonrpc_version_requires_explicit_wrong_header() {
    let payload = serde_json::json!({
        "method": "turn/start",
        "params": {}
    });
    assert_eq!(app_server_broker_invalid_reason(&payload), None);

    let bad_payload = serde_json::json!({
        "jsonrpc": "1.0",
        "method": "turn/start",
        "params": {}
    });
    assert_eq!(
        app_server_broker_invalid_reason(&bad_payload),
        Some("non_jsonrpc_version")
    );
}

#[test]
fn app_server_broker_method_kind_matrix_matches_contract_taxonomy() {
    for (method, expected) in [
        (Some("initialize"), AppServerBrokerMethodKind::Lifecycle),
        (
            Some("notifications/initialized"),
            AppServerBrokerMethodKind::Lifecycle,
        ),
        (Some("custom/ping"), AppServerBrokerMethodKind::Other),
        (None, AppServerBrokerMethodKind::Absent),
    ] {
        assert_eq!(app_server_broker_method_kind(method), expected);
    }
}

#[test]
fn app_server_broker_lifecycle_stage_matrix_matches_request_and_notification_surface() {
    for (method, frame_kind, expected) in [
        (
            Some("initialize"),
            AppServerBrokerFrameKind::Request,
            Some("initialize_request"),
        ),
        (
            Some("notifications/initialized"),
            AppServerBrokerFrameKind::Notification,
            Some("initialized_notification"),
        ),
        (
            Some("thread/start"),
            AppServerBrokerFrameKind::Request,
            Some("thread_start_request"),
        ),
        (
            Some("thread/started"),
            AppServerBrokerFrameKind::Notification,
            Some("thread_started_notification"),
        ),
        (
            Some("thread/resume"),
            AppServerBrokerFrameKind::Request,
            Some("thread_resume_request"),
        ),
        (
            Some("thread/fork"),
            AppServerBrokerFrameKind::Request,
            Some("thread_fork_request"),
        ),
        (
            Some("turn/start"),
            AppServerBrokerFrameKind::Request,
            Some("turn_start_request"),
        ),
        (
            Some("turn/started"),
            AppServerBrokerFrameKind::Notification,
            Some("turn_started_notification"),
        ),
        (
            Some("turn/completed"),
            AppServerBrokerFrameKind::Notification,
            Some("turn_completed_notification"),
        ),
        (
            Some("turn/interrupt"),
            AppServerBrokerFrameKind::Request,
            Some("turn_interrupt_request"),
        ),
        (
            Some("turn/cancel"),
            AppServerBrokerFrameKind::Request,
            Some("turn_interrupt_request"),
        ),
        (Some("custom/ping"), AppServerBrokerFrameKind::Request, None),
        (Some("thread/started"), AppServerBrokerFrameKind::Response, None),
        (None, AppServerBrokerFrameKind::Notification, None),
    ] {
        assert_eq!(
            app_server_broker_lifecycle_stage(method, frame_kind).map(|stage| stage.label()),
            expected
        );
    }
}

#[test]
fn app_server_broker_lifecycle_method_matrix_normalizes_aliases_and_spacing() {
    for (method, expected) in [
        ("initialize", true),
        (" notifications/initialized ", true),
        ("TURN/START", true),
        ("thread/started", true),
        ("turn/completed", true),
        ("custom/ping", false),
    ] {
        assert_eq!(app_server_broker_is_lifecycle_method(method), expected);
    }
}

#[test]
fn app_server_broker_upstream_schema_replay_lifecycle_stages_match_expected_sequence() {
    let replay = app_server_broker_upstream_schema_replay_fixture();
    let expected = [
        Some("thread_start_request"),
        None,
        Some("thread_started_notification"),
        Some("thread_resume_request"),
        None,
        Some("thread_fork_request"),
        None,
        Some("turn_start_request"),
        None,
        Some("turn_started_notification"),
        Some("turn_completed_notification"),
        Some("turn_interrupt_request"),
        None,
    ];

    for (index, (line, expected_stage)) in replay
        .lines()
        .filter(|line| !line.trim().is_empty())
        .zip(expected)
        .enumerate()
    {
        let payload: serde_json::Value =
            serde_json::from_str(line).unwrap_or_else(|_| panic!("replay line {index} should parse"));
        let summary = app_server_broker_diagnostic_summary(&payload);
        assert_eq!(
            app_server_broker_lifecycle_stage(summary.method.as_deref(), summary.frame_kind)
                .map(|stage| stage.label()),
            expected_stage,
            "line {}",
            index + 1
        );
    }
}

#[test]
fn app_server_broker_lifecycle_binding_matrix_captures_stage_and_ids() {
    for (payload, expected_stage, expected_session, expected_thread, expected_turn) in [
        (
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": "thread/started",
                "params": {
                    "threadId": "thr_started",
                    "sessionId": "sess_started"
                }
            }),
            "thread_started_notification",
            Some("sess_started"),
            Some("thr_started"),
            None,
        ),
        (
            serde_json::json!({
                "jsonrpc": "2.0",
                "method": "turn/started",
                "params": {
                    "threadId": "thr_turn",
                    "turnId": "turn_started"
                }
            }),
            "turn_started_notification",
            None,
            Some("thr_turn"),
            Some("turn_started"),
        ),
        (
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": "req-interrupt",
                "method": "turn/cancel",
                "params": {
                    "threadId": "thr_interrupt",
                    "turnId": "turn_interrupt"
                }
            }),
            "turn_interrupt_request",
            None,
            Some("thr_interrupt"),
            Some("turn_interrupt"),
        ),
    ] {
        let binding =
            app_server_broker_lifecycle_binding(&payload).expect("lifecycle binding should exist");
        assert_eq!(binding.stage.label(), expected_stage);
        assert_eq!(binding.metadata.session_id.as_deref(), expected_session);
        assert_eq!(binding.metadata.thread_id.as_deref(), expected_thread);
        assert_eq!(binding.metadata.turn_id.as_deref(), expected_turn);
    }

    assert!(
        app_server_broker_lifecycle_binding(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": "resp-1",
            "result": {}
        }))
        .is_none()
    );
}

#[test]
fn app_server_broker_upstream_schema_replay_lifecycle_bindings_match_expected_identity_sequence() {
    let replay = app_server_broker_upstream_schema_replay_fixture();
    let expected = [
        Some(("thread_start_request", None, None, None)),
        None,
        Some(("thread_started_notification", Some("sess_123"), Some("thr_123"), None)),
        Some(("thread_resume_request", None, Some("thr_123"), None)),
        None,
        Some(("thread_fork_request", None, Some("thr_123"), None)),
        None,
        Some(("turn_start_request", None, Some("thr_123"), None)),
        None,
        Some(("turn_started_notification", None, Some("thr_123"), Some("turn_123"))),
        Some(("turn_completed_notification", None, Some("thr_123"), Some("turn_123"))),
        Some(("turn_interrupt_request", None, Some("thr_123"), Some("turn_123"))),
        None,
    ];

    for (index, (line, expected_binding)) in replay
        .lines()
        .filter(|line| !line.trim().is_empty())
        .zip(expected)
        .enumerate()
    {
        let payload: serde_json::Value =
            serde_json::from_str(line).unwrap_or_else(|_| panic!("replay line {index} should parse"));
        let binding = app_server_broker_lifecycle_binding(&payload);
        match expected_binding {
            Some((stage, session_id, thread_id, turn_id)) => {
                let binding = binding.unwrap_or_else(|| panic!("line {} should bind", index + 1));
                assert_eq!(binding.stage.label(), stage, "line {}", index + 1);
                assert_eq!(binding.metadata.session_id.as_deref(), session_id, "line {}", index + 1);
                assert_eq!(binding.metadata.thread_id.as_deref(), thread_id, "line {}", index + 1);
                assert_eq!(binding.metadata.turn_id.as_deref(), turn_id, "line {}", index + 1);
            }
            None => assert!(binding.is_none(), "line {} should not bind", index + 1),
        }
    }
}

#[test]
fn app_server_broker_affinity_keys_prioritize_turn_then_thread_then_session() {
    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "turn/started",
        "params": {
            "sessionId": "sess_affinity",
            "threadId": "thr_affinity",
            "turnId": "turn_affinity"
        }
    });
    let keys = app_server_broker_affinity_keys(&payload);
    assert_eq!(
        keys.iter()
            .map(|key| (key.kind.label(), key.value.as_str()))
            .collect::<Vec<_>>(),
        vec![
            ("turn", "turn_affinity"),
            ("thread", "thr_affinity"),
            ("session", "sess_affinity"),
        ]
    );

    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "thread/started",
        "params": {
            "sessionId": "sess_thread",
            "threadId": "thr_thread"
        }
    });
    let keys = app_server_broker_affinity_keys(&payload);
    assert_eq!(
        keys.iter()
            .map(|key| (key.kind.label(), key.value.as_str()))
            .collect::<Vec<_>>(),
        vec![("thread", "thr_thread"), ("session", "sess_thread")]
    );

    let payload = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized",
        "params": {
            "sessionId": "sess_init"
        }
    });
    let keys = app_server_broker_affinity_keys(&payload);
    assert_eq!(
        keys.iter()
            .map(|key| (key.kind.label(), key.value.as_str()))
            .collect::<Vec<_>>(),
        vec![("session", "sess_init")]
    );
}

#[test]
fn app_server_broker_upstream_schema_replay_affinity_keys_match_expected_priority_sequence() {
    let replay = app_server_broker_upstream_schema_replay_fixture();
    let expected = [
        Vec::<(&str, &str)>::new(),
        vec![("thread", "thr_123"), ("session", "sess_123")],
        vec![("thread", "thr_123"), ("session", "sess_123")],
        vec![("thread", "thr_123")],
        vec![("thread", "thr_123"), ("session", "sess_123")],
        vec![("thread", "thr_123")],
        vec![("thread", "thr_forked"), ("session", "sess_123")],
        vec![("thread", "thr_123")],
        vec![("turn", "turn_123")],
        vec![("turn", "turn_123"), ("thread", "thr_123")],
        vec![("turn", "turn_123"), ("thread", "thr_123")],
        vec![("turn", "turn_123"), ("thread", "thr_123")],
        Vec::<(&str, &str)>::new(),
    ];

    for (index, (line, expected_keys)) in replay
        .lines()
        .filter(|line| !line.trim().is_empty())
        .zip(expected)
        .enumerate()
    {
        let payload: serde_json::Value =
            serde_json::from_str(line).unwrap_or_else(|_| panic!("replay line {index} should parse"));
        let keys = app_server_broker_affinity_keys(&payload);
        assert_eq!(
            keys.iter()
                .map(|key| (key.kind.label(), key.value.as_str()))
                .collect::<Vec<_>>(),
            expected_keys,
            "line {}",
            index + 1
        );
    }
}

#[test]
fn app_server_broker_upstream_schema_replay_response_metadata_matches_result_payloads() {
    let replay = app_server_broker_upstream_schema_replay_fixture();
    let expected = [
        (2, Some("sess_123"), Some("thr_123"), None, None),
        (5, Some("sess_123"), Some("thr_123"), None, None),
        (7, Some("sess_123"), Some("thr_forked"), None, None),
        (9, None, None, Some("turn_123"), None),
        (13, None, None, None, None),
    ];

    for (((index, line), line_no), (expected_line, session_id, thread_id, turn_id, item_id)) in replay
        .lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
        .filter_map(|(index, line)| {
            let line_no = index + 1;
            if matches!(line_no, 2 | 5 | 7 | 9 | 13) {
                Some(((index, line), line_no))
            } else {
                None
            }
        })
        .zip(expected)
    {
        assert_eq!(expected_line, line_no, "response line selection drifted");
        let payload: serde_json::Value =
            serde_json::from_str(line).unwrap_or_else(|_| panic!("replay line {index} should parse"));
        let summary = app_server_broker_diagnostic_summary(&payload);
        assert_eq!(summary.frame_kind, AppServerBrokerFrameKind::Response, "line {}", line_no);
        assert_eq!(summary.metadata.session_id.as_deref(), session_id, "line {}", line_no);
        assert_eq!(summary.metadata.thread_id.as_deref(), thread_id, "line {}", line_no);
        assert_eq!(summary.metadata.turn_id.as_deref(), turn_id, "line {}", line_no);
        assert_eq!(summary.metadata.item_id.as_deref(), item_id, "line {}", line_no);
    }
}

#[test]
fn app_server_broker_fixture_corpus_matches_expected_diagnostics() {
    for case in app_server_broker_fixture_cases() {
        let summary = app_server_broker_diagnostic_summary(&case.payload);
        assert_eq!(
            summary.valid_jsonrpc, case.expect.valid_jsonrpc,
            "{}",
            case.name
        );
        assert_eq!(summary.frame_kind.label(), case.expect.frame_kind, "{}", case.name);
        assert_eq!(summary.id, case.expect.id, "{}", case.name);
        assert_eq!(summary.method, case.expect.method, "{}", case.name);
        assert_eq!(
            summary
                .method
                .as_deref()
                .is_some_and(app_server_broker_is_lifecycle_method),
            case.expect.is_lifecycle_method,
            "{}",
            case.name
        );
        assert_eq!(
            app_server_broker_method_kind(summary.method.as_deref()).label(),
            case.expect.method_kind,
            "{}",
            case.name
        );
        assert_eq!(
            summary.invalid_reason,
            case.expect.invalid_reason.as_deref(),
            "{}",
            case.name
        );
        assert_eq!(summary.metadata.session_id, case.expect.session_id, "{}", case.name);
        assert_eq!(summary.metadata.thread_id, case.expect.thread_id, "{}", case.name);
        assert_eq!(summary.metadata.turn_id, case.expect.turn_id, "{}", case.name);
        assert_eq!(summary.metadata.item_id, case.expect.item_id, "{}", case.name);
    }
}

#[test]
fn app_server_broker_fixture_corpus_matches_expected_diagnostic_json_surface() {
    for case in app_server_broker_fixture_cases() {
        let rendered = app_server_broker_diagnostic_summary_json(&case.payload);
        assert_eq!(rendered["valid_jsonrpc"], case.expect.valid_jsonrpc, "{}", case.name);
        assert_eq!(rendered["frame_kind"], case.expect.frame_kind, "{}", case.name);
        assert_eq!(rendered["id"], serde_json::to_value(&case.expect.id).unwrap(), "{}", case.name);
        assert_eq!(
            rendered["method"],
            serde_json::to_value(&case.expect.method).unwrap(),
            "{}",
            case.name
        );
        assert_eq!(rendered["method_kind"], case.expect.method_kind, "{}", case.name);
        assert_eq!(
            rendered["is_lifecycle_method"],
            case.expect.is_lifecycle_method,
            "{}",
            case.name
        );
        assert_eq!(
            rendered["lifecycle_stage"],
            serde_json::to_value(&case.expect.lifecycle_stage).unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            rendered["continuation_affinity"],
            expected_affinity_json(&case.expect.continuation_affinity),
            "{}",
            case.name
        );
        assert_eq!(
            rendered["continuation_decision"],
            case.expect.continuation_decision,
            "{}",
            case.name
        );
        assert_eq!(rendered["policy_hint"], expected_policy_hint_json(&case.expect.policy_hint), "{}", case.name);
        assert_eq!(
            rendered["primary_affinity"],
            serde_json::to_value(
                case.expect
                    .primary_affinity
                    .as_ref()
                    .map(|key| serde_json::json!({"kind": key.kind, "value": key.value}))
            )
            .unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            rendered["affinity_keys"],
            serde_json::to_value(
                case.expect
                    .affinity_keys
                    .iter()
                    .map(|key| serde_json::json!({"kind": key.kind, "value": key.value}))
                    .collect::<Vec<_>>()
            )
            .unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            rendered["invalid_reason"],
            serde_json::to_value(&case.expect.invalid_reason).unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            rendered["metadata"]["session_id"],
            serde_json::to_value(&case.expect.session_id).unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            rendered["metadata"]["thread_id"],
            serde_json::to_value(&case.expect.thread_id).unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            rendered["metadata"]["turn_id"],
            serde_json::to_value(&case.expect.turn_id).unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            rendered["metadata"]["item_id"],
            serde_json::to_value(&case.expect.item_id).unwrap(),
            "{}",
            case.name
        );
    }
}

#[test]
fn app_server_broker_preview_line_matches_diagnostic_summary_for_fixture_payloads() {
    for case in app_server_broker_fixture_cases() {
        let line = serde_json::to_string(&case.payload).expect("fixture payload should serialize");
        let rendered = app_server_broker_preview_line(&line);
        assert_eq!(rendered["parse_ok"], serde_json::Value::Bool(true), "{}", case.name);
        assert_eq!(
            rendered["summary"],
            app_server_broker_diagnostic_summary_json(&case.payload),
            "{}",
            case.name
        );
    }
}

#[test]
fn app_server_broker_policy_hint_helper_matches_fixture_corpus() {
    for case in app_server_broker_fixture_cases() {
        assert_policy_hint_struct_matches_expect(
            app_server_broker_policy_hint(&case.payload),
            &case.expect.policy_hint,
            &case.name,
        );
    }
    for case in app_server_broker_frame_fixture_cases() {
        assert_policy_hint_struct_matches_expect(
            app_server_broker_policy_hint(&case.payload),
            &case.expect.policy_hint,
            &case.name,
        );
    }
}

#[test]
fn app_server_broker_provider_switch_policy_preserves_continuation_affinity() {
    for case in app_server_broker_fixture_cases() {
        assert_eq!(
            app_server_broker_allows_provider_switch(&case.payload, false),
            case.expect.policy_hint.rotation_allowed,
            "{}",
            case.name
        );
        assert!(
            app_server_broker_allows_provider_switch(&case.payload, true),
            "{}",
            case.name
        );
    }
    for case in app_server_broker_frame_fixture_cases() {
        assert_eq!(
            app_server_broker_allows_provider_switch(&case.payload, false),
            case.expect.policy_hint.rotation_allowed,
            "{}",
            case.name
        );
        assert!(
            app_server_broker_allows_provider_switch(&case.payload, true),
            "{}",
            case.name
        );
    }
}

#[test]
fn app_server_broker_fixture_corpus_matches_expected_request_parsing() {
    for case in app_server_broker_request_fixture_cases() {
        let request =
            parse_app_server_broker_request(&case.payload).expect("fixture request should parse");
        assert_eq!(request.id, case.expect.id, "{}", case.name);
        assert_eq!(request.raw_method, case.expect.raw_method, "{}", case.name);
        assert_eq!(request.method.label(), case.expect.method, "{}", case.name);
        assert_eq!(
            app_server_broker_method_kind(Some(&request.raw_method)).label(),
            case.expect.method_kind,
            "{}",
            case.name
        );
        assert_eq!(request.metadata.session_id, case.expect.session_id, "{}", case.name);
        assert_eq!(request.metadata.thread_id, case.expect.thread_id, "{}", case.name);
        assert_eq!(request.metadata.turn_id, case.expect.turn_id, "{}", case.name);
        assert_eq!(request.metadata.item_id, case.expect.item_id, "{}", case.name);
    }
}

#[test]
fn app_server_broker_request_fixture_corpus_matches_summary_json_surface() {
    for case in app_server_broker_request_fixture_cases() {
        let request =
            parse_app_server_broker_request(&case.payload).expect("fixture request should parse");
        let rendered = app_server_broker_request_summary_json(&request);
        assert_eq!(rendered["id"], serde_json::to_value(&case.expect.id).unwrap(), "{}", case.name);
        assert_eq!(rendered["raw_method"], case.expect.raw_method, "{}", case.name);
        assert_eq!(rendered["method"], case.expect.method, "{}", case.name);
        assert_eq!(rendered["method_kind"], case.expect.method_kind, "{}", case.name);
        assert_eq!(
            rendered["lifecycle_stage"],
            serde_json::to_value(&case.expect.lifecycle_stage).unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            rendered["continuation_affinity"],
            expected_affinity_json(&case.expect.continuation_affinity),
            "{}",
            case.name
        );
        assert_eq!(
            rendered["continuation_decision"],
            case.expect.continuation_decision,
            "{}",
            case.name
        );
        assert_eq!(rendered["policy_hint"], expected_policy_hint_json(&case.expect.policy_hint), "{}", case.name);
        assert_eq!(
            rendered["primary_affinity"],
            serde_json::to_value(
                case.expect
                    .primary_affinity
                    .as_ref()
                    .map(|key| serde_json::json!({"kind": key.kind, "value": key.value}))
            )
            .unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            rendered["affinity_keys"],
            serde_json::to_value(
                case.expect
                    .affinity_keys
                    .iter()
                    .map(|key| serde_json::json!({"kind": key.kind, "value": key.value}))
                    .collect::<Vec<_>>()
            )
            .unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            rendered["metadata"]["session_id"],
            serde_json::to_value(&case.expect.session_id).unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            rendered["metadata"]["thread_id"],
            serde_json::to_value(&case.expect.thread_id).unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            rendered["metadata"]["turn_id"],
            serde_json::to_value(&case.expect.turn_id).unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            rendered["metadata"]["item_id"],
            serde_json::to_value(&case.expect.item_id).unwrap(),
            "{}",
            case.name
        );
    }
}

#[test]
fn app_server_broker_request_summary_matches_diagnostic_summary_for_request_fixtures() {
    for case in app_server_broker_request_fixture_cases() {
        let request =
            parse_app_server_broker_request(&case.payload).expect("fixture request should parse");
        let request_summary = app_server_broker_request_summary_json(&request);
        let diagnostic_summary = app_server_broker_diagnostic_summary_json(&case.payload);
        assert_eq!(request_summary["id"], diagnostic_summary["id"], "{}", case.name);
        assert_eq!(request_summary["raw_method"], diagnostic_summary["method"], "{}", case.name);
        assert_eq!(request_summary["method_kind"], diagnostic_summary["method_kind"], "{}", case.name);
        assert_eq!(
            request_summary["lifecycle_stage"],
            diagnostic_summary["lifecycle_stage"],
            "{}",
            case.name
        );
        assert_eq!(
            request_summary["lifecycle_schema_file"],
            diagnostic_summary["lifecycle_schema_file"],
            "{}",
            case.name
        );
        assert_eq!(
            request_summary["continuation_affinity"],
            diagnostic_summary["continuation_affinity"],
            "{}",
            case.name
        );
        assert_eq!(
            request_summary["continuation_decision"],
            diagnostic_summary["continuation_decision"],
            "{}",
            case.name
        );
        assert_eq!(request_summary["policy_hint"], diagnostic_summary["policy_hint"], "{}", case.name);
        assert_eq!(
            request_summary["primary_affinity"],
            diagnostic_summary["primary_affinity"],
            "{}",
            case.name
        );
        assert_eq!(
            request_summary["affinity_keys"],
            diagnostic_summary["affinity_keys"],
            "{}",
            case.name
        );
        assert_eq!(
            request_summary["metadata"],
            diagnostic_summary["metadata"],
            "{}",
            case.name
        );
    }
}

#[test]
fn app_server_broker_fixture_corpus_matches_expected_frame_classification() {
    for case in app_server_broker_frame_fixture_cases() {
        let summary = app_server_broker_diagnostic_summary(&case.payload);
        assert_eq!(summary.frame_kind.label(), case.expect.frame_kind, "{}", case.name);
        assert_eq!(summary.method, case.expect.method, "{}", case.name);
        assert_eq!(
            summary.valid_jsonrpc, case.expect.valid_jsonrpc,
            "{}",
            case.name
        );
    }
}

#[test]
fn app_server_broker_frame_fixture_corpus_matches_response_summary_json_surface() {
    for case in app_server_broker_frame_fixture_cases() {
        let rendered = app_server_broker_response_summary_json(&case.payload);
        assert_eq!(rendered["valid_jsonrpc"], case.expect.valid_jsonrpc, "{}", case.name);
        assert_eq!(rendered["frame_kind"], case.expect.frame_kind, "{}", case.name);
        assert_eq!(rendered["id"], serde_json::to_value(&case.expect.id).unwrap(), "{}", case.name);
        assert_eq!(rendered["has_result"], case.expect.has_result, "{}", case.name);
        assert_eq!(rendered["has_error"], case.expect.has_error, "{}", case.name);
        assert_eq!(
            rendered["error_code"],
            serde_json::to_value(&case.expect.error_code).unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            rendered["error_message"],
            serde_json::to_value(&case.expect.error_message).unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            rendered["continuation_affinity"],
            expected_affinity_json(&case.expect.continuation_affinity),
            "{}",
            case.name
        );
        assert_eq!(
            rendered["continuation_decision"],
            case.expect.continuation_decision,
            "{}",
            case.name
        );
        assert_eq!(rendered["policy_hint"], expected_policy_hint_json(&case.expect.policy_hint), "{}", case.name);
        assert_eq!(
            rendered["primary_affinity"],
            serde_json::to_value(
                case.expect
                    .primary_affinity
                    .as_ref()
                    .map(|key| serde_json::json!({"kind": key.kind, "value": key.value}))
            )
            .unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            rendered["affinity_keys"],
            serde_json::to_value(
                case.expect
                    .affinity_keys
                    .iter()
                    .map(|key| serde_json::json!({"kind": key.kind, "value": key.value}))
                    .collect::<Vec<_>>()
            )
            .unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            rendered["metadata"]["session_id"],
            serde_json::to_value(&case.expect.session_id).unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            rendered["metadata"]["thread_id"],
            serde_json::to_value(&case.expect.thread_id).unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            rendered["metadata"]["turn_id"],
            serde_json::to_value(&case.expect.turn_id).unwrap(),
            "{}",
            case.name
        );
        assert_eq!(
            rendered["metadata"]["item_id"],
            serde_json::to_value(&case.expect.item_id).unwrap(),
            "{}",
            case.name
        );
    }
}

#[test]
fn app_server_broker_response_summary_redacts_secret_like_error_messages() {
    let rendered = app_server_broker_response_summary_json(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": "req-1",
        "error": {
            "code": -32000,
            "message": "upstream rejected Authorization: Bearer raw-secret-token"
        }
    }));

    assert_eq!(
        rendered["error_message"],
        "upstream rejected Authorization: Bearer <redacted>"
    );
    assert!(!serde_json::to_string(&rendered)
        .unwrap()
        .contains("raw-secret-token"));
}

#[test]
fn app_server_broker_response_summary_matches_diagnostic_summary_for_frame_fixtures() {
    for case in app_server_broker_frame_fixture_cases() {
        let response_summary = app_server_broker_response_summary_json(&case.payload);
        let diagnostic_summary = app_server_broker_diagnostic_summary_json(&case.payload);
        assert_eq!(response_summary["valid_jsonrpc"], diagnostic_summary["valid_jsonrpc"], "{}", case.name);
        assert_eq!(response_summary["frame_kind"], diagnostic_summary["frame_kind"], "{}", case.name);
        assert_eq!(response_summary["id"], diagnostic_summary["id"], "{}", case.name);
        assert_eq!(
            response_summary["continuation_affinity"],
            diagnostic_summary["continuation_affinity"],
            "{}",
            case.name
        );
        assert_eq!(
            response_summary["continuation_decision"],
            diagnostic_summary["continuation_decision"],
            "{}",
            case.name
        );
        assert_eq!(response_summary["policy_hint"], diagnostic_summary["policy_hint"], "{}", case.name);
        assert_eq!(
            response_summary["primary_affinity"],
            diagnostic_summary["primary_affinity"],
            "{}",
            case.name
        );
        assert_eq!(
            response_summary["affinity_keys"],
            diagnostic_summary["affinity_keys"],
            "{}",
            case.name
        );
        assert_eq!(
            response_summary["metadata"],
            diagnostic_summary["metadata"],
            "{}",
            case.name
        );
    }
}

#[test]
fn app_server_broker_accepts_wire_format_requests_and_reports_disabled_contract() {
    let request = parse_app_server_broker_request(&serde_json::json!({"method":"initialize"}))
        .expect("wire-format request should parse");
    assert_eq!(request.method, AppServerBrokerMethod::Initialize);

    let contract = app_server_broker_contract_json();
    assert_eq!(contract["enabled_by_default"], serde_json::Value::Bool(false));
    assert_eq!(contract["default_mode"], "direct-passthrough");
    assert_eq!(contract["status"], "live-validated-broker");
    assert_eq!(contract["wire_omits_jsonrpc_header"], serde_json::Value::Bool(true));
    assert_eq!(contract["cli"]["json_contract"], serde_json::Value::Bool(true));
    assert_eq!(
        contract["cli"]["experimental_stdio_preview"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["cli"]["experimental_stdio_passthrough_preview"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["protocol_surface_fixture"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["fixture_drift_tests"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["helper_consistency_tests"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["stream_helper_consistency_tests"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["cross_surface_consistency_tests"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["report_aggregation_consistency_tests"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["metadata_surface_fixture"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["metadata_drift_tests"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["output_surface_fixture"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["output_drift_tests"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["runtime_log_surface_fixture"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["runtime_log_drift_tests"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["upstream_codex_schema_imported"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["schema_validation"]["lifecycle_response_schema_hints"],
        serde_json::Value::Bool(true)
    );
    assert!(
        contract["lifecycle_methods"]
            .as_array()
            .unwrap()
            .contains(&serde_json::Value::String("turn/start".to_string()))
    );
    assert_eq!(
        contract["accepted_lifecycle_aliases"],
        serde_json::json!(["notifications/initialized", "turn/cancel"])
    );
    assert!(
        contract["transport"]
            .as_array()
            .unwrap()
            .contains(&serde_json::Value::String("stdio-preview".to_string()))
    );
    assert!(
        contract["transport"]
            .as_array()
            .unwrap()
            .contains(&serde_json::Value::String(
                "stdio-passthrough-preview".to_string()
            ))
    );
    assert!(
        contract["transport"]
            .as_array()
            .unwrap()
            .contains(&serde_json::Value::String("stdio-live".to_string()))
    );
    assert_eq!(
        contract["affinity"]["continuation_affinity_wins"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["affinity"]["decision_kinds"],
        serde_json::json!([
            "fresh",
            "continue-session",
            "continue-thread",
            "continue-turn"
        ])
    );
    assert_eq!(
        contract["affinity"]["policy_modes"],
        serde_json::json!([
            "fresh-selection-ok",
            "preserve-session-affinity",
            "preserve-thread-affinity",
            "preserve-turn-affinity"
        ])
    );
    assert_eq!(
        contract["affinity"]["routing_hints"],
        serde_json::json!([
            "fresh-select-ok",
            "preserve-session-owner",
            "preserve-thread-owner",
            "preserve-turn-owner"
        ])
    );
    assert_eq!(
        contract["affinity"]["commit_boundaries"],
        serde_json::json!(["precommit", "turn-committed"])
    );
    assert_eq!(
        contract["affinity"]["rotation_windows"],
        serde_json::json!(["open", "closed"])
    );
    assert_eq!(
        contract["diagnostics"]["jsonrpc_envelope_validation"],
        serde_json::Value::Bool(true)
    );
    assert!(
        contract["diagnostics"]["method_kinds"]
            .as_array()
            .unwrap()
            .contains(&serde_json::Value::String("other".to_string()))
    );
    assert!(
        contract["diagnostics"]["invalid_reasons"]
            .as_array()
            .unwrap()
            .contains(&serde_json::Value::String(
                "method_with_result_or_error".to_string()
            ))
    );
    assert_eq!(
        contract["diagnostics"]["invalid_frame_reasoning"],
        "shape-based"
    );
    assert_eq!(
        contract["diagnostics"]["response_summary_json"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(contract["diagnostics"]["policy_hint"], serde_json::Value::Bool(true));
    assert_eq!(
        contract["diagnostics"]["commit_boundary_hint"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["diagnostics"]["stdio_preview_report"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["diagnostics"]["stdio_preview_report_envelope"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["diagnostics"]["stdio_preview_pretty_json"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["diagnostics"]["stdio_preview_jsonl"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["diagnostics"]["stdio_preview_session_summary"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["diagnostics"]["stdio_passthrough_preview"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["diagnostics"]["stdio_passthrough_preserves_input"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["diagnostics"]["stdio_passthrough_empty_summary"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["diagnostics"]["stdio_passthrough_write_failures_surface"],
        serde_json::Value::Bool(true)
    );
    assert_eq!(
        contract["diagnostics"]["stdio_diagnostics_write_failures_surface"],
        serde_json::Value::Bool(true)
    );
}

#[test]
fn app_server_broker_status_line_matches_contract_defaults() {
    let status_line = app_server_broker_status_line();
    assert!(status_line.contains("status=live-validated-broker"));
    assert!(status_line.contains("transport=stdio-preview,stdio-passthrough-preview"));
    assert!(status_line.contains("mode=direct-passthrough"));
    assert!(status_line.contains("bidirectional validated stdio live mode is available"));
    assert!(status_line.contains("passthrough remains active"));
}

#[test]
fn app_server_broker_render_stdio_preview_matches_preview_report_json() {
    let input = "{\"jsonrpc\":\"2.0\",\"id\":\"req-1\",\"method\":\"turn/start\",\"params\":{}}\n";
    let rendered = app_server_broker_render_stdio_preview(input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(parsed, app_server_broker_preview_report_json(input));
}

#[test]
fn app_server_broker_write_stdio_preview_stream_emits_jsonl_per_nonblank_line() {
    let input = std::io::Cursor::new(
        "\n\
         {\"jsonrpc\":\"2.0\",\"id\":\"req-1\",\"method\":\"turn/start\",\"params\":{}}\n\
         \n\
         {\"jsonrpc\":\"2.0\"\n",
    );
    let mut output = Vec::new();
    app_server_broker_write_stdio_preview_stream(input, &mut output).unwrap();
    let rendered = String::from_utf8(output).unwrap();
    let lines: Vec<&str> = rendered.lines().collect();
    assert_eq!(lines.len(), 3);
    let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    let second: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    let summary: serde_json::Value = serde_json::from_str(lines[2]).unwrap();
    assert_eq!(first["object"], "app_server_broker.preview_event");
    assert_eq!(first["line"], 2);
    assert_eq!(first["preview"]["parse_ok"], serde_json::Value::Bool(true));
    assert_eq!(first["preview"]["summary"]["frame_kind"], "request");
    assert_eq!(second["object"], "app_server_broker.preview_event");
    assert_eq!(second["line"], 4);
    assert_eq!(second["preview"]["parse_ok"], serde_json::Value::Bool(false));
    assert_eq!(second["preview"]["error"], "invalid_json");
    assert_eq!(summary["object"], "app_server_broker.preview_report");
    assert_eq!(summary["report"]["line_count"], 2);
    assert_eq!(summary["report"]["parsed_count"], 1);
    assert_eq!(summary["report"]["error_count"], 1);
}

#[test]
fn app_server_broker_write_stdio_preview_stream_rejects_oversized_line_before_newline() {
    let oversized =
        "x".repeat(crate::app_server_broker::APP_SERVER_BROKER_MAX_PREVIEW_LINE_BYTES + 2);
    let mut output = Vec::new();
    let err = app_server_broker_write_stdio_preview_stream(
        std::io::Cursor::new(oversized),
        &mut output,
    )
    .expect_err("oversized line without newline should fail closed");

    assert!(
        err.to_string()
            .contains("app-server broker preview line exceeds")
    );
    assert!(output.is_empty());
}

#[test]
fn app_server_broker_command_stdio_preview_emits_empty_session_summary_for_empty_input() {
    let mut output = Vec::new();

    run_app_server_broker_stdio_preview(std::io::Cursor::new(""), &mut output).unwrap();

    let rendered = String::from_utf8(output).unwrap();
    let lines: Vec<&str> = rendered.lines().collect();
    assert_eq!(lines.len(), 1);
    let summary: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(summary["object"], "app_server_broker.preview_report");
    assert_eq!(summary["report"]["line_count"], 0);
    assert_eq!(summary["report"]["parsed_count"], 0);
    assert_eq!(summary["report"]["error_count"], 0);
    assert_eq!(summary["report"]["previews"].as_array().unwrap().len(), 0);
}

#[test]
fn app_server_broker_render_output_matches_status_line_for_human_mode() {
    assert_eq!(
        app_server_broker_render_output(false).unwrap(),
        app_server_broker_status_line()
    );
}

#[test]
fn app_server_broker_command_render_output_matches_status_line_for_human_mode() {
    assert_eq!(
        render_app_server_broker_output(false).unwrap(),
        app_server_broker_status_line()
    );
}

#[test]
fn app_server_broker_render_output_matches_contract_for_json_mode() {
    let rendered = app_server_broker_render_output(true).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(parsed, app_server_broker_contract_json());
}

#[test]
fn app_server_broker_command_render_output_matches_contract_for_json_mode() {
    let rendered = render_app_server_broker_output(true).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(parsed, app_server_broker_contract_json());
}

#[test]
fn app_server_broker_preview_line_reports_valid_jsonrpc_summary() {
    let rendered = app_server_broker_preview_line(
        r#" { "jsonrpc":"2.0", "id":"req-1", "method":"turn/start", "params":{} } "#,
    );
    assert_eq!(rendered["parse_ok"], serde_json::Value::Bool(true));
    assert_eq!(rendered["summary"]["frame_kind"], "request");
    assert_eq!(rendered["summary"]["id"], "req-1");
    assert_eq!(rendered["summary"]["method_kind"], "lifecycle");
}

#[test]
fn app_server_broker_preview_line_redacts_secret_like_jsonrpc_ids() {
    let rendered = app_server_broker_preview_line(
        r#"{"jsonrpc":"2.0","id":"Bearer raw-secret-token","method":"turn/start","params":{}}"#,
    );

    assert_eq!(rendered["parse_ok"], serde_json::Value::Bool(true));
    assert_eq!(rendered["summary"]["id"], "Bearer <redacted>");
    assert!(!serde_json::to_string(&rendered)
        .unwrap()
        .contains("raw-secret-token"));
}

#[test]
fn app_server_broker_preview_line_redacts_secret_like_method_and_metadata() {
    let rendered = app_server_broker_preview_line(
        r#"{"jsonrpc":"2.0","method":"Authorization: Bearer raw-method-token","params":{"session_id":"Bearer raw-session-token"}}"#,
    );

    assert_eq!(rendered["parse_ok"], serde_json::Value::Bool(true));
    assert_eq!(
        rendered["summary"]["method"],
        "Authorization: Bearer <redacted>"
    );
    assert_eq!(
        rendered["summary"]["metadata"]["session_id"],
        "Bearer <redacted>"
    );
    let text = serde_json::to_string(&rendered).unwrap();
    assert!(!text.contains("raw-method-token"));
    assert!(!text.contains("raw-session-token"));
}

#[test]
fn app_server_broker_preview_line_reports_invalid_json_errors() {
    let rendered = app_server_broker_preview_line(r#"{"jsonrpc":"2.0""#);
    assert_eq!(rendered["parse_ok"], serde_json::Value::Bool(false));
    assert_eq!(rendered["error"], "invalid_json");
    assert!(rendered["message"].as_str().is_some_and(|message| !message.is_empty()));
}

#[test]
fn app_server_broker_preview_line_rejects_oversized_input_before_json_parse() {
    let oversized = "x".repeat(crate::app_server_broker::APP_SERVER_BROKER_MAX_PREVIEW_LINE_BYTES + 1);
    let rendered = app_server_broker_preview_line(&oversized);
    assert_eq!(rendered["parse_ok"], serde_json::Value::Bool(false));
    assert_eq!(rendered["error"], "line_too_large");
    assert!(rendered["message"].as_str().is_some_and(|message| {
        message.contains("app-server broker preview line exceeds")
    }));
}

#[test]
fn app_server_broker_preview_lines_skips_blank_lines_and_tracks_line_numbers() {
    let previews = app_server_broker_preview_lines(
        "\n\
         {\"jsonrpc\":\"2.0\",\"id\":\"req-1\",\"method\":\"turn/start\",\"params\":{}}\n\
         \n\
         {\"jsonrpc\":\"2.0\"\n",
    );
    assert_eq!(previews.len(), 2);
    assert_eq!(previews[0]["object"], "app_server_broker.preview_event");
    assert_eq!(previews[0]["line"], 2);
    assert_eq!(previews[0]["preview"]["parse_ok"], serde_json::Value::Bool(true));
    assert_eq!(previews[0]["preview"]["summary"]["frame_kind"], "request");
    assert_eq!(previews[1]["object"], "app_server_broker.preview_event");
    assert_eq!(previews[1]["line"], 4);
    assert_eq!(previews[1]["preview"]["parse_ok"], serde_json::Value::Bool(false));
    assert_eq!(previews[1]["preview"]["error"], "invalid_json");
}

#[test]
fn app_server_broker_preview_report_counts_parsed_and_failed_lines() {
    let report = app_server_broker_preview_report(
        "\n\
         {\"jsonrpc\":\"2.0\",\"id\":\"req-1\",\"method\":\"turn/start\",\"params\":{}}\n\
         {\"jsonrpc\":\"2.0\",\"id\":\"req-2\",\"method\":\"custom/ping\",\"params\":{}}\n\
         {\"jsonrpc\":\"2.0\",\"method\":\"turn/start\",\"result\":{\"ok\":true}}\n\
         {\"jsonrpc\":\"1.0\",\"id\":\"req-3\",\"method\":\"turn/start\"}\n\
         {\"jsonrpc\":\"2.0\",\"id\":\"req-4\",\"result\":{\"ok\":true}}\n",
    );
    assert_eq!(report["line_count"], 5);
    assert_eq!(report["parsed_count"], 5);
    assert_eq!(report["error_count"], 0);
    assert_eq!(report["frame_kind_counts"]["request"], 2);
    assert_eq!(report["frame_kind_counts"]["notification"], 0);
    assert_eq!(report["frame_kind_counts"]["response"], 1);
    assert_eq!(report["frame_kind_counts"]["invalid"], 2);
    assert_eq!(report["method_kind_counts"]["lifecycle"], 3);
    assert_eq!(report["method_kind_counts"]["other"], 1);
    assert_eq!(report["method_kind_counts"]["absent"], 1);
    assert_eq!(report["invalid_reason_counts"]["non_jsonrpc_version"], 1);
    assert_eq!(report["invalid_reason_counts"]["batch_frame_unsupported"], 0);
    assert_eq!(report["invalid_reason_counts"]["non_object_frame"], 0);
    assert_eq!(report["invalid_reason_counts"]["non_scalar_id"], 0);
    assert_eq!(report["invalid_reason_counts"]["non_container_params"], 0);
    assert_eq!(report["invalid_reason_counts"]["non_object_error"], 0);
    assert_eq!(report["invalid_reason_counts"]["non_integer_error_code"], 0);
    assert_eq!(
        report["invalid_reason_counts"]["non_string_error_message"],
        0
    );
    assert_eq!(report["invalid_reason_counts"]["non_string_method"], 0);
    assert_eq!(report["invalid_reason_counts"]["invalid_method_name"], 0);
    assert_eq!(report["invalid_reason_counts"]["result_with_error"], 0);
    assert_eq!(report["invalid_reason_counts"]["missing_response_id"], 0);
    assert_eq!(
        report["invalid_reason_counts"]["method_with_result_or_error"],
        1
    );
    assert_eq!(
        report["invalid_reason_counts"]["missing_method_and_response_payload"],
        0
    );
    assert_eq!(report["previews"].as_array().unwrap().len(), 5);
    assert_eq!(report["previews"][0]["preview"]["summary"]["frame_kind"], "request");
    assert_eq!(report["previews"][2]["preview"]["summary"]["frame_kind"], "invalid");
    assert_eq!(report["previews"][3]["preview"]["summary"]["frame_kind"], "invalid");
    assert_eq!(report["previews"][4]["preview"]["summary"]["frame_kind"], "response");
}

#[test]
fn app_server_broker_preview_report_matches_fixture_corpus() {
    let cases = app_server_broker_fixture_cases();
    let input = cases
        .iter()
        .map(|case| serde_json::to_string(&case.payload).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    let report = app_server_broker_preview_report(&input);

    let request_count = cases
        .iter()
        .filter(|case| case.expect.frame_kind == "request")
        .count();
    let notification_count = cases
        .iter()
        .filter(|case| case.expect.frame_kind == "notification")
        .count();
    let response_count = cases
        .iter()
        .filter(|case| case.expect.frame_kind == "response")
        .count();
    let invalid_count = cases
        .iter()
        .filter(|case| case.expect.frame_kind == "invalid")
        .count();
    let lifecycle_count = cases
        .iter()
        .filter(|case| case.expect.method_kind == "lifecycle")
        .count();
    let other_count = cases
        .iter()
        .filter(|case| case.expect.method_kind == "other")
        .count();
    let absent_count = cases
        .iter()
        .filter(|case| case.expect.method_kind == "absent")
        .count();
    let non_jsonrpc_version_count = cases
        .iter()
        .filter(|case| case.expect.invalid_reason.as_deref() == Some("non_jsonrpc_version"))
        .count();
    let non_object_frame_count = cases
        .iter()
        .filter(|case| case.expect.invalid_reason.as_deref() == Some("non_object_frame"))
        .count();
    let batch_frame_unsupported_count = cases
        .iter()
        .filter(|case| {
            case.expect.invalid_reason.as_deref() == Some("batch_frame_unsupported")
        })
        .count();
    let non_scalar_id_count = cases
        .iter()
        .filter(|case| case.expect.invalid_reason.as_deref() == Some("non_scalar_id"))
        .count();
    let non_container_params_count = cases
        .iter()
        .filter(|case| {
            case.expect.invalid_reason.as_deref() == Some("non_container_params")
        })
        .count();
    let non_object_error_count = cases
        .iter()
        .filter(|case| case.expect.invalid_reason.as_deref() == Some("non_object_error"))
        .count();
    let non_integer_error_code_count = cases
        .iter()
        .filter(|case| {
            case.expect.invalid_reason.as_deref() == Some("non_integer_error_code")
        })
        .count();
    let non_string_error_message_count = cases
        .iter()
        .filter(|case| {
            case.expect.invalid_reason.as_deref() == Some("non_string_error_message")
        })
        .count();
    let non_string_method_count = cases
        .iter()
        .filter(|case| case.expect.invalid_reason.as_deref() == Some("non_string_method"))
        .count();
    let invalid_method_name_count = cases
        .iter()
        .filter(|case| case.expect.invalid_reason.as_deref() == Some("invalid_method_name"))
        .count();
    let result_with_error_count = cases
        .iter()
        .filter(|case| case.expect.invalid_reason.as_deref() == Some("result_with_error"))
        .count();
    let missing_response_id_count = cases
        .iter()
        .filter(|case| case.expect.invalid_reason.as_deref() == Some("missing_response_id"))
        .count();
    let method_with_result_or_error_count = cases
        .iter()
        .filter(|case| {
            case.expect.invalid_reason.as_deref() == Some("method_with_result_or_error")
        })
        .count();
    let missing_method_and_response_payload_count = cases
        .iter()
        .filter(|case| {
            case.expect.invalid_reason.as_deref()
                == Some("missing_method_and_response_payload")
        })
        .count();

    assert_eq!(report["line_count"], cases.len());
    assert_eq!(report["parsed_count"], cases.len());
    assert_eq!(report["error_count"], 0);
    assert_eq!(report["frame_kind_counts"]["request"], request_count);
    assert_eq!(report["frame_kind_counts"]["notification"], notification_count);
    assert_eq!(report["frame_kind_counts"]["response"], response_count);
    assert_eq!(report["frame_kind_counts"]["invalid"], invalid_count);
    assert_eq!(report["method_kind_counts"]["lifecycle"], lifecycle_count);
    assert_eq!(report["method_kind_counts"]["other"], other_count);
    assert_eq!(report["method_kind_counts"]["absent"], absent_count);
    assert_eq!(
        report["invalid_reason_counts"]["non_jsonrpc_version"],
        non_jsonrpc_version_count
    );
    assert_eq!(
        report["invalid_reason_counts"]["non_object_frame"],
        non_object_frame_count
    );
    assert_eq!(
        report["invalid_reason_counts"]["batch_frame_unsupported"],
        batch_frame_unsupported_count
    );
    assert_eq!(
        report["invalid_reason_counts"]["non_string_method"],
        non_string_method_count
    );
    assert_eq!(
        report["invalid_reason_counts"]["invalid_method_name"],
        invalid_method_name_count
    );
    assert_eq!(
        report["invalid_reason_counts"]["non_scalar_id"],
        non_scalar_id_count
    );
    assert_eq!(
        report["invalid_reason_counts"]["non_container_params"],
        non_container_params_count
    );
    assert_eq!(
        report["invalid_reason_counts"]["non_object_error"],
        non_object_error_count
    );
    assert_eq!(
        report["invalid_reason_counts"]["non_integer_error_code"],
        non_integer_error_code_count
    );
    assert_eq!(
        report["invalid_reason_counts"]["non_string_error_message"],
        non_string_error_message_count
    );
    assert_eq!(
        report["invalid_reason_counts"]["result_with_error"],
        result_with_error_count
    );
    assert_eq!(
        report["invalid_reason_counts"]["missing_response_id"],
        missing_response_id_count
    );
    assert_eq!(
        report["invalid_reason_counts"]["method_with_result_or_error"],
        method_with_result_or_error_count
    );
    assert_eq!(
        report["invalid_reason_counts"]["missing_method_and_response_payload"],
        missing_method_and_response_payload_count
    );
    assert_eq!(report["previews"].as_array().unwrap().len(), cases.len());
    assert_eq!(
        report["previews"][0]["object"],
        "app_server_broker.preview_event"
    );
}

#[test]
fn app_server_broker_render_stdio_preview_matches_expected_replay_fixture() {
    let rendered =
        app_server_broker_render_stdio_preview(app_server_broker_stdio_preview_replay_fixture())
            .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(
        parsed["object"],
        "app_server_broker.preview_report"
    );
    assert_eq!(
        parsed["report"],
        app_server_broker_stdio_preview_expected_report_fixture()
    );
}

#[test]
fn app_server_broker_render_stdio_preview_matches_wire_replay_fixture() {
    let rendered =
        app_server_broker_render_stdio_preview(app_server_broker_stdio_preview_wire_replay_fixture())
            .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(parsed["object"], "app_server_broker.preview_report");
    assert_eq!(
        parsed["report"],
        app_server_broker_stdio_preview_wire_expected_report_fixture()
    );
}

#[test]
fn app_server_broker_write_stdio_preview_stream_matches_fixture_corpus() {
    let cases = app_server_broker_fixture_cases();
    let input = cases
        .iter()
        .map(|case| serde_json::to_string(&case.payload).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    let mut output = Vec::new();

    app_server_broker_write_stdio_preview_stream(
        std::io::Cursor::new(format!("{input}\n")),
        &mut output,
    )
    .unwrap();

    let rendered = String::from_utf8(output).unwrap();
    let lines: Vec<&str> = rendered.lines().collect();
    assert_eq!(lines.len(), cases.len() + 1);

    for (index, case) in cases.iter().enumerate() {
        let event: serde_json::Value = serde_json::from_str(lines[index]).unwrap();
        assert_eq!(event["object"], "app_server_broker.preview_event");
        assert_eq!(event["line"], index + 1);
        assert_eq!(
            event["preview"],
            serde_json::json!({
                "parse_ok": true,
                "summary": app_server_broker_diagnostic_summary_json(&case.payload),
            }),
            "{}",
            case.name
        );
    }

    let summary: serde_json::Value = serde_json::from_str(lines[cases.len()]).unwrap();
    assert_eq!(summary["object"], "app_server_broker.preview_report");
    assert_eq!(summary["report"], app_server_broker_preview_report(&input));
}

#[test]
fn app_server_broker_write_stdio_preview_stream_matches_expected_replay_fixture() {
    let replay = app_server_broker_stdio_preview_replay_fixture();
    let expected_report = app_server_broker_stdio_preview_expected_report_fixture();
    let mut output = Vec::new();

    app_server_broker_write_stdio_preview_stream(std::io::Cursor::new(replay), &mut output)
        .unwrap();

    let rendered = String::from_utf8(output).unwrap();
    let lines: Vec<&str> = rendered.lines().collect();
    let previews = expected_report["previews"].as_array().unwrap();
    assert_eq!(lines.len(), previews.len() + 1);

    for (index, expected) in previews.iter().enumerate() {
        let event: serde_json::Value = serde_json::from_str(lines[index]).unwrap();
        assert_eq!(event, *expected);
    }

    let summary: serde_json::Value = serde_json::from_str(lines[previews.len()]).unwrap();
    assert_eq!(summary["object"], "app_server_broker.preview_report");
    assert_eq!(summary["report"], expected_report);
}

#[test]
fn app_server_broker_write_stdio_preview_stream_matches_expected_stream_fixture() {
    let replay = app_server_broker_stdio_preview_replay_fixture();
    let mut output = Vec::new();

    app_server_broker_write_stdio_preview_stream(std::io::Cursor::new(replay), &mut output)
        .unwrap();

    let rendered = String::from_utf8(output).unwrap();
    assert_eq!(
        rendered,
        app_server_broker_stdio_preview_expected_stream_fixture()
    );
}

#[test]
fn app_server_broker_write_stdio_preview_stream_matches_wire_stream_fixture() {
    let replay = app_server_broker_stdio_preview_wire_replay_fixture();
    let mut output = Vec::new();

    app_server_broker_write_stdio_preview_stream(std::io::Cursor::new(replay), &mut output)
        .unwrap();

    let rendered = String::from_utf8(output).unwrap();
    assert_eq!(
        rendered,
        app_server_broker_stdio_preview_wire_expected_stream_fixture()
    );
}

#[test]
fn app_server_broker_preview_lines_match_streamed_preview_events_for_replays() {
    for (name, replay) in [
        ("valid_replay", app_server_broker_stdio_preview_replay_fixture()),
        (
            "malformed_replay",
            app_server_broker_stdio_preview_malformed_replay_fixture(),
        ),
        (
            "wire_replay",
            app_server_broker_stdio_preview_wire_replay_fixture(),
        ),
    ] {
        let expected_previews = app_server_broker_preview_lines(replay);
        let mut output = Vec::new();
        app_server_broker_write_stdio_preview_stream(std::io::Cursor::new(replay), &mut output)
            .unwrap();

        let rendered = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = rendered.lines().collect();
        assert_eq!(lines.len(), expected_previews.len() + 1, "{name}");

        for (index, expected) in expected_previews.iter().enumerate() {
            let streamed: serde_json::Value = serde_json::from_str(lines[index]).unwrap();
            assert_eq!(streamed, *expected, "{name}[{index}]");
        }

        let summary: serde_json::Value =
            serde_json::from_str(lines[expected_previews.len()]).unwrap();
        assert_eq!(summary["object"], "app_server_broker.preview_report", "{name}");
        assert_eq!(
            summary["report"],
            app_server_broker_preview_report(replay),
            "{name}"
        );
    }
}

#[test]
fn app_server_broker_write_stdio_passthrough_preview_stream_mirrors_replay_fixture() {
    let replay = app_server_broker_stdio_preview_replay_fixture();
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_passthrough_preview_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .unwrap();

    assert_eq!(String::from_utf8(passthrough).unwrap(), replay);
    assert_eq!(
        String::from_utf8(diagnostics).unwrap(),
        app_server_broker_stdio_preview_expected_stream_fixture()
    );
}

#[test]
fn app_server_broker_write_stdio_passthrough_preview_stream_mirrors_wire_replay_fixture() {
    let replay = app_server_broker_stdio_preview_wire_replay_fixture();
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_passthrough_preview_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .unwrap();

    assert_eq!(String::from_utf8(passthrough).unwrap(), replay);
    assert_eq!(
        String::from_utf8(diagnostics).unwrap(),
        app_server_broker_stdio_preview_wire_expected_stream_fixture()
    );
}

#[test]
fn app_server_broker_write_stdio_passthrough_preview_stream_logs_runtime_metadata() {
    let _runtime_lock = acquire_test_runtime_lock();
    let _env_lock = TestEnvVarGuard::lock();
    let log_dir = AppServerBrokerRuntimeLogTestDir::new();
    let _log_dir = TestEnvVarGuard::set(
        "PRODEX_RUNTIME_LOG_DIR",
        log_dir.path.to_str().expect("log dir should be utf-8"),
    );
    let input = "{\"jsonrpc\":\"2.0\",\"id\":\"req-1\",\"method\":\"turn/start\",\"params\":{\"session_id\":\"sess-1\",\"thread_id\":\"thread-1\",\"turn_id\":\"turn-1\"}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_passthrough_preview_stream(
        std::io::Cursor::new(input),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect("passthrough preview should succeed");

    assert_eq!(String::from_utf8(passthrough).unwrap(), input);
    let pointer = std::fs::read_to_string(runtime_proxy_latest_log_pointer_path())
        .expect("latest runtime log pointer should exist");
    let log_path = std::path::PathBuf::from(pointer.trim());
    runtime_proxy_flush_logs_for_path(&log_path).expect("runtime log should flush");
    let log = std::fs::read_to_string(&log_path).expect("runtime log should be readable");
    assert!(log.contains("app_server_broker_observe"));
    assert!(log.contains("line=1"));
    assert!(log.contains("frame_kind=request"));
    assert!(log.contains("method=turn/start"));
    assert!(log.contains("session_id=sess-1"));
    assert!(log.contains("thread_id=thread-1"));
    assert!(log.contains("turn_id=turn-1"));
}

#[test]
fn app_server_broker_runtime_log_redacts_secret_like_jsonrpc_ids() {
    let _runtime_lock = acquire_test_runtime_lock();
    let _env_lock = TestEnvVarGuard::lock();
    let log_dir = AppServerBrokerRuntimeLogTestDir::new();
    let _log_dir = TestEnvVarGuard::set(
        "PRODEX_RUNTIME_LOG_DIR",
        log_dir.path.to_str().expect("log dir should be utf-8"),
    );
    let input = "{\"jsonrpc\":\"2.0\",\"id\":\"Bearer raw-secret-token\",\"method\":\"turn/start\",\"params\":{\"session_id\":\"Bearer raw-session-token\",\"turn_id\":\"turn-1\"}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_passthrough_preview_stream(
        std::io::Cursor::new(input),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect("passthrough preview should succeed");

    let pointer = std::fs::read_to_string(runtime_proxy_latest_log_pointer_path())
        .expect("latest runtime log pointer should exist");
    let log_path = std::path::PathBuf::from(pointer.trim());
    runtime_proxy_flush_logs_for_path(&log_path).expect("runtime log should flush");
    let log = std::fs::read_to_string(&log_path).expect("runtime log should be readable");
    assert!(!log.contains("raw-secret-token"), "log contents:\n{log}");
    assert!(!log.contains("raw-session-token"), "log contents:\n{log}");

    let observe_line = log
        .lines()
        .find(|line| {
            runtime_proxy_crate::runtime_proxy_log_event(runtime_log_message_body(line))
                == Some("app_server_broker_observe")
        })
        .expect("observe log line should exist");
    let fields = runtime_proxy_crate::runtime_proxy_log_fields(runtime_log_message_body(
        observe_line,
    ));
    assert_eq!(
        fields.get("frame_id").map(String::as_str),
        Some("Bearer <redacted>")
    );
    assert_eq!(
        fields.get("request_id").map(String::as_str),
        Some("Bearer <redacted>")
    );
    assert_eq!(
        fields.get("session_id").map(String::as_str),
        Some("Bearer <redacted>")
    );
}

#[test]
fn app_server_broker_write_stdio_passthrough_preview_stream_emits_empty_summary_for_empty_input() {
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_passthrough_preview_stream(
        std::io::Cursor::new(""),
        &mut passthrough,
        &mut diagnostics,
    )
    .unwrap();

    assert_eq!(String::from_utf8(passthrough).unwrap(), "");

    let rendered = String::from_utf8(diagnostics).unwrap();
    let lines: Vec<&str> = rendered.lines().collect();
    assert_eq!(lines.len(), 1);
    let summary: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(summary["object"], "app_server_broker.preview_report");
    assert_eq!(summary["report"]["line_count"], 0);
    assert_eq!(summary["report"]["parsed_count"], 0);
    assert_eq!(summary["report"]["error_count"], 0);
    assert_eq!(summary["report"]["previews"].as_array().unwrap().len(), 0);
}

#[test]
fn app_server_broker_write_stdio_passthrough_preview_stream_wraps_passthrough_failures() {
    struct FailingWriter;

    impl std::io::Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "synthetic passthrough write failure",
            ))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let err = app_server_broker_write_stdio_passthrough_preview_stream(
        std::io::Cursor::new(app_server_broker_stdio_preview_replay_fixture()),
        FailingWriter,
        Vec::new(),
    )
    .expect_err("passthrough write failures should be surfaced");

    assert!(err.to_string().contains("synthetic passthrough write failure"));
}

#[test]
fn app_server_broker_write_stdio_passthrough_preview_stream_wraps_diagnostics_failures() {
    struct FailingWriter;

    impl std::io::Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "synthetic diagnostics write failure",
            ))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let err = app_server_broker_write_stdio_passthrough_preview_stream(
        std::io::Cursor::new(app_server_broker_stdio_preview_replay_fixture()),
        Vec::new(),
        FailingWriter,
    )
    .expect_err("diagnostics write failures should be surfaced");

    assert!(err.to_string().contains("synthetic diagnostics write failure"));
}

#[test]
fn app_server_broker_command_stdio_passthrough_preview_mirrors_replay_fixture() {
    let replay = app_server_broker_stdio_preview_replay_fixture();
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    run_app_server_broker_stdio_passthrough_preview(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .unwrap();

    assert_eq!(String::from_utf8(passthrough).unwrap(), replay);
    assert_eq!(
        String::from_utf8(diagnostics).unwrap(),
        app_server_broker_stdio_preview_expected_stream_fixture()
    );
}

#[test]
fn app_server_broker_command_stdio_passthrough_preview_emits_empty_summary_for_empty_input() {
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    run_app_server_broker_stdio_passthrough_preview(
        std::io::Cursor::new(""),
        &mut passthrough,
        &mut diagnostics,
    )
    .unwrap();

    assert_eq!(String::from_utf8(passthrough).unwrap(), "");

    let rendered = String::from_utf8(diagnostics).unwrap();
    let lines: Vec<&str> = rendered.lines().collect();
    assert_eq!(lines.len(), 1);
    let summary: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(summary["object"], "app_server_broker.preview_report");
    assert_eq!(summary["report"]["line_count"], 0);
    assert_eq!(summary["report"]["parsed_count"], 0);
    assert_eq!(summary["report"]["error_count"], 0);
    assert_eq!(summary["report"]["previews"].as_array().unwrap().len(), 0);
}

#[test]
fn app_server_broker_command_stdio_passthrough_preview_wraps_passthrough_failures() {
    struct FailingWriter;

    impl std::io::Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "synthetic passthrough write failure",
            ))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let err = run_app_server_broker_stdio_passthrough_preview(
        std::io::Cursor::new(app_server_broker_stdio_preview_replay_fixture()),
        FailingWriter,
        Vec::new(),
    )
    .expect_err("passthrough write failures should be surfaced");

    assert!(
        err.to_string()
            .contains("failed to stream app-server broker stdio preview")
    );
}

#[test]
fn app_server_broker_command_stdio_passthrough_preview_wraps_diagnostics_failures() {
    struct FailingWriter;

    impl std::io::Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "synthetic diagnostics write failure",
            ))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let err = run_app_server_broker_stdio_passthrough_preview(
        std::io::Cursor::new(app_server_broker_stdio_preview_replay_fixture()),
        Vec::new(),
        FailingWriter,
    )
    .expect_err("diagnostics write failures should be surfaced");

    assert!(
        err.to_string()
            .contains("failed to stream app-server broker stdio preview")
    );
}

#[test]
fn app_server_broker_command_stdio_preview_matches_expected_stream_fixture() {
    let replay = app_server_broker_stdio_preview_replay_fixture();
    let mut output = Vec::new();

    run_app_server_broker_stdio_preview(std::io::Cursor::new(replay), &mut output).unwrap();

    let rendered = String::from_utf8(output).unwrap();
    assert_eq!(
        rendered,
        app_server_broker_stdio_preview_expected_stream_fixture()
    );
}

#[test]
fn app_server_broker_command_stdio_validate_matches_expected_stream_fixture() {
    let replay = app_server_broker_upstream_schema_replay_fixture();
    let mut output = Vec::new();

    run_app_server_broker_stdio_validate(std::io::Cursor::new(replay), &mut output).unwrap();

    assert_eq!(
        String::from_utf8(output).unwrap(),
        app_server_broker_upstream_schema_expected_stream_fixture()
    );
}

#[test]
fn app_server_broker_command_stdio_validate_wraps_validation_failure() {
    let replay = app_server_broker_stdio_preview_malformed_replay_fixture();
    let mut output = Vec::new();

    let err = run_app_server_broker_stdio_validate(std::io::Cursor::new(replay), &mut output)
        .expect_err("malformed replay should fail validation");

    let message = err.to_string();
    assert!(message.contains("failed to validate app-server broker stdio stream"));
    assert_eq!(
        String::from_utf8(output).unwrap(),
        app_server_broker_stdio_preview_malformed_expected_stream_fixture()
    );
}

#[test]
fn app_server_broker_command_stdio_preview_wraps_stream_write_failures() {
    struct FailingWriter;

    impl std::io::Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "synthetic write failure",
            ))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let err = run_app_server_broker_stdio_preview(
        std::io::Cursor::new(app_server_broker_stdio_preview_replay_fixture()),
        FailingWriter,
    )
    .expect_err("write failures should be surfaced");

    assert!(
        err.to_string()
            .contains("failed to stream app-server broker stdio preview")
    );
}

#[test]
fn app_server_broker_render_stdio_preview_matches_malformed_replay_fixture() {
    let rendered = app_server_broker_render_stdio_preview(
        app_server_broker_stdio_preview_malformed_replay_fixture(),
    )
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&rendered).unwrap();
    assert_eq!(
        parsed["object"],
        "app_server_broker.preview_report"
    );
    assert_eq!(
        parsed["report"],
        app_server_broker_stdio_preview_malformed_expected_report_fixture()
    );
}

#[test]
fn app_server_broker_write_stdio_preview_stream_matches_malformed_stream_fixture() {
    let replay = app_server_broker_stdio_preview_malformed_replay_fixture();
    let mut output = Vec::new();

    app_server_broker_write_stdio_preview_stream(std::io::Cursor::new(replay), &mut output)
        .unwrap();

    let rendered = String::from_utf8(output).unwrap();
    assert_eq!(
        rendered,
        app_server_broker_stdio_preview_malformed_expected_stream_fixture()
    );
}

#[test]
fn app_server_broker_write_stdio_passthrough_preview_stream_mirrors_malformed_replay_fixture() {
    let replay = app_server_broker_stdio_preview_malformed_replay_fixture();
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    app_server_broker_write_stdio_passthrough_preview_stream(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .unwrap();

    assert_eq!(String::from_utf8(passthrough).unwrap(), replay);
    assert_eq!(
        String::from_utf8(diagnostics).unwrap(),
        app_server_broker_stdio_preview_malformed_expected_stream_fixture()
    );
}

#[test]
fn app_server_broker_command_stdio_passthrough_preview_mirrors_malformed_replay_fixture() {
    let replay = app_server_broker_stdio_preview_malformed_replay_fixture();
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    run_app_server_broker_stdio_passthrough_preview(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .unwrap();

    assert_eq!(String::from_utf8(passthrough).unwrap(), replay);
    assert_eq!(
        String::from_utf8(diagnostics).unwrap(),
        app_server_broker_stdio_preview_malformed_expected_stream_fixture()
    );
}

#[test]
fn app_server_broker_command_stdio_live_mirrors_replay_fixture() {
    let replay = app_server_broker_upstream_schema_replay_fixture();
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    run_app_server_broker_stdio_live(std::io::Cursor::new(replay), &mut passthrough, &mut diagnostics)
        .unwrap();

    assert_eq!(String::from_utf8(passthrough).unwrap(), replay);
    assert_eq!(
        String::from_utf8(diagnostics).unwrap(),
        app_server_broker_upstream_schema_expected_stream_fixture()
    );
}

#[test]
fn app_server_broker_command_stdio_live_forwards_before_reader_eof() {
    struct FailingReader;

    impl std::io::Read for FailingReader {
        fn read(&mut self, _buffer: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("synthetic read failure"))
        }
    }

    let first = "{\"jsonrpc\":\"2.0\",\"method\":\"custom/event\",\"params\":{}}\n";
    let reader = std::io::BufReader::new(std::io::Cursor::new(first).chain(FailingReader));
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    run_app_server_broker_stdio_live(reader, &mut passthrough, &mut diagnostics)
        .expect_err("reader failure should stop live mode");

    assert_eq!(String::from_utf8(passthrough).unwrap(), first);
    assert!(String::from_utf8(diagnostics).unwrap().contains("custom/event"));
}

#[test]
fn app_server_broker_command_stdio_live_wraps_validation_failure() {
    let replay = "\
{\"jsonrpc\":\"2.0\",\"id\":\"req-1\",\"method\":\"custom/ping\",\"params\":{}}\n\
\n\
{\"jsonrpc\":\"2.0\"\n\
{\"jsonrpc\":\"2.0\",\"id\":\"resp-1\",\"result\":{\"ok\":true}}\n";
    let mut passthrough = Vec::new();
    let mut diagnostics = Vec::new();

    let err = run_app_server_broker_stdio_live(
        std::io::Cursor::new(replay),
        &mut passthrough,
        &mut diagnostics,
    )
    .expect_err("malformed replay should fail live mode");

    assert!(
        err.to_string()
            .contains("failed to run app-server broker stdio live mode")
    );
    assert_eq!(
        String::from_utf8(passthrough).unwrap(),
        "{\"jsonrpc\":\"2.0\",\"id\":\"req-1\",\"method\":\"custom/ping\",\"params\":{}}\n\n"
    );
    let diagnostics = String::from_utf8(diagnostics).unwrap();
    assert!(diagnostics.contains("\"error\":\"invalid_json\""));
    assert!(diagnostics.contains("\"line\":3"));
    assert!(!diagnostics.contains("resp-1"));
}

#[test]
fn app_server_broker_command_stdio_preview_matches_malformed_stream_fixture() {
    let replay = app_server_broker_stdio_preview_malformed_replay_fixture();
    let mut output = Vec::new();

    run_app_server_broker_stdio_preview(std::io::Cursor::new(replay), &mut output).unwrap();

    let rendered = String::from_utf8(output).unwrap();
    assert_eq!(
        rendered,
        app_server_broker_stdio_preview_malformed_expected_stream_fixture()
    );
}

#[test]
fn app_server_broker_extracts_metadata_from_common_jsonrpc_shapes() {
    let request = parse_app_server_broker_request(&serde_json::json!({
        "jsonrpc": "2.0",
        "id": "req-1",
        "method": "turn/start",
        "params": {
            "sessionId": "sess-1",
            "thread": {"id": "thread-1"},
            "turn": {"id": "turn-1"},
            "item": {"id": "item-1"}
        }
    }))
    .expect("valid request should parse");

    assert_eq!(request.metadata.session_id.as_deref(), Some("sess-1"));
    assert_eq!(request.metadata.thread_id.as_deref(), Some("thread-1"));
    assert_eq!(request.metadata.turn_id.as_deref(), Some("turn-1"));
    assert_eq!(request.metadata.item_id.as_deref(), Some("item-1"));
}

#[test]
fn app_server_broker_extracts_client_metadata_ids_when_present() {
    let metadata = app_server_broker_metadata_from_value(&serde_json::json!({
        "jsonrpc": "2.0",
        "method": "thread/resume",
        "params": {
            "client_metadata": {
                "session_id": "sess-2",
                "thread_id": "thread-2",
                "turn_id": "turn-2",
                "item_id": "item-2"
            }
        }
    }));

    assert_eq!(metadata.session_id.as_deref(), Some("sess-2"));
    assert_eq!(metadata.thread_id.as_deref(), Some("thread-2"));
    assert_eq!(metadata.turn_id.as_deref(), Some("turn-2"));
    assert_eq!(metadata.item_id.as_deref(), Some("item-2"));
}

#[test]
fn app_server_broker_classifies_request_notification_response_and_invalid_frames() {
    assert_eq!(
        app_server_broker_frame_kind(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": "req-1",
            "method": "turn/start",
            "params": {}
        })),
        AppServerBrokerFrameKind::Request
    );
    assert_eq!(
        app_server_broker_frame_kind(&serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
            "params": {}
        })),
        AppServerBrokerFrameKind::Notification
    );
    assert_eq!(
        app_server_broker_frame_kind(&serde_json::json!({
            "jsonrpc": "2.0",
            "id": "req-1",
            "result": {"ok": true}
        })),
        AppServerBrokerFrameKind::Response
    );
    assert_eq!(
        app_server_broker_frame_kind(&serde_json::json!({
            "jsonrpc": "1.0",
            "method": "turn/start"
        })),
        AppServerBrokerFrameKind::Invalid
    );
}

#[test]
fn app_server_broker_diagnostic_summary_marks_invalid_envelopes_without_metadata() {
    let summary = app_server_broker_diagnostic_summary(&serde_json::json!({
        "jsonrpc": "1.0",
        "method": "turn/start",
        "params": {
            "thread": {"id": "thread-missing-jsonrpc"}
        }
    }));

    assert!(!summary.valid_jsonrpc);
    assert_eq!(summary.frame_kind, AppServerBrokerFrameKind::Invalid);
    assert_eq!(summary.method.as_deref(), Some("turn/start"));
    assert!(summary.metadata.is_empty());
}

#[test]
fn app_server_broker_diagnostic_summary_keeps_method_and_sparse_metadata() {
    let summary = app_server_broker_diagnostic_summary(&serde_json::json!({
        "jsonrpc": "2.0",
        "method": "thread/start",
        "params": {
            "context": {
                "threadId": "thread-ctx-1"
            }
        }
    }));

    assert!(summary.valid_jsonrpc);
    assert_eq!(summary.frame_kind, AppServerBrokerFrameKind::Notification);
    assert_eq!(summary.method.as_deref(), Some("thread/start"));
    assert_eq!(summary.metadata.thread_id.as_deref(), Some("thread-ctx-1"));
    assert!(summary.metadata.session_id.is_none());
    assert!(summary.metadata.turn_id.is_none());
    assert!(summary.metadata.item_id.is_none());
}

#[test]
fn app_server_broker_malformed_envelope_matrix_stays_invalid_and_metadata_free() {
    for payload in [
        serde_json::json!(null),
        serde_json::json!([]),
        serde_json::json!({"jsonrpc":"2.0"}),
        serde_json::json!({"jsonrpc":"2.0","id":"req-only"}),
        serde_json::json!({"jsonrpc":"2.0","method":17}),
        serde_json::json!({"jsonrpc":"2.0","result":{"ok":true},"method":"turn/start"}),
    ] {
        let summary = app_server_broker_diagnostic_summary(&payload);
        assert_eq!(summary.frame_kind, AppServerBrokerFrameKind::Invalid);
        assert!(summary.metadata.is_empty(), "{payload}");
    }

    let invalid_but_informative = app_server_broker_diagnostic_summary(&serde_json::json!({
        "jsonrpc":"2.0",
        "params":{"sessionId":"sess-1"}
    }));
    assert_eq!(invalid_but_informative.frame_kind, AppServerBrokerFrameKind::Invalid);
    assert_eq!(
        invalid_but_informative.metadata.session_id.as_deref(),
        Some("sess-1")
    );
}

#[test]
fn app_server_broker_invalid_reason_matrix_matches_contract_taxonomy() {
    for (payload, expected) in [
        (
            serde_json::json!({"jsonrpc":"1.0","method":"turn/start"}),
            Some("non_jsonrpc_version"),
        ),
        (
            serde_json::json!({"jsonrpc":"2.0","method":"turn/start","result":{"ok":true}}),
            Some("method_with_result_or_error"),
        ),
        (
            serde_json::json!({"jsonrpc":"2.0","params":{"sessionId":"sess-1"}}),
            Some("missing_method_and_response_payload"),
        ),
        (
            serde_json::json!({"jsonrpc":"2.0","id":"req-1","method":"turn/start","params":{}}),
            None,
        ),
    ] {
        assert_eq!(app_server_broker_invalid_reason(&payload), expected, "{payload}");
    }
}
