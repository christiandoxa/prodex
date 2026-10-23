//! Native script text only; the host resolves paths and writes executables.
use crate::{
    MojoError,
    json::{StringView, render_native_text},
};

/// Native command-interpreter syntax selected by the host platform.
#[derive(Clone, Copy)]
#[repr(i64)]
pub enum RtkWrapperPlatform {
    Posix = 0,
    Windows = 1,
}

/// The three existing executable wrapper roles.
#[derive(Clone, Copy)]
#[repr(i64)]
pub enum RtkWrapperKind {
    Command = 0,
    Shell = 1,
    Unavailable = 2,
}

/// Borrowed command arguments and zero or three host-resolved state paths.
pub struct RtkWrapperInput<'a> {
    pub platform: RtkWrapperPlatform,
    pub kind: RtkWrapperKind,
    pub rtk: &'a str,
    pub command: &'a str,
    pub arguments: &'a [&'a str],
    pub state_paths: &'a [&'a str],
}

unsafe extern "C" {
    fn prodex_rtk_wrapper_script_v1(
        abi_version: i64,
        platform: i64,
        kind: i64,
        texts_address: u64,
        arguments_address: u64,
        argument_count: i64,
        states_address: u64,
        state_count: i64,
        measuring: i64,
        output_address: u64,
        capacity: i64,
        metadata_address: u64,
    ) -> i64;
}

/// Render one complete wrapper with compiled quoting, routing and line endings.
pub fn render_rtk_wrapper(input: RtkWrapperInput<'_>) -> Result<String, MojoError> {
    let texts = [StringView::from(input.rtk), StringView::from(input.command)];
    let arguments = input
        .arguments
        .iter()
        .map(|value| StringView::from(*value))
        .collect::<Vec<_>>();
    let states = input
        .state_paths
        .iter()
        .map(|value| StringView::from(*value))
        .collect::<Vec<_>>();
    let argument_count = i64::try_from(arguments.len()).map_err(|_| MojoError::InvalidInput)?;
    let state_count = i64::try_from(states.len()).map_err(|_| MojoError::InvalidInput)?;
    render_native_text(|measuring, output, capacity, metadata| unsafe {
        prodex_rtk_wrapper_script_v1(
            1,
            input.platform as i64,
            input.kind as i64,
            texts.as_ptr() as u64,
            arguments.as_ptr() as u64,
            argument_count,
            states.as_ptr() as u64,
            state_count,
            measuring,
            output,
            capacity,
            metadata,
        )
    })
}

#[cfg(test)]
mod tests;
