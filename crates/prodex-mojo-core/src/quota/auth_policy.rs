use super::*;

pub fn quota_auth_summary_kind(
    auth_mode: Option<&str>,
    has_chatgpt_token: bool,
    has_api_key: bool,
    has_bedrock_api_key: bool,
) -> Result<QuotaAuthSummaryKind, crate::MojoError> {
    let raw = auth_mode.unwrap_or_default();
    let result = unsafe {
        prodex_quota_auth_summary_kind_v1(
            QUOTA_MODEL_POLICY_ABI_VERSION,
            raw.as_ptr() as usize as u64,
            i64::try_from(raw.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::from(auth_mode.is_some()),
            i64::from(has_chatgpt_token),
            i64::from(has_api_key),
            i64::from(has_bedrock_api_key),
        )
    };
    match result {
        0 => Ok(QuotaAuthSummaryKind::Chatgpt),
        1 => Ok(QuotaAuthSummaryKind::BedrockApiKey),
        2 => Ok(QuotaAuthSummaryKind::ApiKey),
        3 => Ok(QuotaAuthSummaryKind::Other),
        -1 => Err(crate::MojoError::InvalidInput),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaUsageAuthKind {
    ChatgptEligible,
    BedrockApiKey,
    ApiKey,
}

pub fn quota_usage_auth_kind(
    auth_mode: Option<&str>,
    has_api_key: bool,
    has_bedrock_api_key: bool,
) -> Result<QuotaUsageAuthKind, crate::MojoError> {
    let raw = auth_mode.unwrap_or_default();
    let result = unsafe {
        prodex_quota_usage_auth_kind_v1(
            QUOTA_MODEL_POLICY_ABI_VERSION,
            raw.as_ptr() as usize as u64,
            i64::try_from(raw.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::from(auth_mode.is_some()),
            i64::from(has_api_key),
            i64::from(has_bedrock_api_key),
        )
    };
    match result {
        0 => Ok(QuotaUsageAuthKind::ChatgptEligible),
        1 => Ok(QuotaUsageAuthKind::BedrockApiKey),
        2 => Ok(QuotaUsageAuthKind::ApiKey),
        -1 => Err(crate::MojoError::InvalidInput),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn quota_auth_needs_proactive_refresh(
    expires_at: Option<i64>,
    last_refresh: Option<i64>,
    now: i64,
    expiry_skew_seconds: i64,
    refresh_interval_days: i64,
) -> Result<bool, crate::MojoError> {
    let result = unsafe {
        prodex_quota_auth_proactive_refresh_v1(
            QUOTA_MODEL_POLICY_ABI_VERSION,
            i64::from(expires_at.is_some()),
            expires_at.unwrap_or_default(),
            i64::from(last_refresh.is_some()),
            last_refresh.unwrap_or_default(),
            now,
            expiry_skew_seconds,
            refresh_interval_days,
        )
    };
    match result {
        0 => Ok(false),
        1 => Ok(true),
        -1 => Err(crate::MojoError::InvalidInput),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn quota_auth_filter_parse(raw: &str) -> Result<QuotaAuthFilterPlan, crate::MojoError> {
    let mut output = vec![0_u8; raw.len().max(1)];
    let mut written = 0_i64;
    let mut kind = -1_i64;
    let status = unsafe {
        prodex_quota_auth_filter_parse_v1(
            QUOTA_MODEL_POLICY_ABI_VERSION,
            raw.as_ptr() as usize as u64,
            i64::try_from(raw.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
            (&mut kind as *mut i64) as usize as u64,
        )
    };
    quota_model_policy_status(status)?;
    let written = usize::try_from(written).map_err(|_| crate::MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(crate::MojoError::InvalidOutput);
    }
    match kind {
        0 => Ok(QuotaAuthFilterPlan::All),
        1 => String::from_utf8(output[..written].to_vec())
            .map(QuotaAuthFilterPlan::Label)
            .map_err(|_| crate::MojoError::InvalidOutput),
        2 => Ok(QuotaAuthFilterPlan::QuotaCompatible),
        3 => Ok(QuotaAuthFilterPlan::NonQuotaCompatible),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn quota_auth_filter_matches(
    kind: i64,
    filter_label: &str,
    auth_label: &str,
    quota_compatible: bool,
) -> Result<bool, crate::MojoError> {
    let value = unsafe {
        prodex_quota_auth_filter_matches_v1(
            QUOTA_MODEL_POLICY_ABI_VERSION,
            kind,
            filter_label.as_ptr() as usize as u64,
            i64::try_from(filter_label.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            auth_label.as_ptr() as usize as u64,
            i64::try_from(auth_label.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::from(quota_compatible),
        )
    };
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}
