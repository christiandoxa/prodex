use super::catalog::{
    CATALOG_CHOICE_CATALOG, CATALOG_CHOICE_CUSTOM, CATALOG_CHOICE_PROVIDER_DEFAULT,
    CATALOG_MAX_IDENTIFIER_BYTES, CATALOG_MAX_INPUT_MODELS, CATALOG_MAX_MODELS,
};
use super::*;

const CATALOG_MAX_QUERY_BYTES: usize = 65_536;

fn plan_role(role: CatalogPlanRole) -> i64 {
    match role {
        CatalogPlanRole::Main => 0,
        CatalogPlanRole::SubAgent => 1,
    }
}

type CatalogPlanViews = (
    Vec<RichCatalogPlanModel>,
    Vec<RichStringView>,
    Vec<RichStringView>,
);

fn plan_models(models: &[CatalogPlanModel<'_>]) -> Result<CatalogPlanViews, MojoError> {
    if models.len() > CATALOG_MAX_MODELS {
        return Err(MojoError::InvalidInput);
    }
    let (effort_count, alias_count) = models
        .iter()
        .try_fold((0_usize, 0_usize), |(effort_count, alias_count), model| {
            effort_count
                .checked_add(model.efforts.len())
                .and_then(|effort_count| {
                    alias_count
                        .checked_add(model.aliases.len())
                        .map(|alias_count| (effort_count, alias_count))
                })
        })
        .ok_or(MojoError::InvalidInput)?;
    if effort_count > CATALOG_MAX_INPUT_MODELS || alias_count > CATALOG_MAX_INPUT_MODELS {
        return Err(MojoError::InvalidInput);
    }
    let mut efforts = Vec::with_capacity(effort_count);
    let mut aliases = Vec::with_capacity(alias_count);
    let mut rich_models = Vec::with_capacity(models.len());
    for model in models {
        if model.id.len() > CATALOG_MAX_IDENTIFIER_BYTES
            || model.label.len() > CATALOG_MAX_QUERY_BYTES
            || model
                .default_effort
                .is_some_and(|effort| effort.len() > CATALOG_MAX_QUERY_BYTES)
            || model
                .efforts
                .iter()
                .any(|effort| effort.len() > CATALOG_MAX_QUERY_BYTES)
            || model
                .aliases
                .iter()
                .any(|alias| alias.len() > CATALOG_MAX_IDENTIFIER_BYTES)
        {
            return Err(MojoError::InvalidInput);
        }
        let effort_start = i64::try_from(efforts.len()).map_err(|_| MojoError::InvalidInput)?;
        efforts.extend(model.efforts.iter().copied().map(view));
        let alias_start = i64::try_from(aliases.len()).map_err(|_| MojoError::InvalidInput)?;
        aliases.extend(model.aliases.iter().copied().map(view));
        let flags = i64::from(model.supported)
            | (i64::from(model.hidden) << 1)
            | (i64::from(!model.listed) << 2);
        rich_models.push(RichCatalogPlanModel {
            id: view(model.id),
            label: view(model.label),
            default_effort: model.default_effort.map(view).unwrap_or_default(),
            priority: i64::try_from(model.priority).map_err(|_| MojoError::InvalidInput)?,
            flags,
            effort_start,
            effort_count: i64::try_from(model.efforts.len())
                .map_err(|_| MojoError::InvalidInput)?,
            alias_start,
            alias_count: i64::try_from(model.aliases.len()).map_err(|_| MojoError::InvalidInput)?,
        });
    }
    Ok((rich_models, efforts, aliases))
}

fn checked_output_bytes(models: &[CatalogPlanModel<'_>]) -> Result<usize, MojoError> {
    models.iter().try_fold(1_usize, |total, model| {
        total
            .checked_add(model.id.len())
            .and_then(|total| total.checked_add(model.label.len()))
            .and_then(|total| {
                model
                    .default_effort
                    .map_or(Some(total), |effort| total.checked_add(effort.len()))
            })
            .and_then(|total| {
                model
                    .efforts
                    .iter()
                    .try_fold(total, |total, effort| total.checked_add(effort.len()))
            })
            .ok_or(MojoError::InvalidInput)
    })
}

fn checked_slice_text(output: &[u8], value: RichSlice) -> Result<Option<String>, MojoError> {
    if value.offset < 0 {
        if value.len != 0 {
            return Err(MojoError::InvalidOutput);
        }
        return Ok(None);
    }
    Ok(Some(
        std::str::from_utf8(slice(output, value)?)
            .map_err(|_| MojoError::InvalidOutput)?
            .to_string(),
    ))
}

/// Filters, orders, deduplicates, and returns a dynamic production catalog.
pub fn plan_dynamic_catalog(
    models: &[CatalogPlanModel<'_>],
) -> Result<CatalogChoicesPlan, MojoError> {
    ensure_rich_abi()?;
    if models.len() > CATALOG_MAX_MODELS {
        return Err(MojoError::InvalidInput);
    }
    let (rich_models, efforts, aliases) = plan_models(models)?;
    let choice_capacity = models.len().checked_add(2).ok_or(MojoError::InvalidInput)?;
    let effort_capacity = efforts.len();
    let mut choices = vec![RichCatalogPlanChoice::default(); choice_capacity];
    let mut output_ids = vec![RichSlice::default(); choice_capacity];
    let mut output_labels = vec![RichSlice::default(); choice_capacity];
    let mut output_efforts = vec![RichSlice::default(); effort_capacity.max(1)];
    let mut output = vec![0_u8; checked_output_bytes(models)?];
    let mut result = RichCatalogPlanResult::default();
    let status = unsafe {
        prodex_mojo_rich_catalog_choices_v2(
            RICH_ABI_VERSION,
            address(&rich_models),
            i64::try_from(rich_models.len()).map_err(|_| MojoError::InvalidInput)?,
            address(&efforts),
            i64::try_from(efforts.len()).map_err(|_| MojoError::InvalidInput)?,
            address(&aliases),
            i64::try_from(aliases.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(choices.as_mut_ptr()),
            mojo_mut_pointer_address(output_ids.as_mut_ptr()),
            mojo_mut_pointer_address(output_labels.as_mut_ptr()),
            i64::try_from(choice_capacity).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(output_efforts.as_mut_ptr()),
            i64::try_from(effort_capacity).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut result),
        )
    };
    if status != 0 {
        return Err(status_error(status, 6, result.issue_kind, 0, 0));
    }
    validate_catalog_plan_result(&result, choices.len(), output_efforts.len(), output.len())?;
    let choice_count = result.choices_written as usize;
    let output = &output[..result.output_written as usize];
    let output_efforts = &output_efforts[..result.efforts_written as usize];
    validate_catalog_choice_sentinels(&choices, choice_count)?;
    let planned = (1..choice_count - 1)
        .map(|index| {
            planned_catalog_model(
                index,
                models,
                &choices,
                &output_ids,
                &output_labels,
                output_efforts,
                output,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CatalogChoicesPlan { models: planned })
}

fn validate_catalog_plan_result(
    result: &RichCatalogPlanResult,
    choice_capacity: usize,
    effort_capacity: usize,
    output_capacity: usize,
) -> Result<(), MojoError> {
    if result.abi_version != RICH_ABI_VERSION
        || result.choices_written < 2
        || result.choices_written as usize > choice_capacity
        || result.required_choices != result.choices_written
        || result.efforts_written < 0
        || result.efforts_written as usize > effort_capacity
        || result.required_efforts != result.efforts_written
        || result.output_written < 0
        || result.output_written as usize > output_capacity
        || result.required_output != result.output_written
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(())
}

fn validate_catalog_choice_sentinels(
    choices: &[RichCatalogPlanChoice],
    choice_count: usize,
) -> Result<(), MojoError> {
    if choices[0].kind != CATALOG_CHOICE_PROVIDER_DEFAULT
        || choices[0].index != -1
        || choices[choice_count - 1].kind != CATALOG_CHOICE_CUSTOM
        || choices[choice_count - 1].index != -1
    {
        return Err(MojoError::InvalidOutput);
    }
    Ok(())
}

fn planned_catalog_model(
    index: usize,
    models: &[CatalogPlanModel<'_>],
    choices: &[RichCatalogPlanChoice],
    output_ids: &[RichSlice],
    output_labels: &[RichSlice],
    output_efforts: &[RichSlice],
    output: &[u8],
) -> Result<CatalogPlannedModel, MojoError> {
    let choice = choices[index];
    if choice.kind != CATALOG_CHOICE_CATALOG {
        return Err(MojoError::InvalidOutput);
    }
    let model_index = usize::try_from(choice.index)
        .ok()
        .filter(|index| *index < models.len())
        .ok_or(MojoError::InvalidOutput)?;
    let id = checked_slice_text(output, output_ids[index])?.ok_or(MojoError::InvalidOutput)?;
    if id != models[model_index].id.trim() {
        return Err(MojoError::InvalidOutput);
    }
    let label =
        checked_slice_text(output, output_labels[index])?.ok_or(MojoError::InvalidOutput)?;
    if label != models[model_index].label.trim() {
        return Err(MojoError::InvalidOutput);
    }
    let effort_count =
        usize::try_from(choice.effort_count).map_err(|_| MojoError::InvalidOutput)?;
    let efforts = catalog_model_efforts(choice, effort_count, output_efforts, output)?;
    Ok(CatalogPlannedModel { id, label, efforts })
}

fn catalog_model_efforts(
    choice: RichCatalogPlanChoice,
    effort_count: usize,
    output_efforts: &[RichSlice],
    output: &[u8],
) -> Result<Vec<String>, MojoError> {
    if choice.effort_start < 0 {
        if effort_count != 0 {
            return Err(MojoError::InvalidOutput);
        }
        return Ok(Vec::new());
    }
    let effort_start =
        usize::try_from(choice.effort_start).map_err(|_| MojoError::InvalidOutput)?;
    let effort_end = effort_start
        .checked_add(effort_count)
        .ok_or(MojoError::InvalidOutput)?;
    if effort_end > output_efforts.len() {
        return Err(MojoError::InvalidOutput);
    }
    output_efforts[effort_start..effort_end]
        .iter()
        .map(|effort| checked_slice_text(output, *effort)?.ok_or(MojoError::InvalidOutput))
        .collect()
}

/// Resolves model/effort precedence for main or sub-agent launches.
pub fn plan_catalog_configuration(
    input: CatalogConfigurationInput<'_>,
) -> Result<CatalogConfigurationPlan, MojoError> {
    ensure_rich_abi()?;
    if input.models.len() > CATALOG_MAX_MODELS
        || input.fallback_efforts.len() > CATALOG_MAX_INPUT_MODELS
    {
        return Err(MojoError::InvalidInput);
    }
    if input
        .fallback_efforts
        .iter()
        .any(|effort| effort.len() > CATALOG_MAX_QUERY_BYTES)
        || [
            input.current,
            input.provider_default,
            input.catalog_default,
            input.explicit_model,
            input.remembered_model,
            input.explicit_effort,
            input.remembered_effort,
        ]
        .into_iter()
        .flatten()
        .any(|value| value.len() > CATALOG_MAX_QUERY_BYTES)
    {
        return Err(MojoError::InvalidInput);
    }
    let (models, efforts, aliases) = plan_models(input.models)?;
    let fallback_efforts = input
        .fallback_efforts
        .iter()
        .copied()
        .map(view)
        .collect::<Vec<_>>();
    let optional_output = checked_text_bytes(
        [
            input.current,
            input.provider_default,
            input.catalog_default,
            input.explicit_model,
            input.remembered_model,
            input.explicit_effort,
            input.remembered_effort,
        ]
        .into_iter()
        .flatten(),
    )?;
    let fallback_output = checked_text_bytes(input.fallback_efforts.iter().copied())?;
    let output_bytes = checked_output_bytes(input.models)?
        .checked_add(optional_output)
        .and_then(|total| total.checked_add(fallback_output))
        .ok_or(MojoError::InvalidInput)?;
    let mut output = vec![0_u8; output_bytes];
    let mut result = RichCatalogPlanResult::default();
    let optional = |value: Option<&str>| value.map(view).unwrap_or_default();
    let status = unsafe {
        prodex_mojo_rich_catalog_config_v1(
            RICH_ABI_VERSION,
            plan_role(input.role),
            address(&models),
            i64::try_from(models.len()).map_err(|_| MojoError::InvalidInput)?,
            address(&efforts),
            i64::try_from(efforts.len()).map_err(|_| MojoError::InvalidInput)?,
            address(&aliases),
            i64::try_from(aliases.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_pointer_address(&optional(input.current)),
            i64::from(input.current.is_some()),
            mojo_pointer_address(&optional(input.provider_default)),
            i64::from(input.provider_default.is_some()),
            mojo_pointer_address(&optional(input.catalog_default)),
            i64::from(input.catalog_default.is_some()),
            mojo_pointer_address(&optional(input.explicit_model)),
            i64::from(input.explicit_model.is_some()),
            mojo_pointer_address(&optional(input.remembered_model)),
            i64::from(input.remembered_model.is_some()),
            mojo_pointer_address(&optional(input.explicit_effort)),
            i64::from(input.explicit_effort.is_some()),
            mojo_pointer_address(&optional(input.remembered_effort)),
            i64::from(input.remembered_effort.is_some()),
            address(&fallback_efforts),
            i64::try_from(fallback_efforts.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut result),
        )
    };
    if status != 0 {
        return Err(status_error(status, 6, result.issue_kind, 0, 0));
    }
    if result.abi_version != RICH_ABI_VERSION
        || result.output_written < 0
        || result.output_written as usize > output.len()
        || result.required_output != result.output_written
    {
        return Err(MojoError::InvalidOutput);
    }
    if result.issue_kind != 0 {
        return Err(issue(
            6,
            result.issue_kind,
            0,
            result.issue_index,
            result.issue_offset,
            result.issue_length,
        ));
    }
    let output = &output[..result.output_written as usize];
    Ok(CatalogConfigurationPlan {
        selected_model: checked_slice_text(output, result.selected_model)?,
        selected_effort: checked_slice_text(output, result.selected_effort)?,
        default_effort: checked_slice_text(output, result.default_effort)?,
    })
}

/// Selects which launch configuration semantics the planner applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogPlanRole {
    Main,
    SubAgent,
}

/// Borrowed catalog metadata passed to one coarse Mojo planning call.
#[derive(Debug, Clone, Copy)]
pub struct CatalogPlanModel<'a> {
    pub id: &'a str,
    pub aliases: &'a [&'a str],
    pub label: &'a str,
    pub priority: u64,
    pub supported: bool,
    pub hidden: bool,
    pub listed: bool,
    pub efforts: &'a [&'a str],
    pub default_effort: Option<&'a str>,
}

/// Ordered, filtered catalog models returned by Mojo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogPlannedModel {
    pub id: String,
    pub label: String,
    pub efforts: Vec<String>,
}

/// Validated dynamic catalog choice plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogChoicesPlan {
    pub models: Vec<CatalogPlannedModel>,
}

/// Inputs for remembered/default model and effort resolution.
#[derive(Debug, Clone, Copy)]
pub struct CatalogConfigurationInput<'a> {
    pub role: CatalogPlanRole,
    pub models: &'a [CatalogPlanModel<'a>],
    pub current: Option<&'a str>,
    pub provider_default: Option<&'a str>,
    pub catalog_default: Option<&'a str>,
    pub explicit_model: Option<&'a str>,
    pub remembered_model: Option<&'a str>,
    pub explicit_effort: Option<&'a str>,
    pub remembered_effort: Option<&'a str>,
    pub fallback_efforts: &'a [&'a str],
}

/// Resolved model and reasoning-effort values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogConfigurationPlan {
    pub selected_model: Option<String>,
    pub selected_effort: Option<String>,
    pub default_effort: Option<String>,
}

fn checked_text_bytes<'a>(values: impl IntoIterator<Item = &'a str>) -> Result<usize, MojoError> {
    values.into_iter().try_fold(0_usize, |total, value| {
        total
            .checked_add(value.len())
            .ok_or(MojoError::InvalidInput)
    })
}
unsafe extern "C" {
    fn prodex_mojo_rich_catalog_choices_v2(
        abi_version: i64,
        models: u64,
        model_count: i64,
        efforts: u64,
        effort_count: i64,
        aliases: u64,
        alias_count: i64,
        output_choices: u64,
        output_ids: u64,
        output_labels: u64,
        choice_capacity: i64,
        output_efforts: u64,
        effort_capacity: i64,
        output: u64,
        output_capacity: i64,
        result: u64,
    ) -> i64;
    fn prodex_mojo_rich_catalog_config_v1(
        abi_version: i64,
        role: i64,
        models: u64,
        model_count: i64,
        efforts: u64,
        effort_count: i64,
        aliases: u64,
        alias_count: i64,
        current: u64,
        current_present: i64,
        provider_default: u64,
        provider_default_present: i64,
        catalog_default: u64,
        catalog_default_present: i64,
        explicit_model: u64,
        explicit_model_present: i64,
        remembered_model: u64,
        remembered_model_present: i64,
        explicit_effort: u64,
        explicit_effort_present: i64,
        remembered_effort: u64,
        remembered_effort_present: i64,
        fallback_efforts: u64,
        fallback_effort_count: i64,
        output: u64,
        output_capacity: i64,
        result: u64,
    ) -> i64;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dynamic_catalog_accepts_models_without_efforts() {
        let plan = plan_dynamic_catalog(&[CatalogPlanModel {
            id: "model",
            aliases: &[],
            label: "Model",
            priority: 0,
            supported: true,
            hidden: false,
            listed: true,
            efforts: &[],
            default_effort: None,
        }])
        .unwrap();

        assert_eq!(
            plan.models,
            [CatalogPlannedModel {
                id: "model".to_string(),
                label: "Model".to_string(),
                efforts: Vec::new(),
            }]
        );
    }

    #[test]
    fn configuration_matches_trimmed_catalog_ids() {
        let plan = plan_catalog_configuration(CatalogConfigurationInput {
            role: CatalogPlanRole::Main,
            models: &[CatalogPlanModel {
                id: " model ",
                aliases: &[],
                label: "Model",
                priority: 0,
                supported: true,
                hidden: false,
                listed: true,
                efforts: &["medium"],
                default_effort: None,
            }],
            current: None,
            provider_default: Some("model"),
            catalog_default: None,
            explicit_model: None,
            remembered_model: None,
            explicit_effort: None,
            remembered_effort: None,
            fallback_efforts: &[],
        })
        .unwrap();

        assert_eq!(plan.selected_model.as_deref(), Some("model"));
        assert_eq!(plan.selected_effort.as_deref(), Some("medium"));
    }
}
