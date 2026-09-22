//! Versioned, caller-owned Codex argument plans. No native OS string or heap
//! object crosses the boundary; opaque non-UTF-8 values remain Rust-owned.
use crate::MojoError;

const ABI_VERSION: i64 = 1;
const METADATA_WORDS: usize = 11;

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

#[repr(C)]
#[derive(Clone, Copy)]
struct ArgumentView {
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

fn views(arguments: &[Option<&str>]) -> Result<Vec<ArgumentView>, MojoError> {
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

fn status(value: i64) -> Result<(), MojoError> {
    match value {
        0 => Ok(()),
        1 | 2 => Err(MojoError::InvalidInput),
        3 => Err(MojoError::Capacity),
        4 => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

fn index(value: i64, count: usize) -> Result<usize, MojoError> {
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

fn boolean(value: i64) -> Result<bool, MojoError> {
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
            use LaunchArgumentOperation as Op;
            let needs_index = matches!(piece.kind, 0 | 2);
            let argument_index = if needs_index {
                let i = index(piece.index, input.len())?;
                if std::mem::replace(&mut seen[i], true) {
                    return Err(MojoError::InvalidOutput);
                }
                i
            } else {
                if piece.index != -1 || piece.offset != 0 {
                    return Err(MojoError::InvalidOutput);
                }
                0
            };
            Ok(match piece.kind {
                0 if piece.offset == 0 => LaunchArgument::Original(argument_index),
                1 if matches!(operation, Op::NormalizeProfile | Op::Prepare) => {
                    LaunchArgument::Profile
                }
                2 if matches!(operation, Op::NormalizeProfile | Op::Prepare) => {
                    let offset =
                        usize::try_from(piece.offset).map_err(|_| MojoError::InvalidOutput)?;
                    let value = arguments[argument_index].ok_or(MojoError::InvalidOutput)?;
                    if offset != "--profile-v2=".len() || value.get(offset..).is_none() {
                        return Err(MojoError::InvalidOutput);
                    }
                    LaunchArgument::ProfileInline {
                        index: argument_index,
                        offset,
                    }
                }
                3 if matches!(
                    operation,
                    Op::NormalizeRun | Op::RetargetTui | Op::RetargetExec | Op::Prepare
                ) =>
                {
                    LaunchArgument::Resume
                }
                4 if operation == Op::RetargetExec => LaunchArgument::Exec,
                5 if matches!(operation, Op::RetargetTui | Op::RetargetExec) => {
                    LaunchArgument::Session
                }
                6 if operation == Op::Prepare => LaunchArgument::FullAccess,
                _ => return Err(MojoError::InvalidOutput),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(LaunchArgumentPlan {
        arguments,
        flag: boolean(meta[1])?,
    })
}
