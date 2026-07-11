use prodex_domain::{
    ApiVersion, ApiVersionDecision, ApiVersionError, ApiVersionErrorStatus, ApiVersionPolicy,
    ApiVersionStatus, ConcurrencyError, ConcurrencyErrorStatus, Cursor, CursorError,
    CursorErrorStatus, EntityTag, EntityTagError, EntityTagErrorStatus, Page, PageRequest,
    ResourceVersion, ResourceVersionError, ResourceVersionErrorStatus, evaluate_api_version,
    plan_api_version_error_response, plan_concurrency_error_response, plan_cursor_error_response,
    plan_entity_tag_error_response, plan_resource_version_error_response, require_matching_etag,
    require_matching_version,
};

#[test]
fn api_version_formats_as_stable_public_version() {
    let version = ApiVersion::new(1, 2);

    assert_eq!(version.to_string(), "v1.2");
    assert_eq!(format!("{version:?}"), "ApiVersion(\"<redacted>\")");
}

#[test]
fn current_api_version_is_allowed() {
    let policies = [ApiVersionPolicy {
        version: ApiVersion::new(1, 0),
        status: ApiVersionStatus::Current,
    }];

    assert_eq!(
        evaluate_api_version(ApiVersion::new(1, 0), &policies, 1_700_000_000_000),
        Ok(ApiVersionDecision::Allowed)
    );
}

#[test]
fn deprecated_api_version_is_allowed_until_sunset() {
    let policies = [ApiVersionPolicy {
        version: ApiVersion::new(1, 0),
        status: ApiVersionStatus::Deprecated {
            deprecated_at_unix_ms: 1_600_000_000_000,
            sunset_at_unix_ms: Some(1_800_000_000_000),
        },
    }];

    assert_eq!(
        evaluate_api_version(ApiVersion::new(1, 0), &policies, 1_700_000_000_000),
        Ok(ApiVersionDecision::AllowedDeprecated {
            deprecated_at_unix_ms: 1_600_000_000_000,
            sunset_at_unix_ms: Some(1_800_000_000_000),
        })
    );
}

#[test]
fn api_version_policy_and_decision_debug_output_is_stable_and_redacted() {
    let policy = ApiVersionPolicy {
        version: ApiVersion::new(1, 0),
        status: ApiVersionStatus::Deprecated {
            deprecated_at_unix_ms: 1_600_000_000_000,
            sunset_at_unix_ms: Some(1_800_000_000_000),
        },
    };
    let decision = evaluate_api_version(ApiVersion::new(1, 0), &[policy], 1_700_000_000_000)
        .expect("deprecated version allowed");

    let rendered = format!("{policy:?} {decision:?}");
    for sensitive in ["v1.0", "1600000000000", "1800000000000"] {
        assert!(
            !rendered.contains(sensitive),
            "API version debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("version: \"<redacted>\""));
    assert!(rendered.contains("deprecated_at_unix_ms: \"<redacted>\""));
}

#[test]
fn sunset_or_unknown_api_versions_are_rejected() {
    let requested = ApiVersion::new(1, 0);
    let policies = [ApiVersionPolicy {
        version: requested,
        status: ApiVersionStatus::Deprecated {
            deprecated_at_unix_ms: 1_600_000_000_000,
            sunset_at_unix_ms: Some(1_700_000_000_000),
        },
    }];

    assert_eq!(
        evaluate_api_version(requested, &policies, 1_700_000_000_000),
        Err(ApiVersionError::Sunset {
            requested,
            sunset_at_unix_ms: 1_700_000_000_000,
        })
    );
    assert_eq!(
        evaluate_api_version(ApiVersion::new(9, 9), &policies, 1_600_000_000_000),
        Err(ApiVersionError::Unsupported {
            requested: ApiVersion::new(9, 9),
        })
    );
}

#[test]
fn api_version_error_responses_are_stable_and_redacted() {
    let requested = ApiVersion::new(9, 9);
    let unsupported = ApiVersionError::Unsupported { requested };
    assert_eq!(unsupported.to_string(), "API version is unsupported");
    assert!(!unsupported.to_string().contains(&requested.to_string()));
    let unsupported_response = plan_api_version_error_response(&unsupported);

    assert_eq!(unsupported_response.status, ApiVersionErrorStatus::NotFound);
    assert_eq!(unsupported_response.code, "api_version_unsupported");
    assert_eq!(unsupported_response.message, "API version is unsupported");
    assert!(
        !unsupported_response
            .message
            .contains(&requested.to_string())
    );

    let sunset = ApiVersionError::Sunset {
        requested,
        sunset_at_unix_ms: 1_700_000_000_000,
    };
    assert_eq!(sunset.to_string(), "API version is no longer available");
    assert!(!sunset.to_string().contains(&requested.to_string()));
    assert!(!sunset.to_string().contains("1700000000000"));
    assert!(!sunset.to_string().contains("sunset"));
    let sunset_response = plan_api_version_error_response(&sunset);

    assert_eq!(sunset_response.status, ApiVersionErrorStatus::Gone);
    assert_eq!(sunset_response.code, "api_version_sunset");
    assert_eq!(
        sunset_response.message,
        "API version is no longer available"
    );
    assert!(!sunset_response.message.contains(&requested.to_string()));
    assert!(!sunset_response.message.contains("1700000000000"));
}

#[test]
fn api_version_error_debug_output_is_stable_and_redacted() {
    let requested = ApiVersion::new(9, 9);
    let errors = [
        ApiVersionError::Unsupported { requested },
        ApiVersionError::Sunset {
            requested,
            sunset_at_unix_ms: 1_700_000_000_000,
        },
    ];

    let rendered = format!("{errors:?}");
    for sensitive in [requested.to_string(), "1700000000000".to_string()] {
        assert!(
            !rendered.contains(&sensitive),
            "API version error debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("Unsupported"));
    assert!(rendered.contains("\"<redacted>\""));
}

#[test]
fn api_version_error_response_plan_debug_output_is_stable_and_redacted() {
    let response = plan_api_version_error_response(&ApiVersionError::Unsupported {
        requested: ApiVersion::new(9, 9),
    });

    assert_eq!(
        format!("{response:?}"),
        "ApiVersionErrorResponsePlan { status: NotFound, code: \"api_version_unsupported\", message: \"API version is unsupported\" }"
    );
}

#[test]
fn page_request_defaults_and_clamps_limits() {
    assert_eq!(
        PageRequest::new(None, None).limit,
        PageRequest::DEFAULT_LIMIT
    );
    assert_eq!(PageRequest::new(Some(0), None).limit, 1);
    assert_eq!(
        PageRequest::new(Some(10_000), None).limit,
        PageRequest::MAX_LIMIT
    );
}

#[test]
fn cursor_rejects_empty_and_overlong_values() {
    assert_eq!(Cursor::new(""), Err(CursorError::Empty));
    assert_eq!(
        Cursor::new(" "),
        Err(CursorError::InvalidCharacter {
            index: 0,
            character: ' ',
        })
    );
    assert_eq!(
        Cursor::new("x".repeat(513)),
        Err(CursorError::TooLong { length: 513 })
    );
    assert_eq!(
        Cursor::new("tenant cursor"),
        Err(CursorError::InvalidCharacter {
            index: 6,
            character: ' ',
        })
    );
    assert_eq!(
        Cursor::new(" tenant-cursor"),
        Err(CursorError::InvalidCharacter {
            index: 0,
            character: ' ',
        })
    );
    assert_eq!(
        Cursor::new("tenant\ncursor"),
        Err(CursorError::InvalidCharacter {
            index: 6,
            character: '\n',
        })
    );
    assert_eq!(
        Cursor::new("tenant-é"),
        Err(CursorError::InvalidCharacter {
            index: 7,
            character: 'é',
        })
    );
}

#[test]
fn cursor_error_responses_are_stable_and_redacted() {
    let empty_response = plan_cursor_error_response(&CursorError::Empty);

    assert_eq!(
        CursorError::Empty.to_string(),
        "pagination cursor is invalid"
    );
    assert!(!CursorError::Empty.to_string().contains("empty"));
    assert_eq!(empty_response.status, CursorErrorStatus::BadRequest);
    assert_eq!(empty_response.code, "pagination_cursor_invalid");
    assert_eq!(empty_response.message, "pagination cursor is invalid");

    let too_long = CursorError::TooLong { length: 513 };
    assert_eq!(too_long.to_string(), "pagination cursor is invalid");
    assert!(!too_long.to_string().contains("513"));
    assert!(!too_long.to_string().contains("too long"));
    let too_long_response = plan_cursor_error_response(&too_long);

    assert_eq!(too_long_response, empty_response);
    assert!(!too_long_response.message.contains("513"));
    assert!(!too_long_response.message.contains("too long"));

    let invalid = CursorError::InvalidCharacter {
        index: 6,
        character: '\n',
    };
    assert_eq!(invalid.to_string(), "pagination cursor is invalid");
    assert!(!invalid.to_string().contains("6"));
    assert!(!invalid.to_string().contains("\\n"));
    let invalid_response = plan_cursor_error_response(&invalid);
    assert_eq!(invalid_response, empty_response);
    assert!(!invalid_response.message.contains("6"));
    assert!(!invalid_response.message.contains("\\n"));
}

#[test]
fn cursor_error_debug_output_is_stable_and_redacted() {
    let errors = [
        CursorError::Empty,
        CursorError::TooLong { length: 513 },
        CursorError::InvalidCharacter {
            index: 6,
            character: '\n',
        },
    ];

    let rendered = format!("{errors:?}");
    for sensitive in ["513", "6", "\\n"] {
        assert!(
            !rendered.contains(sensitive),
            "cursor error debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("InvalidCharacter"));
    assert!(rendered.contains("\"<redacted>\""));
}

#[test]
fn cursor_error_response_plan_debug_output_is_stable_and_redacted() {
    let response = plan_cursor_error_response(&CursorError::Empty);

    assert_eq!(
        format!("{response:?}"),
        "CursorErrorResponsePlan { status: BadRequest, code: \"pagination_cursor_invalid\", message: \"pagination cursor is invalid\" }"
    );
}

#[test]
fn cursor_debug_output_is_stable_and_redacted() {
    let cursor = Cursor::new("opaque-secret-cursor").unwrap();

    let rendered = format!("{cursor:?}");
    assert!(!rendered.contains("opaque-secret-cursor"));
    assert_eq!(rendered, "Cursor(\"<redacted>\")");
}

#[test]
fn page_request_debug_output_is_stable_and_redacted() {
    let cursor = Cursor::new("opaque-secret-cursor").unwrap();
    let request = PageRequest::new(Some(25), Some(cursor));

    let rendered = format!("{request:?}");
    assert!(!rendered.contains("opaque-secret-cursor"));
    assert_eq!(
        rendered,
        "PageRequest { limit: 25, cursor: Some(\"<redacted>\") }"
    );
}

#[test]
fn entity_tag_debug_output_is_stable_and_redacted() {
    let tag = EntityTag::new("W/\"secret-old\"").unwrap();

    let rendered = format!("{tag:?}");
    assert!(!rendered.contains("secret-old"));
    assert_eq!(rendered, "EntityTag(\"<redacted>\")");
}

#[test]
fn page_carries_items_and_next_cursor() {
    let cursor = Cursor::new("opaque-next").unwrap();
    let page = Page::new(vec!["tenant-a", "tenant-b"], Some(cursor.clone()));

    assert_eq!(page.items, vec!["tenant-a", "tenant-b"]);
    assert_eq!(page.next_cursor, Some(cursor));
}

#[test]
fn page_debug_output_is_stable_and_redacted() {
    let cursor = Cursor::new("opaque-next").unwrap();
    let page = Page::new(vec!["tenant-a", "tenant-b"], Some(cursor));

    let rendered = format!("{page:?}");
    assert!(!rendered.contains("tenant-a"));
    assert!(!rendered.contains("tenant-b"));
    assert!(!rendered.contains("opaque-next"));
    assert_eq!(
        rendered,
        "Page { item_count: 2, next_cursor: Some(\"<redacted>\") }"
    );
}

#[test]
fn resource_version_is_non_zero_and_can_advance() {
    assert_eq!(ResourceVersion::new(0), Err(ResourceVersionError::Zero));
    assert_eq!(
        ResourceVersion::new(41).unwrap().next().unwrap().value(),
        42
    );
    assert_eq!(
        ResourceVersion::new(u64::MAX).unwrap().next(),
        Err(ResourceVersionError::Overflow)
    );
}

#[test]
fn entity_tag_and_resource_version_error_responses_are_stable_and_redacted() {
    let empty_tag = plan_entity_tag_error_response(&EntityTagError::Empty);

    assert_eq!(EntityTagError::Empty.to_string(), "entity tag is invalid");
    assert!(!EntityTagError::Empty.to_string().contains("empty"));
    assert_eq!(empty_tag.status, EntityTagErrorStatus::BadRequest);
    assert_eq!(empty_tag.code, "entity_tag_invalid");
    assert_eq!(empty_tag.message, "entity tag is invalid");

    let too_long_tag = EntityTagError::TooLong { length: 257 };
    assert_eq!(too_long_tag.to_string(), "entity tag is invalid");
    assert!(!too_long_tag.to_string().contains("257"));
    assert!(!too_long_tag.to_string().contains("too long"));
    let long_tag = plan_entity_tag_error_response(&too_long_tag);

    assert_eq!(long_tag, empty_tag);
    assert!(!long_tag.message.contains("257"));
    assert!(!long_tag.message.contains("too long"));

    let invalid_tag = EntityTagError::InvalidCharacter {
        index: 2,
        character: '\n',
    };
    assert_eq!(invalid_tag.to_string(), "entity tag is invalid");
    assert!(!invalid_tag.to_string().contains("2"));
    assert!(!invalid_tag.to_string().contains("\\n"));
    let invalid_tag_response = plan_entity_tag_error_response(&invalid_tag);
    assert_eq!(invalid_tag_response, empty_tag);
    assert!(!invalid_tag_response.message.contains("2"));
    assert!(!invalid_tag_response.message.contains("\\n"));

    let version = plan_resource_version_error_response(&ResourceVersionError::Zero);

    assert_eq!(
        ResourceVersionError::Zero.to_string(),
        "resource version is invalid"
    );
    assert_eq!(
        ResourceVersionError::Overflow.to_string(),
        "resource version is invalid"
    );
    assert!(!ResourceVersionError::Zero.to_string().contains("zero"));
    assert!(
        !ResourceVersionError::Overflow
            .to_string()
            .contains("advance")
    );
    assert_eq!(version.status, ResourceVersionErrorStatus::BadRequest);
    assert_eq!(version.code, "resource_version_invalid");
    assert_eq!(version.message, "resource version is invalid");
    assert!(!version.message.contains("zero"));
    assert_eq!(
        plan_resource_version_error_response(&ResourceVersionError::Overflow),
        version
    );
}

#[test]
fn entity_tag_error_debug_output_is_stable_and_redacted() {
    let errors = [
        EntityTagError::Empty,
        EntityTagError::TooLong { length: 257 },
        EntityTagError::InvalidCharacter {
            index: 2,
            character: '\n',
        },
    ];

    let rendered = format!("{errors:?}");
    for sensitive in ["257", "2", "\\n"] {
        assert!(
            !rendered.contains(sensitive),
            "entity tag error debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("InvalidCharacter"));
    assert!(rendered.contains("\"<redacted>\""));
}

#[test]
fn entity_tag_error_response_plan_debug_output_is_stable_and_redacted() {
    let response = plan_entity_tag_error_response(&EntityTagError::Empty);

    assert_eq!(
        format!("{response:?}"),
        "EntityTagErrorResponsePlan { status: BadRequest, code: \"entity_tag_invalid\", message: \"entity tag is invalid\" }"
    );
}

#[test]
fn resource_version_error_debug_output_is_stable_and_redacted() {
    let errors = [ResourceVersionError::Zero, ResourceVersionError::Overflow];

    assert_eq!(format!("{errors:?}"), "[Zero, Overflow]");
}

#[test]
fn resource_version_debug_output_is_stable_and_redacted() {
    let version = ResourceVersion::new(42).unwrap();

    let rendered = format!("{version:?}");
    assert!(!rendered.contains("42"));
    assert_eq!(rendered, "ResourceVersion(\"<redacted>\")");
}

#[test]
fn resource_version_error_response_plan_debug_output_is_stable_and_redacted() {
    let response = plan_resource_version_error_response(&ResourceVersionError::Zero);

    assert_eq!(
        format!("{response:?}"),
        "ResourceVersionErrorResponsePlan { status: BadRequest, code: \"resource_version_invalid\", message: \"resource version is invalid\" }"
    );
}

#[test]
fn entity_tag_rejects_whitespace_control_and_non_ascii_values() {
    assert_eq!(EntityTag::new(""), Err(EntityTagError::Empty));
    assert_eq!(
        EntityTag::new(" "),
        Err(EntityTagError::InvalidCharacter {
            index: 0,
            character: ' ',
        })
    );
    assert_eq!(
        EntityTag::new("W/\"old tag\""),
        Err(EntityTagError::InvalidCharacter {
            index: 6,
            character: ' ',
        })
    );
    assert_eq!(
        EntityTag::new(" W/\"old\""),
        Err(EntityTagError::InvalidCharacter {
            index: 0,
            character: ' ',
        })
    );
    assert_eq!(
        EntityTag::new("W/\"old\ntag\""),
        Err(EntityTagError::InvalidCharacter {
            index: 6,
            character: '\n',
        })
    );
    assert_eq!(
        EntityTag::new("W/\"é\""),
        Err(EntityTagError::InvalidCharacter {
            index: 3,
            character: 'é',
        })
    );
}

#[test]
fn matching_version_precondition_allows_mutation() {
    let actual = ResourceVersion::new(7).unwrap();

    assert_eq!(require_matching_version(Some(actual), actual), Ok(()));
}

#[test]
fn missing_version_precondition_rejects_mutation() {
    let actual = ResourceVersion::new(7).unwrap();

    assert_eq!(
        require_matching_version(None, actual),
        Err(ConcurrencyError::MissingPrecondition)
    );
}

#[test]
fn stale_version_precondition_rejects_mutation() {
    let expected = ResourceVersion::new(6).unwrap();
    let actual = ResourceVersion::new(7).unwrap();

    assert_eq!(
        require_matching_version(Some(expected), actual),
        Err(ConcurrencyError::VersionMismatch { expected, actual })
    );
}

#[test]
fn version_precondition_detects_mismatch_without_leaking_versions_in_display() {
    let expected = ResourceVersion::new(6).unwrap();
    let actual = ResourceVersion::new(7).unwrap();
    let err = require_matching_version(Some(expected), actual).unwrap_err();

    assert!(matches!(err, ConcurrencyError::VersionMismatch { .. }));
    assert!(!err.to_string().contains("6"));
    assert!(!err.to_string().contains("7"));
    assert_eq!(err.to_string(), "resource version precondition failed");
}

#[test]
fn etag_precondition_detects_mismatch_without_leaking_expected_value_in_display() {
    let expected = EntityTag::new("W/\"secret-old\"").unwrap();
    let actual = EntityTag::from_version(ResourceVersion::new(9).unwrap());
    let err = require_matching_etag(Some(expected), actual).unwrap_err();

    assert!(matches!(err, ConcurrencyError::EntityTagMismatch { .. }));
    assert!(!err.to_string().contains("secret-old"));
    assert_eq!(err.to_string(), "entity tag precondition failed");
}

#[test]
fn concurrency_error_responses_are_stable_and_redacted() {
    assert_eq!(
        ConcurrencyError::MissingPrecondition.to_string(),
        "mutation requires a concurrency precondition"
    );
    assert!(
        !ConcurrencyError::MissingPrecondition
            .to_string()
            .contains("version")
    );
    assert!(
        !ConcurrencyError::MissingPrecondition
            .to_string()
            .contains("entity tag")
    );
    let missing = plan_concurrency_error_response(&ConcurrencyError::MissingPrecondition);

    assert_eq!(missing.status, ConcurrencyErrorStatus::PreconditionRequired);
    assert_eq!(missing.code, "mutation_precondition_required");
    assert_eq!(
        missing.message,
        "mutation requires a concurrency precondition"
    );

    let expected = ResourceVersion::new(6).unwrap();
    let actual = ResourceVersion::new(7).unwrap();
    let version =
        plan_concurrency_error_response(&ConcurrencyError::VersionMismatch { expected, actual });

    assert_eq!(version.status, ConcurrencyErrorStatus::Conflict);
    assert_eq!(version.code, "resource_version_conflict");
    assert_eq!(version.message, "resource version precondition failed");
    assert!(!version.message.contains("6"));
    assert!(!version.message.contains("7"));

    let supplied = EntityTag::new("W/\"secret-old\"").unwrap();
    let current = EntityTag::from_version(ResourceVersion::new(9).unwrap());
    let etag = plan_concurrency_error_response(&ConcurrencyError::EntityTagMismatch {
        expected: supplied,
        actual: current,
    });

    assert_eq!(etag.status, ConcurrencyErrorStatus::Conflict);
    assert_eq!(etag.code, "entity_tag_conflict");
    assert_eq!(etag.message, "entity tag precondition failed");
    assert!(!etag.message.contains("secret-old"));
    assert!(!etag.message.contains("9"));
}

#[test]
fn concurrency_error_debug_output_is_stable_and_redacted() {
    let expected = ResourceVersion::new(6).unwrap();
    let actual = ResourceVersion::new(7).unwrap();
    let supplied = EntityTag::new("W/\"secret-old\"").unwrap();
    let current = EntityTag::from_version(ResourceVersion::new(9).unwrap());
    let errors = [
        ConcurrencyError::MissingPrecondition,
        ConcurrencyError::VersionMismatch { expected, actual },
        ConcurrencyError::EntityTagMismatch {
            expected: supplied,
            actual: current,
        },
    ];

    let rendered = format!("{errors:?}");
    for sensitive in ["6", "7", "9", "secret-old"] {
        assert!(
            !rendered.contains(sensitive),
            "concurrency error debug output leaked sensitive token {sensitive}: {rendered}"
        );
    }
    assert!(rendered.contains("VersionMismatch"));
    assert!(rendered.contains("\"<redacted>\""));
}

#[test]
fn concurrency_error_response_plan_debug_output_is_stable_and_redacted() {
    let response = plan_concurrency_error_response(&ConcurrencyError::MissingPrecondition);

    assert_eq!(
        format!("{response:?}"),
        "ConcurrencyErrorResponsePlan { status: PreconditionRequired, code: \"mutation_precondition_required\", message: \"mutation requires a concurrency precondition\" }"
    );
}
