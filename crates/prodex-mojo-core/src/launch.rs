//! Versioned, caller-owned Codex argument plans. No native OS string or heap
//! object crosses the boundary; opaque non-UTF-8 values remain Rust-owned.
#[path = "launch/runtime_feature_plan.rs"]
mod runtime_feature_plan;
pub use runtime_feature_plan::{
    RuntimeFeatureClockSource, RuntimeFeatureConfigInput, RuntimeFeatureConfigPlan,
    RuntimeFeatureWebSearchMode, plan_runtime_feature_config,
};

use crate::MojoError;

const ABI_VERSION: i64 = 1;
const METADATA_WORDS: usize = 11;
const SCAN_SUPER_OVERRIDES: i64 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum LaunchArgumentOperation {
    NormalizeRun = 1,
    RetargetTui = 2,
    RetargetExec = 3,
    NormalizeProfile = 6,
    ExtractDryRun = 7,
    Prepare = 8,
    ScopeConfig = 9,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LaunchArgument {
    Original(usize),
    Profile,
    ProfileInline { index: usize, offset: usize },
    Resume,
    Exec,
    Session,
    FullAccess,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchArgumentPlan {
    pub arguments: Vec<LaunchArgument>,
    /// Dry-run presence for ExtractDryRun; review presence for Prepare.
    pub flag: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchArgumentInspection<'a> {
    pub first_positional: Option<usize>,
    pub resume_command: Option<usize>,
    pub resume_session: Option<&'a str>,
    pub config_insertion: usize,
    pub governed_insertion: usize,
    pub is_exec: bool,
    pub is_review: bool,
    pub dry_run: bool,
    pub model: Option<&'a str>,
}

/// Stable tag for a Prodex-owned override found in the Codex argument tail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum SuperOverrideKind {
    Provider = 1,
    Cli = 2,
    ApiKey = 3,
    SubAgentProvider = 4,
    SubAgentModel = 5,
    SubAgentReasoningEffort = 6,
    SubAgentUrl = 7,
    SubAgentMaxConcurrency = 8,
    LocalModel = 9,
    Profile = 10,
    BaseUrl = 11,
    Url = 12,
    LocalContextWindow = 13,
    LocalAutoCompactTokenLimit = 14,
    Tool = 15,
    RequiredTool = 16,
    WebSearch = 17,
    RolloutBudgetTokens = 18,
    RolloutBudgetReminders = 19,
    RolloutBudgetSamplingWeight = 20,
    RolloutBudgetPrefillWeight = 21,
    CurrentTimeReminderInterval = 22,
    CurrentTimeClockSource = 23,
    NoAutoRotate = 24,
    AutoRotate = 25,
    AutoRedeem = 26,
    SkipQuotaCheck = 27,
    DryRun = 28,
    NoProxy = 29,
    Presidio = 30,
    NoPresidio = 31,
    SubAgent = 32,
    NoSubAgent = 33,
    FullAccess = 34,
    CurrentTimeReminder = 35,
    RespectSystemProxy = 36,
    NoRespectSystemProxy = 37,
}

const SUPER_OVERRIDE_KINDS: [SuperOverrideKind; 37] = [
    SuperOverrideKind::Provider,
    SuperOverrideKind::Cli,
    SuperOverrideKind::ApiKey,
    SuperOverrideKind::SubAgentProvider,
    SuperOverrideKind::SubAgentModel,
    SuperOverrideKind::SubAgentReasoningEffort,
    SuperOverrideKind::SubAgentUrl,
    SuperOverrideKind::SubAgentMaxConcurrency,
    SuperOverrideKind::LocalModel,
    SuperOverrideKind::Profile,
    SuperOverrideKind::BaseUrl,
    SuperOverrideKind::Url,
    SuperOverrideKind::LocalContextWindow,
    SuperOverrideKind::LocalAutoCompactTokenLimit,
    SuperOverrideKind::Tool,
    SuperOverrideKind::RequiredTool,
    SuperOverrideKind::WebSearch,
    SuperOverrideKind::RolloutBudgetTokens,
    SuperOverrideKind::RolloutBudgetReminders,
    SuperOverrideKind::RolloutBudgetSamplingWeight,
    SuperOverrideKind::RolloutBudgetPrefillWeight,
    SuperOverrideKind::CurrentTimeReminderInterval,
    SuperOverrideKind::CurrentTimeClockSource,
    SuperOverrideKind::NoAutoRotate,
    SuperOverrideKind::AutoRotate,
    SuperOverrideKind::AutoRedeem,
    SuperOverrideKind::SkipQuotaCheck,
    SuperOverrideKind::DryRun,
    SuperOverrideKind::NoProxy,
    SuperOverrideKind::Presidio,
    SuperOverrideKind::NoPresidio,
    SuperOverrideKind::SubAgent,
    SuperOverrideKind::NoSubAgent,
    SuperOverrideKind::FullAccess,
    SuperOverrideKind::CurrentTimeReminder,
    SuperOverrideKind::RespectSystemProxy,
    SuperOverrideKind::NoRespectSystemProxy,
];

impl SuperOverrideKind {
    fn from_tag(tag: i64) -> Option<Self> {
        let index = usize::try_from(tag.checked_sub(1)?).ok()?;
        SUPER_OVERRIDE_KINDS.get(index).copied()
    }

    const fn takes_value(self) -> bool {
        matches!(
            self,
            Self::Provider
                | Self::Cli
                | Self::ApiKey
                | Self::SubAgentProvider
                | Self::SubAgentModel
                | Self::SubAgentReasoningEffort
                | Self::SubAgentUrl
                | Self::SubAgentMaxConcurrency
                | Self::LocalModel
                | Self::Profile
                | Self::BaseUrl
                | Self::Url
                | Self::LocalContextWindow
                | Self::LocalAutoCompactTokenLimit
                | Self::Tool
                | Self::RequiredTool
                | Self::WebSearch
                | Self::RolloutBudgetTokens
                | Self::RolloutBudgetReminders
                | Self::RolloutBudgetSamplingWeight
                | Self::RolloutBudgetPrefillWeight
                | Self::CurrentTimeReminderInterval
                | Self::CurrentTimeClockSource
        )
    }
}

/// One classified argument, with its borrowed value and number of consumed tokens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScannedSuperOverride<'a> {
    /// Classified Prodex override.
    pub kind: SuperOverrideKind,
    /// UTF-8 value, absent for boolean flags or a missing required value.
    pub value: Option<&'a str>,
    /// One for inline, boolean, or missing-value forms; two for split values.
    pub consumed_count: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct ArgumentView {
    address: u64,
    length: u64,
    valid_utf8: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct ArgumentPiece {
    kind: i64,
    index: i64,
    offset: i64,
}

const _: () = {
    assert!(std::mem::size_of::<ArgumentView>() == 24);
    assert!(std::mem::align_of::<ArgumentView>() == 8);
    assert!(std::mem::size_of::<ArgumentPiece>() == 24);
    assert!(std::mem::align_of::<ArgumentPiece>() == 8);
    assert!(std::mem::size_of::<usize>() == 8);
};

unsafe extern "C" {
    fn prodex_mojo_launch_args_v1(
        version: i64,
        operation: i64,
        full_access: i64,
        arguments: u64,
        count: i64,
        output: u64,
        capacity: i64,
        scratch: u64,
        metadata: u64,
    ) -> i64;
}

pub(super) fn views(arguments: &[Option<&str>]) -> Result<Vec<ArgumentView>, MojoError> {
    if arguments.len() > (i64::MAX as usize / 24).saturating_sub(3) {
        return Err(MojoError::InvalidInput);
    }
    arguments
        .iter()
        .map(|arg| match arg {
            Some(value) => Ok(ArgumentView {
                address: value.as_ptr() as u64,
                length: u64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?,
                valid_utf8: 1,
            }),
            None => Ok(ArgumentView {
                address: 0,
                length: 0,
                valid_utf8: 0,
            }),
        })
        .collect()
}

pub(super) fn status(value: i64) -> Result<(), MojoError> {
    match value {
        0 => Ok(()),
        1 | 2 => Err(MojoError::InvalidInput),
        3 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub(super) fn index(value: i64, count: usize) -> Result<usize, MojoError> {
    usize::try_from(value)
        .ok()
        .filter(|&value| value < count)
        .ok_or(MojoError::InvalidOutput)
}

fn optional_index(value: i64, count: usize) -> Result<Option<usize>, MojoError> {
    if value == -1 {
        Ok(None)
    } else {
        index(value, count).map(Some)
    }
}

pub(super) fn boolean(value: i64) -> Result<bool, MojoError> {
    match value {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn inspect_launch_arguments<'a>(
    arguments: &[Option<&'a str>],
) -> Result<LaunchArgumentInspection<'a>, MojoError> {
    let input = views(arguments)?;
    let mut meta = [0_i64; METADATA_WORDS];
    // SAFETY: input and metadata have their exact declared C layouts and live
    // through the synchronous call. Operation zero never accesses plan buffers.
    status(unsafe {
        prodex_mojo_launch_args_v1(
            ABI_VERSION,
            0,
            0,
            input.as_ptr() as u64,
            input.len() as i64,
            0,
            0,
            0,
            meta.as_mut_ptr() as u64,
        )
    })?;
    let session = optional_index(meta[2], input.len())?
        .map(|i| arguments[i].ok_or(MojoError::InvalidOutput))
        .transpose()?;
    let model = optional_index(meta[8], input.len())?
        .map(|i| {
            let value = arguments[i].ok_or(MojoError::InvalidOutput)?;
            let start = usize::try_from(meta[9]).map_err(|_| MojoError::InvalidOutput)?;
            let length = usize::try_from(meta[10]).map_err(|_| MojoError::InvalidOutput)?;
            let end = start.checked_add(length).ok_or(MojoError::InvalidOutput)?;
            value.get(start..end).ok_or(MojoError::InvalidOutput)
        })
        .transpose()?;
    Ok(LaunchArgumentInspection {
        first_positional: optional_index(meta[0], input.len())?,
        resume_command: optional_index(meta[1], input.len())?,
        resume_session: session,
        config_insertion: index(meta[3], input.len() + 1)?,
        governed_insertion: index(meta[4], input.len() + 1)?,
        is_exec: boolean(meta[5])?,
        is_review: boolean(meta[6])?,
        dry_run: boolean(meta[7])?,
        model,
    })
}

fn launch_argument_piece_index(
    piece: &ArgumentPiece,
    input_len: usize,
    seen: &mut [bool],
) -> Result<Option<usize>, MojoError> {
    if matches!(piece.kind, 0 | 2) {
        let index = index(piece.index, input_len)?;
        if std::mem::replace(&mut seen[index], true) {
            return Err(MojoError::InvalidOutput);
        }
        return Ok(Some(index));
    }
    if piece.index != -1 || piece.offset != 0 {
        return Err(MojoError::InvalidOutput);
    }
    Ok(None)
}

fn decode_launch_argument(
    piece: &ArgumentPiece,
    operation: LaunchArgumentOperation,
    arguments: &[Option<&str>],
    argument_index: Option<usize>,
) -> Result<LaunchArgument, MojoError> {
    use LaunchArgumentOperation as Op;

    match piece.kind {
        0 if piece.offset == 0 => Ok(LaunchArgument::Original(
            argument_index.ok_or(MojoError::InvalidOutput)?,
        )),
        1 if matches!(operation, Op::NormalizeProfile | Op::Prepare) => Ok(LaunchArgument::Profile),
        2 if matches!(operation, Op::NormalizeProfile | Op::Prepare) => {
            let index = argument_index.ok_or(MojoError::InvalidOutput)?;
            let offset = usize::try_from(piece.offset).map_err(|_| MojoError::InvalidOutput)?;
            let value = arguments[index].ok_or(MojoError::InvalidOutput)?;
            if offset != "--profile-v2=".len() || value.get(offset..).is_none() {
                return Err(MojoError::InvalidOutput);
            }
            Ok(LaunchArgument::ProfileInline { index, offset })
        }
        3 if matches!(
            operation,
            Op::NormalizeRun | Op::RetargetTui | Op::RetargetExec | Op::Prepare
        ) =>
        {
            Ok(LaunchArgument::Resume)
        }
        4 if operation == Op::RetargetExec => Ok(LaunchArgument::Exec),
        5 if matches!(operation, Op::RetargetTui | Op::RetargetExec) => Ok(LaunchArgument::Session),
        6 if operation == Op::Prepare => Ok(LaunchArgument::FullAccess),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn plan_launch_arguments(
    arguments: &[Option<&str>],
    operation: LaunchArgumentOperation,
    full_access: bool,
) -> Result<LaunchArgumentPlan, MojoError> {
    let input = views(arguments)?;
    let capacity = input.len().checked_add(3).ok_or(MojoError::InvalidInput)?;
    let mut output = vec![ArgumentPiece::default(); capacity];
    let mut scratch = vec![ArgumentPiece::default(); capacity];
    let mut meta = [0_i64; METADATA_WORDS];
    // SAFETY: distinct, aligned input/output/scratch arenas and metadata outlive
    // this call; the kernel is bounded by the supplied count and capacity.
    status(unsafe {
        prodex_mojo_launch_args_v1(
            ABI_VERSION,
            operation as i64,
            i64::from(full_access),
            input.as_ptr() as u64,
            input.len() as i64,
            output.as_mut_ptr() as u64,
            capacity as i64,
            scratch.as_mut_ptr() as u64,
            meta.as_mut_ptr() as u64,
        )
    })?;
    let written = index(meta[0], capacity + 1)?;
    let mut seen = vec![false; input.len()];
    let arguments = output[..written]
        .iter()
        .map(|piece| {
            let argument_index = launch_argument_piece_index(piece, input.len(), &mut seen)?;
            decode_launch_argument(piece, operation, arguments, argument_index)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LaunchArgumentPlan {
        arguments,
        flag: boolean(meta[1])?,
    })
}

/// Classify Prodex overrides without interpreting argument values.
///
/// The returned vector aligns with the input. `None` marks an unrecognized
/// argument; callers retain OS strings and perform typed validation in Rust.
pub fn scan_super_overrides<'a>(
    arguments: &[Option<&'a str>],
) -> Result<Vec<Option<ScannedSuperOverride<'a>>>, MojoError> {
    let input = views(arguments)?;
    let capacity = input.len().checked_add(3).ok_or(MojoError::InvalidInput)?;
    let mut output = vec![ArgumentPiece::default(); capacity];
    let mut scratch = vec![ArgumentPiece::default(); capacity];
    let mut meta = [0_i64; METADATA_WORDS];
    // SAFETY: distinct, aligned input/output/scratch arenas and metadata outlive
    // this synchronous scan; operation 10 writes exactly one piece per input.
    status(unsafe {
        prodex_mojo_launch_args_v1(
            ABI_VERSION,
            SCAN_SUPER_OVERRIDES,
            0,
            input.as_ptr() as u64,
            input.len() as i64,
            output.as_mut_ptr() as u64,
            capacity as i64,
            scratch.as_mut_ptr() as u64,
            meta.as_mut_ptr() as u64,
        )
    })?;
    if usize::try_from(meta[0]).ok() != Some(input.len()) {
        return Err(MojoError::InvalidOutput);
    }

    output[..input.len()]
        .iter()
        .enumerate()
        .map(|(index, piece)| decode_super_override(piece, index, arguments))
        .collect()
}

fn decode_super_override<'a>(
    piece: &ArgumentPiece,
    index: usize,
    arguments: &[Option<&'a str>],
) -> Result<Option<ScannedSuperOverride<'a>>, MojoError> {
    if piece.offset < 0 {
        return Err(MojoError::InvalidOutput);
    }
    if piece.kind == 0 {
        let expected = i64::try_from(index).map_err(|_| MojoError::InvalidOutput)?;
        return (piece.index == expected && piece.offset == 0)
            .then_some(None)
            .ok_or(MojoError::InvalidOutput);
    }

    let kind = SuperOverrideKind::from_tag(piece.kind).ok_or(MojoError::InvalidOutput)?;
    let value = decode_super_override_value(kind, piece, index, arguments)?;
    let consumed_count = decode_super_override_consumed_count(piece, index)?;
    Ok(Some(ScannedSuperOverride {
        kind,
        value,
        consumed_count,
    }))
}

fn decode_super_override_value<'a>(
    kind: SuperOverrideKind,
    piece: &ArgumentPiece,
    index: usize,
    arguments: &[Option<&'a str>],
) -> Result<Option<&'a str>, MojoError> {
    if !kind.takes_value() {
        return (piece.index == -1 && piece.offset == 0)
            .then_some(None)
            .ok_or(MojoError::InvalidOutput);
    }
    if piece.offset > 0 {
        return decode_inline_super_override_value(piece, index, arguments).map(Some);
    }
    if piece.index == -1 {
        return Ok(None);
    }
    let expected = i64::try_from(index + 1).map_err(|_| MojoError::InvalidOutput)?;
    if piece.index != expected {
        return Err(MojoError::InvalidOutput);
    }
    arguments
        .get(index + 1)
        .copied()
        .flatten()
        .map(Some)
        .ok_or(MojoError::InvalidOutput)
}

fn decode_inline_super_override_value<'a>(
    piece: &ArgumentPiece,
    index: usize,
    arguments: &[Option<&'a str>],
) -> Result<&'a str, MojoError> {
    let expected = i64::try_from(index).map_err(|_| MojoError::InvalidOutput)?;
    if piece.index != expected {
        return Err(MojoError::InvalidOutput);
    }
    let value = arguments[index].ok_or(MojoError::InvalidOutput)?;
    let offset = usize::try_from(piece.offset).map_err(|_| MojoError::InvalidOutput)?;
    if offset == 0 || value.as_bytes().get(offset - 1) != Some(&b'=') {
        return Err(MojoError::InvalidOutput);
    }
    value.get(offset..).ok_or(MojoError::InvalidOutput)
}

fn decode_super_override_consumed_count(
    piece: &ArgumentPiece,
    index: usize,
) -> Result<usize, MojoError> {
    if piece.offset > 0 {
        return Ok(1);
    }
    let split_index = i64::try_from(index + 1).map_err(|_| MojoError::InvalidOutput)?;
    Ok(usize::from(piece.index == split_index) + 1)
}
