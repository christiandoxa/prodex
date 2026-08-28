use super::*;

#[derive(Debug, Clone, Copy)]
pub struct PolicyAliasInput<'a> {
    pub alias: &'a str,
    pub models: &'a [&'a str],
    pub strategy: Option<&'a str>,
    pub metrics: &'a [&'a str],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyModel {
    pub model: String,
    pub model_index: usize,
    pub metric_match: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyAliasPlan {
    pub models: Vec<PolicyModel>,
}

#[derive(Debug, Clone, Copy)]
pub struct PolicyRouteModel<'a> {
    pub model: &'a str,
    pub input_cost: Option<u64>,
    pub output_cost: Option<u64>,
    pub policy_latency: Option<u64>,
    pub state_latency: Option<u64>,
    pub in_flight: u64,
    pub rpm_limit: Option<u64>,
    pub rpm_used: u64,
    pub tpm_limit: Option<u64>,
    pub tpm_used: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolicyRoutePlan {
    pub selected_index: Option<usize>,
    pub ordered_indices: Vec<usize>,
}

unsafe extern "C" {
    fn prodex_mojo_rich_policy_route_v1(
        abi_version: i64,
        strategy: u64,
        request_id: u64,
        estimated_tokens: u64,
        inputs: u64,
        input_count: i64,
        ordered_indices: u64,
        ordered_capacity: i64,
        result: u64,
    ) -> i64;
}

pub fn validate_policy_alias(input: PolicyAliasInput<'_>) -> Result<PolicyAliasPlan, MojoError> {
    ensure_rich_abi()?;
    let models = input
        .models
        .iter()
        .map(|value| view(value))
        .collect::<Vec<_>>();
    let metrics = input
        .metrics
        .iter()
        .map(|value| view(value))
        .collect::<Vec<_>>();
    let mut output = vec![
        0_u8;
        input
            .models
            .iter()
            .map(|value| value.len())
            .sum::<usize>()
            .max(1)
    ];
    let mut output_models = vec![RichPolicyModel::default(); input.models.len().max(1)];
    let mut result = RichPolicyResult::default();
    let rich_input = RichPolicyInput {
        alias_view: view(input.alias),
        models: mojo_pointer_address(models.as_ptr()),
        model_count: i64::try_from(models.len()).map_err(|_| MojoError::InvalidInput)?,
        strategy: input.strategy.map(view).unwrap_or_default(),
        metrics: mojo_pointer_address(metrics.as_ptr()),
        metric_count: i64::try_from(metrics.len()).map_err(|_| MojoError::InvalidInput)?,
    };
    let status = unsafe {
        prodex_mojo_rich_policy_alias_v2(
            RICH_ABI_VERSION,
            mojo_pointer_address(&rich_input),
            mojo_pointer_address(output_models.as_mut_ptr()),
            i64::try_from(input.models.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut result),
        )
    };
    if status != 0 {
        return Err(status_error(
            status,
            3,
            result.issue_kind,
            result.issue_offset,
            result.issue_length,
        ));
    }
    if result.issue_kind != 0 {
        return Err(issue(
            3,
            result.issue_kind,
            result.issue_field,
            result.issue_index,
            result.issue_offset,
            result.issue_length,
        ));
    }
    if result.models_written != input.models.len() as i64
        || result.output_written < 0
        || result.output_written as usize > output.len()
    {
        return Err(MojoError::InvalidOutput);
    }
    let output = &output[..result.output_written as usize];
    let mut planned = Vec::with_capacity(input.models.len());
    for record in &output_models[..input.models.len()] {
        let model = std::str::from_utf8(slice(output, record.model)?)
            .map_err(|_| MojoError::InvalidOutput)?
            .to_string();
        let model_index =
            usize::try_from(record.model_index).map_err(|_| MojoError::InvalidOutput)?;
        let metric_match = if record.metric_match < 0 {
            None
        } else {
            Some(usize::try_from(record.metric_match).map_err(|_| MojoError::InvalidOutput)?)
        };
        if model_index >= input.models.len()
            || metric_match.is_some_and(|index| index >= input.metrics.len())
        {
            return Err(MojoError::InvalidOutput);
        }
        planned.push(PolicyModel {
            model,
            model_index,
            metric_match,
        });
    }
    Ok(PolicyAliasPlan { models: planned })
}

pub fn plan_route_policy(
    strategy: &str,
    request_id: u64,
    estimated_tokens: u64,
    models: &[PolicyRouteModel<'_>],
) -> Result<PolicyRoutePlan, MojoError> {
    ensure_rich_abi()?;
    if models.len() > 256 {
        return Err(MojoError::InvalidInput);
    }
    let inputs = models
        .iter()
        .map(|model| RichPolicyRouteInput {
            model: view(model.model),
            input_cost: model.input_cost.unwrap_or_default(),
            input_cost_present: i64::from(model.input_cost.is_some()),
            output_cost: model.output_cost.unwrap_or_default(),
            output_cost_present: i64::from(model.output_cost.is_some()),
            policy_latency: model.policy_latency.unwrap_or_default(),
            policy_latency_present: i64::from(model.policy_latency.is_some()),
            state_latency: model.state_latency.unwrap_or_default(),
            state_latency_present: i64::from(model.state_latency.is_some()),
            in_flight: model.in_flight,
            rpm_limit: model.rpm_limit.unwrap_or_default(),
            rpm_limit_present: i64::from(model.rpm_limit.is_some()),
            rpm_used: model.rpm_used,
            tpm_limit: model.tpm_limit.unwrap_or_default(),
            tpm_limit_present: i64::from(model.tpm_limit.is_some()),
            tpm_used: model.tpm_used,
        })
        .collect::<Vec<_>>();
    let mut ordered_indices = vec![-1_i64; models.len().max(1)];
    let mut result = RichPolicyRouteResult::default();
    let strategy = view(strategy);
    let status = unsafe {
        prodex_mojo_rich_policy_route_v1(
            RICH_ABI_VERSION,
            mojo_pointer_address(&strategy),
            request_id,
            estimated_tokens,
            mojo_pointer_address(inputs.as_ptr()),
            i64::try_from(inputs.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(ordered_indices.as_mut_ptr()),
            i64::try_from(ordered_indices.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut result),
        )
    };
    if status != 0 {
        return Err(status_error(status, 7, 0, 0, 0));
    }
    if result.issue_kind != 0 {
        return Err(issue(
            7,
            result.issue_kind,
            0,
            result.issue_index,
            result.issue_offset,
            result.issue_length,
        ));
    }
    if result.abi_version != RICH_ABI_VERSION
        || result.ordered_written < 0
        || result.ordered_written as usize > ordered_indices.len()
        || result.required_ordered != result.ordered_written
    {
        return Err(MojoError::InvalidOutput);
    }
    let selected_index = match result.selected_index {
        -1 => None,
        index if index >= 0 => Some(
            usize::try_from(index)
                .ok()
                .filter(|index| *index < models.len())
                .ok_or(MojoError::InvalidOutput)?,
        ),
        _ => return Err(MojoError::InvalidOutput),
    };
    let mut seen = vec![false; models.len()];
    let ordered_indices = ordered_indices[..result.ordered_written as usize]
        .iter()
        .map(|index| {
            let index = usize::try_from(*index).map_err(|_| MojoError::InvalidOutput)?;
            if index >= models.len() || seen[index] {
                return Err(MojoError::InvalidOutput);
            }
            seen[index] = true;
            Ok(index)
        })
        .collect::<Result<Vec<_>, MojoError>>()?;
    if selected_index.is_some_and(|index| !seen[index]) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(PolicyRoutePlan {
        selected_index,
        ordered_indices,
    })
}
