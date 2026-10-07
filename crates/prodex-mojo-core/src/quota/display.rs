use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopilotQuotaDisplay {
    pub ready: bool,
    pub status: String,
    pub main: String,
}

fn load_copilot_feature_key(index: i64) -> Result<String, crate::MojoError> {
    let mut output = [0_u8; 32];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_quota_copilot_feature_key_v1(
            QUOTA_MODEL_POLICY_ABI_VERSION,
            index,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    quota_model_policy_status(status)?;
    let written = usize::try_from(written).map_err(|_| crate::MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(crate::MojoError::InvalidOutput);
    }
    String::from_utf8(output[..written].to_vec()).map_err(|_| crate::MojoError::InvalidOutput)
}

pub fn quota_copilot_feature_key(index: usize) -> Result<&'static str, crate::MojoError> {
    use std::sync::OnceLock;

    static KEYS: OnceLock<Result<Vec<String>, crate::MojoError>> = OnceLock::new();
    if index >= 2 {
        return Err(crate::MojoError::InvalidInput);
    }
    match KEYS.get_or_init(|| (0_i64..2).map(load_copilot_feature_key).collect()) {
        Ok(keys) => keys
            .get(index)
            .map(String::as_str)
            .ok_or(crate::MojoError::InvalidOutput),
        Err(error) => Err(*error),
    }
}

pub fn quota_copilot_display(
    chat_remaining: Option<i64>,
    chat_total: Option<i64>,
    completions_remaining: Option<i64>,
    completions_total: Option<i64>,
) -> Result<CopilotQuotaDisplay, crate::MojoError> {
    const STATUS_CAPACITY: usize = 16;
    const MAIN_CAPACITY: usize = 128;
    let mut status_output = [0_u8; STATUS_CAPACITY];
    let mut status_written = -1_i64;
    let mut main_output = [0_u8; MAIN_CAPACITY];
    let mut main_written = -1_i64;
    let mut ready = -1_i64;
    let status = unsafe {
        prodex_quota_copilot_display_v1(
            QUOTA_MODEL_POLICY_ABI_VERSION,
            i64::from(chat_remaining.is_some()),
            chat_remaining.unwrap_or_default(),
            i64::from(chat_total.is_some()),
            chat_total.unwrap_or_default(),
            i64::from(completions_remaining.is_some()),
            completions_remaining.unwrap_or_default(),
            i64::from(completions_total.is_some()),
            completions_total.unwrap_or_default(),
            status_output.as_mut_ptr() as usize as u64,
            i64::try_from(status_output.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            (&mut status_written as *mut i64) as usize as u64,
            main_output.as_mut_ptr() as usize as u64,
            i64::try_from(main_output.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            (&mut main_written as *mut i64) as usize as u64,
            (&mut ready as *mut i64) as usize as u64,
        )
    };
    quota_model_policy_status(status)?;
    let ready = match ready {
        0 => false,
        1 => true,
        _ => return Err(crate::MojoError::InvalidOutput),
    };
    let status_written =
        usize::try_from(status_written).map_err(|_| crate::MojoError::InvalidOutput)?;
    let main_written =
        usize::try_from(main_written).map_err(|_| crate::MojoError::InvalidOutput)?;
    if status_written > status_output.len() || main_written > main_output.len() {
        return Err(crate::MojoError::InvalidOutput);
    }
    let status = String::from_utf8(status_output[..status_written].to_vec())
        .map_err(|_| crate::MojoError::InvalidOutput)?;
    let main = String::from_utf8(main_output[..main_written].to_vec())
        .map_err(|_| crate::MojoError::InvalidOutput)?;
    Ok(CopilotQuotaDisplay {
        ready,
        status,
        main,
    })
}

pub fn quota_copilot_main_remaining_percent(
    chat_remaining: Option<i64>,
    chat_total: Option<i64>,
    completions_remaining: Option<i64>,
    completions_total: Option<i64>,
) -> Result<Option<i64>, crate::MojoError> {
    let mut percent = 0_i64;
    let mut present = -1_i64;
    let status = unsafe {
        prodex_quota_copilot_main_remaining_percent_v1(
            QUOTA_MODEL_POLICY_ABI_VERSION,
            i64::from(chat_remaining.is_some()),
            chat_remaining.unwrap_or_default(),
            i64::from(chat_total.is_some()),
            chat_total.unwrap_or_default(),
            i64::from(completions_remaining.is_some()),
            completions_remaining.unwrap_or_default(),
            i64::from(completions_total.is_some()),
            completions_total.unwrap_or_default(),
            (&mut percent as *mut i64) as usize as u64,
            (&mut present as *mut i64) as usize as u64,
        )
    };
    quota_model_policy_status(status)?;
    match present {
        0 => Ok(None),
        1 => Ok(Some(percent)),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn quota_ready_pool_remaining(
    five_hour_remaining: i64,
    weekly_remaining: i64,
    ready_profiles: usize,
    five_hour_profiles: usize,
    weekly_profiles: usize,
) -> Result<String, crate::MojoError> {
    let mut output = [0_u8; 160];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_quota_ready_pool_remaining_v1(
            QUOTA_MODEL_POLICY_ABI_VERSION,
            five_hour_remaining,
            weekly_remaining,
            i64::try_from(ready_profiles).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::try_from(five_hour_profiles).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::try_from(weekly_profiles).map_err(|_| crate::MojoError::InvalidInput)?,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    quota_model_policy_status(status)?;
    decode_quota_text(&output, written)
}

pub fn quota_info_pool_remaining(
    total_remaining: i64,
    profiles_with_data: usize,
    reset: Option<&str>,
) -> Result<String, crate::MojoError> {
    let (reset_address, reset_length) = quota_text_address(reset);
    if reset_length == i64::MAX {
        return Err(crate::MojoError::InvalidInput);
    }
    let capacity = reset
        .map(str::len)
        .unwrap_or_default()
        .saturating_add(128)
        .max(1);
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_quota_info_pool_remaining_v1(
            QUOTA_MODEL_POLICY_ABI_VERSION,
            total_remaining,
            i64::try_from(profiles_with_data).map_err(|_| crate::MojoError::InvalidInput)?,
            reset_address,
            reset_length,
            i64::from(reset.is_some()),
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    quota_model_policy_status(status)?;
    decode_quota_text(&output, written)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiQuotaDisplay {
    pub ready: bool,
    pub status: String,
    pub main: String,
    pub remaining_percent: Option<i64>,
}

pub(super) fn decode_quota_text(output: &[u8], written: i64) -> Result<String, crate::MojoError> {
    let written = usize::try_from(written).map_err(|_| crate::MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(crate::MojoError::InvalidOutput);
    }
    String::from_utf8(output[..written].to_vec()).map_err(|_| crate::MojoError::InvalidOutput)
}

pub fn quota_gemini_bucket_label(
    model_id: Option<&str>,
    token_type: Option<&str>,
) -> Result<String, crate::MojoError> {
    let (model_address, model_length) = quota_text_address(model_id);
    let (token_address, token_length) = quota_text_address(token_type);
    if model_length == i64::MAX || token_length == i64::MAX {
        return Err(crate::MojoError::InvalidInput);
    }
    let capacity = model_id
        .map(str::len)
        .unwrap_or_default()
        .max(token_type.map(str::len).unwrap_or_default())
        .max(6);
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_quota_gemini_bucket_label_v1(
            QUOTA_MODEL_POLICY_ABI_VERSION,
            model_address,
            model_length,
            i64::from(model_id.is_some()),
            token_address,
            token_length,
            i64::from(token_type.is_some()),
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    quota_model_policy_status(status)?;
    decode_quota_text(&output, written)
}

pub fn quota_gemini_bucket_summary(
    label: &str,
    numeric: GeminiBucketNumericOutput,
) -> Result<String, crate::MojoError> {
    let capacity = label
        .len()
        .checked_add(64)
        .ok_or(crate::MojoError::InvalidInput)?;
    let mut output = vec![0_u8; capacity.max(1)];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_quota_gemini_bucket_summary_v1(
            QUOTA_MODEL_POLICY_ABI_VERSION,
            label.as_ptr() as usize as u64,
            i64::try_from(label.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            i64::from(numeric.remaining.is_some()),
            numeric.remaining.unwrap_or_default(),
            i64::from(numeric.total.is_some()),
            numeric.total.unwrap_or_default(),
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    quota_model_policy_status(status)?;
    decode_quota_text(&output, written)
}

pub fn quota_gemini_display(
    numeric: &[GeminiBucketNumericOutput],
) -> Result<GeminiQuotaDisplay, crate::MojoError> {
    let mut remaining = Vec::with_capacity(numeric.len());
    let mut remaining_present = Vec::with_capacity(numeric.len());
    let mut percent = Vec::with_capacity(numeric.len());
    let mut percent_present = Vec::with_capacity(numeric.len());
    let mut exhausted = Vec::with_capacity(numeric.len());
    for value in numeric {
        remaining.push(value.remaining.unwrap_or_default());
        remaining_present.push(i64::from(value.remaining.is_some()));
        percent.push(value.remaining_percent.unwrap_or_default());
        percent_present.push(i64::from(value.remaining_percent.is_some()));
        exhausted.push(i64::from(value.exhausted));
    }

    let mut status_output = [0_u8; 16];
    let mut status_written = -1_i64;
    let mut main_output = [0_u8; 128];
    let mut main_written = -1_i64;
    let mut ready = -1_i64;
    let mut min_percent = 0_i64;
    let mut min_percent_present = -1_i64;
    let status = unsafe {
        prodex_quota_gemini_display_v1(
            QUOTA_MODEL_POLICY_ABI_VERSION,
            remaining.as_ptr() as usize as u64,
            remaining_present.as_ptr() as usize as u64,
            percent.as_ptr() as usize as u64,
            percent_present.as_ptr() as usize as u64,
            exhausted.as_ptr() as usize as u64,
            i64::try_from(numeric.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            status_output.as_mut_ptr() as usize as u64,
            i64::try_from(status_output.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            (&mut status_written as *mut i64) as usize as u64,
            main_output.as_mut_ptr() as usize as u64,
            i64::try_from(main_output.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            (&mut main_written as *mut i64) as usize as u64,
            (&mut ready as *mut i64) as usize as u64,
            (&mut min_percent as *mut i64) as usize as u64,
            (&mut min_percent_present as *mut i64) as usize as u64,
        )
    };
    quota_model_policy_status(status)?;
    let ready = match ready {
        0 => false,
        1 => true,
        _ => return Err(crate::MojoError::InvalidOutput),
    };
    let remaining_percent = match min_percent_present {
        0 => None,
        1 => Some(min_percent),
        _ => return Err(crate::MojoError::InvalidOutput),
    };
    Ok(GeminiQuotaDisplay {
        ready,
        status: decode_quota_text(&status_output, status_written)?,
        main: decode_quota_text(&main_output, main_written)?,
        remaining_percent,
    })
}
