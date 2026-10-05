use anyhow::Result;
use std::cmp::Ordering;
use std::error::Error as StdError;
use std::fmt;

#[derive(Debug)]
struct UpdateNoticeMojoError(prodex_mojo_core::MojoError);

impl fmt::Display for UpdateNoticeMojoError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Mojo update-notice policy failed: {:?}", self.0)
    }
}

impl StdError for UpdateNoticeMojoError {}

pub(super) fn map_update_notice_mojo<T>(
    result: std::result::Result<T, prodex_mojo_core::MojoError>,
) -> Result<T> {
    result.map_err(|error| anyhow::Error::new(UpdateNoticeMojoError(error)))
}

pub(super) fn is_update_notice_mojo_error(error: &anyhow::Error) -> bool {
    error.downcast_ref::<UpdateNoticeMojoError>().is_some()
}

pub(super) fn release_version_is_valid(version: &str) -> Result<bool> {
    map_update_notice_mojo(
        prodex_mojo_core::update_notice_policy::release_version_is_valid(version),
    )
}

pub fn version_is_newer(candidate: &str, current: &str) -> Result<bool> {
    Ok(matches!(
        map_update_notice_mojo(
            prodex_mojo_core::update_notice_policy::compare_release_versions(
                candidate,
                current,
                prodex_mojo_core::update_notice_policy::ReleaseVersionOrder::Total,
            )
        )?,
        Some(Ordering::Greater)
    ))
}
