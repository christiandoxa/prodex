use super::*;

#[test]
fn observed_token_accounting_uses_real_usage_and_available_window() {
    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(128_000),
            reserved_output_tokens: 8_000,
            current_input_tokens: 40_000,
            current_request_body_bytes: 0,
            current_request_estimated_tokens: None,
            observed_usage: vec![
                RuntimeTokenUsage {
                    input_tokens: 20_000,
                    cached_input_tokens: 5_000,
                    output_tokens: 1_000,
                    reasoning_tokens: 400,
                },
                RuntimeTokenUsage {
                    input_tokens: 42_000,
                    cached_input_tokens: 10_000,
                    output_tokens: 2_000,
                    reasoning_tokens: 700,
                },
            ],
        });

    assert_eq!(accounting.observed_turns, 2);
    assert_eq!(accounting.observed_input_tokens, 62_000);
    assert_eq!(accounting.observed_cached_input_tokens, 15_000);
    assert_eq!(accounting.observed_uncached_input_tokens, 47_000);
    assert_eq!(accounting.observed_output_tokens, 3_000);
    assert_eq!(accounting.observed_reasoning_tokens, 1_100);
    assert_eq!(accounting.observed_total_tokens, 65_000);
    assert_eq!(accounting.observed_context_tokens, 66_100);
    assert_eq!(accounting.last_input_tokens, 42_000);
    assert_eq!(accounting.last_accounted_input_tokens, 42_000);
    assert_eq!(accounting.last_observed_context_tokens, 44_700);
    assert_eq!(
        accounting.effective_input_source,
        SmartContextTokenAccountingSource::ObservedHistory
    );
    assert_eq!(accounting.effective_input_tokens, 42_000);
    assert_eq!(accounting.available_context_tokens, Some(78_000));
    assert!(accounting.accounting_risks.is_empty());
    assert_eq!(
        smart_context_token_budget_tier_from_accounting(&accounting),
        SmartContextTokenBudgetTier::Exact
    );
}

#[test]
fn observed_token_accounting_uses_cached_only_history_as_input_fallback() {
    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(32_000),
            reserved_output_tokens: 4_000,
            current_input_tokens: 0,
            current_request_body_bytes: 0,
            current_request_estimated_tokens: None,
            observed_usage: vec![RuntimeTokenUsage {
                cached_input_tokens: 16_000,
                ..RuntimeTokenUsage::default()
            }],
        });

    assert_eq!(accounting.last_input_tokens, 0);
    assert_eq!(accounting.last_accounted_input_tokens, 16_000);
    assert_eq!(accounting.last_observed_context_tokens, 16_000);
    assert_eq!(
        accounting.effective_input_source,
        SmartContextTokenAccountingSource::ObservedHistory
    );
    assert_eq!(accounting.effective_input_tokens, 16_000);
    assert_eq!(accounting.available_context_tokens, Some(12_000));
    assert!(accounting.accounting_risks.is_empty());
}

#[test]
fn observed_token_accounting_uses_body_estimate_when_tokens_unknown() {
    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(64_000),
            reserved_output_tokens: 4_000,
            current_input_tokens: 0,
            current_request_body_bytes: 80_001,
            current_request_estimated_tokens: None,
            observed_usage: Vec::new(),
        });

    assert_eq!(
        smart_context_estimate_tokens_from_body_bytes(80_001),
        20_001
    );
    assert_eq!(accounting.estimated_current_request_tokens, 20_001);
    assert_eq!(accounting.current_request_accounted_tokens, 20_001);
    assert_eq!(
        accounting.effective_input_source,
        SmartContextTokenAccountingSource::CurrentRequestBodyEstimate
    );
    assert_eq!(accounting.effective_input_tokens, 20_001);
    assert_eq!(accounting.available_context_tokens, Some(39_999));
    assert!(smart_context_accounting_safe_for_adaptive_policy(
        &accounting
    ));
}

#[test]
fn observed_token_accounting_accepts_content_aware_body_estimate() {
    let body = "word ".repeat(100);
    let estimate = smart_context_estimate_tokens_from_body(body.as_bytes());

    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(32_000),
            reserved_output_tokens: 4_000,
            current_input_tokens: 0,
            current_request_body_bytes: body.len(),
            current_request_estimated_tokens: Some(estimate),
            observed_usage: Vec::new(),
        });

    assert!(estimate > 0);
    assert!(estimate < smart_context_estimate_tokens_from_body_bytes(body.len()));
    assert_eq!(accounting.estimated_current_request_tokens, estimate);
    assert_eq!(accounting.current_request_accounted_tokens, estimate);
    assert_eq!(
        accounting.effective_input_source,
        SmartContextTokenAccountingSource::CurrentRequestBodyEstimate
    );
}

#[test]
fn observed_token_accounting_calibrates_body_estimate_from_recent_usage_with_floor() {
    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(64_000),
            reserved_output_tokens: 4_000,
            current_input_tokens: 0,
            current_request_body_bytes: 80_000,
            current_request_estimated_tokens: Some(20_000),
            observed_usage: vec![RuntimeTokenUsage {
                input_tokens: 8_000,
                ..RuntimeTokenUsage::default()
            }],
        });

    assert_eq!(accounting.estimated_current_request_tokens, 10_064);
    assert_eq!(accounting.current_request_accounted_tokens, 10_064);
    assert_eq!(
        accounting.effective_input_source,
        SmartContextTokenAccountingSource::CurrentRequestBodyEstimate
    );
    assert_eq!(accounting.available_context_tokens, Some(49_936));
}

#[test]
fn observed_token_accounting_with_calibration_preserves_unbucketed_behavior() {
    let input = SmartContextObservedTokenAccountingInput {
        model_context_window_tokens: Some(64_000),
        reserved_output_tokens: 4_000,
        current_input_tokens: 0,
        current_request_body_bytes: 80_000,
        current_request_estimated_tokens: Some(20_000),
        observed_usage: vec![RuntimeTokenUsage {
            input_tokens: 8_000,
            ..RuntimeTokenUsage::default()
        }],
    };

    let legacy = smart_context_observed_token_accounting(input.clone());
    let explicit = smart_context_observed_token_accounting_with_calibration(
        SmartContextObservedTokenAccountingCalibrationInput {
            accounting: input,
            calibration_bucket_key: None,
            calibration_samples: Vec::new(),
        },
    );

    assert_eq!(explicit, legacy);
    assert_eq!(explicit.estimated_current_request_tokens, 10_064);
}

#[test]
fn observed_token_accounting_calibrates_separately_by_bucket() {
    let responses_bucket = SmartContextTokenCalibrationBucketKey {
        route: Some("responses".to_string()),
        model: Some("gpt-5".to_string()),
        profile: Some("alpha".to_string()),
        transport: Some("http".to_string()),
    };
    let compact_bucket = SmartContextTokenCalibrationBucketKey {
        route: Some("compact".to_string()),
        model: Some("gpt-5".to_string()),
        profile: Some("alpha".to_string()),
        transport: Some("http".to_string()),
    };
    let samples = vec![
        SmartContextTokenCalibrationSample {
            bucket_key: Some(compact_bucket.clone()),
            usage: RuntimeTokenUsage {
                input_tokens: 30_000,
                ..RuntimeTokenUsage::default()
            },
        },
        SmartContextTokenCalibrationSample {
            bucket_key: Some(responses_bucket.clone()),
            usage: RuntimeTokenUsage {
                input_tokens: 8_000,
                ..RuntimeTokenUsage::default()
            },
        },
    ];

    let responses = smart_context_observed_token_accounting_with_calibration(
        SmartContextObservedTokenAccountingCalibrationInput {
            accounting: SmartContextObservedTokenAccountingInput {
                model_context_window_tokens: Some(64_000),
                reserved_output_tokens: 4_000,
                current_input_tokens: 0,
                current_request_body_bytes: 80_000,
                current_request_estimated_tokens: Some(20_000),
                observed_usage: Vec::new(),
            },
            calibration_bucket_key: Some(responses_bucket),
            calibration_samples: samples.clone(),
        },
    );
    let compact = smart_context_observed_token_accounting_with_calibration(
        SmartContextObservedTokenAccountingCalibrationInput {
            accounting: SmartContextObservedTokenAccountingInput {
                model_context_window_tokens: Some(64_000),
                reserved_output_tokens: 4_000,
                current_input_tokens: 0,
                current_request_body_bytes: 80_000,
                current_request_estimated_tokens: Some(20_000),
                observed_usage: Vec::new(),
            },
            calibration_bucket_key: Some(compact_bucket),
            calibration_samples: samples,
        },
    );

    assert_eq!(responses.estimated_current_request_tokens, 10_064);
    assert_eq!(compact.estimated_current_request_tokens, 20_000);
}

#[test]
fn observed_token_accounting_prefers_exact_bucket_before_calibration_fallbacks() {
    let target = smart_context_test_calibration_bucket(
        Some("responses"),
        Some("gpt-5.4"),
        Some("alpha"),
        Some("http"),
    );
    let samples = vec![
        smart_context_test_calibration_sample(
            Some(smart_context_test_calibration_bucket(
                Some("responses"),
                Some("gpt-5.4"),
                Some("beta"),
                Some("websocket"),
            )),
            16_000,
        ),
        smart_context_test_calibration_sample(
            Some(smart_context_test_calibration_bucket(
                Some("responses"),
                Some("other-model"),
                Some("alpha"),
                Some("http"),
            )),
            17_000,
        ),
        smart_context_test_calibration_sample(None, 18_000),
        smart_context_test_calibration_sample(Some(target.clone()), 8_000),
    ];

    let estimate = smart_context_test_calibrated_bucket_estimate(target, samples);

    assert_eq!(estimate, 10_064);
}

#[test]
fn observed_token_accounting_falls_back_to_model_family_before_profile_route() {
    let target = smart_context_test_calibration_bucket(
        Some("responses"),
        Some(" GPT_5.4-2026-05-01 "),
        Some("alpha"),
        Some("http"),
    );
    let samples = vec![
        smart_context_test_calibration_sample(
            Some(smart_context_test_calibration_bucket(
                Some("responses"),
                Some("other-model"),
                Some("alpha"),
                Some("websocket"),
            )),
            16_000,
        ),
        smart_context_test_calibration_sample(
            Some(smart_context_test_calibration_bucket(
                Some("compact"),
                Some("gpt-5.4"),
                Some("beta"),
                Some("websocket"),
            )),
            12_000,
        ),
    ];

    let estimate = smart_context_test_calibrated_bucket_estimate(target, samples);

    assert_eq!(estimate, 13_564);
}

#[test]
fn observed_token_accounting_falls_back_to_profile_route_before_route_transport() {
    let target = smart_context_test_calibration_bucket(
        Some("responses"),
        Some("gpt-5.2"),
        Some("alpha"),
        Some("http"),
    );
    let samples = vec![
        smart_context_test_calibration_sample(
            Some(smart_context_test_calibration_bucket(
                Some("responses"),
                Some("other-model"),
                Some("beta"),
                Some("http"),
            )),
            17_000,
        ),
        smart_context_test_calibration_sample(
            Some(smart_context_test_calibration_bucket(
                Some("responses"),
                Some("gpt-5.4"),
                Some("alpha"),
                Some("websocket"),
            )),
            12_000,
        ),
    ];

    let estimate = smart_context_test_calibrated_bucket_estimate(target, samples);

    assert_eq!(estimate, 13_564);
}

#[test]
fn observed_token_accounting_falls_back_to_route_transport_and_global_compatible_samples() {
    let target = smart_context_test_calibration_bucket(
        Some("responses"),
        Some("gpt-5.2"),
        Some("alpha"),
        Some("http"),
    );
    let route_transport_samples = vec![smart_context_test_calibration_sample(
        Some(smart_context_test_calibration_bucket(
            Some("responses"),
            Some("other-model"),
            Some("beta"),
            Some("http"),
        )),
        12_000,
    )];
    let global_samples = vec![smart_context_test_calibration_sample(None, 14_000)];

    let route_transport_estimate =
        smart_context_test_calibrated_bucket_estimate(target.clone(), route_transport_samples);
    let global_estimate = smart_context_test_calibrated_bucket_estimate(target, global_samples);

    assert_eq!(route_transport_estimate, 13_564);
    assert_eq!(global_estimate, 15_814);
}

fn smart_context_test_calibration_bucket(
    route: Option<&str>,
    model: Option<&str>,
    profile: Option<&str>,
    transport: Option<&str>,
) -> SmartContextTokenCalibrationBucketKey {
    SmartContextTokenCalibrationBucketKey {
        route: route.map(str::to_string),
        model: model.map(str::to_string),
        profile: profile.map(str::to_string),
        transport: transport.map(str::to_string),
    }
}

fn smart_context_test_calibration_sample(
    bucket_key: Option<SmartContextTokenCalibrationBucketKey>,
    input_tokens: u64,
) -> SmartContextTokenCalibrationSample {
    SmartContextTokenCalibrationSample {
        bucket_key,
        usage: RuntimeTokenUsage {
            input_tokens,
            ..RuntimeTokenUsage::default()
        },
    }
}

fn smart_context_test_calibrated_bucket_estimate(
    bucket_key: SmartContextTokenCalibrationBucketKey,
    samples: Vec<SmartContextTokenCalibrationSample>,
) -> u64 {
    smart_context_observed_token_accounting_with_calibration(
        SmartContextObservedTokenAccountingCalibrationInput {
            accounting: SmartContextObservedTokenAccountingInput {
                model_context_window_tokens: Some(64_000),
                reserved_output_tokens: 4_000,
                current_input_tokens: 0,
                current_request_body_bytes: 80_000,
                current_request_estimated_tokens: Some(20_000),
                observed_usage: Vec::new(),
            },
            calibration_bucket_key: Some(bucket_key),
            calibration_samples: samples,
        },
    )
    .estimated_current_request_tokens
}

#[test]
fn observed_token_accounting_uses_recent_high_water_mark_for_calibration_safety() {
    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(64_000),
            reserved_output_tokens: 4_000,
            current_input_tokens: 0,
            current_request_body_bytes: 80_000,
            current_request_estimated_tokens: Some(20_000),
            observed_usage: vec![
                RuntimeTokenUsage {
                    input_tokens: 18_000,
                    ..RuntimeTokenUsage::default()
                },
                RuntimeTokenUsage {
                    input_tokens: 8_000,
                    ..RuntimeTokenUsage::default()
                },
            ],
        });

    assert_eq!(
        accounting.estimated_current_request_tokens, 20_000,
        "recent high-water usage should prevent unsafe over-shrink"
    );
    assert_eq!(accounting.available_context_tokens, Some(40_000));
}

#[test]
fn observed_token_accounting_does_not_inflate_small_body_from_history_calibration() {
    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(64_000),
            reserved_output_tokens: 4_000,
            current_input_tokens: 48_000,
            current_request_body_bytes: b"small current request body payload".len(),
            current_request_estimated_tokens: Some(smart_context_estimate_tokens_from_body(
                b"small current request body payload",
            )),
            observed_usage: vec![RuntimeTokenUsage {
                input_tokens: 48_000,
                ..RuntimeTokenUsage::default()
            }],
        });

    assert!(
        accounting.estimated_current_request_tokens < 100,
        "history calibration must not inflate a small current body into prior context size"
    );
    assert_eq!(accounting.current_request_accounted_tokens, 48_000);
    assert_eq!(accounting.available_context_tokens, Some(12_000));
}

#[test]
fn observed_token_accounting_uses_larger_history_to_avoid_underbudget() {
    let accounting =
        smart_context_observed_token_accounting(SmartContextObservedTokenAccountingInput {
            model_context_window_tokens: Some(128_000),
            reserved_output_tokens: 8_000,
            current_input_tokens: 10_000,
            current_request_body_bytes: 20_000,
            current_request_estimated_tokens: None,
            observed_usage: vec![RuntimeTokenUsage {
                input_tokens: 100_000,
                output_tokens: 2_000,
                reasoning_tokens: 500,
                ..RuntimeTokenUsage::default()
            }],
        });

    assert_eq!(accounting.current_request_accounted_tokens, 10_000);
    assert_eq!(
        accounting.effective_input_source,
        SmartContextTokenAccountingSource::ObservedHistory
    );
    assert_eq!(accounting.effective_input_tokens, 100_000);
    assert_eq!(accounting.available_context_tokens, Some(20_000));
}
