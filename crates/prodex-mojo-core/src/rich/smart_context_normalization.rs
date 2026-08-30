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
const SMART_CONTEXT_CAPSULE_MAX_COUNT: usize = 65_536;

unsafe extern "C" {
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

fn checked_output_capacity(input_len: usize) -> Result<usize, MojoError> {
    if input_len > SMART_CONTEXT_NORMALIZATION_MAX_BYTES {
        return Err(MojoError::InvalidInput);
    }
    input_len
        .checked_mul(2)
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
    let capacity = checked_output_capacity(text.len())?;
    let input = view(text);
    let mut output = vec![0_u8; capacity.max(1)];
    let mut written = 0_i64;
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
    if status != 0 {
        return Err(normalization_status_error(status));
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    let output = output.get(..written).ok_or(MojoError::InvalidOutput)?;
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
