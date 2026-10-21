use super::MojoError;

pub(super) fn validated_optional_raw_span(
    start: i64,
    end: i64,
    raw: &str,
) -> Result<Option<(usize, usize)>, MojoError> {
    if start == -1 && end == -1 {
        return Ok(None);
    }
    let start = usize::try_from(start).map_err(|_| MojoError::InvalidOutput)?;
    let end = usize::try_from(end).map_err(|_| MojoError::InvalidOutput)?;
    if end <= start || end > raw.len() {
        return Err(MojoError::InvalidOutput);
    }
    let token = raw.get(start..end).ok_or(MojoError::InvalidOutput)?;
    if !token.starts_with('"') || !token.ends_with('"') {
        return Err(MojoError::InvalidOutput);
    }
    Ok(Some((start, end)))
}
