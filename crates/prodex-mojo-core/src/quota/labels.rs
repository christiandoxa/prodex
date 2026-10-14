//! Mojo label result decoding and process-lifetime label caches.
use super::*;

pub(super) fn load_quota_display_label(kind: i64, value: i64) -> Result<String, crate::MojoError> {
    let mut output = [0_u8; QUOTA_DISPLAY_LABEL_MAX_BYTES];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_quota_display_label_v1(
            QUOTA_MODEL_POLICY_ABI_VERSION,
            kind,
            value,
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

pub fn quota_blocked_status_label(kind: i64) -> Result<&'static str, crate::MojoError> {
    use std::sync::OnceLock;

    static LABELS: OnceLock<Result<Vec<String>, crate::MojoError>> = OnceLock::new();
    let kind = usize::try_from(kind)
        .ok()
        .filter(|kind| *kind <= QUOTA_BLOCKED_KIND_FIVE_HOUR as usize)
        .ok_or(crate::MojoError::InvalidInput)?;
    match LABELS.get_or_init(|| {
        (QUOTA_BLOCKED_KIND_NONE..=QUOTA_BLOCKED_KIND_FIVE_HOUR)
            .map(|value| load_quota_display_label(QUOTA_DISPLAY_LABEL_BLOCKED_STATUS, value))
            .collect()
    }) {
        Ok(labels) => labels
            .get(kind)
            .map(String::as_str)
            .ok_or(crate::MojoError::InvalidOutput),
        Err(error) => Err(*error),
    }
}

pub fn quota_usage_auth_sync_source_label(source: i64) -> Result<&'static str, crate::MojoError> {
    use std::sync::OnceLock;

    static LABELS: OnceLock<Result<Vec<String>, crate::MojoError>> = OnceLock::new();
    let source = usize::try_from(source)
        .ok()
        .filter(|source| *source < 2)
        .ok_or(crate::MojoError::InvalidInput)?;
    match LABELS.get_or_init(|| {
        (0_i64..2)
            .map(|value| load_quota_display_label(QUOTA_DISPLAY_LABEL_AUTH_SYNC_SOURCE, value))
            .collect()
    }) {
        Ok(labels) => labels
            .get(source)
            .map(String::as_str)
            .ok_or(crate::MojoError::InvalidOutput),
        Err(error) => Err(*error),
    }
}

pub fn quota_auth_summary_label(kind: i64) -> Result<&'static str, crate::MojoError> {
    use std::sync::OnceLock;

    static LABELS: OnceLock<Result<Vec<String>, crate::MojoError>> = OnceLock::new();
    let kind = usize::try_from(kind)
        .ok()
        .filter(|kind| *kind < 7)
        .ok_or(crate::MojoError::InvalidInput)?;
    match LABELS.get_or_init(|| {
        (0_i64..7)
            .map(|value| load_quota_display_label(QUOTA_DISPLAY_LABEL_AUTH_SUMMARY, value))
            .collect()
    }) {
        Ok(labels) => labels
            .get(kind)
            .map(String::as_str)
            .ok_or(crate::MojoError::InvalidOutput),
        Err(error) => Err(*error),
    }
}

pub fn quota_window_label(seconds: Option<i64>) -> Result<String, crate::MojoError> {
    let mut output = [0_u8; QUOTA_DISPLAY_LABEL_MAX_BYTES];
    let mut written = -1_i64;
    let mut kind = -1_i64;
    let status = unsafe {
        prodex_quota_window_label_plan_v1(
            QUOTA_MODEL_POLICY_ABI_VERSION,
            i64::from(seconds.is_some()),
            seconds.unwrap_or_default(),
            (&mut kind as *mut i64) as usize as u64,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    quota_model_policy_status(status)?;
    if !(0..=4).contains(&kind) {
        return Err(crate::MojoError::InvalidOutput);
    }
    let written = usize::try_from(written).map_err(|_| crate::MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(crate::MojoError::InvalidOutput);
    }
    String::from_utf8(output[..written].to_vec()).map_err(|_| crate::MojoError::InvalidOutput)
}

pub fn quota_workspace_label(
    workspace_name: Option<&str>,
    workspace_id: Option<&str>,
) -> Result<Option<String>, crate::MojoError> {
    let parts = |value: Option<&str>| -> Result<(u64, i64, i64), crate::MojoError> {
        match value {
            Some(value) => Ok((
                value.as_ptr() as usize as u64,
                i64::try_from(value.len()).map_err(|_| crate::MojoError::InvalidInput)?,
                1,
            )),
            None => Ok((0, 0, 0)),
        }
    };
    let (name_address, name_length, name_present) = parts(workspace_name)?;
    let (id_address, id_length, id_present) = parts(workspace_id)?;
    let capacity = workspace_name
        .map(str::len)
        .unwrap_or_default()
        .max(
            workspace_id
                .map(str::len)
                .unwrap_or_default()
                .saturating_add(3),
        )
        .max(1);
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    let mut present = -1_i64;
    let status = unsafe {
        prodex_quota_workspace_label_v1(
            QUOTA_MODEL_POLICY_ABI_VERSION,
            name_address,
            name_length,
            name_present,
            id_address,
            id_length,
            id_present,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
            (&mut present as *mut i64) as usize as u64,
        )
    };
    quota_model_policy_status(status)?;
    match present {
        0 => Ok(None),
        1 => display::decode_quota_text(&output, written).map(Some),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}
