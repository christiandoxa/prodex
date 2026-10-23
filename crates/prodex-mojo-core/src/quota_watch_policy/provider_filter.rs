//! Provider-filter parser and native-profile policy; no Rust alias fallback.
use super::{FILTER_AGY, FILTER_ALL};
use crate::{MojoError, text_output::render_measured_string};

/// A validated canonical filter tag or the exact compiled CLI diagnostic.
#[derive(Debug, PartialEq, Eq)]
pub enum ProviderFilterParse {
    Known(i64),
    Invalid(String),
}

unsafe extern "C" {
    fn prodex_quota_watch_provider_filter_parse_v1(
        abi_version: i64,
        input_address: u64,
        input_length: i64,
        measuring: i64,
        output_address: u64,
        capacity: i64,
        metadata_address: u64,
    ) -> i64;
    fn prodex_quota_watch_profile_filter_matches_v1(
        abi_version: i64,
        filter_kind: i64,
        profile_kind: i64,
        result_address: u64,
    ) -> i64;
}

/// Parse aliases and normalize error text through the canonical quota-watch ABI.
pub fn parse_provider_filter(value: &str) -> Result<ProviderFilterParse, MojoError> {
    let length = i64::try_from(value.len()).map_err(|_| MojoError::InvalidInput)?;
    let mut selected = None;
    let diagnostic = render_measured_string(|output| {
        let (phase, address, capacity) = match output {
            None => (1, 0, 0),
            Some(output) => (
                0,
                output.as_mut_ptr() as u64,
                i64::try_from(output.len()).map_err(|_| MojoError::Capacity)?,
            ),
        };
        let mut metadata = [-2_i64, -1];
        crate::launch::status(unsafe {
            prodex_quota_watch_provider_filter_parse_v1(
                1,
                value.as_ptr() as u64,
                length,
                phase,
                address,
                capacity,
                metadata.as_mut_ptr() as u64,
            )
        })?;
        if !(-1..=FILTER_AGY).contains(&metadata[0])
            || selected.is_some_and(|kind| kind != metadata[0])
        {
            return Err(MojoError::InvalidOutput);
        }
        selected = Some(metadata[0]);
        usize::try_from(metadata[1]).map_err(|_| MojoError::InvalidOutput)
    })?;
    match selected {
        Some(-1) if !diagnostic.is_empty() => Ok(ProviderFilterParse::Invalid(diagnostic)),
        Some(kind) if (FILTER_ALL..=FILTER_AGY).contains(&kind) && diagnostic.is_empty() => {
            Ok(ProviderFilterParse::Known(kind))
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Match a native profile's projected provider tag without exposing its fields.
pub fn filter_matches_profile(filter_kind: i64, profile_kind: i64) -> Result<bool, MojoError> {
    let mut output = -1_i64;
    crate::launch::status(unsafe {
        prodex_quota_watch_profile_filter_matches_v1(
            1,
            filter_kind,
            profile_kind,
            (&mut output as *mut i64) as u64,
        )
    })?;
    super::output_bool(output)
}

#[cfg(test)]
mod tests;
