#[path = "smart_context_normalization/fingerprint_wire.rs"]
mod fingerprint_wire;
pub use fingerprint_wire::{
    smart_context_scope_digest_text, smart_context_scope_parse, smart_context_scope_preimage,
    smart_context_static_digest_text, smart_context_static_overflow_accumulate,
    smart_context_static_overflow_preimage, smart_context_static_truncation_line,
};

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
    fn prodex_mojo_smart_context_static_digest_text_v1(
        abi_version: i64,
        mode: i64,
        digest: u64,
        digest_len: i64,
        output: u64,
        capacity: i64,
        written: u64,
    ) -> i64;
    fn prodex_mojo_smart_context_static_truncation_line_v1(
        abi_version: i64,
        item_count: u64,
        total_bytes: u64,
        hash_view: u64,
        output: u64,
        capacity: i64,
        written: u64,
    ) -> i64;
    fn prodex_mojo_smart_context_static_overflow_preimage_v1(
        abi_version: i64,
        id_view: u64,
        hash_view: u64,
        byte_length: u64,
        host_usize_bytes: i64,
        output: u64,
        output_capacity: i64,
        written: u64,
    ) -> i64;
    fn prodex_mojo_smart_context_static_overflow_accumulate_v1(
        abi_version: i64,
        aggregate: u64,
        digest: u64,
        output: u64,
        output_capacity: i64,
    ) -> i64;
    fn prodex_mojo_smart_context_scope_parse_v1(
        abi_version: i64,
        input: u64,
        output: u64,
        capacity: i64,
        written: u64,
        valid: u64,
    ) -> i64;
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
#[path = "smart_context_normalization/static_canonicalization_tests.rs"]
mod static_canonicalization_tests;
