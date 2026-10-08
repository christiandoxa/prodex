//! Thin scalar-policy ABI for Mojo-owned provider reasoning, flags, and limits.

const PROVIDER_SCALAR_POLICY_ABI_VERSION: i64 = 1;
const PROVIDER_SCALAR_POLICY_REASONING_EFFORT: i64 = 0;
const PROVIDER_SCALAR_POLICY_COPILOT_PROMPT_LIMIT: i64 = 1;
const PROVIDER_SCALAR_POLICY_BOOLEAN_TOKEN: i64 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderReasoningEffortClass {
    None,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
    Max,
    Ultra,
    Unknown,
}

unsafe extern "C" {
    fn prodex_provider_reasoning_effort_label_v1(
        abi_version: i64,
        effort: i64,
        output_address: u64,
        output_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_provider_scalar_policy_v1(
        abi_version: i64,
        operation: i64,
        address: u64,
        length: i64,
    ) -> i64;
}

fn provider_scalar_policy(operation: i64, value: &str) -> Result<i64, crate::MojoError> {
    let length = i64::try_from(value.len()).map_err(|_| crate::MojoError::InvalidInput)?;
    let result = unsafe {
        prodex_provider_scalar_policy_v1(
            PROVIDER_SCALAR_POLICY_ABI_VERSION,
            operation,
            value.as_ptr() as usize as u64,
            length,
        )
    };
    match result {
        -3 => Err(crate::MojoError::AbiMismatch),
        -2 => Err(crate::MojoError::InvalidInput),
        value => Ok(value),
    }
}

fn load_provider_reasoning_effort_label(effort: i64) -> Result<Option<String>, crate::MojoError> {
    let mut output = [0_u8; 16];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_provider_reasoning_effort_label_v1(
            PROVIDER_SCALAR_POLICY_ABI_VERSION,
            effort,
            output.as_mut_ptr() as usize as u64,
            i64::try_from(output.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        -4 => return Err(crate::MojoError::AbiMismatch),
        -2 => return Err(crate::MojoError::Capacity),
        -1 => return Err(crate::MojoError::InvalidInput),
        _ => return Err(crate::MojoError::InvalidOutput),
    }
    let written = usize::try_from(written).map_err(|_| crate::MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(crate::MojoError::InvalidOutput);
    }
    if written == 0 {
        return Ok(None);
    }
    String::from_utf8(output[..written].to_vec())
        .map(Some)
        .map_err(|_| crate::MojoError::InvalidOutput)
}

pub fn provider_reasoning_effort_label(
    effort: i64,
) -> Result<Option<&'static str>, crate::MojoError> {
    use std::sync::OnceLock;

    static LABELS: OnceLock<Result<Vec<Option<String>>, crate::MojoError>> = OnceLock::new();
    let effort_index = usize::try_from(effort)
        .ok()
        .filter(|value| *value <= 8)
        .ok_or(crate::MojoError::InvalidInput)?;
    match LABELS.get_or_init(|| {
        (0_i64..=8)
            .map(load_provider_reasoning_effort_label)
            .collect()
    }) {
        Ok(labels) => Ok(labels
            .get(effort_index)
            .ok_or(crate::MojoError::InvalidOutput)?
            .as_deref()),
        Err(error) => Err(*error),
    }
}

pub fn provider_reasoning_effort_class(
    value: &str,
) -> Result<ProviderReasoningEffortClass, crate::MojoError> {
    Ok(
        match provider_scalar_policy(PROVIDER_SCALAR_POLICY_REASONING_EFFORT, value)? {
            0 => ProviderReasoningEffortClass::None,
            1 => ProviderReasoningEffortClass::Minimal,
            2 => ProviderReasoningEffortClass::Low,
            3 => ProviderReasoningEffortClass::Medium,
            4 => ProviderReasoningEffortClass::High,
            5 => ProviderReasoningEffortClass::XHigh,
            6 => ProviderReasoningEffortClass::Max,
            7 => ProviderReasoningEffortClass::Ultra,
            8 => ProviderReasoningEffortClass::Unknown,
            _ => return Err(crate::MojoError::InvalidOutput),
        },
    )
}

pub fn provider_boolean_token(value: &str) -> Result<Option<bool>, crate::MojoError> {
    match provider_scalar_policy(PROVIDER_SCALAR_POLICY_BOOLEAN_TOKEN, value)? {
        -1 => Ok(None),
        0 => Ok(Some(false)),
        1 => Ok(Some(true)),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}

pub fn provider_copilot_prompt_token_limit(model: &str) -> Result<Option<usize>, crate::MojoError> {
    match provider_scalar_policy(PROVIDER_SCALAR_POLICY_COPILOT_PROMPT_LIMIT, model)? {
        -1 => Ok(None),
        value if value > 0 => usize::try_from(value)
            .map(Some)
            .map_err(|_| crate::MojoError::InvalidOutput),
        _ => Err(crate::MojoError::InvalidOutput),
    }
}
