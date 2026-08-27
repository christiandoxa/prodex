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
