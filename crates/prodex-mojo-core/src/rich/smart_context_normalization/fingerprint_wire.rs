use super::{
    MojoError, RICH_ABI_VERSION, ensure_rich_abi, mojo_mut_pointer_address, mojo_pointer_address,
    normalization_status_error, prodex_mojo_smart_context_scope_digest_text_v1,
    prodex_mojo_smart_context_scope_parse_v1, prodex_mojo_smart_context_scope_preimage_v1,
    prodex_mojo_smart_context_static_digest_text_v1,
    prodex_mojo_smart_context_static_overflow_accumulate_v1,
    prodex_mojo_smart_context_static_overflow_preimage_v1,
    prodex_mojo_smart_context_static_truncation_line_v1, view,
};

/// Produce an exact, versioned preimage for the host SHA-256 primitive.
/// Mojo is the only owner of the field lengths, order and scope separator.
pub fn smart_context_scope_preimage(fields: [&str; 5]) -> Result<Vec<u8>, MojoError> {
    ensure_rich_abi()?;
    let capacity = fields
        .iter()
        .try_fold(
            b"prodex-smart-context-scope-v1\0".len() + 5 * 8,
            |size, field| size.checked_add(field.len()),
        )
        .ok_or(MojoError::InvalidInput)?;
    let views = fields.map(view);
    let mut output = vec![0u8; capacity];
    let mut written = -1i64;
    let status = unsafe {
        prodex_mojo_smart_context_scope_preimage_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(views.as_ptr()),
            views.len() as i64,
            mojo_mut_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut written),
        )
    };
    if status != 0 {
        return Err(normalization_status_error(status));
    }
    let length = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if length != capacity {
        return Err(MojoError::InvalidOutput);
    }
    Ok(output)
}

/// Encode a cryptographically hashed scope identifier through the Mojo wire
/// policy. Rust remains responsible only for the 32-byte SHA-256 primitive.
pub fn smart_context_scope_digest_text(digest: &[u8; 32]) -> Result<String, MojoError> {
    ensure_rich_abi()?;
    let mut output = [0u8; 73];
    let mut written = -1i64;
    let status = unsafe {
        prodex_mojo_smart_context_scope_digest_text_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(digest.as_ptr()),
            digest.len() as i64,
            mojo_mut_pointer_address(output.as_mut_ptr()),
            output.len() as i64,
            mojo_mut_pointer_address(&mut written),
        )
    };
    if status != 0 {
        return Err(normalization_status_error(status));
    }
    if written != output.len() as i64 {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output.to_vec()).map_err(|_| MojoError::InvalidOutput)
}

/// Parse the historical scscope1: identifier strictly through compiled Mojo.
pub fn smart_context_scope_parse(value: &str) -> Result<Option<String>, MojoError> {
    ensure_rich_abi()?;
    let input = view(value);
    let mut output = [0u8; 73];
    let mut written = -1i64;
    let mut valid = -1i64;
    let status = unsafe {
        prodex_mojo_smart_context_scope_parse_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(&input),
            mojo_mut_pointer_address(output.as_mut_ptr()),
            output.len() as i64,
            mojo_mut_pointer_address(&mut written),
            mojo_mut_pointer_address(&mut valid),
        )
    };
    if status != 0 {
        return Err(normalization_status_error(status));
    }
    match (valid, written) {
        (0, 0) => Ok(None),
        (1, 73) => String::from_utf8(output.to_vec())
            .map(Some)
            .map_err(|_| MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Produce the exact platform-width overflow record preimage through Mojo.
/// Host SHA-256 remains separate and is fed only the Mojo-authored bytes.
pub fn smart_context_static_overflow_preimage(
    id: &str,
    content_hash: &str,
    byte_len: usize,
) -> Result<Vec<u8>, MojoError> {
    ensure_rich_abi()?;
    let capacity = id
        .len()
        .checked_add(content_hash.len())
        .and_then(|value| value.checked_add(2 + size_of::<usize>()))
        .ok_or(MojoError::InvalidInput)?;
    let id_view = view(id);
    let hash_view = view(content_hash);
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_mojo_smart_context_static_overflow_preimage_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(&id_view),
            mojo_pointer_address(&hash_view),
            u64::try_from(byte_len).map_err(|_| MojoError::InvalidInput)?,
            size_of::<usize>() as i64,
            mojo_mut_pointer_address(output.as_mut_ptr()),
            i64::try_from(capacity).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut written),
        )
    };
    if status != 0 {
        return Err(normalization_status_error(status));
    }
    if usize::try_from(written).map_err(|_| MojoError::InvalidOutput)? != capacity {
        return Err(MojoError::InvalidOutput);
    }
    Ok(output)
}

/// Combine the SHA-256 result into the rolling 32-byte overflow checksum
/// with the exact Mojo-owned wrapping addition semantics.
pub fn smart_context_static_overflow_accumulate(
    aggregate: &[u8; 32],
    sha256_digest: &[u8; 32],
) -> Result<[u8; 32], MojoError> {
    ensure_rich_abi()?;
    let mut output = [0_u8; 32];
    let status = unsafe {
        prodex_mojo_smart_context_static_overflow_accumulate_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(aggregate.as_ptr()),
            mojo_pointer_address(sha256_digest.as_ptr()),
            mojo_mut_pointer_address(output.as_mut_ptr()),
            output.len() as i64,
        )
    };
    if status != 0 {
        return Err(normalization_status_error(status));
    }
    Ok(output)
}

/// Render a host SHA-256 digest for static-context fingerprinting in
/// compiled Mojo. Unprefixed output is the rolling overflow digest,
/// prefixed output is the final stable scpc2: fingerprint.
pub fn smart_context_static_digest_text(
    digest: &[u8; 32],
    prompt_cache_prefix: bool,
) -> Result<String, MojoError> {
    ensure_rich_abi()?;
    let mut output = [0_u8; 70];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_mojo_smart_context_static_digest_text_v1(
            RICH_ABI_VERSION,
            i64::from(prompt_cache_prefix),
            mojo_pointer_address(digest.as_ptr()),
            32,
            mojo_mut_pointer_address(output.as_mut_ptr()),
            output.len() as i64,
            mojo_mut_pointer_address(&mut written),
        )
    };
    if status != 0 {
        return Err(normalization_status_error(status));
    }
    let expected = if prompt_cache_prefix { 70 } else { 64 };
    if written != expected {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(output[..expected as usize].to_vec()).map_err(|_| MojoError::InvalidOutput)
}

/// Render one versioned overflow/truncation metadata line only in Mojo.
/// Rust passes borrowed already-validated count and hash facts.
pub fn smart_context_static_truncation_line(
    item_count: usize,
    total_bytes: usize,
    overflow_hash: &str,
) -> Result<String, MojoError> {
    ensure_rich_abi()?;
    let digest = view(overflow_hash);
    let mut output = [0_u8; 160];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_mojo_smart_context_static_truncation_line_v1(
            RICH_ABI_VERSION,
            u64::try_from(item_count).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(total_bytes).map_err(|_| MojoError::InvalidInput)?,
            mojo_pointer_address(&digest),
            mojo_mut_pointer_address(output.as_mut_ptr()),
            output.len() as i64,
            mojo_mut_pointer_address(&mut written),
        )
    };
    if status != 0 {
        return Err(normalization_status_error(status));
    }
    let count = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    let bytes = output.get(..count).ok_or(MojoError::InvalidOutput)?;
    String::from_utf8(bytes.to_vec()).map_err(|_| MojoError::InvalidOutput)
}
