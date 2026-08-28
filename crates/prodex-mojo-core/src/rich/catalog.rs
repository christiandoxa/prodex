use super::*;

const CATALOG_MAX_MODELS: usize = 1_024;
const CATALOG_MAX_INPUT_MODELS: usize = 65_536;
const CATALOG_MAX_IDENTIFIER_BYTES: usize = 4_096;

#[derive(Debug, Clone, Copy)]
pub struct CatalogModel<'a> {
    pub id: &'a str,
    pub aliases: &'a [&'a str],
}

#[derive(Debug, Clone, Copy)]
pub struct CatalogReasoningModel<'a> {
    pub id: &'a str,
    pub aliases: &'a [&'a str],
    pub efforts: &'a [&'a str],
    pub default_effort: Option<&'a str>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogReasoningPlan {
    pub model_index: Option<usize>,
    pub supported_efforts: Vec<String>,
    pub selected_effort: Option<String>,
    pub default_effort: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogChoice {
    ProviderDefault,
    Catalog(usize),
    Configured(usize),
    Current,
    Custom,
}

const CATALOG_CHOICE_PROVIDER_DEFAULT: i64 = 0;
const CATALOG_CHOICE_CATALOG: i64 = 1;
const CATALOG_CHOICE_CONFIGURED: i64 = 2;
const CATALOG_CHOICE_CURRENT: i64 = 3;
const CATALOG_CHOICE_CUSTOM: i64 = 4;

unsafe extern "C" {
    fn prodex_mojo_rich_catalog_resolve_v1(
        abi_version: i64,
        model_ids: u64,
        model_count: i64,
        aliases: u64,
        alias_models: u64,
        alias_count: i64,
        query: u64,
        output_index: u64,
    ) -> i64;
    fn prodex_mojo_rich_catalog_choices_v1(
        abi_version: i64,
        model_ids: u64,
        model_count: i64,
        aliases: u64,
        alias_models: u64,
        alias_count: i64,
        configured: u64,
        configured_count: i64,
        current: u64,
        current_present: i64,
        output_kinds: u64,
        output_indices: u64,
        output_capacity: i64,
        output_count: u64,
    ) -> i64;
    fn prodex_mojo_rich_catalog_merge_v1(
        abi_version: i64,
        model_ids: u64,
        model_count: i64,
        aliases: u64,
        alias_models: u64,
        alias_count: i64,
        additional: u64,
        additional_count: i64,
        accepted_indices: u64,
        output_capacity: i64,
        output_count: u64,
    ) -> i64;
    fn prodex_mojo_rich_catalog_reasoning_v1(
        abi_version: i64,
        model_ids: u64,
        model_count: i64,
        aliases: u64,
        alias_models: u64,
        alias_count: i64,
        efforts: u64,
        effort_models: u64,
        effort_count: i64,
        defaults: u64,
        requested_model: u64,
        requested_present: i64,
        fallback_model: u64,
        fallback_present: i64,
        requested_effort: u64,
        effort_present: i64,
        output_efforts: u64,
        effort_capacity: i64,
        output: u64,
        output_capacity: i64,
        result: u64,
    ) -> i64;
}

struct CatalogViews {
    model_ids: Vec<RichStringView>,
    aliases: Vec<RichStringView>,
    alias_models: Vec<i64>,
}

type ReasoningViews = (
    CatalogViews,
    Vec<RichStringView>,
    Vec<i64>,
    Vec<RichStringView>,
);

fn views(models: &[CatalogModel<'_>]) -> Result<CatalogViews, MojoError> {
    if models.len() > CATALOG_MAX_MODELS {
        return Err(MojoError::InvalidInput);
    }
    let alias_count = models
        .iter()
        .try_fold(0_usize, |count, model| {
            count.checked_add(model.aliases.len())
        })
        .ok_or(MojoError::InvalidInput)?;
    if alias_count > CATALOG_MAX_INPUT_MODELS {
        return Err(MojoError::InvalidInput);
    }
    let mut model_ids = Vec::with_capacity(models.len());
    let mut aliases = Vec::with_capacity(alias_count);
    let mut alias_models = Vec::with_capacity(alias_count);
    for (model_index, model) in models.iter().enumerate() {
        if model.id.len() > CATALOG_MAX_IDENTIFIER_BYTES
            || model
                .aliases
                .iter()
                .any(|alias| alias.len() > CATALOG_MAX_IDENTIFIER_BYTES)
        {
            return Err(MojoError::InvalidInput);
        }
        model_ids.push(view(model.id));
        for alias in model.aliases {
            aliases.push(view(alias));
            alias_models.push(i64::try_from(model_index).map_err(|_| MojoError::InvalidInput)?);
        }
    }
    Ok(CatalogViews {
        model_ids,
        aliases,
        alias_models,
    })
}

fn address<T>(values: &[T]) -> u64 {
    if values.is_empty() {
        0
    } else {
        mojo_pointer_address(values.as_ptr())
    }
}

fn status(status: i64) -> Result<(), MojoError> {
    if status == 0 {
        Ok(())
    } else {
        Err(status_error(status, 6, 0, 0, 0))
    }
}

fn count_address(value: &mut i64) -> u64 {
    mojo_mut_pointer_address(value)
}

fn reasoning_views(models: &[CatalogReasoningModel<'_>]) -> Result<ReasoningViews, MojoError> {
    let catalog = views(
        &models
            .iter()
            .map(|model| CatalogModel {
                id: model.id,
                aliases: model.aliases,
            })
            .collect::<Vec<_>>(),
    )?;
    let effort_count = models
        .iter()
        .try_fold(0_usize, |count, model| {
            count.checked_add(model.efforts.len())
        })
        .ok_or(MojoError::InvalidInput)?;
    if effort_count > CATALOG_MAX_INPUT_MODELS {
        return Err(MojoError::InvalidInput);
    }
    let mut efforts = Vec::with_capacity(effort_count);
    let mut effort_models = Vec::with_capacity(effort_count);
    let mut defaults = Vec::with_capacity(models.len());
    for (model_index, model) in models.iter().enumerate() {
        defaults.push(model.default_effort.map(view).unwrap_or_default());
        for effort in model.efforts {
            if effort.len() > CATALOG_MAX_IDENTIFIER_BYTES {
                return Err(MojoError::InvalidInput);
            }
            efforts.push(view(effort));
            effort_models.push(i64::try_from(model_index).map_err(|_| MojoError::InvalidInput)?);
        }
    }
    Ok((catalog, efforts, effort_models, defaults))
}

pub fn resolve_catalog_model(
    models: &[CatalogModel<'_>],
    model: &str,
) -> Result<Option<usize>, MojoError> {
    ensure_rich_abi()?;
    if model.len() > CATALOG_MAX_IDENTIFIER_BYTES {
        return Ok(None);
    }
    let views = views(models)?;
    let query = view(model);
    let mut output_index = -1_i64;
    status(unsafe {
        prodex_mojo_rich_catalog_resolve_v1(
            RICH_ABI_VERSION,
            address(&views.model_ids),
            i64::try_from(views.model_ids.len()).map_err(|_| MojoError::InvalidInput)?,
            address(&views.aliases),
            address(&views.alias_models),
            i64::try_from(views.aliases.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_pointer_address(&query),
            count_address(&mut output_index),
        )
    })?;
    match output_index {
        -1 => Ok(None),
        index if index >= 0 => usize::try_from(index)
            .ok()
            .filter(|index| *index < models.len())
            .map(Some)
            .ok_or(MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn plan_catalog_choices(
    models: &[CatalogModel<'_>],
    configured_models: &[&str],
    current_model: Option<&str>,
) -> Result<Vec<CatalogChoice>, MojoError> {
    ensure_rich_abi()?;
    if configured_models.len() > CATALOG_MAX_INPUT_MODELS {
        return Err(MojoError::InvalidInput);
    }
    let views = views(models)?;
    let configured = configured_models
        .iter()
        .map(|model| view(model))
        .collect::<Vec<_>>();
    let current_model = current_model.filter(|model| !model.trim().is_empty());
    let current = current_model.map(view).unwrap_or_default();
    let mut kinds = vec![0_i64; CATALOG_MAX_MODELS + 2];
    let mut indices = vec![-1_i64; CATALOG_MAX_MODELS + 2];
    let mut output_count = 0_i64;
    status(unsafe {
        prodex_mojo_rich_catalog_choices_v1(
            RICH_ABI_VERSION,
            address(&views.model_ids),
            i64::try_from(views.model_ids.len()).map_err(|_| MojoError::InvalidInput)?,
            address(&views.aliases),
            address(&views.alias_models),
            i64::try_from(views.aliases.len()).map_err(|_| MojoError::InvalidInput)?,
            address(&configured),
            i64::try_from(configured.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_pointer_address(&current),
            i64::from(current_model.is_some()),
            mojo_mut_pointer_address(kinds.as_mut_ptr()),
            mojo_mut_pointer_address(indices.as_mut_ptr()),
            i64::try_from(kinds.len()).map_err(|_| MojoError::InvalidInput)?,
            count_address(&mut output_count),
        )
    })?;
    let output_count = usize::try_from(output_count).map_err(|_| MojoError::InvalidOutput)?;
    if output_count > kinds.len() {
        return Err(MojoError::InvalidOutput);
    }
    kinds[..output_count]
        .iter()
        .zip(&indices[..output_count])
        .map(|(&kind, &index)| match kind {
            CATALOG_CHOICE_PROVIDER_DEFAULT if index == -1 => Ok(CatalogChoice::ProviderDefault),
            CATALOG_CHOICE_CATALOG => usize::try_from(index)
                .ok()
                .filter(|index| *index < models.len())
                .map(CatalogChoice::Catalog)
                .ok_or(MojoError::InvalidOutput),
            CATALOG_CHOICE_CONFIGURED => usize::try_from(index)
                .ok()
                .filter(|index| *index < configured_models.len())
                .map(CatalogChoice::Configured)
                .ok_or(MojoError::InvalidOutput),
            CATALOG_CHOICE_CURRENT if index == -1 => Ok(CatalogChoice::Current),
            CATALOG_CHOICE_CUSTOM if index == -1 => Ok(CatalogChoice::Custom),
            _ => Err(MojoError::InvalidOutput),
        })
        .collect()
}

pub fn merge_catalog_ids(
    models: &[CatalogModel<'_>],
    additional_ids: &[&str],
) -> Result<Vec<usize>, MojoError> {
    ensure_rich_abi()?;
    if additional_ids.len() > CATALOG_MAX_INPUT_MODELS {
        return Err(MojoError::InvalidInput);
    }
    let views = views(models)?;
    let additional = additional_ids
        .iter()
        .map(|model| view(model))
        .collect::<Vec<_>>();
    let mut accepted = vec![-1_i64; additional.len().max(1)];
    let mut output_count = 0_i64;
    status(unsafe {
        prodex_mojo_rich_catalog_merge_v1(
            RICH_ABI_VERSION,
            address(&views.model_ids),
            i64::try_from(views.model_ids.len()).map_err(|_| MojoError::InvalidInput)?,
            address(&views.aliases),
            address(&views.alias_models),
            i64::try_from(views.aliases.len()).map_err(|_| MojoError::InvalidInput)?,
            address(&additional),
            i64::try_from(additional.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(accepted.as_mut_ptr()),
            i64::try_from(additional.len()).map_err(|_| MojoError::InvalidInput)?,
            count_address(&mut output_count),
        )
    })?;
    let output_count = usize::try_from(output_count).map_err(|_| MojoError::InvalidOutput)?;
    if output_count > additional_ids.len() {
        return Err(MojoError::InvalidOutput);
    }
    accepted[..output_count]
        .iter()
        .map(|index| {
            usize::try_from(*index)
                .ok()
                .filter(|index| *index < additional_ids.len())
                .ok_or(MojoError::InvalidOutput)
        })
        .collect()
}

pub fn resolve_catalog_reasoning(
    models: &[CatalogReasoningModel<'_>],
    requested_model: Option<&str>,
    fallback_model: Option<&str>,
    requested_effort: Option<&str>,
) -> Result<CatalogReasoningPlan, MojoError> {
    ensure_rich_abi()?;
    let (views, efforts, effort_models, defaults) = reasoning_views(models)?;
    let output_capacity = effort_models.len().max(1);
    let output_bytes = models
        .iter()
        .filter_map(|model| model.default_effort)
        .chain(
            models
                .iter()
                .flat_map(|model| model.efforts.iter().copied()),
        )
        .try_fold(0_usize, |total, effort| total.checked_add(effort.len()))
        .ok_or(MojoError::InvalidInput)?
        .max(1);
    let requested_model = requested_model.map(view);
    let fallback_model = fallback_model.map(view);
    let requested_effort = requested_effort.map(view);
    let mut output_efforts = vec![RichSlice::default(); output_capacity];
    let mut output = vec![0_u8; output_bytes];
    let mut result = RichCatalogReasoningResult::default();
    let status = unsafe {
        prodex_mojo_rich_catalog_reasoning_v1(
            RICH_ABI_VERSION,
            address(&views.model_ids),
            i64::try_from(views.model_ids.len()).map_err(|_| MojoError::InvalidInput)?,
            address(&views.aliases),
            address(&views.alias_models),
            i64::try_from(views.aliases.len()).map_err(|_| MojoError::InvalidInput)?,
            address(&efforts),
            address(&effort_models),
            i64::try_from(efforts.len()).map_err(|_| MojoError::InvalidInput)?,
            address(&defaults),
            requested_model
                .as_ref()
                .map_or(0, |value| mojo_pointer_address(value as *const _)),
            i64::from(requested_model.is_some()),
            fallback_model
                .as_ref()
                .map_or(0, |value| mojo_pointer_address(value as *const _)),
            i64::from(fallback_model.is_some()),
            requested_effort
                .as_ref()
                .map_or(0, |value| mojo_pointer_address(value as *const _)),
            i64::from(requested_effort.is_some()),
            mojo_mut_pointer_address(output_efforts.as_mut_ptr()),
            i64::try_from(output_efforts.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut result),
        )
    };
    if status != 0 {
        return Err(status_error(status, 6, 0, 0, 0));
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
    if result.abi_version != RICH_ABI_VERSION
        || result.efforts_written < 0
        || result.efforts_written as usize > output_efforts.len()
        || result.output_written < 0
        || result.output_written as usize > output.len()
    {
        return Err(MojoError::InvalidOutput);
    }
    let model_index = match result.model_index {
        -1 => None,
        index if index >= 0 => Some(
            usize::try_from(index)
                .ok()
                .filter(|index| *index < models.len())
                .ok_or(MojoError::InvalidOutput)?,
        ),
        _ => return Err(MojoError::InvalidOutput),
    };
    let output = &output[..result.output_written as usize];
    let supported_efforts = output_efforts[..result.efforts_written as usize]
        .iter()
        .map(|effort| {
            Ok(std::str::from_utf8(slice(output, *effort)?)
                .map_err(|_| MojoError::InvalidOutput)?
                .to_string())
        })
        .collect::<Result<Vec<_>, MojoError>>()?;
    let read_slice = |value: RichSlice| -> Result<Option<String>, MojoError> {
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
    };
    Ok(CatalogReasoningPlan {
        model_index,
        supported_efforts,
        selected_effort: read_slice(result.selected_effort)?,
        default_effort: read_slice(result.default_effort)?,
    })
}
