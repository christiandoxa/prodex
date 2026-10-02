use crate::MojoError;

const ABI_VERSION: i64 = 1;
const METRICS_PER_ROW: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuditUsageRowInput {
    pub recorded_at_epoch: i64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
    pub cost_micros: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AuditUsageSummary {
    pub requests: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cached_input_tokens: u64,
    pub reasoning_tokens: u64,
    pub total_tokens: u64,
    pub cost_micros: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AuditBudgetFlags {
    pub request_limit_reached: bool,
    pub token_limit_reached: bool,
    pub cost_limit_reached: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditBudgetEvaluationPlan {
    pub allowed: bool,
    pub reasons: Vec<String>,
}

unsafe extern "C" {
    fn prodex_audit_usage_token_normalize_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        fallback_address: u64,
        fallback_length: i64,
        max_chars: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_audit_usage_total_v1(
        abi_version: i64,
        current_total: u64,
        input_tokens: u64,
        output_tokens: u64,
        reasoning_tokens: u64,
        output_address: u64,
    ) -> i64;
    fn prodex_audit_usage_summary_v1(
        abi_version: i64,
        epochs_address: u64,
        metrics_address: u64,
        row_count: i64,
        since_epoch: i64,
        until_epoch: i64,
        output_address: u64,
    ) -> i64;
    fn prodex_audit_budget_flags_v1(
        abi_version: i64,
        requests: u64,
        total_tokens: u64,
        cost_micros: u64,
        max_requests_present: i64,
        max_requests: u64,
        max_tokens_present: i64,
        max_tokens: u64,
        max_cost_present: i64,
        max_cost_micros: u64,
        output_address: u64,
    ) -> i64;
    fn prodex_audit_budget_evaluation_v1(
        abi_version: i64,
        requests: u64,
        total_tokens: u64,
        cost_micros: u64,
        max_requests_present: i64,
        max_requests: u64,
        max_tokens_present: i64,
        max_tokens: u64,
        max_cost_present: i64,
        max_cost_micros: u64,
        output_address: u64,
        output_capacity: i64,
        lengths_address: u64,
        allowed_address: u64,
    ) -> i64;
    fn prodex_audit_query_has_filters_v1(
        abi_version: i64,
        component_present: i64,
        action_present: i64,
        outcome_present: i64,
    ) -> i64;
    fn prodex_audit_query_matches_v1(
        abi_version: i64,
        component_address: u64,
        component_length: i64,
        component_present: i64,
        action_address: u64,
        action_length: i64,
        action_present: i64,
        outcome_address: u64,
        outcome_length: i64,
        outcome_present: i64,
        event_component_address: u64,
        event_component_length: i64,
        event_action_address: u64,
        event_action_length: i64,
        event_outcome_address: u64,
        event_outcome_length: i64,
    ) -> i64;
    fn prodex_audit_query_format_v1(
        abi_version: i64,
        component_address: u64,
        component_length: i64,
        component_present: i64,
        action_address: u64,
        action_length: i64,
        action_present: i64,
        outcome_address: u64,
        outcome_length: i64,
        outcome_present: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_audit_search_scope_format_v1(
        abi_version: i64,
        searched_bytes: u64,
        log_size_bytes: u64,
        search_start_byte: u64,
        read_limit_bytes: u64,
        limited: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_audit_truncate_text_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        max_chars: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_audit_profile_name_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        input_present: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
        present_address: u64,
    ) -> i64;
    fn prodex_audit_account_hint_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        input_present: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
        present_address: u64,
    ) -> i64;
    fn prodex_audit_email_domain_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        input_present: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
        present_address: u64,
    ) -> i64;
}

fn status(value: i64) -> Result<(), MojoError> {
    match value {
        0 => Ok(()),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn normalize_usage_token(
    value: &str,
    fallback: &str,
    max_chars: usize,
) -> Result<String, MojoError> {
    let capacity = max_chars.max(fallback.len());
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    status(unsafe {
        prodex_audit_usage_token_normalize_v1(
            ABI_VERSION,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            fallback.as_ptr() as usize as u64,
            i64::try_from(fallback.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(max_chars).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..written].to_vec()).map_err(|_| MojoError::InvalidOutput)
}

pub fn normalized_total_tokens(
    current_total: u64,
    input_tokens: u64,
    output_tokens: u64,
    reasoning_tokens: u64,
) -> Result<u64, MojoError> {
    let mut output = 0_u64;
    status(unsafe {
        prodex_audit_usage_total_v1(
            ABI_VERSION,
            current_total,
            input_tokens,
            output_tokens,
            reasoning_tokens,
            (&mut output as *mut u64) as usize as u64,
        )
    })?;
    Ok(output)
}

pub fn summarize_usage(
    rows: &[AuditUsageRowInput],
    since_epoch: i64,
    until_epoch: i64,
) -> Result<AuditUsageSummary, MojoError> {
    let mut epochs = Vec::with_capacity(rows.len());
    let mut metrics = Vec::with_capacity(rows.len().saturating_mul(METRICS_PER_ROW));
    for row in rows {
        epochs.push(row.recorded_at_epoch);
        metrics.extend_from_slice(&[
            row.input_tokens,
            row.output_tokens,
            row.cached_input_tokens,
            row.reasoning_tokens,
            row.total_tokens,
            row.cost_micros,
        ]);
    }
    let mut output = [0_u64; 7];
    status(unsafe {
        prodex_audit_usage_summary_v1(
            ABI_VERSION,
            epochs.as_ptr() as usize as u64,
            metrics.as_ptr() as usize as u64,
            i64::try_from(rows.len()).map_err(|_| MojoError::InvalidInput)?,
            since_epoch,
            until_epoch,
            output.as_mut_ptr() as usize as u64,
        )
    })?;
    Ok(AuditUsageSummary {
        requests: output[0],
        input_tokens: output[1],
        output_tokens: output[2],
        cached_input_tokens: output[3],
        reasoning_tokens: output[4],
        total_tokens: output[5],
        cost_micros: output[6],
    })
}

pub fn budget_flags(
    summary: AuditUsageSummary,
    max_requests: Option<u64>,
    max_tokens: Option<u64>,
    max_cost_micros: Option<u64>,
) -> Result<AuditBudgetFlags, MojoError> {
    let mut flags = 0_u64;
    status(unsafe {
        prodex_audit_budget_flags_v1(
            ABI_VERSION,
            summary.requests,
            summary.total_tokens,
            summary.cost_micros,
            i64::from(max_requests.is_some()),
            max_requests.unwrap_or_default(),
            i64::from(max_tokens.is_some()),
            max_tokens.unwrap_or_default(),
            i64::from(max_cost_micros.is_some()),
            max_cost_micros.unwrap_or_default(),
            (&mut flags as *mut u64) as usize as u64,
        )
    })?;
    if flags & !0b111 != 0 {
        return Err(MojoError::InvalidOutput);
    }
    Ok(AuditBudgetFlags {
        request_limit_reached: flags & 1 != 0,
        token_limit_reached: flags & 2 != 0,
        cost_limit_reached: flags & 4 != 0,
    })
}

pub fn budget_evaluation(
    summary: AuditUsageSummary,
    max_requests: Option<u64>,
    max_tokens: Option<u64>,
    max_cost_micros: Option<u64>,
) -> Result<AuditBudgetEvaluationPlan, MojoError> {
    let mut output = [0_u8; 256];
    let mut lengths = [0_i64; 4];
    let mut allowed = -1_i64;
    status(unsafe {
        prodex_audit_budget_evaluation_v1(
            ABI_VERSION,
            summary.requests,
            summary.total_tokens,
            summary.cost_micros,
            i64::from(max_requests.is_some()),
            max_requests.unwrap_or_default(),
            i64::from(max_tokens.is_some()),
            max_tokens.unwrap_or_default(),
            i64::from(max_cost_micros.is_some()),
            max_cost_micros.unwrap_or_default(),
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            lengths.as_mut_ptr() as usize as u64,
            (&mut allowed as *mut i64) as usize as u64,
        )
    })?;
    let count = usize::try_from(lengths[0]).map_err(|_| MojoError::InvalidOutput)?;
    if count > 3 {
        return Err(MojoError::InvalidOutput);
    }
    let allowed = match allowed {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    if allowed != (count == 0) {
        return Err(MojoError::InvalidOutput);
    }
    let mut cursor = 0_usize;
    let mut reasons = Vec::with_capacity(count);
    for length in lengths.iter().skip(1).take(count) {
        let length = usize::try_from(*length).map_err(|_| MojoError::InvalidOutput)?;
        let end = cursor.checked_add(length).ok_or(MojoError::InvalidOutput)?;
        let bytes = output.get(cursor..end).ok_or(MojoError::InvalidOutput)?;
        reasons.push(String::from_utf8(bytes.to_vec()).map_err(|_| MojoError::InvalidOutput)?);
        cursor = end;
    }
    Ok(AuditBudgetEvaluationPlan { allowed, reasons })
}

fn optional_text_parts(value: Option<&str>) -> Result<(u64, i64, i64), MojoError> {
    match value {
        Some(value) => Ok((
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            1,
        )),
        None => Ok((0, 0, 0)),
    }
}

fn decode_text(output: &[u8], written: i64) -> Result<String, MojoError> {
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..written].to_vec()).map_err(|_| MojoError::InvalidOutput)
}

pub fn query_has_filters(
    component: Option<&str>,
    action: Option<&str>,
    outcome: Option<&str>,
) -> Result<bool, MojoError> {
    let value = unsafe {
        prodex_audit_query_has_filters_v1(
            ABI_VERSION,
            i64::from(component.is_some()),
            i64::from(action.is_some()),
            i64::from(outcome.is_some()),
        )
    };
    match value {
        0 => Ok(false),
        1 => Ok(true),
        -4 => Err(MojoError::AbiMismatch),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn query_matches(
    component: Option<&str>,
    action: Option<&str>,
    outcome: Option<&str>,
    event_component: &str,
    event_action: &str,
    event_outcome: &str,
) -> Result<bool, MojoError> {
    let (component_address, component_length, component_present) = optional_text_parts(component)?;
    let (action_address, action_length, action_present) = optional_text_parts(action)?;
    let (outcome_address, outcome_length, outcome_present) = optional_text_parts(outcome)?;
    let value = unsafe {
        prodex_audit_query_matches_v1(
            ABI_VERSION,
            component_address,
            component_length,
            component_present,
            action_address,
            action_length,
            action_present,
            outcome_address,
            outcome_length,
            outcome_present,
            event_component.as_ptr() as usize as u64,
            i64::try_from(event_component.len()).map_err(|_| MojoError::InvalidInput)?,
            event_action.as_ptr() as usize as u64,
            i64::try_from(event_action.len()).map_err(|_| MojoError::InvalidInput)?,
            event_outcome.as_ptr() as usize as u64,
            i64::try_from(event_outcome.len()).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    match value {
        0 => Ok(false),
        1 => Ok(true),
        -4 => Err(MojoError::AbiMismatch),
        -1 => Err(MojoError::InvalidInput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn format_query(
    component: Option<&str>,
    action: Option<&str>,
    outcome: Option<&str>,
) -> Result<String, MojoError> {
    let (component_address, component_length, component_present) = optional_text_parts(component)?;
    let (action_address, action_length, action_present) = optional_text_parts(action)?;
    let (outcome_address, outcome_length, outcome_present) = optional_text_parts(outcome)?;
    let capacity = component.map(str::len).unwrap_or_default()
        + action.map(str::len).unwrap_or_default()
        + outcome.map(str::len).unwrap_or_default()
        + 40;
    let mut output = vec![0_u8; capacity.max(4)];
    let mut written = -1_i64;
    status(unsafe {
        prodex_audit_query_format_v1(
            ABI_VERSION,
            component_address,
            component_length,
            component_present,
            action_address,
            action_length,
            action_present,
            outcome_address,
            outcome_length,
            outcome_present,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    decode_text(&output, written)
}

pub fn format_search_scope(
    searched_bytes: u64,
    log_size_bytes: u64,
    search_start_byte: u64,
    read_limit_bytes: u64,
    limited: bool,
) -> Result<String, MojoError> {
    let mut output = [0_u8; 192];
    let mut written = -1_i64;
    status(unsafe {
        prodex_audit_search_scope_format_v1(
            ABI_VERSION,
            searched_bytes,
            log_size_bytes,
            search_start_byte,
            read_limit_bytes,
            i64::from(limited),
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    decode_text(&output, written)
}

pub fn truncate_text(value: &str, max_chars: usize) -> Result<String, MojoError> {
    let capacity = value.len().saturating_add(3).max(1);
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    status(unsafe {
        prodex_audit_truncate_text_v1(
            ABI_VERSION,
            value.as_ptr() as usize as u64,
            i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(max_chars).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    })?;
    decode_text(&output, written)
}

type AuditMetadataPolicyFn = unsafe extern "C" fn(i64, u64, i64, i64, u64, i64, u64, u64) -> i64;

fn metadata_text(
    policy: AuditMetadataPolicyFn,
    value: Option<&str>,
) -> Result<Option<String>, MojoError> {
    let (address, length, present) = optional_text_parts(value)?;
    let capacity = value
        .map(str::len)
        .unwrap_or_default()
        .saturating_add(3)
        .max(1);
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    let mut output_present = -1_i64;
    status(unsafe {
        policy(
            ABI_VERSION,
            address,
            length,
            present,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
            (&mut output_present as *mut i64) as usize as u64,
        )
    })?;
    match output_present {
        0 => Ok(None),
        1 => decode_text(&output, written).map(Some),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn profile_name(value: Option<&str>) -> Result<Option<String>, MojoError> {
    metadata_text(prodex_audit_profile_name_v1, value)
}

pub fn account_hint(value: Option<&str>) -> Result<Option<String>, MojoError> {
    metadata_text(prodex_audit_account_hint_v1, value)
}

pub fn email_domain(value: Option<&str>) -> Result<Option<String>, MojoError> {
    metadata_text(prodex_audit_email_domain_v1, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_usage_policy_preserves_normalization_summary_and_limits() {
        assert_eq!(
            normalize_usage_token("  Project A β  ", "unknown", 64).unwrap(),
            "project-a"
        );
        assert_eq!(
            normalize_usage_token(" β ", "global", 64).unwrap(),
            "global"
        );
        assert_eq!(
            normalized_total_tokens(0, u64::MAX, 1, 7).unwrap(),
            u64::MAX
        );
        assert_eq!(normalized_total_tokens(9, 1, 2, 3).unwrap(), 9);

        let summary = summarize_usage(
            &[
                AuditUsageRowInput {
                    recorded_at_epoch: 10,
                    input_tokens: 2,
                    output_tokens: 3,
                    cached_input_tokens: 4,
                    reasoning_tokens: 5,
                    total_tokens: 10,
                    cost_micros: 7,
                },
                AuditUsageRowInput {
                    recorded_at_epoch: 20,
                    input_tokens: u64::MAX,
                    output_tokens: 1,
                    cached_input_tokens: 1,
                    reasoning_tokens: 1,
                    total_tokens: u64::MAX,
                    cost_micros: u64::MAX,
                },
            ],
            10,
            20,
        )
        .unwrap();
        assert_eq!(summary.requests, 2);
        assert_eq!(summary.input_tokens, u64::MAX);
        assert_eq!(summary.total_tokens, u64::MAX);
        assert_eq!(summary.cost_micros, u64::MAX);
        assert_eq!(
            budget_flags(summary, Some(2), Some(u64::MAX), None).unwrap(),
            AuditBudgetFlags {
                request_limit_reached: true,
                token_limit_reached: true,
                cost_limit_reached: false,
            }
        );
        assert_eq!(
            budget_evaluation(summary, Some(2), Some(u64::MAX), None).unwrap(),
            AuditBudgetEvaluationPlan {
                allowed: false,
                reasons: vec![
                    "request limit reached (2/2)".to_string(),
                    format!("token limit reached ({}/{})", u64::MAX, u64::MAX),
                ],
            }
        );
        assert_eq!(
            budget_evaluation(
                AuditUsageSummary {
                    requests: 1,
                    total_tokens: 2,
                    cost_micros: 3,
                    ..AuditUsageSummary::default()
                },
                Some(9),
                Some(9),
                Some(9),
            )
            .unwrap(),
            AuditBudgetEvaluationPlan {
                allowed: true,
                reasons: Vec::new(),
            }
        );
        assert!(query_has_filters(Some("profile"), None, None).unwrap());
        assert!(!query_has_filters(None, None, None).unwrap());
        assert!(
            query_matches(
                Some("profile"),
                None,
                Some("success"),
                "profile",
                "add",
                "success",
            )
            .unwrap()
        );
        assert!(!query_matches(Some("runtime"), None, None, "profile", "add", "success",).unwrap());
        assert_eq!(
            format_query(Some("profile"), None, Some("success")).unwrap(),
            "component=profile outcome=success"
        );
        assert_eq!(format_query(None, None, None).unwrap(), "none");
        assert_eq!(
            format_search_scope(512, 1024, 512, 512, true).unwrap(),
            "searched 512 of 1024 bytes (byte range 512..1024) limited to last 512 bytes"
        );
        assert_eq!(truncate_text("αβγδε", 3).unwrap(), "αβγ...");
        assert_eq!(truncate_text("αβγ", 3).unwrap(), "αβγ");
        assert_eq!(
            profile_name(Some("  Team β  ")).unwrap(),
            Some("Team β".to_string())
        );
        assert_eq!(profile_name(Some("   ")).unwrap(), None);
        assert_eq!(
            account_hint(Some(" demo-account-1234 ")).unwrap(),
            Some("...1234".to_string())
        );
        assert_eq!(
            account_hint(Some("🙂αβγδε")).unwrap(),
            Some("...βγδε".to_string())
        );
        assert_eq!(
            email_domain(Some(" Example@Sub.DOMAIN.TEST ")).unwrap(),
            Some("sub.domain.test".to_string())
        );
        assert_eq!(
            email_domain(Some("local@ignored@Example.COM")).unwrap(),
            Some("example.com".to_string())
        );
        assert_eq!(email_domain(Some("missing-at")).unwrap(), None);
    }
}
