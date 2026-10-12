use crate::MojoError;

const ABI_VERSION: i64 = 1;

unsafe extern "C" {
    fn prodex_provider_error_native_event_classify_v1(
        abi: i64,
        event_type_address: u64,
        event_type_length: i64,
        error_present: i64,
        error_type_address: u64,
        error_type_length: i64,
        error_type_present: i64,
        error_code_address: u64,
        error_code_length: i64,
        error_code_present: i64,
        top_code_address: u64,
        top_code_length: i64,
        top_code_present: i64,
        output_address: u64,
    ) -> i64;
}

fn parts(value: Option<&str>) -> Result<(u64, i64, i64), MojoError> {
    let Some(value) = value else {
        return Ok((0, 0, 0));
    };
    Ok((
        value.as_ptr() as usize as u64,
        i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
        1,
    ))
}

/// Classifies a bounded native SSE error event after Rust acquires its JSON fields.
///
/// Mojo owns the event gate, `/error/type` → `/error/code` → `code` precedence,
/// and provider classification. Rust retains SSE framing and JSON ownership.
pub fn classify(
    event_type: Option<&str>,
    error_present: bool,
    error_type: Option<&str>,
    error_code: Option<&str>,
    top_code: Option<&str>,
) -> Result<Option<i64>, MojoError> {
    let (event_type_address, event_type_length, _) = parts(event_type)?;
    let (error_type_address, error_type_length, error_type_present) = parts(error_type)?;
    let (error_code_address, error_code_length, error_code_present) = parts(error_code)?;
    let (top_code_address, top_code_length, top_code_present) = parts(top_code)?;
    let mut output = [-2_i64, 0];
    let status = unsafe {
        prodex_provider_error_native_event_classify_v1(
            ABI_VERSION,
            event_type_address,
            event_type_length,
            i64::from(error_present),
            error_type_address,
            error_type_length,
            error_type_present,
            error_code_address,
            error_code_length,
            error_code_present,
            top_code_address,
            top_code_length,
            top_code_present,
            output.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => match output[0] {
            -1 => Ok(None),
            0..=4 => Ok(Some(output[0])),
            5 => Ok(None),
            _ => Err(MojoError::InvalidOutput),
        },
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::classify;

    #[test]
    fn native_event_gate_and_code_precedence_are_mojo_owned() {
        assert_eq!(
            classify(None, false, Some("rate_limit_exceeded"), None, None).unwrap(),
            None
        );
        assert_eq!(
            classify(
                Some("error"),
                true,
                Some("rate_limit_exceeded"),
                Some("insufficient_quota"),
                Some("invalid_api_key")
            )
            .unwrap(),
            Some(2)
        );
        assert_eq!(
            classify(None, true, None, Some("insufficient_quota"), None).unwrap(),
            Some(1)
        );
        assert_eq!(classify(None, true, None, None, None).unwrap(), None);
    }
}
