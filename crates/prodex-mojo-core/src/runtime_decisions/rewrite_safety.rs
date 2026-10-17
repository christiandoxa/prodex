use super::{
    prodex_smart_context_json_shape_scan_v1, prodex_smart_context_rewrite_quality_risk_v1,
    prodex_smart_context_rewrite_sample_policy_v1,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartContextJsonShapeViolation {
    DepthLimit,
    NodeLimit,
}

/// Evaluate one already-ordered bounded serde traversal batch in compiled
/// Mojo. The host retains only Value ownership and traversal mechanics.
pub fn smart_context_json_shape_scan(
    depths: &[u64],
    previous_nodes: usize,
) -> Result<Option<SmartContextJsonShapeViolation>, crate::MojoError> {
    if depths.is_empty() || depths.len() > 1_024 {
        return Err(crate::MojoError::InvalidInput);
    }
    let mut violation = -1_i64;
    let status = unsafe {
        prodex_smart_context_json_shape_scan_v1(
            1,
            depths.as_ptr() as u64,
            i64::try_from(depths.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            u64::try_from(previous_nodes).map_err(|_| crate::MojoError::InvalidInput)?,
            &mut violation,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => crate::MojoError::InvalidInput,
            4 => crate::MojoError::AbiMismatch,
            _ => crate::MojoError::InvalidOutput,
        });
    }
    match violation {
        0 => Ok(None),
        1 => Ok(Some(SmartContextJsonShapeViolation::DepthLimit)),
        2 => Ok(Some(SmartContextJsonShapeViolation::NodeLimit)),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SmartContextRewriteQualityInput {
    pub upstream_context_errors: u16,
    pub previous_response_not_found: bool,
    pub invalid_tool_call_continuation: bool,
    pub missing_artifact_requests: u16,
    pub repeated_tool_call_count: u16,
    pub model_reread_requests: u16,
    pub corrective_user_messages: u16,
    pub test_or_build_failed_after_rewrite: bool,
    pub task_completed: Option<bool>,
}

fn rewrite_quality_metrics(input: SmartContextRewriteQualityInput) -> [i64; 9] {
    [
        i64::from(input.upstream_context_errors),
        i64::from(input.previous_response_not_found),
        i64::from(input.invalid_tool_call_continuation),
        i64::from(input.missing_artifact_requests),
        i64::from(input.repeated_tool_call_count),
        i64::from(input.model_reread_requests),
        i64::from(input.corrective_user_messages),
        i64::from(input.test_or_build_failed_after_rewrite),
        match input.task_completed {
            None => 0,
            Some(false) => 1,
            Some(true) => 2,
        },
    ]
}

#[derive(Debug, Clone, Copy)]
pub struct SmartContextRewriteSamplePolicyInput<'a> {
    pub rewrite_kind: &'a str,
    pub status: &'a str,
    pub fallback: bool,
    pub tokens_before: u64,
    pub tokens_after: u64,
    pub quality: SmartContextRewriteQualityInput,
}

/// Mojo alone decides if a runtime rewrite sample is relevant and safe to
/// count. None means no sample, Some(false) a risky/fallback sample.
pub fn smart_context_rewrite_sample_policy(
    input: SmartContextRewriteSamplePolicyInput<'_>,
) -> Result<Option<bool>, crate::MojoError> {
    let quality = rewrite_quality_metrics(input.quality);
    let mut output = [-1_i64; 3];
    let status = unsafe {
        prodex_smart_context_rewrite_sample_policy_v1(
            1,
            input.rewrite_kind.as_ptr() as usize as u64,
            i64::try_from(input.rewrite_kind.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            input.status.as_ptr() as usize as u64,
            i64::try_from(input.status.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::from(input.fallback),
            input.tokens_before,
            input.tokens_after,
            quality.as_ptr() as usize as u64,
            i64::try_from(quality.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            3,
        )
    };
    match status {
        0 => {}
        1 => return Err(crate::MojoError::InvalidInput),
        4 => return Err(crate::MojoError::AbiMismatch),
        _ => return Err(crate::MojoError::InvalidOutput),
    }
    match output {
        [0, 0, 0 | 1] => Ok(None),
        [1, 0, 0 | 1] => Ok(Some(false)),
        [1, 1, 0] => Ok(Some(true)),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

/// Classify upstream rewrite damage through Mojo, without a Rust OR policy.
pub fn smart_context_rewrite_telemetry_quality_risk(
    input: SmartContextRewriteQualityInput,
) -> Result<bool, crate::MojoError> {
    let values = rewrite_quality_metrics(input);
    let mut result = -1i64;
    let status = unsafe {
        prodex_smart_context_rewrite_quality_risk_v1(
            values.as_ptr() as u64,
            values.len() as i64,
            &mut result,
        )
    };
    if status != 0 {
        return Err(crate::MojoError::InvalidInput);
    }
    match result {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}
