use super::{
    MojoError, RICH_ABI_VERSION, ensure_rich_abi, mojo_mut_pointer_address, mojo_pointer_address,
    status_error, view,
};

/// Which volatile-value policy the normalization kernel should apply.
#[repr(i64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmartContextNormalizationMode {
    CommandOutput = 0,
    StaticContext = 1,
}

/// The already-ordered inputs for memory-capsule admission.
///
/// Rust keeps the existing relevance/id ordering; Mojo owns the bounded
/// token-budget decision over that ordered input.
#[derive(Debug, Clone, Copy)]
pub struct SmartContextCapsuleInput {
    pub token_cost: usize,
    pub required: bool,
}

/// The admission result for an ordered memory-capsule list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SmartContextCapsulePlan {
    pub selected: Vec<bool>,
    pub used_tokens: usize,
}

const SMART_CONTEXT_NORMALIZATION_MAX_BYTES: usize = 4 * 1024 * 1024;
const SMART_CONTEXT_NORMALIZATION_INITIAL_OUTPUT_MULTIPLIER: usize = 2;
const SMART_CONTEXT_NORMALIZATION_MAX_OUTPUT_MULTIPLIER: usize = 5;
const SMART_CONTEXT_NORMALIZATION_CAPACITY_STATUS: i64 = 3;
const SMART_CONTEXT_CAPSULE_MAX_COUNT: usize = 65_536;

unsafe extern "C" {
    fn prodex_mojo_smart_context_scope_preimage_v1(
        abi_version: i64,
        fields: u64,
        field_count: i64,
        output: u64,
        capacity: i64,
        written: u64,
    ) -> i64;
    fn prodex_mojo_smart_context_scope_digest_text_v1(
        abi_version: i64,
        digest: u64,
        digest_len: i64,
        output: u64,
        capacity: i64,
        written: u64,
    ) -> i64;
    fn prodex_mojo_smart_context_fnv_hash_text_v1(
        abi_version: i64,
        mode: i64,
        source_address: u64,
        source_length: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_mojo_smart_context_static_bounded_text_v1(
        abi_version: i64,
        input: u64,
        digest: u64,
        output: u64,
        capacity: i64,
        written: u64,
    ) -> i64;
    fn prodex_mojo_smart_context_static_bounded_id_v1(
        abi_version: i64,
        id: u64,
        hash: u64,
        output: u64,
        output_capacity: i64,
        written: u64,
    ) -> i64;
    fn prodex_mojo_smart_context_static_prompt_payload_v1(
        abi_version: i64,
        ids: u64,
        texts: u64,
        sizes: u64,
        count: i64,
        output: u64,
        output_capacity: i64,
        written: u64,
    ) -> i64;
    fn prodex_mojo_smart_context_static_canonicalize_v1(
        abi_version: i64,
        input: u64,
        output: u64,
        output_capacity: i64,
        written: u64,
    ) -> i64;
    fn prodex_mojo_smart_context_normalization_v1(
        abi_version: i64,
        input: u64,
        operation: i64,
        output: u64,
        output_capacity: i64,
        written: u64,
        decision: u64,
    ) -> i64;
    fn prodex_mojo_smart_context_budget_tier_v1(
        abi_version: i64,
        available_tokens: u64,
        tier: u64,
    ) -> i64;
    fn prodex_mojo_smart_context_memory_capsule_budget_v1(
        abi_version: i64,
        available_context_tokens: u64,
        available_present: i64,
        mode: i64,
        tier: i64,
        max_rehydrate_tokens: u64,
        reason_bits: u64,
        accounting_safe: i64,
        budget: u64,
    ) -> i64;
    fn prodex_mojo_smart_context_capsule_plan_v1(
        abi_version: i64,
        token_costs: u64,
        required: u64,
        selected: u64,
        used_tokens: u64,
        count: i64,
        token_budget: u64,
    ) -> i64;
}

fn checked_output_capacity(input_len: usize, multiplier: usize) -> Result<usize, MojoError> {
    if input_len > SMART_CONTEXT_NORMALIZATION_MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    input_len
        .checked_mul(multiplier)
        .and_then(|value| value.checked_add(16))
        .ok_or(MojoError::InvalidInput)
}

fn normalization_status_error(status: i64) -> MojoError {
    status_error(status, 7, 1, -1, 0)
}

/// Normalize command-output or static-context volatile values in Mojo.
pub fn normalize_smart_context_volatile(
    text: &str,
    mode: SmartContextNormalizationMode,
) -> Result<String, MojoError> {
    ensure_rich_abi()?;
    // `1%` and `1s` expand from two input bytes to ten output bytes.
    let max_capacity = checked_output_capacity(
        text.len(),
        SMART_CONTEXT_NORMALIZATION_MAX_OUTPUT_MULTIPLIER,
    )?;
    let mut capacity = checked_output_capacity(
        text.len(),
        SMART_CONTEXT_NORMALIZATION_INITIAL_OUTPUT_MULTIPLIER,
    )?;
    let input = view(text);
    let mut output = vec![0_u8; capacity.max(1)];
    let mut written = 0_i64;
    let status = loop {
        let status = unsafe {
            prodex_mojo_smart_context_normalization_v1(
                RICH_ABI_VERSION,
                mojo_pointer_address(&input),
                mode as i64,
                mojo_mut_pointer_address(output.as_mut_ptr()),
                i64::try_from(capacity).map_err(|_| MojoError::InvalidInput)?,
                mojo_mut_pointer_address(&mut written),
                0,
            )
        };
        if status != SMART_CONTEXT_NORMALIZATION_CAPACITY_STATUS || capacity == max_capacity {
            break status;
        }
        capacity = max_capacity;
        output.resize(capacity.max(1), 0);
        written = 0;
    };
    if status != 0 {
        return Err(normalization_status_error(status));
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    let output = output.get(..written).ok_or(MojoError::InvalidOutput)?;
    String::from_utf8(output.to_vec()).map_err(|_| MojoError::InvalidOutput)
}

/// Fully canonicalize static context in the compiled Mojo owner. Rust
/// validates UTF-8 and output capacity but never re-normalizes lines.
pub fn canonicalize_smart_context_static_text(text: &str) -> Result<String, MojoError> {
    ensure_rich_abi()?;
    let maximum = checked_output_capacity(
        text.len(),
        SMART_CONTEXT_NORMALIZATION_MAX_OUTPUT_MULTIPLIER,
    )?;
    let mut capacity = checked_output_capacity(
        text.len(),
        SMART_CONTEXT_NORMALIZATION_INITIAL_OUTPUT_MULTIPLIER,
    )?;
    let input = view(text);
    let mut output = vec![0_u8; capacity.max(1)];
    let mut written = 0_i64;
    let status = loop {
        let status = unsafe {
            prodex_mojo_smart_context_static_canonicalize_v1(
                RICH_ABI_VERSION,
                mojo_pointer_address(&input),
                mojo_mut_pointer_address(output.as_mut_ptr()),
                i64::try_from(capacity).map_err(|_| MojoError::InvalidInput)?,
                mojo_mut_pointer_address(&mut written),
            )
        };
        if status != SMART_CONTEXT_NORMALIZATION_CAPACITY_STATUS || capacity == maximum {
            break status;
        }
        capacity = maximum;
        output.resize(capacity.max(1), 0);
        written = 0;
    };
    if status != 0 {
        return Err(normalization_status_error(status));
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    let bytes = output.get(..written).ok_or(MojoError::InvalidOutput)?;
    String::from_utf8(bytes.to_vec()).map_err(|_| MojoError::InvalidOutput)
}

/// Borrowed static payload pieces. The caller owns normalization, IDs and hashes;
/// Mojo exclusively owns the serialized prompt-cache byte format.
#[derive(Debug, Clone, Copy)]
pub struct SmartContextStaticPayloadItem<'a> {
    pub id: &'a str,
    pub canonical_text: &'a str,
    pub byte_len: usize,
}

pub fn smart_context_static_prompt_payload(
    items: &[SmartContextStaticPayloadItem<'_>],
) -> Result<String, MojoError> {
    ensure_rich_abi()?;
    if items.len() > 128 {
        return Err(MojoError::Capacity);
    }
    let capacity = items
        .iter()
        .try_fold(48usize, |size, item| {
            size.checked_add(item.id.len())?
                .checked_add(item.canonical_text.len())?
                .checked_add(64)
        })
        .ok_or(MojoError::InvalidInput)?;
    if capacity > 64 * 1024 * 1024 {
        return Err(MojoError::Capacity);
    }
    let ids = items.iter().map(|item| view(item.id)).collect::<Vec<_>>();
    let texts = items
        .iter()
        .map(|item| view(item.canonical_text))
        .collect::<Vec<_>>();
    let sizes = items
        .iter()
        .map(|item| u64::try_from(item.byte_len).map_err(|_| MojoError::InvalidInput))
        .collect::<Result<Vec<_>, _>>()?;
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_mojo_smart_context_static_prompt_payload_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(ids.as_ptr()),
            mojo_pointer_address(texts.as_ptr()),
            mojo_pointer_address(sizes.as_ptr()),
            i64::try_from(items.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(output.as_mut_ptr()),
            i64::try_from(capacity).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut written),
        )
    };
    if status != 0 {
        return Err(normalization_status_error(status));
    }
    let length = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    let bytes = output.get(..length).ok_or(MojoError::InvalidOutput)?;
    String::from_utf8(bytes.to_vec()).map_err(|_| MojoError::InvalidOutput)
}

/// Mojo alone chooses trimmed ID bytes, path separators and UTF-8 cutoff.
/// The host computes SHA-256 only if the compiled kernel requests it.
pub fn smart_context_static_bounded_id(
    id: &str,
    compute_hash: impl FnOnce() -> String,
) -> Result<String, MojoError> {
    ensure_rich_abi()?;
    let input = view(id);
    let mut output = vec![0_u8; 512];
    let mut written = -1_i64;
    let mut hash = None;
    let mut compute_hash = Some(compute_hash);
    loop {
        let hash_view = hash.as_deref().map(view);
        let status = unsafe {
            prodex_mojo_smart_context_static_bounded_id_v1(
                RICH_ABI_VERSION,
                mojo_pointer_address(&input),
                hash_view
                    .as_ref()
                    .map_or(0, |value| mojo_pointer_address(value)),
                mojo_mut_pointer_address(output.as_mut_ptr()),
                i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
                mojo_mut_pointer_address(&mut written),
            )
        };
        if status == 5 && hash.is_none() {
            hash = Some(compute_hash.take().ok_or(MojoError::InvalidOutput)?());
            continue;
        }
        if status != 0 {
            return Err(normalization_status_error(status));
        }
        let length = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
        if length > output.len() {
            return Err(MojoError::InvalidOutput);
        }
        output.truncate(length);
        return String::from_utf8(output).map_err(|_| MojoError::InvalidOutput);
    }
}

/// Apply the complete Mojo-owned static text size bound, Unicode cutoff,
/// volatile normalization and long-content marker. Rust supplies SHA-256
/// only when the compiled policy explicitly requests the digest.
pub fn smart_context_static_bounded_text(
    text: &str,
    compute_hash: impl FnOnce() -> String,
) -> Result<String, MojoError> {
    ensure_rich_abi()?;
    let maximum = checked_output_capacity(
        text.len().min(256 * 1024),
        SMART_CONTEXT_NORMALIZATION_MAX_OUTPUT_MULTIPLIER,
    )?
    .checked_add(256)
    .ok_or(MojoError::InvalidInput)?;
    let input = view(text);
    let mut output = vec![0_u8; maximum];
    let mut written = -1_i64;
    let mut hash = None;
    let mut compute_hash = Some(compute_hash);
    loop {
        let hash_view = hash.as_deref().map(view);
        let status = unsafe {
            prodex_mojo_smart_context_static_bounded_text_v1(
                RICH_ABI_VERSION,
                mojo_pointer_address(&input),
                hash_view
                    .as_ref()
                    .map_or(0, |value| mojo_pointer_address(value)),
                mojo_mut_pointer_address(output.as_mut_ptr()),
                i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
                mojo_mut_pointer_address(&mut written),
            )
        };
        if status == 5 && hash.is_none() {
            hash = Some(compute_hash.take().ok_or(MojoError::InvalidOutput)?());
            continue;
        }
        if status != 0 {
            return Err(normalization_status_error(status));
        }
        let length = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
        if length > output.len() {
            return Err(MojoError::InvalidOutput);
        }
        output.truncate(length);
        return String::from_utf8(output).map_err(|_| MojoError::InvalidOutput);
    }
}

/// Legacy FNV-1a fingerprint serialization is wholly Mojo-owned.
/// Mode 0 returns 16 lowercase hex digits; mode 1 adds the scv: prefix.
pub fn smart_context_legacy_fnv_text(
    input: &[u8],
    with_volatile_prefix: bool,
) -> Result<String, MojoError> {
    ensure_rich_abi()?;
    let mut buffer = [0u8; 20];
    let mut written = -1i64;
    let status = unsafe {
        prodex_mojo_smart_context_fnv_hash_text_v1(
            RICH_ABI_VERSION,
            i64::from(with_volatile_prefix),
            mojo_pointer_address(input.as_ptr()),
            i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(buffer.as_mut_ptr()),
            buffer.len() as i64,
            mojo_mut_pointer_address(&mut written),
        )
    };
    if status != 0 {
        return Err(normalization_status_error(status));
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written != if with_volatile_prefix { 20 } else { 16 } {
        return Err(MojoError::InvalidOutput);
    }
    String::from_utf8(buffer[..written].to_vec()).map_err(|_| MojoError::InvalidOutput)
}

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

/// Classify one static-context line as volatile metadata.
pub fn smart_context_static_context_noise_line(line: &str) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let input = view(line);
    let mut decision = 0_i64;
    let status = unsafe {
        prodex_mojo_smart_context_normalization_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(&input),
            2,
            0,
            0,
            0,
            mojo_mut_pointer_address(&mut decision),
        )
    };
    if status != 0 {
        return Err(normalization_status_error(status));
    }
    match decision {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Classify the available-context budget using the shared Mojo thresholds.
pub fn smart_context_budget_tier(available_tokens: u64) -> Result<i64, MojoError> {
    ensure_rich_abi()?;
    let mut tier = u64::MAX;
    let status = unsafe {
        prodex_mojo_smart_context_budget_tier_v1(
            RICH_ABI_VERSION,
            available_tokens,
            mojo_mut_pointer_address(&mut tier),
        )
    };
    if status != 0 {
        return Err(normalization_status_error(status));
    }
    if tier <= 3 {
        Ok(tier as i64)
    } else {
        Err(MojoError::InvalidOutput)
    }
}

/// Plan the memory-capsule token budget from the already-decoded policy.
pub fn smart_context_memory_capsule_token_budget(
    available_context_tokens: Option<u64>,
    mode: i64,
    tier: i64,
    max_rehydrate_tokens: u64,
    reason_bits: u64,
    accounting_safe: bool,
) -> Result<u64, MojoError> {
    ensure_rich_abi()?;
    let mut budget = 0_u64;
    let status = unsafe {
        prodex_mojo_smart_context_memory_capsule_budget_v1(
            RICH_ABI_VERSION,
            available_context_tokens.unwrap_or_default(),
            i64::from(available_context_tokens.is_some()),
            mode,
            tier,
            max_rehydrate_tokens,
            reason_bits,
            i64::from(accounting_safe),
            mojo_mut_pointer_address(&mut budget),
        )
    };
    if status != 0 {
        return Err(normalization_status_error(status));
    }
    Ok(budget)
}

/// Admit an ordered capsule list without moving host-owned strings across the ABI.
pub fn plan_smart_context_capsules(
    inputs: &[SmartContextCapsuleInput],
    token_budget: usize,
) -> Result<SmartContextCapsulePlan, MojoError> {
    ensure_rich_abi()?;
    if inputs.len() > SMART_CONTEXT_CAPSULE_MAX_COUNT {
        return Err(MojoError::InvalidInput);
    }
    let token_costs = inputs
        .iter()
        .map(|input| u64::try_from(input.token_cost).map_err(|_| MojoError::InvalidInput))
        .collect::<Result<Vec<_>, _>>()?;
    let required = inputs
        .iter()
        .map(|input| i64::from(input.required))
        .collect::<Vec<_>>();
    let mut selected = vec![0_i64; inputs.len().max(1)];
    let mut used_tokens = 0_u64;
    let status = unsafe {
        prodex_mojo_smart_context_capsule_plan_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(token_costs.as_ptr()),
            mojo_pointer_address(required.as_ptr()),
            mojo_mut_pointer_address(selected.as_mut_ptr()),
            mojo_mut_pointer_address(&mut used_tokens),
            i64::try_from(inputs.len()).map_err(|_| MojoError::InvalidInput)?,
            u64::try_from(token_budget).map_err(|_| MojoError::InvalidInput)?,
        )
    };
    if status != 0 {
        return Err(normalization_status_error(status));
    }
    if used_tokens > u64::try_from(token_budget).map_err(|_| MojoError::InvalidInput)? {
        return Err(MojoError::InvalidOutput);
    }
    let selected = selected
        .get(..inputs.len())
        .ok_or(MojoError::InvalidOutput)?
        .iter()
        .map(|value| match value {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(MojoError::InvalidOutput),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SmartContextCapsulePlan {
        selected,
        used_tokens: usize::try_from(used_tokens).map_err(|_| MojoError::InvalidOutput)?,
    })
}

#[cfg(test)]
mod static_canonicalization_tests {
    use super::*;

    #[test]
    fn mojo_scope_preimage_preserves_each_field_boundary_and_utf8_bytes() {
        let fields = ["tenant", "東京🧭", "openai", "/work/repo", ""];
        let output = smart_context_scope_preimage(fields).unwrap();
        let mut expected = b"prodex-smart-context-scope-v1\0".to_vec();
        for field in fields {
            expected.extend_from_slice(&(field.len() as u64).to_be_bytes());
            expected.extend_from_slice(field.as_bytes());
        }
        assert_eq!(output, expected);
        assert_ne!(
            smart_context_scope_preimage(["a", "bc", "", "", ""]).unwrap(),
            smart_context_scope_preimage(["ab", "c", "", "", ""]).unwrap(),
        );
    }

    #[test]
    fn mojo_scope_digest_emits_exact_prefixed_lowercase_sha256_hex() {
        let mut digest = [0u8; 32];
        digest[0] = 0xab;
        digest[31] = 0x0f;
        assert_eq!(
            smart_context_scope_digest_text(&digest).unwrap(),
            format!("scscope1:ab{}0f", "00".repeat(30)),
        );
        assert_eq!(
            smart_context_scope_digest_text(&[0u8; 32]).unwrap(),
            format!("scscope1:{}", "00".repeat(32)),
        );
    }

    #[test]
    fn mojo_scope_abi_rejects_version_and_short_capacity() {
        let mut output = [77u8; 73];
        let mut written = 123i64;
        let digest = [0u8; 32];
        let views = ["t", "p", "r", "w", "s"].map(view);
        unsafe {
            assert_eq!(
                prodex_mojo_smart_context_scope_preimage_v1(
                    RICH_ABI_VERSION + 1,
                    mojo_pointer_address(views.as_ptr()),
                    5,
                    mojo_mut_pointer_address(output.as_mut_ptr()),
                    output.len() as i64,
                    mojo_mut_pointer_address(&mut written),
                ),
                4,
            );
            assert_eq!(
                prodex_mojo_smart_context_scope_preimage_v1(
                    RICH_ABI_VERSION,
                    mojo_pointer_address(views.as_ptr()),
                    4,
                    mojo_mut_pointer_address(output.as_mut_ptr()),
                    output.len() as i64,
                    mojo_mut_pointer_address(&mut written),
                ),
                1,
            );
            assert_eq!(
                prodex_mojo_smart_context_scope_digest_text_v1(
                    RICH_ABI_VERSION,
                    mojo_pointer_address(digest.as_ptr()),
                    31,
                    mojo_mut_pointer_address(output.as_mut_ptr()),
                    output.len() as i64,
                    mojo_mut_pointer_address(&mut written),
                ),
                1,
            );
            assert_eq!(
                prodex_mojo_smart_context_scope_digest_text_v1(
                    RICH_ABI_VERSION,
                    mojo_pointer_address(digest.as_ptr()),
                    32,
                    mojo_mut_pointer_address(output.as_mut_ptr()),
                    72,
                    mojo_mut_pointer_address(&mut written),
                ),
                3,
            );
        }
        assert_eq!(output, [77; 73]);
        assert_eq!(written, 123);
    }

    #[test]
    fn mojo_legacy_fnv_preserves_64_bit_hash_and_text_formats() {
        for (input, expected) in [
            (&b""[..], "cbf29ce484222325"),
            (&b"a"[..], "af63dc4c8601ec8c"),
            (&b"foobar"[..], "85944171f73967e8"),
            (&b"\0"[..], "af63bd4c8601b7df"),
        ] {
            assert_eq!(
                smart_context_legacy_fnv_text(input, false).unwrap(),
                expected
            );
            assert_eq!(
                smart_context_legacy_fnv_text(input, true).unwrap(),
                format!("scv:{expected}"),
            );
        }
        let input = "東京🧭".as_bytes();
        let fingerprint = smart_context_legacy_fnv_text(input, false).unwrap();
        assert_eq!(fingerprint.len(), 16);
        assert_eq!(
            smart_context_legacy_fnv_text(input, true).unwrap(),
            format!("scv:{fingerprint}"),
        );
    }

    #[test]
    fn mojo_legacy_fnv_abi_fails_before_writing_for_invalid_input() {
        let mut output = [73u8; 20];
        let mut written = 99i64;
        unsafe {
            assert_eq!(
                prodex_mojo_smart_context_fnv_hash_text_v1(
                    RICH_ABI_VERSION + 1,
                    0,
                    0,
                    0,
                    mojo_mut_pointer_address(output.as_mut_ptr()),
                    20,
                    mojo_mut_pointer_address(&mut written),
                ),
                4,
            );
            assert_eq!(
                prodex_mojo_smart_context_fnv_hash_text_v1(
                    RICH_ABI_VERSION,
                    2,
                    0,
                    0,
                    mojo_mut_pointer_address(output.as_mut_ptr()),
                    20,
                    mojo_mut_pointer_address(&mut written),
                ),
                1,
            );
            assert_eq!(
                prodex_mojo_smart_context_fnv_hash_text_v1(
                    RICH_ABI_VERSION,
                    0,
                    0,
                    3,
                    mojo_mut_pointer_address(output.as_mut_ptr()),
                    20,
                    mojo_mut_pointer_address(&mut written),
                ),
                1,
            );
            assert_eq!(
                prodex_mojo_smart_context_fnv_hash_text_v1(
                    RICH_ABI_VERSION,
                    0,
                    0,
                    0,
                    mojo_mut_pointer_address(output.as_mut_ptr()),
                    15,
                    mojo_mut_pointer_address(&mut written),
                ),
                3,
            );
        }
        assert_eq!(output, [73; 20]);
        assert_eq!(written, 99);
    }

    #[test]
    fn bounded_static_text_uses_mojo_prefix_normalization_and_lazy_digest() {
        use std::cell::Cell;
        assert_eq!(
            smart_context_static_bounded_text(
                "Generated at: 2026-10-15\nRule: Keep affinity\n",
                || {
                    panic!("short text must not request a digest");
                },
            )
            .unwrap(),
            "Rule: Keep affinity",
        );
        let text = "🧭".repeat(65_537);
        let count = Cell::new(0);
        let result = smart_context_static_bounded_text(&text, || {
            count.set(count.get() + 1);
            "sc2:verified".to_string()
        })
        .unwrap();
        assert_eq!(count.get(), 1);
        assert_eq!(
            result,
            format!(
                "{}\npsc static fingerprint item truncated bytes={} hash=sc2:verified",
                "🧭".repeat(65_536),
                text.len()
            )
        );
        let exact = "é".repeat(131_072);
        assert_eq!(
            smart_context_static_bounded_text(&exact, || {
                panic!("exact 256 KiB source must not be truncated");
            })
            .unwrap(),
            exact
        );
    }

    #[test]
    fn bounded_static_text_abi_requires_digest_without_changing_output() {
        let text = "🧭".repeat(65_537);
        let input = view(&text);
        let mut output = [77_u8; 8];
        let mut written = 55_i64;
        unsafe {
            assert_eq!(
                prodex_mojo_smart_context_static_bounded_text_v1(
                    RICH_ABI_VERSION + 1,
                    mojo_pointer_address(&input),
                    0,
                    mojo_mut_pointer_address(output.as_mut_ptr()),
                    8,
                    mojo_mut_pointer_address(&mut written),
                ),
                4,
            );
            assert_eq!(
                prodex_mojo_smart_context_static_bounded_text_v1(
                    RICH_ABI_VERSION,
                    mojo_pointer_address(&input),
                    0,
                    mojo_mut_pointer_address(output.as_mut_ptr()),
                    8,
                    mojo_mut_pointer_address(&mut written),
                ),
                5,
            );
        }
        assert_eq!(output, [77; 8]);
        assert_eq!(written, 55);
    }

    #[test]
    fn bounded_static_id_uses_mojo_utf8_cutoff_and_lazy_host_hash() {
        use std::cell::Cell;
        assert_eq!(
            smart_context_static_bounded_id("  \\folder\\🧭  ", || {
                panic!("a short ID must not require SHA-256")
            })
            .unwrap(),
            "/folder/🧭"
        );
        assert_eq!(
            smart_context_static_bounded_id(&"é".repeat(128), || {
                panic!("an exactly 256-byte ID is not truncated")
            })
            .unwrap(),
            "é".repeat(128),
        );
        let input = "🧭".repeat(65);
        let hashes = Cell::new(0);
        let output = smart_context_static_bounded_id(&input, || {
            hashes.set(hashes.get() + 1);
            "sc2:verified".to_owned()
        })
        .unwrap();
        assert_eq!(hashes.get(), 1);
        assert_eq!(output, format!("{}<id-hash=sc2:verified>", "🧭".repeat(64)));
        assert_eq!(
            smart_context_static_bounded_id("  \u{2003}\t  ", || {
                panic!("whitespace-only input must not be hashed")
            })
            .unwrap(),
            "",
        );
    }

    #[test]
    fn bounded_static_id_rejects_invalid_abi_and_short_capacity() {
        let id = view("hello");
        let mut output = [21_u8; 2];
        let mut written = 77_i64;
        unsafe {
            assert_eq!(
                prodex_mojo_smart_context_static_bounded_id_v1(
                    RICH_ABI_VERSION + 1,
                    mojo_pointer_address(&id),
                    0,
                    mojo_mut_pointer_address(output.as_mut_ptr()),
                    2,
                    mojo_mut_pointer_address(&mut written),
                ),
                4,
            );
            assert_eq!(
                prodex_mojo_smart_context_static_bounded_id_v1(
                    RICH_ABI_VERSION,
                    mojo_pointer_address(&id),
                    0,
                    mojo_mut_pointer_address(output.as_mut_ptr()),
                    2,
                    mojo_mut_pointer_address(&mut written),
                ),
                3,
            );
        }
    }

    #[test]
    fn static_payload_serialization_is_mojo_owned_and_byte_exact() {
        assert_eq!(
            smart_context_static_prompt_payload(&[]).unwrap(),
            "prodex-smart-context-static-prompt-cache-v1\n",
        );
        let entries = [
            SmartContextStaticPayloadItem {
                id: "AGENTS.md",
                canonical_text: "rules\n",
                byte_len: 6,
            },
            SmartContextStaticPayloadItem {
                id: "🧭",
                canonical_text: "東京",
                byte_len: 6,
            },
        ];
        assert_eq!(
            smart_context_static_prompt_payload(&entries).unwrap(),
            "prodex-smart-context-static-prompt-cache-v1\n\
             id-bytes:9\nAGENTS.md\ntext-bytes:6\nrules\n\n\
             id-bytes:4\n🧭\ntext-bytes:6\n東京\n",
        );
    }

    #[test]
    fn static_payload_rejects_abi_and_count_before_writing_output() {
        let entry = view("hello");
        let mut output = [71_u8; 64];
        let mut written = 88_i64;
        unsafe {
            assert_eq!(
                prodex_mojo_smart_context_static_prompt_payload_v1(
                    RICH_ABI_VERSION + 1,
                    mojo_pointer_address(&entry),
                    mojo_pointer_address(&entry),
                    0,
                    1,
                    mojo_mut_pointer_address(output.as_mut_ptr()),
                    64,
                    mojo_mut_pointer_address(&mut written),
                ),
                4,
            );
            assert_eq!(
                prodex_mojo_smart_context_static_prompt_payload_v1(
                    RICH_ABI_VERSION,
                    0,
                    0,
                    0,
                    1,
                    mojo_mut_pointer_address(output.as_mut_ptr()),
                    64,
                    mojo_mut_pointer_address(&mut written),
                ),
                1,
            );
        }
        assert_eq!(output, [71; 64]);
        assert_eq!(written, 88);
    }

    #[test]
    fn static_pipeline_canonicalizes_newlines_noise_and_unicode_without_rust_fallback() {
        let input = "\r\n \t\r\nGenerated at: 2026-10-15T11:00:00Z\r\n \
                     Rule: Keep profile affinity  \u{2003}\r\n\r\n\
                     request_id=123e4567-e89b-12d3-a456-426614174000\r\n\
                     Rule: Keep route identity\n  \n";
        assert_eq!(
            canonicalize_smart_context_static_text(input).unwrap(),
            " Rule: Keep profile affinity\n\nRule: Keep route identity",
        );
        for (source, expected) in [
            ("", ""),
            ("\n\r\n\r", ""),
            ("  \u{2003}hello  \u{00a0}\r\n", "  \u{2003}hello"),
            ("alpha\n\n\n", "alpha"),
            ("\n\nalpha\n\nbeta\n\n", "alpha\n\nbeta"),
            ("first\rsecond\r\nthird\n", "first\nsecond\nthird"),
            ("\u{1f9ed} a\u{2028}b  ", "\u{1f9ed} a\u{2028}b"),
        ] {
            assert_eq!(
                canonicalize_smart_context_static_text(source).unwrap(),
                expected,
                "{source:?}",
            );
        }
    }

    #[test]
    fn static_pipeline_keeps_semantic_text_and_redacts_inline_volatile_ids() {
        let before = "Trace request_id=123e4567-e89b-12d3-a456-426614174000 \
                      at 2026-05-04T01:02:03Z\nRule: Keep affinity";
        let after = "Trace request_id=123e4567-e89b-12d3-a456-426614174999 \
                     at 2026-05-05T09:08:07Z\nRule: Keep affinity";
        let expected = "Trace request_id=<id> at <timestamp>\nRule: Keep affinity";
        assert_eq!(
            canonicalize_smart_context_static_text(before).unwrap(),
            expected
        );
        assert_eq!(
            canonicalize_smart_context_static_text(after).unwrap(),
            expected
        );
    }

    #[test]
    fn static_pipeline_rejects_invalid_abi_before_mutating_output() {
        let input = view("hi");
        let mut output = [93_u8; 8];
        let mut written = 77_i64;
        unsafe {
            assert_eq!(
                prodex_mojo_smart_context_static_canonicalize_v1(
                    RICH_ABI_VERSION + 1,
                    mojo_pointer_address(&input),
                    mojo_mut_pointer_address(output.as_mut_ptr()),
                    8,
                    mojo_mut_pointer_address(&mut written),
                ),
                4,
            );
            assert_eq!(
                prodex_mojo_smart_context_static_canonicalize_v1(
                    RICH_ABI_VERSION,
                    mojo_pointer_address(&input),
                    mojo_mut_pointer_address(output.as_mut_ptr()),
                    0,
                    mojo_mut_pointer_address(&mut written),
                ),
                1,
            );
        }
        assert_eq!(output, [93; 8]);
        assert_eq!(written, 77);
    }
}
