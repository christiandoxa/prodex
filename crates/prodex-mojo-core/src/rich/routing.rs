use super::*;

#[derive(Debug, Clone, Copy)]
pub struct RouteInput<'a> {
    pub provider: &'a str,
    pub model: &'a str,
    pub capabilities: &'a str,
    pub hard_eligible: bool,
    pub health: i64,
    pub load: i64,
    pub quota_headroom: Option<i64>,
    pub cost: i64,
    pub latency: i64,
    pub risk: i64,
    pub priority: i64,
    pub affinity: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteCandidate {
    pub provider: String,
    pub model: String,
    pub capability_mask: u8,
    pub eligible: bool,
    pub reason: i64,
    pub score: i64,
    pub components: [i64; 7],
    pub weighted_total: u64,
    pub input_index: usize,
    pub duplicate_of: Option<usize>,
    pub provider_order: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoutePlan {
    pub candidates: Vec<RouteCandidate>,
    pub ordered_indices: Vec<usize>,
    pub selected_index: Option<usize>,
}

fn ffi_route_inputs(inputs: &[RouteInput<'_>]) -> Vec<RichRouteInput> {
    inputs
        .iter()
        .map(|input| RichRouteInput {
            provider: view(input.provider),
            model: view(input.model),
            capabilities: view(input.capabilities),
            hard_eligible: i64::from(input.hard_eligible),
            health: input.health,
            load: input.load,
            quota_headroom: input.quota_headroom.unwrap_or_default(),
            quota_present: i64::from(input.quota_headroom.is_some()),
            cost: input.cost,
            latency: input.latency,
            risk: input.risk,
            priority: input.priority,
            affinity: i64::from(input.affinity),
        })
        .collect()
}

fn route_output_capacity(inputs: &[RouteInput<'_>]) -> Result<usize, MojoError> {
    inputs
        .iter()
        .try_fold(0_usize, |total, input| {
            total
                .checked_add(input.provider.len())
                .and_then(|value| value.checked_add(input.model.len()))
        })
        .ok_or(MojoError::InvalidInput)
}

fn decode_route_candidates(
    records: &[RichRouteRecord],
    output: &[u8],
    input_count: usize,
) -> Result<Vec<RouteCandidate>, MojoError> {
    let mut candidates = Vec::with_capacity(input_count);
    for record in records {
        let provider = std::str::from_utf8(slice(output, record.provider)?)
            .map_err(|_| MojoError::InvalidOutput)?
            .to_string();
        let model = std::str::from_utf8(slice(output, record.model)?)
            .map_err(|_| MojoError::InvalidOutput)?
            .to_string();
        let capability_mask =
            u8::try_from(record.capability_mask).map_err(|_| MojoError::InvalidOutput)?;
        let components = record.components;
        if components.iter().any(|value| *value < 0 || *value > 10_000) || record.weighted_total < 0
        {
            return Err(MojoError::InvalidOutput);
        }
        let input_index =
            usize::try_from(record.input_index).map_err(|_| MojoError::InvalidOutput)?;
        let duplicate_of = if record.duplicate_of < 0 {
            None
        } else {
            Some(usize::try_from(record.duplicate_of).map_err(|_| MojoError::InvalidOutput)?)
        };
        if input_index >= input_count
            || duplicate_of.is_some_and(|index| index >= input_count || index >= input_index)
        {
            return Err(MojoError::InvalidOutput);
        }
        candidates.push(RouteCandidate {
            provider,
            model,
            capability_mask,
            eligible: record.eligible == 1,
            reason: record.reason,
            score: record.score,
            components,
            weighted_total: u64::try_from(record.weighted_total)
                .map_err(|_| MojoError::InvalidOutput)?,
            input_index,
            duplicate_of,
            provider_order: record.provider_order,
        });
    }
    Ok(candidates)
}

fn decode_ordered_indices(
    ordered: &[i64],
    candidates: &[RouteCandidate],
) -> Result<Vec<usize>, MojoError> {
    let mut seen = vec![false; candidates.len()];
    let mut indices = Vec::with_capacity(ordered.len());
    for value in ordered {
        let index = usize::try_from(*value).map_err(|_| MojoError::InvalidOutput)?;
        if index >= candidates.len() || seen[index] || !candidates[index].eligible {
            return Err(MojoError::InvalidOutput);
        }
        seen[index] = true;
        indices.push(index);
    }
    Ok(indices)
}

fn decode_selected_index(
    selected_index: i64,
    ordered_indices: &[usize],
) -> Result<Option<usize>, MojoError> {
    if selected_index < 0 {
        return Ok(None);
    }
    let index = usize::try_from(selected_index).map_err(|_| MojoError::InvalidOutput)?;
    if ordered_indices.first() != Some(&index) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(Some(index))
}

pub fn plan_routes(
    inputs: &[RouteInput<'_>],
    required_capabilities: &str,
    weights: [i64; 7],
) -> Result<RoutePlan, MojoError> {
    ensure_rich_abi()?;
    let rich_inputs = ffi_route_inputs(inputs);
    let input_count = inputs.len();
    let capacity = inputs.len().max(1);
    let scratch_capacity = hash_capacity(capacity)?;
    let output_capacity = route_output_capacity(inputs)?;
    let mut records = vec![RichRouteRecord::default(); capacity];
    let mut ordered = vec![-1_i64; capacity];
    let mut output = vec![0_u8; output_capacity.max(1)];
    let mut hash_slots = vec![-1_i64; scratch_capacity];
    let mut result = RichRouteResult::default();
    let status = unsafe {
        prodex_mojo_rich_route_plan_v2(
            RICH_ABI_VERSION,
            rich_inputs.as_ptr(),
            i64::try_from(input_count).map_err(|_| MojoError::InvalidInput)?,
            view(required_capabilities),
            records.as_mut_ptr(),
            i64::try_from(input_count).map_err(|_| MojoError::InvalidInput)?,
            ordered.as_mut_ptr(),
            i64::try_from(input_count).map_err(|_| MojoError::InvalidInput)?,
            output.as_mut_ptr(),
            i64::try_from(output_capacity).map_err(|_| MojoError::InvalidInput)?,
            hash_slots.as_mut_ptr(),
            i64::try_from(scratch_capacity).map_err(|_| MojoError::InvalidInput)?,
            weights[0],
            weights[1],
            weights[2],
            weights[3],
            weights[4],
            weights[5],
            weights[6],
            &mut result,
        )
    };
    if status != 0 {
        return Err(status_error(
            status,
            2,
            result.issue_kind,
            result.issue_offset,
            result.issue_length,
        ));
    }
    if result.abi_version != RICH_ABI_VERSION
        || result.candidates_written != input_count as i64
        || result.ordered_written < 0
        || result.ordered_written as usize > inputs.len()
        || result.output_written < 0
        || result.output_written as usize > output_capacity
    {
        return Err(MojoError::InvalidOutput);
    }
    let output = &output[..result.output_written as usize];
    let candidates = decode_route_candidates(&records[..input_count], output, input_count)?;
    let ordered_indices =
        decode_ordered_indices(&ordered[..result.ordered_written as usize], &candidates)?;
    let selected_index = decode_selected_index(result.selected_index, &ordered_indices)?;
    Ok(RoutePlan {
        candidates,
        ordered_indices,
        selected_index,
    })
}
