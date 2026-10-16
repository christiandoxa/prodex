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
