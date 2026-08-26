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
            rich_items.as_ptr(),
            i64::try_from(items.len()).map_err(|_| MojoError::InvalidInput)?,
            rich_available.as_ptr(),
            i64::try_from(available.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(token_budget).map_err(|_| MojoError::InvalidInput)?,
            tier,
            actions.as_mut_ptr(),
            i64::try_from(items.len()).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr(),
            i64::try_from(output_capacity).map_err(|_| MojoError::InvalidInput)?,
            hash_slots.as_mut_ptr(),
            i64::try_from(scratch_capacity).map_err(|_| MojoError::InvalidInput)?,
            &mut result,
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
