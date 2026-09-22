//! Config assignment matching over borrowed arguments and override keys.
//! Values stay with the native caller; this operation returns only typed plans.
use crate::{
    MojoError,
    launch::{boolean, index, status, views},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchConfigArgument {
    Original,
    Separate(usize),
    LongInline(usize),
    ShortInline(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchConfigPlan {
    pub arguments: Vec<LaunchConfigArgument>,
    pub replaced: Vec<bool>,
}

unsafe extern "C" {
    fn prodex_mojo_launch_config_v1(
        version: i64,
        arguments: u64,
        argument_count: i64,
        keys: u64,
        key_count: i64,
        output: u64,
        output_count: i64,
        replaced: u64,
        replaced_count: i64,
    ) -> i64;
}

pub fn plan_launch_config(
    arguments: &[Option<&str>],
    keys: &[&str],
) -> Result<LaunchConfigPlan, MojoError> {
    let input = views(arguments)?;
    let keys = views(&keys.iter().copied().map(Some).collect::<Vec<_>>())?;
    let mut output = vec![[0_i64; 2]; input.len()];
    let mut replaced = vec![0_i64; keys.len()];
    // SAFETY: exact-layout borrowed views and separate caller-owned buffers
    // outlive the synchronous call. Every returned tag/index is validated below.
    status(unsafe {
        prodex_mojo_launch_config_v1(
            1,
            input.as_ptr() as u64,
            input.len() as i64,
            keys.as_ptr() as u64,
            keys.len() as i64,
            output.as_mut_ptr() as u64,
            output.len() as i64,
            replaced.as_mut_ptr() as u64,
            replaced.len() as i64,
        )
    })?;
    let mut observed = vec![false; keys.len()];
    let arguments = output
        .iter()
        .map(|&[tag, key]| {
            if tag == 0 && key == -1 {
                return Ok(LaunchConfigArgument::Original);
            }
            let key = index(key, keys.len())?;
            observed[key] = true;
            match tag {
                1 => Ok(LaunchConfigArgument::Separate(key)),
                2 => Ok(LaunchConfigArgument::LongInline(key)),
                3 => Ok(LaunchConfigArgument::ShortInline(key)),
                _ => Err(MojoError::InvalidOutput),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    let replaced = replaced
        .into_iter()
        .map(boolean)
        .collect::<Result<Vec<_>, _>>()?;
    if observed != replaced {
        return Err(MojoError::InvalidOutput);
    }
    Ok(LaunchConfigPlan {
        arguments,
        replaced,
    })
}
