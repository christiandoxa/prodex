use crate::MojoError;

const ABI_VERSION: i64 = 1;

#[repr(i64)]
#[derive(Clone, Copy)]
enum Operation {
    ConcurrencyParse = 0,
    ConcurrencyValidate = 1,
    ReasoningEffort = 2,
    ModelNonempty = 3,
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
    }
}
