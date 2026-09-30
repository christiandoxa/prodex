use crate::MojoError;

const ABI_VERSION: i64 = 1;

#[repr(i64)]
#[derive(Clone, Copy)]
enum Operation {
    ConcurrencyParse = 0,
    ConcurrencyValidate = 1,
    ReasoningEffort = 2,
    ModelNonempty = 3,
    ProviderUrlPolicy = 4,
    ChildSpecScalarPolicy = 5,
    PromptSteps = 6,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConcurrencySourcePlan {
    Default,
    Preset,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedConcurrency {
    pub value: u16,
    pub source: ConcurrencySourcePlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConcurrencyParseViolation {
    Syntax,
    Overflow,
    OutOfRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningEffortPlan {
    None,
    Minimal,
    Low,
    Medium,
    High,
    XHigh,
    Max,
    Ultra,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderUrlViolation {
    LocalRequiresUrl,
    NonLocalRejectsUrl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildSpecScalarViolation {
    InvalidRecursionMarker,
    InvalidTaskSize,
}

unsafe extern "C" {
    fn prodex_sub_agent_policy_v1(
        abi_version: i64,
        operation: i64,
        address: u64,
        length: i64,
        scalar: i64,
        result_address: u64,
    ) -> i64;
}

fn call(operation: Operation, input: &str, scalar: i64) -> Result<[i64; 3], MojoError> {
    let mut result = [0_i64; 3];
    let status = unsafe {
        prodex_sub_agent_policy_v1(
            ABI_VERSION,
            operation as i64,
            input.as_ptr() as usize as u64,
            i64::try_from(input.len()).map_err(|_| MojoError::InvalidInput)?,
            scalar,
            result.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => Ok(result),
        1 => Err(MojoError::InvalidInput),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn parse_concurrency(
    value: &str,
) -> Result<Result<ParsedConcurrency, ConcurrencyParseViolation>, MojoError> {
    let result = call(Operation::ConcurrencyParse, value, 0)?;
    match result[0] {
        0 => {
            let value = u16::try_from(result[1]).map_err(|_| MojoError::InvalidOutput)?;
            let source = match result[2] {
                0 => ConcurrencySourcePlan::Default,
                1 => ConcurrencySourcePlan::Preset,
                2 => ConcurrencySourcePlan::Custom,
                _ => return Err(MojoError::InvalidOutput),
            };
            Ok(Ok(ParsedConcurrency { value, source }))
        }
        1 => Ok(Err(ConcurrencyParseViolation::Syntax)),
        2 => Ok(Err(ConcurrencyParseViolation::Overflow)),
        3 => Ok(Err(ConcurrencyParseViolation::OutOfRange)),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn concurrency_valid(value: u16) -> Result<bool, MojoError> {
    let result = call(Operation::ConcurrencyValidate, "", i64::from(value))?;
    match result[0] {
        0 => Ok(true),
        3 => Ok(false),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn reasoning_effort(value: &str) -> Result<Option<ReasoningEffortPlan>, MojoError> {
    let result = call(Operation::ReasoningEffort, value, 0)?;
    match result[0] {
        0 => Ok(Some(match result[1] {
            0 => ReasoningEffortPlan::None,
            1 => ReasoningEffortPlan::Minimal,
            2 => ReasoningEffortPlan::Low,
            3 => ReasoningEffortPlan::Medium,
            4 => ReasoningEffortPlan::High,
            5 => ReasoningEffortPlan::XHigh,
            6 => ReasoningEffortPlan::Max,
            7 => ReasoningEffortPlan::Ultra,
            _ => return Err(MojoError::InvalidOutput),
        })),
        1 => Ok(None),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn model_nonempty(value: &str) -> Result<bool, MojoError> {
    let result = call(Operation::ModelNonempty, value, 0)?;
    match result[1] {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn provider_url_violation(
    provider_is_local: bool,
    url_present: bool,
) -> Result<Option<ProviderUrlViolation>, MojoError> {
    let scalar = i64::from(provider_is_local) | (i64::from(url_present) << 1);
    let result = call(Operation::ProviderUrlPolicy, "", scalar)?;
    match result[0] {
        0 => Ok(None),
        1 => Ok(Some(ProviderUrlViolation::LocalRequiresUrl)),
        2 => Ok(Some(ProviderUrlViolation::NonLocalRejectsUrl)),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubAgentPromptStepPlan {
    pub provider: bool,
    pub local_url: bool,
    pub model: bool,
    pub reasoning_effort: bool,
    pub max_concurrency: bool,
}

pub fn prompt_step_plan(
    provider_explicit: bool,
    provider_is_local: bool,
    url_present: bool,
    model_explicit: bool,
    effort_explicit: bool,
) -> Result<SubAgentPromptStepPlan, MojoError> {
    let scalar = i64::from(provider_explicit)
        | (i64::from(provider_is_local) << 1)
        | (i64::from(url_present) << 2)
        | (i64::from(model_explicit) << 3)
        | (i64::from(effort_explicit) << 4);
    let result = call(Operation::PromptSteps, "", scalar)?;
    if result[0] != 0 || result[1] < 0 || result[1] > 31 {
        return Err(MojoError::InvalidOutput);
    }
    let mask = result[1];
    Ok(SubAgentPromptStepPlan {
        provider: mask & 1 != 0,
        local_url: mask & 2 != 0,
        model: mask & 4 != 0,
        reasoning_effort: mask & 8 != 0,
        max_concurrency: mask & 16 != 0,
    })
}

pub fn child_spec_scalar_violation(
    recursion_marker: &str,
    task_max_bytes: usize,
) -> Result<Option<ChildSpecScalarViolation>, MojoError> {
    let scalar = i64::try_from(task_max_bytes).map_err(|_| MojoError::InvalidInput)?;
    let result = call(Operation::ChildSpecScalarPolicy, recursion_marker, scalar)?;
    match result[0] {
        0 => Ok(None),
        1 => Ok(Some(ChildSpecScalarViolation::InvalidRecursionMarker)),
        2 => Ok(Some(ChildSpecScalarViolation::InvalidTaskSize)),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sub_agent_policy_kernel_matches_public_cli_contract() {
        assert_eq!(
            parse_concurrency(" default ").unwrap().unwrap(),
            ParsedConcurrency {
                value: 4,
                source: ConcurrencySourcePlan::Default,
            }
        );
        assert_eq!(
            parse_concurrency("8").unwrap().unwrap(),
            ParsedConcurrency {
                value: 8,
                source: ConcurrencySourcePlan::Preset,
            }
        );
        assert_eq!(
            parse_concurrency("23").unwrap().unwrap(),
            ParsedConcurrency {
                value: 23,
                source: ConcurrencySourcePlan::Custom,
            }
        );
        assert_eq!(
            parse_concurrency("1e2").unwrap(),
            Err(ConcurrencyParseViolation::Syntax)
        );
        assert_eq!(
            parse_concurrency("999999999999999999999").unwrap(),
            Err(ConcurrencyParseViolation::Overflow)
        );
        assert_eq!(
            parse_concurrency("65").unwrap(),
            Err(ConcurrencyParseViolation::OutOfRange)
        );
        assert!(concurrency_valid(64).unwrap());
        assert!(!concurrency_valid(0).unwrap());

        assert_eq!(
            reasoning_effort(" XHIGH ").unwrap(),
            Some(ReasoningEffortPlan::XHigh)
        );
        assert_eq!(reasoning_effort("extreme").unwrap(), None);
        assert!(model_nonempty(" 模型/β-🦀 ").unwrap());
        assert!(!model_nonempty(" \t\u{3000} ").unwrap());
        assert_eq!(provider_url_violation(true, true).unwrap(), None);
        assert_eq!(
            provider_url_violation(true, false).unwrap(),
            Some(ProviderUrlViolation::LocalRequiresUrl)
        );
        assert_eq!(
            provider_url_violation(false, true).unwrap(),
            Some(ProviderUrlViolation::NonLocalRejectsUrl)
        );
        assert_eq!(
            child_spec_scalar_violation("PRODEX_SUB_AGENT", 65_536).unwrap(),
            None
        );
        assert_eq!(
            child_spec_scalar_violation("bad", 65_536).unwrap(),
            Some(ChildSpecScalarViolation::InvalidRecursionMarker)
        );
        assert_eq!(
            child_spec_scalar_violation("PRODEX_SUB_AGENT", 0).unwrap(),
            Some(ChildSpecScalarViolation::InvalidTaskSize)
        );
        assert_eq!(
            prompt_step_plan(false, true, false, false, false).unwrap(),
            SubAgentPromptStepPlan {
                provider: true,
                local_url: true,
                model: true,
                reasoning_effort: true,
                max_concurrency: true,
            }
        );
        assert_eq!(
            prompt_step_plan(true, false, false, true, true).unwrap(),
            SubAgentPromptStepPlan {
                provider: false,
                local_url: false,
                model: false,
                reasoning_effort: false,
                max_concurrency: true,
            }
        );
    }
}
