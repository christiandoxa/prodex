use super::*;

#[test]
fn presidio_transport_policy_contract_is_mojo_authoritative() {
    assert!(!presidio_fail_closed(false, false, false, false, false).unwrap());
    assert!(presidio_fail_closed(true, false, false, false, false).unwrap());
    assert!(presidio_fail_closed(false, true, false, false, false).unwrap());
    assert!(presidio_fail_closed(false, false, true, false, false).unwrap());
    assert!(presidio_fail_closed(false, false, false, true, false).unwrap());
    assert!(presidio_fail_closed(false, false, false, false, true).unwrap());
    assert!(!presidio_local_inspection_required(true, false, false, false).unwrap());
    assert!(presidio_local_inspection_required(false, false, false, false).unwrap());
    assert!(presidio_external_coverage_denied(true, false).unwrap());
    assert!(!presidio_external_coverage_denied(true, true).unwrap());
    assert!(!presidio_external_coverage_denied(false, false).unwrap());
}

#[test]
fn redaction_kernel_smoke() {
    assert!(key_looks_sensitive("X-API-Key").unwrap());
    assert_eq!(
        json_field_plan("arguments").unwrap(),
        JsonFieldPlan {
            inspection_mode: JsonInspectionMode::AllStrings,
            sensitive_kind: JsonSensitiveKind::None,
            known_protocol_metadata: false,
            unsupported_modality: false,
            tools_field: false,
        }
    );
    assert_eq!(
        json_field_plan("private_key").unwrap().sensitive_kind,
        JsonSensitiveKind::PrivateKey
    );
    assert!(json_field_plan("model").unwrap().known_protocol_metadata);
    assert!(json_field_plan("input_image").unwrap().unsupported_modality);
    assert!(json_field_plan("tools").unwrap().tools_field);
    assert_eq!(
        redact_secret_like_text("Authorization: Bearer synthetic-token").unwrap(),
        "Authorization: Bearer <redacted>"
    );
    assert_eq!(
        redact_gateway_text("user@example.test 4111-1111-1111-1111").unwrap(),
        "<redacted><redacted>"
    );
}

#[test]
fn local_inspection_masks_supported_values_and_returns_byte_ranges() {
    let value = "héllo user@example.test | Bearer token-123 | sk-proj-1234567890 | card 4111-1111-1111-1111";
    let redaction = local_inspect_and_redact(value, None, 8).unwrap();

    assert_eq!(
        redaction.text,
        "héllo <redacted> | Bearer <redacted> | <redacted> | card <redacted>"
    );
    let range = |needle: &str| {
        let start = value.find(needle).unwrap();
        (start, start + needle.len())
    };
    let (email_start, email_end) = range("user@example.test");
    let (token_start, token_end) = range("token-123");
    let (api_key_start, api_key_end) = range("sk-proj-1234567890");
    let (account_start, account_end) = range("4111-1111-1111-1111");
    assert_eq!(
        redaction.matches,
        vec![
            LocalInspectionMatch {
                start: email_start,
                end: email_end,
                kind: LocalInspectionFindingKind::EmailAddress
            },
            LocalInspectionMatch {
                start: token_start,
                end: token_end,
                kind: LocalInspectionFindingKind::AccessToken
            },
            LocalInspectionMatch {
                start: api_key_start,
                end: api_key_end,
                kind: LocalInspectionFindingKind::ApiKey
            },
            LocalInspectionMatch {
                start: account_start,
                end: account_end,
                kind: LocalInspectionFindingKind::FinancialAccount
            },
        ]
    );
}

#[test]
fn local_inspection_resolves_overlaps_and_sensitive_field_override() {
    let labeled = local_inspect_and_redact("prefix::sk-aaaaaaaa", None, 8).unwrap();
    assert_eq!(labeled.text, "prefix::<redacted>");
    assert_eq!(
        labeled.matches,
        vec![LocalInspectionMatch {
            start: 8,
            end: 19,
            kind: LocalInspectionFindingKind::ApiKey,
        }]
    );

    let sensitive = local_inspect_and_redact(
        "tenant value",
        Some(LocalInspectionFindingKind::TenantSensitive),
        8,
    )
    .unwrap();
    assert_eq!(sensitive.text, "<redacted>");
    assert_eq!(
        sensitive.matches,
        vec![LocalInspectionMatch {
            start: 0,
            end: 12,
            kind: LocalInspectionFindingKind::TenantSensitive,
        }]
    );
    let empty =
        local_inspect_and_redact("", Some(LocalInspectionFindingKind::TenantSensitive), 8).unwrap();
    assert_eq!(empty.text, "");
    assert!(empty.matches.is_empty());
}

#[test]
fn local_inspection_rejects_more_matches_than_the_caller_capacity() {
    assert_eq!(
        local_inspect_and_redact("a@example.test b@example.test", None, 1),
        Err(MojoError::Capacity)
    );
    assert_eq!(
        local_inspect_and_redact("ordinary text", None, 0).unwrap(),
        LocalInspectionResult {
            text: "ordinary text".to_string(),
            matches: Vec::new(),
        }
    );
    assert_eq!(
        local_inspect_and_redact("a@example.test", None, 0),
        Err(MojoError::Capacity)
    );
}
