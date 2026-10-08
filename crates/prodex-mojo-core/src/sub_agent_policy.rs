use crate::MojoError;

mod rendering;
pub use rendering::{
    SubAgentDryRunRender, SubAgentOverlayRender, redact_session_argument,
    render_disabled_dry_run_report, render_enabled_dry_run_report, render_overlay,
    render_slot_lock_name,
};

const ABI_VERSION: i64 = 1;

#[repr(i64)]
#[derive(Clone, Copy)]
enum Operation {
    ConcurrencyParse = 0,
    ConcurrencyValidate = 1,
    ReasoningEffort = 2,
    ModelNonempty = 3,
    ProviderUrlPolicy = 4,
    ChildSpecScalarPolicy = 5,
    PromptSteps = 6,
    CatalogStatus = 7,
    EffortSuggestionMask = 8,
    RecursionDecision = 9,
    SlotPlanStep = 10,
    SlotLockErrorAction = 11,
    ChildOutcome = 12,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderUrlViolation {
    LocalRequiresUrl,
    NonLocalRejectsUrl,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildSpecScalarViolation {
    InvalidRecursionMarker,
    InvalidTaskSize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogStatusPlan {
    NoDynamicCatalog,
    Available,
    Degraded,
}

/// The normalized model id and visibility decision for one dynamic catalog entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogEntryPlan {
    /// Selected id field and its trimmed UTF-8 byte range in that field.
    pub model_id: Option<(usize, usize, usize)>,
    /// Whether the Rust JSON adapter should populate its canonical `id` field.
    pub set_id: bool,
    /// Whether the model should be offered to the caller.
    pub selectable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecursionDecision {
    Allowed,
    Disabled,
    InternalLauncher,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotPlanStep {
    Candidate { index: u16 },
    Retire { index: u16 },
    Ensure { index: u16 },
    LimitReached { exit_code: i32 },
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotLockErrorAction {
    TryNext,
    BlockResize,
    Propagate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildOutcomeAction {
    Success,
    Cancelled {
        exit_code: i32,
        output_incomplete: bool,
    },
    ChildFailed {
        output_incomplete: bool,
    },
    OutputIncomplete,
    NoOutput,
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
    fn prodex_sub_agent_child_argv_plan_v1(
        abi_version: i64,
        provider_class: i64,
        presidio_enabled: i64,
        tool_count: i64,
        model_present: i64,
        effort_present: i64,
        actions_address: u64,
        action_capacity: i64,
        written_address: u64,
    ) -> i64;
    fn prodex_sub_agent_catalog_entry_v1(
        abi_version: i64,
        fields_address: u64,
        field_count: i64,
        flags: i64,
        result_address: u64,
    ) -> i64;
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CatalogStringView {
    ptr: u64,
    len: u64,
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

pub fn provider_url_violation(
    provider_is_local: bool,
    url_present: bool,
) -> Result<Option<ProviderUrlViolation>, MojoError> {
    let scalar = i64::from(provider_is_local) | (i64::from(url_present) << 1);
    let result = call(Operation::ProviderUrlPolicy, "", scalar)?;
    match result[0] {
        0 => Ok(None),
        1 => Ok(Some(ProviderUrlViolation::LocalRequiresUrl)),
        2 => Ok(Some(ProviderUrlViolation::NonLocalRejectsUrl)),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildArgvAction {
    Super,
    NoSubAgent,
    Presidio,
    NoPresidio,
    RequireTool,
    OpenAiProvider,
    LocalProvider,
    NamedProvider,
    Model,
    Effort,
    Exec,
    Task,
}

pub fn child_argv_plan(
    provider_is_openai: bool,
    provider_is_local: bool,
    presidio_enabled: bool,
    tool_count: usize,
    model_present: bool,
    effort_present: bool,
) -> Result<Vec<ChildArgvAction>, MojoError> {
    if provider_is_openai && provider_is_local {
        return Err(MojoError::InvalidInput);
    }
    let provider_class = if provider_is_openai {
        0
    } else if provider_is_local {
        1
    } else {
        2
    };
    let capacity = tool_count.checked_add(8).ok_or(MojoError::InvalidInput)?;
    let mut actions = vec![-1_i64; capacity];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_sub_agent_child_argv_plan_v1(
            ABI_VERSION,
            provider_class,
            i64::from(presidio_enabled),
            i64::try_from(tool_count).map_err(|_| MojoError::InvalidInput)?,
            i64::from(model_present),
            i64::from(effort_present),
            actions.as_mut_ptr() as usize as u64,
            i64::try_from(actions.len()).map_err(|_| MojoError::InvalidInput)?,
            (&mut written as *mut i64) as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        2 => return Err(MojoError::Capacity),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > actions.len() {
        return Err(MojoError::InvalidOutput);
    }
    actions.truncate(written);
    actions
        .into_iter()
        .map(|action| {
            Ok(match action {
                0 => ChildArgvAction::Super,
                1 => ChildArgvAction::NoSubAgent,
                2 => ChildArgvAction::Presidio,
                3 => ChildArgvAction::NoPresidio,
                4 => ChildArgvAction::RequireTool,
                5 => ChildArgvAction::OpenAiProvider,
                6 => ChildArgvAction::LocalProvider,
                7 => ChildArgvAction::NamedProvider,
                8 => ChildArgvAction::Model,
                9 => ChildArgvAction::Effort,
                10 => ChildArgvAction::Exec,
                11 => ChildArgvAction::Task,
                _ => return Err(MojoError::InvalidOutput),
            })
        })
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubAgentPromptStepPlan {
    pub provider: bool,
    pub local_url: bool,
    pub model: bool,
    pub reasoning_effort: bool,
    pub max_concurrency: bool,
}

pub fn prompt_step_plan(
    provider_explicit: bool,
    provider_is_local: bool,
    url_present: bool,
    model_explicit: bool,
    effort_explicit: bool,
) -> Result<SubAgentPromptStepPlan, MojoError> {
    let scalar = i64::from(provider_explicit)
        | (i64::from(provider_is_local) << 1)
        | (i64::from(url_present) << 2)
        | (i64::from(model_explicit) << 3)
        | (i64::from(effort_explicit) << 4);
    let result = call(Operation::PromptSteps, "", scalar)?;
    if result[0] != 0 || result[1] < 0 || result[1] > 31 {
        return Err(MojoError::InvalidOutput);
    }
    let mask = result[1];
    Ok(SubAgentPromptStepPlan {
        provider: mask & 1 != 0,
        local_url: mask & 2 != 0,
        model: mask & 4 != 0,
        reasoning_effort: mask & 8 != 0,
        max_concurrency: mask & 16 != 0,
    })
}

pub fn child_spec_scalar_violation(
    recursion_marker: &str,
    task_max_bytes: usize,
) -> Result<Option<ChildSpecScalarViolation>, MojoError> {
    let scalar = i64::try_from(task_max_bytes).map_err(|_| MojoError::InvalidInput)?;
    let result = call(Operation::ChildSpecScalarPolicy, recursion_marker, scalar)?;
    match result[0] {
        0 => Ok(None),
        1 => Ok(Some(ChildSpecScalarViolation::InvalidRecursionMarker)),
        2 => Ok(Some(ChildSpecScalarViolation::InvalidTaskSize)),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Chooses the dynamic model-catalog status from observed entries and load state.
pub fn catalog_status(has_models: bool, degraded: bool) -> Result<CatalogStatusPlan, MojoError> {
    let scalar = i64::from(has_models) | (i64::from(degraded) << 1);
    let result = call(Operation::CatalogStatus, "", scalar)?;
    match result[0] {
        0 => Ok(CatalogStatusPlan::NoDynamicCatalog),
        1 => Ok(CatalogStatusPlan::Available),
        2 => Ok(CatalogStatusPlan::Degraded),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Selects and normalizes one provider catalog entry using its JSON field values.
pub fn catalog_entry_plan(
    id_fields: [Option<&str>; 5],
    supported_in_api_false: bool,
    hidden_true: bool,
    visibility: Option<&str>,
) -> Result<CatalogEntryPlan, MojoError> {
    let values = id_fields
        .into_iter()
        .chain([visibility])
        .map(|value| -> Result<_, MojoError> {
            let value = value.unwrap_or_default();
            Ok(CatalogStringView {
                ptr: value.as_ptr() as usize as u64,
                len: u64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
            })
        })
        .collect::<Result<Vec<_>, MojoError>>()?;
    let flags = i64::from(supported_in_api_false)
        | (i64::from(hidden_true) << 1)
        | (i64::from(visibility.is_some()) << 2);
    let mut result = [-1_i64; 5];
    let status = unsafe {
        prodex_sub_agent_catalog_entry_v1(
            ABI_VERSION,
            values.as_ptr() as usize as u64,
            i64::try_from(values.len()).map_err(|_| MojoError::InvalidInput)?,
            flags,
            result.as_mut_ptr() as usize as u64,
        )
    };
    match status {
        0 => {}
        1 => return Err(MojoError::InvalidInput),
        4 => return Err(MojoError::AbiMismatch),
        _ => return Err(MojoError::InvalidOutput),
    }
    let model_id = match result[0] {
        -1 if result[1] == 0 && result[2] == 0 => None,
        field if (0..5).contains(&field) => {
            let field = usize::try_from(field).map_err(|_| MojoError::InvalidOutput)?;
            let start = usize::try_from(result[1]).map_err(|_| MojoError::InvalidOutput)?;
            let end = usize::try_from(result[2]).map_err(|_| MojoError::InvalidOutput)?;
            let Some(value) = id_fields[field] else {
                return Err(MojoError::InvalidOutput);
            };
            if start >= end || value.get(start..end).is_none() {
                return Err(MojoError::InvalidOutput);
            }
            Some((field, start, end))
        }
        _ => return Err(MojoError::InvalidOutput),
    };
    let set_id = match result[3] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    let selectable = match result[4] {
        0 => false,
        1 => true,
        _ => return Err(MojoError::InvalidOutput),
    };
    Ok(CatalogEntryPlan {
        model_id,
        set_id,
        selectable,
    })
}

/// Uses the full effort list when a model is absent from the provider catalog.
pub fn use_all_effort_suggestions(model_catalogued: bool) -> Result<bool, MojoError> {
    let scalar = i64::from(model_catalogued);
    let result = call(Operation::EffortSuggestionMask, "", scalar)?;
    match result[0] {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Resolves the recursion marker and the hidden launcher's exact authorization value.
pub fn recursion_decision(
    recursion_marker_present: bool,
    launcher_marker_is_one: bool,
) -> Result<RecursionDecision, MojoError> {
    let scalar = i64::from(recursion_marker_present) | (i64::from(launcher_marker_is_one) << 1);
    let result = call(Operation::RecursionDecision, "", scalar)?;
    match result[0] {
        0 => Ok(RecursionDecision::Allowed),
        1 => Ok(RecursionDecision::Disabled),
        2 => Ok(RecursionDecision::InternalLauncher),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Plans one admission or slot-reconciliation step; Rust performs the file operations.
pub fn slot_plan_step(limit: u16, cursor: u16, reconcile: bool) -> Result<SlotPlanStep, MojoError> {
    let scalar = i64::from(reconcile) | (i64::from(limit) << 2) | (i64::from(cursor) << 9);
    let result = call(Operation::SlotPlanStep, "", scalar)?;
    let index = || u16::try_from(result[1]).map_err(|_| MojoError::InvalidOutput);
    match result[0] {
        0 if !reconcile && index()? == cursor => Ok(SlotPlanStep::Candidate { index: cursor }),
        1 if reconcile => Ok(SlotPlanStep::Retire { index: index()? }),
        2 if reconcile => Ok(SlotPlanStep::Ensure { index: index()? }),
        3 if !reconcile => Ok(SlotPlanStep::LimitReached {
            exit_code: i32::try_from(result[2]).map_err(|_| MojoError::InvalidOutput)?,
        }),
        4 if reconcile => Ok(SlotPlanStep::Complete),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Classifies a platform lock error and chooses the caller's next action.
pub fn slot_lock_error_action(
    reconcile: bool,
    would_block: bool,
    raw_code_matches_lock_contention: bool,
) -> Result<SlotLockErrorAction, MojoError> {
    let scalar = i64::from(reconcile)
        | (i64::from(would_block) << 1)
        | (i64::from(raw_code_matches_lock_contention) << 2);
    let result = call(Operation::SlotLockErrorAction, "", scalar)?;
    match result[0] {
        0 => Ok(SlotLockErrorAction::TryNext),
        1 => Ok(SlotLockErrorAction::BlockResize),
        2 => Ok(SlotLockErrorAction::Propagate),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Classifies the child result after Rust has collected OS status and output facts.
pub fn child_outcome(
    cancelled: bool,
    child_succeeded: bool,
    output_incomplete: bool,
    has_output: bool,
) -> Result<ChildOutcomeAction, MojoError> {
    let scalar = i64::from(cancelled)
        | (i64::from(child_succeeded) << 1)
        | (i64::from(output_incomplete) << 2)
        | (i64::from(has_output) << 3);
    let result = call(Operation::ChildOutcome, "", scalar)?;
    let incomplete = result[2] == 1;
    match result[0] {
        0 if result[2] == 0 => Ok(ChildOutcomeAction::Success),
        1 if result[2] == 0 || incomplete => Ok(ChildOutcomeAction::Cancelled {
            exit_code: i32::try_from(result[1]).map_err(|_| MojoError::InvalidOutput)?,
            output_incomplete: incomplete,
        }),
        2 if result[2] == 0 || incomplete => Ok(ChildOutcomeAction::ChildFailed {
            output_incomplete: incomplete,
        }),
        3 if result[2] == 0 => Ok(ChildOutcomeAction::OutputIncomplete),
        4 if result[2] == 0 => Ok(ChildOutcomeAction::NoOutput),
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
        assert_eq!(provider_url_violation(true, true).unwrap(), None);
        assert_eq!(
            provider_url_violation(true, false).unwrap(),
            Some(ProviderUrlViolation::LocalRequiresUrl)
        );
        assert_eq!(
            provider_url_violation(false, true).unwrap(),
            Some(ProviderUrlViolation::NonLocalRejectsUrl)
        );
        assert_eq!(
            child_spec_scalar_violation("PRODEX_SUB_AGENT", 65_536).unwrap(),
            None
        );
        assert_eq!(
            child_spec_scalar_violation("bad", 65_536).unwrap(),
            Some(ChildSpecScalarViolation::InvalidRecursionMarker)
        );
        assert_eq!(
            child_spec_scalar_violation("PRODEX_SUB_AGENT", 0).unwrap(),
            Some(ChildSpecScalarViolation::InvalidTaskSize)
        );
        assert_eq!(
            prompt_step_plan(false, true, false, false, false).unwrap(),
            SubAgentPromptStepPlan {
                provider: true,
                local_url: true,
                model: true,
                reasoning_effort: true,
                max_concurrency: true,
            }
        );
        assert_eq!(
            prompt_step_plan(true, false, false, true, true).unwrap(),
            SubAgentPromptStepPlan {
                provider: false,
                local_url: false,
                model: false,
                reasoning_effort: false,
                max_concurrency: true,
            }
        );
        assert_eq!(
            child_argv_plan(false, false, true, 2, true, true).unwrap(),
            vec![
                ChildArgvAction::Super,
                ChildArgvAction::NoSubAgent,
                ChildArgvAction::Presidio,
                ChildArgvAction::RequireTool,
                ChildArgvAction::RequireTool,
                ChildArgvAction::NamedProvider,
                ChildArgvAction::Model,
                ChildArgvAction::Effort,
                ChildArgvAction::Exec,
                ChildArgvAction::Task,
            ]
        );
        assert_eq!(
            child_argv_plan(true, false, false, 0, false, false).unwrap(),
            vec![
                ChildArgvAction::Super,
                ChildArgvAction::NoSubAgent,
                ChildArgvAction::NoPresidio,
                ChildArgvAction::OpenAiProvider,
                ChildArgvAction::Exec,
                ChildArgvAction::Task,
            ]
        );
    }

    #[test]
    fn app_policy_plans_cover_slots_catalogs_recursion_and_child_results() {
        assert_eq!(
            catalog_status(false, true).unwrap(),
            CatalogStatusPlan::Degraded
        );
        assert_eq!(
            catalog_status(false, false).unwrap(),
            CatalogStatusPlan::NoDynamicCatalog
        );
        assert_eq!(
            catalog_status(true, false).unwrap(),
            CatalogStatusPlan::Available
        );
        assert_eq!(
            catalog_entry_plan(
                [Some("  "), Some(" model "), None, None, None],
                false,
                false,
                Some("LiSt"),
            )
            .unwrap(),
            CatalogEntryPlan {
                model_id: Some((1, 1, 6)),
                set_id: true,
                selectable: true,
            }
        );
        assert_eq!(
            catalog_entry_plan(
                [Some(" canonical "), Some("lower"), None, None, None],
                false,
                false,
                None,
            )
            .unwrap(),
            CatalogEntryPlan {
                model_id: Some((0, 1, 10)),
                set_id: false,
                selectable: true,
            }
        );
        assert!(
            !catalog_entry_plan([None, None, None, None, None], true, false, Some("list"))
                .unwrap()
                .selectable
        );
        assert!(
            !catalog_entry_plan([None, None, None, None, None], false, true, Some("list"))
                .unwrap()
                .selectable
        );
        assert!(
            !catalog_entry_plan([None, None, None, None, None], false, false, Some(" list "))
                .unwrap()
                .selectable
        );
        assert!(use_all_effort_suggestions(false).unwrap());
        assert!(!use_all_effort_suggestions(true).unwrap());

        assert_eq!(
            recursion_decision(false, false).unwrap(),
            RecursionDecision::Allowed
        );
        assert_eq!(
            recursion_decision(true, false).unwrap(),
            RecursionDecision::Disabled
        );
        assert_eq!(
            recursion_decision(true, true).unwrap(),
            RecursionDecision::InternalLauncher
        );

        assert_eq!(
            slot_plan_step(2, 0, false).unwrap(),
            SlotPlanStep::Candidate { index: 0 }
        );
        assert_eq!(
            slot_plan_step(2, 1, false).unwrap(),
            SlotPlanStep::Candidate { index: 1 }
        );
        assert_eq!(
            slot_plan_step(2, 2, false).unwrap(),
            SlotPlanStep::LimitReached { exit_code: 75 }
        );
        assert_eq!(
            slot_plan_step(2, 0, true).unwrap(),
            SlotPlanStep::Retire { index: 2 }
        );
        assert_eq!(
            slot_plan_step(2, 61, true).unwrap(),
            SlotPlanStep::Retire { index: 63 }
        );
        assert_eq!(
            slot_plan_step(2, 62, true).unwrap(),
            SlotPlanStep::Ensure { index: 0 }
        );
        assert_eq!(
            slot_plan_step(2, 63, true).unwrap(),
            SlotPlanStep::Ensure { index: 1 }
        );
        assert_eq!(slot_plan_step(2, 64, true).unwrap(), SlotPlanStep::Complete);
        assert!(matches!(
            slot_plan_step(0, 0, false),
            Err(MojoError::InvalidInput)
        ));
        assert_eq!(
            slot_lock_error_action(false, true, false).unwrap(),
            SlotLockErrorAction::TryNext
        );
        assert_eq!(
            slot_lock_error_action(true, false, true).unwrap(),
            SlotLockErrorAction::BlockResize
        );
        assert_eq!(
            slot_lock_error_action(false, false, false).unwrap(),
            SlotLockErrorAction::Propagate
        );

        assert_eq!(
            child_outcome(true, true, true, false).unwrap(),
            ChildOutcomeAction::Cancelled {
                exit_code: 130,
                output_incomplete: true,
            }
        );
        assert_eq!(
            child_outcome(false, false, true, false).unwrap(),
            ChildOutcomeAction::ChildFailed {
                output_incomplete: true,
            }
        );
        assert_eq!(
            child_outcome(false, true, true, true).unwrap(),
            ChildOutcomeAction::OutputIncomplete
        );
        assert_eq!(
            child_outcome(false, true, false, false).unwrap(),
            ChildOutcomeAction::NoOutput
        );
        assert_eq!(
            child_outcome(false, true, false, true).unwrap(),
            ChildOutcomeAction::Success
        );
    }
}
