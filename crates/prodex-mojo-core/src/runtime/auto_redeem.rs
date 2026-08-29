use super::RuntimeStringView;

/// Candidate facts for one bounded auto-redeem pool planning call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoRedeemCandidateInput<'a> {
    pub plan_type: Option<&'a str>,
    pub available_count: i64,
    pub weekly_status: i64,
    pub weekly_reset_at: i64,
    pub inflight_count: i64,
    pub health_sort_key: i64,
    pub order_index: i64,
}

const RUNTIME_AUTO_REDEEM_ABI_VERSION: i64 = 6;
const RUNTIME_AUTO_REDEEM_FIELD_COUNT: usize = 6;
const RUNTIME_AUTO_REDEEM_MAX_PLAN_BYTES: usize = 4_096;
pub const RUNTIME_AUTO_REDEEM_PLAN_MAX_COUNT: usize = 256;
const RUNTIME_AUTO_REDEEM_STATUS_INVALID: i64 = 1;
const RUNTIME_AUTO_REDEEM_STATUS_TEXT: i64 = 2;
const RUNTIME_AUTO_REDEEM_STATUS_ABI: i64 = 4;

unsafe extern "C" {
    fn prodex_runtime_auto_redeem_plan_batch(
        abi_version: i64,
        plan_types_address: u64,
        fields_address: u64,
        selected_index_address: u64,
        count: i64,
        now: i64,
    ) -> i64;
}

pub fn auto_redeem_plan_self_test() -> bool {
    auto_redeem_plan_batch(
        &[
            AutoRedeemCandidateInput {
                plan_type: Some("free"),
                available_count: 1,
                weekly_status: 3,
                weekly_reset_at: 10_000,
                inflight_count: 0,
                health_sort_key: 0,
                order_index: 0,
            },
            AutoRedeemCandidateInput {
                plan_type: Some(" Pro-5X "),
                available_count: 1,
                weekly_status: 3,
                weekly_reset_at: 10_000,
                inflight_count: 0,
                health_sort_key: 0,
                order_index: 1,
            },
            AutoRedeemCandidateInput {
                plan_type: Some("PLUS"),
                available_count: 1,
                weekly_status: 3,
                weekly_reset_at: 10_000,
                inflight_count: 0,
                health_sort_key: 0,
                order_index: 2,
            },
        ],
        1_000,
    )
    .is_ok_and(|selected| selected == Some(2))
}

pub fn auto_redeem_plan_batch(
    inputs: &[AutoRedeemCandidateInput<'_>],
    now: i64,
) -> Result<Option<usize>, crate::MojoError> {
    if inputs.len() > RUNTIME_AUTO_REDEEM_PLAN_MAX_COUNT
        || inputs.iter().any(|input| {
            input
                .plan_type
                .is_some_and(|plan| plan.len() > RUNTIME_AUTO_REDEEM_MAX_PLAN_BYTES)
                || input.weekly_status < 0
                || input.weekly_status > 4
                || input.inflight_count < 0
                || input.health_sort_key < 0
                || input.order_index < 0
        })
    {
        return Err(crate::MojoError::InvalidInput);
    }
    if inputs.is_empty() {
        return Ok(None);
    }

    let plan_types = inputs
        .iter()
        .map(|input| {
            input
                .plan_type
                .map(|plan| RuntimeStringView {
                    ptr: plan.as_ptr() as usize as u64,
                    len: plan.len() as u64,
                })
                .unwrap_or(RuntimeStringView { ptr: 0, len: 0 })
        })
        .collect::<Vec<_>>();
    let mut fields = Vec::with_capacity(inputs.len() * RUNTIME_AUTO_REDEEM_FIELD_COUNT);
    for input in inputs {
        fields.extend([
            input.available_count,
            input.weekly_status,
            input.weekly_reset_at,
            input.inflight_count,
            input.health_sort_key,
            input.order_index,
        ]);
    }
    let mut selected_index = -1_i64;
    let status = unsafe {
        prodex_runtime_auto_redeem_plan_batch(
            RUNTIME_AUTO_REDEEM_ABI_VERSION,
            plan_types.as_ptr() as usize as u64,
            fields.as_ptr() as usize as u64,
            &mut selected_index as *mut i64 as usize as u64,
            i64::try_from(inputs.len()).map_err(|_| crate::MojoError::InvalidInput)?,
            now,
        )
    };
    if status != 0 {
        return Err(match status {
            RUNTIME_AUTO_REDEEM_STATUS_INVALID | RUNTIME_AUTO_REDEEM_STATUS_TEXT => {
                crate::MojoError::InvalidInput
            }
            RUNTIME_AUTO_REDEEM_STATUS_ABI => crate::MojoError::AbiMismatch,
            _ => crate::MojoError::InvalidOutput,
        });
    }
    match selected_index {
        -1 => Ok(None),
        index => usize::try_from(index)
            .ok()
            .filter(|index| *index < inputs.len())
            .map(Some)
            .ok_or(crate::MojoError::InvalidOutput),
    }
}
