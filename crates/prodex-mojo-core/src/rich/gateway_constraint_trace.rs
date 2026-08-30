use super::{ensure_rich_abi, mojo_mut_pointer_address, mojo_pointer_address};
use crate::MojoError;

const GATEWAY_CONSTRAINT_TRACE_ABI_VERSION: i64 = 1;
const GATEWAY_CONSTRAINT_TRACE_MAX_CANDIDATES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewayConstraintTraceRejectionStage {
    EndpointCapability,
    RequestConstraints,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewayConstraintTraceAffinityOutcome {
    NotApplicable,
    Retained,
    Exhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewayConstraintTraceTerminalOutcome {
    Selected,
    NoCandidate,
    AffinityExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayConstraintTracePlan {
    pub ordered_indices: Vec<usize>,
    pub rejection_stages: Vec<Option<GatewayConstraintTraceRejectionStage>>,
    pub endpoint_supported: bool,
    pub request_constraints_passed: Option<bool>,
    pub affinity_outcome: GatewayConstraintTraceAffinityOutcome,
    pub terminal_outcome: GatewayConstraintTraceTerminalOutcome,
}

unsafe extern "C" {
    fn prodex_mojo_gateway_constraint_trace_v1(
        abi_version: i64,
        eligible: u64,
        decisions: u64,
        endpoint_unsupported_decision: i64,
        candidate_count: i64,
        selected_index: i64,
        hard_affinity: i64,
        ordered_indices: u64,
        ordered_capacity: i64,
        rejection_stages: u64,
        rejection_capacity: i64,
        endpoint_supported: u64,
        request_constraints_outcome: u64,
        affinity_outcome: u64,
        terminal_outcome: u64,
    ) -> i64;
}

pub fn plan_gateway_constraint_trace(
    eligible: &[bool],
    decisions: &[i64],
    endpoint_unsupported_decision: i64,
    selected_index: Option<usize>,
    hard_affinity: bool,
) -> Result<GatewayConstraintTracePlan, MojoError> {
    ensure_rich_abi()?;
    if eligible.len() != decisions.len()
        || eligible.len() > GATEWAY_CONSTRAINT_TRACE_MAX_CANDIDATES
        || endpoint_unsupported_decision < 0
    {
        return Err(MojoError::InvalidInput);
    }
    let selected_index = match selected_index {
        Some(index) if index < eligible.len() => {
            i64::try_from(index).map_err(|_| MojoError::InvalidInput)?
        }
        Some(_) => return Err(MojoError::InvalidInput),
        None => -1,
    };
    let eligible = eligible
        .iter()
        .map(|value| i64::from(*value))
        .collect::<Vec<_>>();
    let mut ordered_indices = vec![-1_i64; eligible.len().max(1)];
    let mut rejection_stages = vec![0_i64; eligible.len().max(1)];
    let mut endpoint_supported = -1_i64;
    let mut request_constraints_outcome = -1_i64;
    let mut affinity_outcome = -1_i64;
    let mut terminal_outcome = -1_i64;
    let status = unsafe {
        prodex_mojo_gateway_constraint_trace_v1(
            GATEWAY_CONSTRAINT_TRACE_ABI_VERSION,
            mojo_pointer_address(eligible.as_ptr()),
            mojo_pointer_address(decisions.as_ptr()),
            endpoint_unsupported_decision,
            i64::try_from(eligible.len()).map_err(|_| MojoError::InvalidInput)?,
            selected_index,
            i64::from(hard_affinity),
            mojo_mut_pointer_address(ordered_indices.as_mut_ptr()),
            i64::try_from(ordered_indices.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(rejection_stages.as_mut_ptr()),
            i64::try_from(rejection_stages.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut endpoint_supported),
            mojo_mut_pointer_address(&mut request_constraints_outcome),
            mojo_mut_pointer_address(&mut affinity_outcome),
            mojo_mut_pointer_address(&mut terminal_outcome),
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        3 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let endpoint_supported = match endpoint_supported {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    let request_constraints_passed = match request_constraints_outcome {
        -1 => None,
        0 => Some(false),
        1 => Some(true),
        _ => return Err(MojoError::InvalidOutput),
    };
    let affinity_outcome = match affinity_outcome {
        0 => GatewayConstraintTraceAffinityOutcome::NotApplicable,
        1 => GatewayConstraintTraceAffinityOutcome::Retained,
        2 => GatewayConstraintTraceAffinityOutcome::Exhausted,
        _ => return Err(MojoError::InvalidOutput),
    };
    let terminal_outcome = match terminal_outcome {
        0 => GatewayConstraintTraceTerminalOutcome::Selected,
        1 => GatewayConstraintTraceTerminalOutcome::NoCandidate,
        2 => GatewayConstraintTraceTerminalOutcome::AffinityExhausted,
        _ => return Err(MojoError::InvalidOutput),
    };
    let mut seen = vec![false; eligible.len()];
    let ordered_indices = ordered_indices
        .into_iter()
        .take(eligible.len())
        .map(|index| {
            let index = usize::try_from(index).map_err(|_| MojoError::InvalidOutput)?;
            if index >= seen.len() || seen[index] {
                return Err(MojoError::InvalidOutput);
            }
            seen[index] = true;
            Ok(index)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if seen.iter().any(|seen| !seen) {
        return Err(MojoError::InvalidOutput);
    }
    let rejection_stages = rejection_stages
        .into_iter()
        .take(eligible.len())
        .map(|stage| match stage {
            0 => Ok(None),
            1 => Ok(Some(
                GatewayConstraintTraceRejectionStage::EndpointCapability,
            )),
            2 => Ok(Some(
                GatewayConstraintTraceRejectionStage::RequestConstraints,
            )),
            _ => Err(MojoError::InvalidOutput),
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(GatewayConstraintTracePlan {
        ordered_indices,
        rejection_stages,
        endpoint_supported,
        request_constraints_passed,
        affinity_outcome,
        terminal_outcome,
    })
}
