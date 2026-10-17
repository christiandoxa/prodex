use super::*;

#[test]
fn mojo_rewrite_quality_risk_covers_each_negative_signal() {
    let clean = SmartContextRewriteQualityInput::default();
    assert_eq!(
        smart_context_rewrite_telemetry_quality_risk(clean),
        Ok(false)
    );
    assert_eq!(
        smart_context_rewrite_telemetry_quality_risk(SmartContextRewriteQualityInput {
            task_completed: Some(true),
            ..clean
        }),
        Ok(false),
    );
    let risks = [
        SmartContextRewriteQualityInput {
            upstream_context_errors: u16::MAX,
            ..clean
        },
        SmartContextRewriteQualityInput {
            previous_response_not_found: true,
            ..clean
        },
        SmartContextRewriteQualityInput {
            invalid_tool_call_continuation: true,
            ..clean
        },
        SmartContextRewriteQualityInput {
            missing_artifact_requests: 1,
            ..clean
        },
        SmartContextRewriteQualityInput {
            repeated_tool_call_count: 1,
            ..clean
        },
        SmartContextRewriteQualityInput {
            model_reread_requests: 1,
            ..clean
        },
        SmartContextRewriteQualityInput {
            corrective_user_messages: 1,
            ..clean
        },
        SmartContextRewriteQualityInput {
            test_or_build_failed_after_rewrite: true,
            ..clean
        },
        SmartContextRewriteQualityInput {
            task_completed: Some(false),
            ..clean
        },
    ];
    for risk in risks {
        assert_eq!(
            smart_context_rewrite_telemetry_quality_risk(risk),
            Ok(true),
            "{risk:?}"
        );
    }
}

#[test]
fn mojo_rewrite_quality_risk_rejects_corrupt_ffi_values_without_output() {
    let mut fields = [0i64; 9];
    let mut decision = -7i64;
    unsafe {
        assert_eq!(
            prodex_smart_context_rewrite_quality_risk_v1(fields.as_ptr() as u64, 8, &mut decision,),
            1
        );
        assert_eq!(
            prodex_smart_context_rewrite_quality_risk_v1(0, 9, &mut decision,),
            1
        );
        fields[8] = 3;
        assert_eq!(
            prodex_smart_context_rewrite_quality_risk_v1(fields.as_ptr() as u64, 9, &mut decision,),
            1
        );
        fields[8] = 0;
        fields[1] = 2;
        assert_eq!(
            prodex_smart_context_rewrite_quality_risk_v1(fields.as_ptr() as u64, 9, &mut decision,),
            1
        );
    }
    assert_eq!(decision, -7);
}

#[test]
fn compiled_rewrite_sample_policy_owns_inclusion_safety_and_quality() {
    let clean = SmartContextRewriteQualityInput::default();
    let input = |rewrite_kind, status, fallback, tokens_before, tokens_after, quality| {
        SmartContextRewriteSamplePolicyInput {
            rewrite_kind,
            status,
            fallback,
            tokens_before,
            tokens_after,
            quality,
        }
    };
    for (kind, status, fallback, before, after, expected) in [
        ("untouched", "unknown", false, 100, 50, None),
        ("untouched", "unknown", true, 100, 50, Some(false)),
        ("rewritten", "ok_saved", false, 100, 50, Some(true)),
        ("minified", "ok_minified", false, 100, 50, Some(true)),
        (
            "rewritten",
            "ok_surgical_rehydrate",
            false,
            100,
            50,
            Some(true),
        ),
        ("rewritten", "failed", false, 100, 50, Some(false)),
        ("untouched", "ok_saved", false, 100, 50, Some(false)),
        ("rewritten", "ok_saved", false, 100, 100, Some(false)),
        ("rewritten", "ok_saved", false, 100, 120, Some(false)),
        ("rewritten", "ok_saved", true, 100, 50, Some(false)),
        ("rewritten ", "ok_saved", false, 100, 50, Some(false)),
        ("rewritten", " ok_saved", false, 100, 50, Some(false)),
        ("rewrite-🧭", "fail", false, 100, 50, None),
    ] {
        assert_eq!(
            smart_context_rewrite_sample_policy(input(
                kind, status, fallback, before, after, clean
            )),
            Ok(expected),
            "{kind:?}/{status:?}/{fallback}/{before}/{after}",
        );
    }
    for quality in [
        SmartContextRewriteQualityInput {
            upstream_context_errors: 1,
            ..clean
        },
        SmartContextRewriteQualityInput {
            invalid_tool_call_continuation: true,
            ..clean
        },
        SmartContextRewriteQualityInput {
            model_reread_requests: 1,
            ..clean
        },
        SmartContextRewriteQualityInput {
            task_completed: Some(false),
            ..clean
        },
    ] {
        assert_eq!(
            smart_context_rewrite_sample_policy(input(
                "rewritten",
                "ok_saved",
                false,
                100,
                50,
                quality
            )),
            Ok(Some(false)),
            "{quality:?}",
        );
    }
}

#[test]
fn compiled_rewrite_sample_policy_strict_abi_rejects_invalid_inputs_without_writes() {
    let kind = b"rewritten";
    let status = b"ok_saved";
    let metrics = [0_i64; 9];
    let mut output = [88_i64; 3];
    let k = kind.as_ptr() as usize as u64;
    let v = status.as_ptr() as usize as u64;
    let m = metrics.as_ptr() as usize as u64;
    let o = output.as_mut_ptr() as usize as u64;
    let call = |version,
                kind_addr,
                kind_len,
                status_len,
                fallback,
                metrics_addr,
                metrics_count,
                out,
                capacity| unsafe {
        prodex_smart_context_rewrite_sample_policy_v1(
            version,
            kind_addr,
            kind_len,
            v,
            status_len,
            fallback,
            100,
            50,
            metrics_addr,
            metrics_count,
            out,
            capacity,
        )
    };
    assert_eq!(call(2, k, 9, 8, 0, m, 9, o, 3), 4);
    assert_eq!(call(1, 0, 9, 8, 0, m, 9, o, 3), 1);
    assert_eq!(call(1, k, -1, 8, 0, m, 9, o, 3), 1);
    assert_eq!(call(1, k, 9, -1, 0, m, 9, o, 3), 1);
    assert_eq!(call(1, k, 9, 8, 2, m, 9, o, 3), 1);
    assert_eq!(call(1, k, 9, 8, 0, 0, 9, o, 3), 1);
    assert_eq!(call(1, k, 9, 8, 0, m, 8, o, 3), 1);
    assert_eq!(call(1, k, 9, 8, 0, m, 9, o, 2), 1);
    assert_eq!(call(1, k, 9, 8, 0, m, 9, 0, 3), 1);
    let invalid_utf8 = [0xff_u8];
    assert_eq!(
        call(
            1,
            invalid_utf8.as_ptr() as usize as u64,
            1,
            8,
            0,
            m,
            9,
            o,
            3
        ),
        1
    );
    assert_eq!(output, [88; 3]);
}
