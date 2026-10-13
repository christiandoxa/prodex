use super::{
    QUOTA_DISPLAY_LABEL_ERROR_SUMMARY, QUOTA_ERROR_KIND_OTHER, QUOTA_ERROR_KIND_UNKNOWN,
    load_quota_display_label, quota_text_address,
};

unsafe extern "C" {
    fn prodex_quota_error_summary_kind(address: u64, length: i64) -> i64;
}

pub fn quota_error_summary_kind(value: &str) -> Result<i64, crate::MojoError> {
    let (address, length) = quota_text_address(Some(value));
    if length == i64::MAX {
        return Err(crate::MojoError::InvalidInput);
    }
    let kind = unsafe { prodex_quota_error_summary_kind(address, length) };
    if (QUOTA_ERROR_KIND_UNKNOWN..=QUOTA_ERROR_KIND_OTHER).contains(&kind) {
        Ok(kind)
    } else {
        Err(crate::MojoError::InvalidOutput)
    }
}

/// Render the canonical quota error label through the compiled Mojo owner.
///
/// `Ok(None)` means an unclassified upstream message must be preserved verbatim,
/// rather than silently replacing it with a guessed error category.
pub fn quota_error_summary_label(kind: i64) -> Result<Option<String>, crate::MojoError> {
    if !(QUOTA_ERROR_KIND_UNKNOWN..=QUOTA_ERROR_KIND_OTHER).contains(&kind) {
        return Err(crate::MojoError::InvalidInput);
    }
    if kind == QUOTA_ERROR_KIND_OTHER {
        return Ok(None);
    }
    load_quota_display_label(QUOTA_DISPLAY_LABEL_ERROR_SUMMARY, kind).map(Some)
}
