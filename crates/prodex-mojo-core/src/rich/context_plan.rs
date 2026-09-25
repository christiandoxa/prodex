use super::*;

#[derive(Debug, Clone, Copy)]
pub struct ContextPlanItem<'a> {
    pub id: &'a str,
    pub token_cost: usize,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextPlanAction {
    pub id: String,
    pub action: i64,
    pub reason: i64,
    pub token_cost: usize,
    pub input_index: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextPlan {
    pub actions: Vec<ContextPlanAction>,
    pub used_tokens: usize,
}

#[cfg(feature = "mojo-runtime")]
const SMART_CONTEXT_REHYDRATE_ORDER_ABI_VERSION: i64 = 1;
#[cfg(feature = "mojo-runtime")]
const SMART_CONTEXT_REHYDRATE_MAX_IDENTIFIER_BYTES: usize = 4_096;

#[cfg(feature = "mojo-runtime")]
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
struct SmartContextRehydrateOrderItem {
    id: RichStringView,
    token_cost: u64,
    required: i64,
}

#[cfg(feature = "mojo-runtime")]
const _: () = {
    assert!(std::mem::size_of::<SmartContextRehydrateOrderItem>() == 32);
    assert!(std::mem::offset_of!(SmartContextRehydrateOrderItem, id) == 0);
    assert!(std::mem::offset_of!(SmartContextRehydrateOrderItem, token_cost) == 16);
    assert!(std::mem::offset_of!(SmartContextRehydrateOrderItem, required) == 24);
};

#[cfg(feature = "mojo-runtime")]
unsafe extern "C" {
    fn prodex_smart_context_rehydrate_order_v1(
        abi_version: i64,
        items: u64,
        item_count: i64,
        available: u64,
        available_count: i64,
        ordered_indices: u64,
        indices_capacity: i64,
        availability_tags: u64,
        availability_capacity: i64,
    ) -> i64;
}

impl ContextPlanItem<'_> {
    /// Plans rehydration using Mojo-owned ordering and the runtime rehydration planner.
    #[cfg(feature = "mojo-runtime")]
    pub fn plan_smart_context_rehydrate(
        items: &[ContextPlanItem<'_>],
        available: &[&str],
        token_budget: usize,
        tier: i64,
    ) -> Result<ContextPlan, MojoError> {
        use crate::runtime::{
            SMART_CONTEXT_REHYDRATE_ACTION_BUDGET, SMART_CONTEXT_REHYDRATE_ACTION_MINIMAL,
            SMART_CONTEXT_REHYDRATE_ACTION_MISSING, SMART_CONTEXT_REHYDRATE_ACTION_REHYDRATE,
            SMART_CONTEXT_REHYDRATE_EXACT_TIER, SMART_CONTEXT_REHYDRATE_MAX_COUNT,
            SMART_CONTEXT_REHYDRATE_MINIMAL_TIER, SmartContextRehydrateInput,
            smart_context_rehydrate_plan_batch,
        };

        if items.len() > SMART_CONTEXT_REHYDRATE_MAX_COUNT
            || available.len() > SMART_CONTEXT_REHYDRATE_MAX_COUNT
            || !(SMART_CONTEXT_REHYDRATE_MINIMAL_TIER..=SMART_CONTEXT_REHYDRATE_EXACT_TIER)
                .contains(&tier)
            || u64::try_from(token_budget).is_err()
            || items.iter().any(|item| {
                item.id.len() > SMART_CONTEXT_REHYDRATE_MAX_IDENTIFIER_BYTES
                    || u64::try_from(item.token_cost).is_err()
            })
            || available
                .iter()
                .any(|id| id.len() > SMART_CONTEXT_REHYDRATE_MAX_IDENTIFIER_BYTES)
        {
            return Err(MojoError::InvalidInput);
        }

        let rich_items = items
            .iter()
            .map(|item| {
                Ok(SmartContextRehydrateOrderItem {
                    id: view(item.id),
                    token_cost: u64::try_from(item.token_cost)
                        .map_err(|_| MojoError::InvalidInput)?,
                    required: i64::from(item.required),
                })
            })
            .collect::<Result<Vec<_>, MojoError>>()?;
        let rich_available = available.iter().map(|id| view(id)).collect::<Vec<_>>();
        let mut ordered_indices = vec![0_i64; items.len().max(1)];
        let mut availability_tags = vec![0_i64; items.len().max(1)];
        let status = unsafe {
            prodex_smart_context_rehydrate_order_v1(
                SMART_CONTEXT_REHYDRATE_ORDER_ABI_VERSION,
                mojo_pointer_address(rich_items.as_ptr()),
                i64::try_from(items.len()).map_err(|_| MojoError::InvalidInput)?,
                mojo_pointer_address(rich_available.as_ptr()),
                i64::try_from(available.len()).map_err(|_| MojoError::InvalidInput)?,
                mojo_pointer_address(ordered_indices.as_mut_ptr()),
                i64::try_from(items.len()).map_err(|_| MojoError::InvalidInput)?,
                mojo_pointer_address(availability_tags.as_mut_ptr()),
                i64::try_from(items.len()).map_err(|_| MojoError::InvalidInput)?,
            )
        };
        match status {
            0 => {}
            1 => return Err(MojoError::InvalidInput),
            2 => return Err(MojoError::AbiMismatch),
            3 => return Err(MojoError::Capacity),
            _ => return Err(MojoError::InvalidOutput),
        }

        let mut seen = vec![false; items.len()];
        let mut plan_inputs = Vec::with_capacity(items.len());
        for (order, &raw_index) in ordered_indices[..items.len()].iter().enumerate() {
            let input_index = usize::try_from(raw_index).map_err(|_| MojoError::InvalidOutput)?;
            let item = items.get(input_index).ok_or(MojoError::InvalidOutput)?;
            if std::mem::replace(&mut seen[input_index], true) {
                return Err(MojoError::InvalidOutput);
            }
            let available = match availability_tags[order] {
                0 => false,
                1 => true,
                _ => return Err(MojoError::InvalidOutput),
            };
            plan_inputs.push(SmartContextRehydrateInput {
                token_cost: u64::try_from(item.token_cost).map_err(|_| MojoError::InvalidInput)?,
                required: item.required,
                available,
            });
        }
        if seen.iter().any(|was_seen| !was_seen) {
            return Err(MojoError::InvalidOutput);
        }

        let plan = smart_context_rehydrate_plan_batch(&plan_inputs, token_budget, tier)?;
        if plan.action_tags.len() != items.len() {
            return Err(MojoError::InvalidOutput);
        }
        let actions = plan
            .action_tags
            .into_iter()
            .enumerate()
            .map(|(order, tag)| {
                let input_index = usize::try_from(ordered_indices[order])
                    .map_err(|_| MojoError::InvalidOutput)?;
                let item = items.get(input_index).ok_or(MojoError::InvalidOutput)?;
                let (action, reason) = match tag {
                    SMART_CONTEXT_REHYDRATE_ACTION_REHYDRATE => (1, 0),
                    SMART_CONTEXT_REHYDRATE_ACTION_MISSING => (0, 1),
                    SMART_CONTEXT_REHYDRATE_ACTION_BUDGET => (0, 2),
                    SMART_CONTEXT_REHYDRATE_ACTION_MINIMAL => (0, 3),
                    _ => return Err(MojoError::InvalidOutput),
                };
                Ok(ContextPlanAction {
                    id: item.id.to_owned(),
                    action,
                    reason,
                    token_cost: item.token_cost,
                    input_index,
                })
            })
            .collect::<Result<Vec<_>, MojoError>>()?;

        Ok(ContextPlan {
            actions,
            used_tokens: usize::try_from(plan.used_tokens).map_err(|_| MojoError::InvalidOutput)?,
        })
    }
}

pub fn plan_context_items(
    items: &[ContextPlanItem<'_>],
    available: &[&str],
    token_budget: usize,
    tier: i64,
) -> Result<ContextPlan, MojoError> {
    ensure_rich_abi()?;
    let rich_items = items
        .iter()
        .map(|item| {
            Ok(RichPlanItem {
                id: view(item.id),
                token_cost: i64::try_from(item.token_cost).map_err(|_| MojoError::InvalidInput)?,
                required: i64::from(item.required),
            })
        })
        .collect::<Result<Vec<_>, MojoError>>()?;
    let rich_available = available
        .iter()
        .map(|value| view(value))
        .collect::<Vec<_>>();
    let output_capacity = items
        .iter()
        .try_fold(0_usize, |total, item| total.checked_add(item.id.len()))
        .ok_or(MojoError::InvalidInput)?;
    let scratch_capacity = hash_capacity(available.len())?;
    let mut output = vec![0_u8; output_capacity.max(1)];
    let mut actions = vec![RichPlanAction::default(); items.len().max(1)];
    let mut hash_slots = vec![-1_i64; scratch_capacity];
    let mut result = RichPlanResult::default();
    let status = unsafe {
        prodex_mojo_rich_context_plan_v2(
            RICH_ABI_VERSION,
            mojo_pointer_address(rich_items.as_ptr()),
            i64::try_from(items.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_pointer_address(rich_available.as_ptr()),
            i64::try_from(available.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(token_budget).map_err(|_| MojoError::InvalidInput)?,
            tier,
            mojo_pointer_address(actions.as_mut_ptr()),
            i64::try_from(items.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_pointer_address(output.as_mut_ptr()),
            i64::try_from(output_capacity).map_err(|_| MojoError::InvalidInput)?,
            mojo_pointer_address(hash_slots.as_mut_ptr()),
            i64::try_from(scratch_capacity).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut result),
        )
    };
    if status != 0 {
        return Err(status_error(
            status,
            5,
            result.issue_kind,
            result.issue_offset,
            result.issue_length,
        ));
    }
    if result.actions_written != items.len() as i64
        || result.output_written < 0
        || result.output_written as usize > output.len()
        || result.used_tokens < 0
    {
        return Err(MojoError::InvalidOutput);
    }
    let output = &output[..result.output_written as usize];
    let mut planned = Vec::with_capacity(items.len());
    for action in &actions[..items.len()] {
        let id = std::str::from_utf8(slice(output, action.id)?)
            .map_err(|_| MojoError::InvalidOutput)?
            .to_string();
        let input_index =
            usize::try_from(action.input_index).map_err(|_| MojoError::InvalidOutput)?;
        let token_cost =
            usize::try_from(action.token_cost).map_err(|_| MojoError::InvalidOutput)?;
        if input_index >= items.len()
            || token_cost != items[input_index].token_cost
            || id != items[input_index].id
        {
            return Err(MojoError::InvalidOutput);
        }
        planned.push(ContextPlanAction {
            id,
            action: action.action,
            reason: action.reason,
            token_cost,
            input_index,
        });
    }
    Ok(ContextPlan {
        actions: planned,
        used_tokens: usize::try_from(result.used_tokens).map_err(|_| MojoError::InvalidOutput)?,
    })
}
