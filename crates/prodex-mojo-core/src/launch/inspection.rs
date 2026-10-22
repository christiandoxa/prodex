//! Native launch-inspection observations and checked ABI decoding.

use super::{
    ABI_VERSION, METADATA_WORDS, boolean, index, optional_index, prodex_mojo_launch_args_v1,
    status, views,
};
use crate::MojoError;

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
