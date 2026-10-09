/// Caller-owned facts used by the Mojo request-admission planner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextBodyAdmissionInput {
    pub body_bytes: u64,
    pub websocket: bool,
    pub route_supported: bool,
    pub content_type_supported: bool,
    pub marker_present: bool,
    pub static_context_required: bool,
    pub websocket_generate_false: bool,
}

/// Caller-owned facts used after Serde has acquired a JSON value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmartContextBodyShapeInput {
    pub json_valid: bool,
    pub json_shape_valid: bool,
    pub json_shape_reason: u8,
    pub rewrite_candidate: bool,
    pub static_context_changed: bool,
}

unsafe extern "C" {
    fn prodex_smart_context_body_admission_plan_v1(
        abi_version: i64,
        body_bytes: u64,
        transport: i64,
        route_supported: i64,
        content_type_supported: i64,
        marker_present: i64,
        static_context_required: i64,
        websocket_generate_false: i64,
        reason_address: u64,
    ) -> i64;
    fn prodex_smart_context_body_shape_plan_v1(
        abi_version: i64,
        json_valid: i64,
        json_shape_valid: i64,
        json_shape_reason: i64,
        rewrite_candidate: i64,
        static_context_changed: i64,
        reason_address: u64,
    ) -> i64;
    fn prodex_smart_context_rewrite_outcome_plan_v1(
        abi_version: i64,
        body_bytes_before: u64,
        body_bytes_after: u64,
        rehydrated_refs: u64,
        tool_outputs_condensed: u64,
        duplicate_texts: u64,
        static_context_deltas: u64,
        outcome_address: u64,
    ) -> i64;
    fn prodex_smart_context_telemetry_label_v1(
        abi_version: i64,
        kind: i64,
        value: u64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
}

const SMART_CONTEXT_POLICY_ABI_VERSION: i64 = 1;

pub fn smart_context_body_admission_reason(
    input: SmartContextBodyAdmissionInput,
) -> Result<Option<&'static str>, crate::MojoError> {
    let mut reason = -1_i64;
    let status = unsafe {
        prodex_smart_context_body_admission_plan_v1(
            SMART_CONTEXT_POLICY_ABI_VERSION,
            input.body_bytes,
            i64::from(input.websocket),
            i64::from(input.route_supported),
            i64::from(input.content_type_supported),
            i64::from(input.marker_present),
            i64::from(input.static_context_required),
            i64::from(input.websocket_generate_false),
            &mut reason as *mut i64 as usize as u64,
        )
    };
    if status == 4 {
        return Err(crate::MojoError::AbiMismatch);
    }
    if status != 0 || !matches!(reason, 0..=6) {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(match reason {
        0 => None,
        1 => Some("unsupported_route"),
        2 => Some("unsupported_content_type"),
        3 => Some("below_minimum_body"),
        4 => Some("websocket_generate_false"),
        5 => Some("websocket_large_payload"),
        6 => Some("body_too_large"),
        _ => unreachable!(),
    })
}

pub fn smart_context_body_shape_reason(
    input: SmartContextBodyShapeInput,
) -> Result<Option<&'static str>, crate::MojoError> {
    let mut reason = -1_i64;
    let status = unsafe {
        prodex_smart_context_body_shape_plan_v1(
            SMART_CONTEXT_POLICY_ABI_VERSION,
            i64::from(input.json_valid),
            i64::from(input.json_shape_valid),
            i64::from(input.json_shape_reason),
            i64::from(input.rewrite_candidate),
            i64::from(input.static_context_changed),
            &mut reason as *mut i64 as usize as u64,
        )
    };
    if status == 4 {
        return Err(crate::MojoError::AbiMismatch);
    }
    if status != 0 || !matches!(reason, 0..=4) {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(match reason {
        0 => None,
        1 => Some("invalid_json"),
        2 => Some("json_depth_limit"),
        3 => Some("json_node_limit"),
        4 => Some("no_duplicate_candidate"),
        _ => unreachable!(),
    })
}

pub fn smart_context_rewrite_outcome(
    body_bytes_before: usize,
    body_bytes_after: usize,
    rehydrated_refs: usize,
    tool_outputs_condensed: usize,
    duplicate_texts: usize,
    static_context_deltas: usize,
) -> Result<&'static str, crate::MojoError> {
    let mut outcome = -1_i64;
    let status = unsafe {
        prodex_smart_context_rewrite_outcome_plan_v1(
            SMART_CONTEXT_POLICY_ABI_VERSION,
            u64::try_from(body_bytes_before).unwrap_or(u64::MAX),
            u64::try_from(body_bytes_after).unwrap_or(u64::MAX),
            u64::try_from(rehydrated_refs).unwrap_or(u64::MAX),
            u64::try_from(tool_outputs_condensed).unwrap_or(u64::MAX),
            u64::try_from(duplicate_texts).unwrap_or(u64::MAX),
            u64::try_from(static_context_deltas).unwrap_or(u64::MAX),
            &mut outcome as *mut i64 as usize as u64,
        )
    };
    if status == 4 {
        return Err(crate::MojoError::AbiMismatch);
    }
    if status != 0 || !matches!(outcome, 0..=3) {
        return Err(crate::MojoError::InvalidOutput);
    }
    Ok(match outcome {
        0 => "ok_rehydrate_exact",
        1 => "ok_saved",
        2 => "zero_savings",
        3 => "growth",
        _ => unreachable!(),
    })
}

/// Render a bounded telemetry label through the same Mojo policy that selected its tag.
pub fn smart_context_telemetry_label(kind: i64, value: u64) -> Result<String, crate::MojoError> {
    let mut output = [0_u8; 1024];
    let mut written = 0_i64;
    let status = unsafe {
        prodex_smart_context_telemetry_label_v1(
            SMART_CONTEXT_POLICY_ABI_VERSION,
            kind,
            value,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            &mut written as *mut i64 as usize as u64,
        )
    };
    match status {
        0 => {}
        3 => return Err(crate::MojoError::Capacity),
        4 => return Err(crate::MojoError::AbiMismatch),
        _ => return Err(crate::MojoError::InvalidOutput),
    }
    let written = usize::try_from(written)
        .ok()
        .filter(|written| *written <= output.len())
        .ok_or(crate::MojoError::InvalidOutput)?;
    String::from_utf8(output[..written].to_vec()).map_err(|_| crate::MojoError::InvalidOutput)
}

#[cfg(test)]
mod request_plan_tests {
    use super::*;

    #[test]
    fn body_admission_plan_preserves_safety_precedence_and_limits() {
        let base = SmartContextBodyAdmissionInput {
            body_bytes: 2_048,
            websocket: false,
            route_supported: true,
            content_type_supported: true,
            marker_present: false,
            static_context_required: false,
            websocket_generate_false: false,
        };
        assert_eq!(smart_context_body_admission_reason(base).unwrap(), None);
        assert_eq!(
            smart_context_body_admission_reason(SmartContextBodyAdmissionInput {
                route_supported: false,
                ..base
            })
            .unwrap(),
            Some("unsupported_route")
        );
        assert_eq!(
            smart_context_body_admission_reason(SmartContextBodyAdmissionInput {
                body_bytes: 1,
                ..base
            })
            .unwrap(),
            Some("below_minimum_body")
        );
        assert_eq!(
            smart_context_body_admission_reason(SmartContextBodyAdmissionInput {
                websocket: true,
                websocket_generate_false: true,
                ..base
            })
            .unwrap(),
            Some("websocket_generate_false")
        );
        assert_eq!(
            smart_context_body_admission_reason(SmartContextBodyAdmissionInput {
                body_bytes: 256 * 1024 + 1,
                ..base
            })
            .unwrap(),
            Some("body_too_large")
        );
    }

    #[test]
    fn body_shape_plan_covers_malformed_json_limits_and_noop() {
        assert_eq!(
            smart_context_body_shape_reason(SmartContextBodyShapeInput {
                json_valid: false,
                json_shape_valid: true,
                json_shape_reason: 0,
                rewrite_candidate: false,
                static_context_changed: false,
            })
            .unwrap(),
            Some("invalid_json")
        );
        assert_eq!(
            smart_context_body_shape_reason(SmartContextBodyShapeInput {
                json_valid: true,
                json_shape_valid: false,
                json_shape_reason: 1,
                rewrite_candidate: false,
                static_context_changed: false,
            })
            .unwrap(),
            Some("json_depth_limit")
        );
        assert_eq!(
            smart_context_body_shape_reason(SmartContextBodyShapeInput {
                json_valid: true,
                json_shape_valid: true,
                json_shape_reason: 0,
                rewrite_candidate: false,
                static_context_changed: false,
            })
            .unwrap(),
            Some("no_duplicate_candidate")
        );
    }

    #[test]
    fn rewrite_outcome_and_telemetry_labels_are_mojo_owned() {
        assert_eq!(
            smart_context_rewrite_outcome(100, 101, 1, 0, 0, 0).unwrap(),
            "ok_rehydrate_exact"
        );
        assert_eq!(
            smart_context_rewrite_outcome(100, 80, 0, 0, 0, 0).unwrap(),
            "ok_saved"
        );
        assert_eq!(
            smart_context_telemetry_label(4, 1 | (1 << 5)).unwrap(),
            "tool_output,rehydration"
        );
        assert_eq!(smart_context_telemetry_label(5, 0).unwrap(), "-");
    }
}
