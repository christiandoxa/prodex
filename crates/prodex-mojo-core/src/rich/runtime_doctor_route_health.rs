//! Route-health grouping ABI; Mojo remains the semantic owner.
use super::{MojoError, ensure_rich_abi, mojo_mut_pointer_address, mojo_pointer_address, view};

const RUNTIME_DOCTOR_ROUTE_HEALTH_GROUPS_ABI_VERSION: i64 = 1;
const RUNTIME_DOCTOR_ROUTE_MARKER_FIELDS_ABI_VERSION: i64 = 1;
pub const RUNTIME_DOCTOR_ROUTE_MARKER_U32: i64 = 0;
pub const RUNTIME_DOCTOR_ROUTE_MARKER_I64: i64 = 1;
pub const RUNTIME_DOCTOR_ROUTE_HEALTH_GROUPS_MAX_ROWS: usize = 65_536;

unsafe extern "C" {
    fn prodex_mojo_runtime_doctor_route_marker_matches_v1(
        abi_version: i64,
        marker_profile: u64,
        marker_profile_present: i64,
        marker_route: u64,
        marker_route_present: i64,
        requested_profile: u64,
        requested_route: u64,
        result: u64,
    ) -> i64;
    fn prodex_mojo_runtime_doctor_route_marker_number_v1(
        abi_version: i64,
        value: u64,
        present: i64,
        kind: i64,
        output: u64,
    ) -> i64;
    fn prodex_mojo_runtime_doctor_route_health_groups_v1(
        abi_version: i64,
        profiles: u64,
        routes: u64,
        event_flags: u64,
        count: i64,
        representative_indices: u64,
        event_counts: u64,
        latest_event_indices: u64,
        profile_row_indices: u64,
        output_count: u64,
    ) -> i64;
    fn prodex_mojo_runtime_doctor_route_latest_reason_v1(
        abi_version: i64,
        event_profiles: u64,
        event_routes: u64,
        event_details: u64,
        event_count: i64,
        latest_index: i64,
        requested_profile: u64,
        requested_route: u64,
        output: u64,
    ) -> i64;
}

/// Check a marker's profile/route scope, including absent keys and the '-'
/// wildcard, exclusively through the compiled route-health owner.
pub fn runtime_doctor_route_marker_matches(
    marker_profile: Option<&str>,
    marker_route: Option<&str>,
    requested_profile: &str,
    requested_route: &str,
) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let marker_profile_view = view(marker_profile.unwrap_or(""));
    let marker_route_view = view(marker_route.unwrap_or(""));
    let requested_profile_view = view(requested_profile);
    let requested_route_view = view(requested_route);
    let mut result = -1_i64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_route_marker_matches_v1(
            RUNTIME_DOCTOR_ROUTE_MARKER_FIELDS_ABI_VERSION,
            mojo_pointer_address(&marker_profile_view),
            i64::from(marker_profile.is_some()),
            mojo_pointer_address(&marker_route_view),
            i64::from(marker_route.is_some()),
            mojo_pointer_address(&requested_profile_view),
            mojo_pointer_address(&requested_route_view),
            mojo_mut_pointer_address(&mut result),
        )
    };
    match (status, result) {
        (0, 0) => Ok(false),
        (0, 1) => Ok(true),
        (1, _) => Err(MojoError::InvalidInput),
        (4, _) => Err(MojoError::AbiMismatch),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Parse the score/u32 and expiry/i64 diagnostic field grammars in Mojo.
/// Rust supplies the requested numeric type and receives optional typed data.
pub fn runtime_doctor_route_marker_number(
    value: Option<&str>,
    kind: i64,
) -> Result<Option<i64>, MojoError> {
    ensure_rich_abi()?;
    if kind != RUNTIME_DOCTOR_ROUTE_MARKER_U32 && kind != RUNTIME_DOCTOR_ROUTE_MARKER_I64 {
        return Err(MojoError::InvalidInput);
    }
    let value_view = view(value.unwrap_or(""));
    let mut output = [-1_i64; 2];
    let status = unsafe {
        prodex_mojo_runtime_doctor_route_marker_number_v1(
            RUNTIME_DOCTOR_ROUTE_MARKER_FIELDS_ABI_VERSION,
            mojo_pointer_address(&value_view),
            i64::from(value.is_some()),
            kind,
            mojo_mut_pointer_address(output.as_mut_ptr()),
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    match output[0] {
        0 if output[1] == 0 => Ok(None),
        1 if kind == RUNTIME_DOCTOR_ROUTE_MARKER_I64
            || (0..=i64::from(u32::MAX)).contains(&output[1]) =>
        {
            Ok(Some(output[1]))
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

/// One grouped route-health row returned by the Mojo reducer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeDoctorRouteHealthGroup {
    pub representative_index: usize,
    pub event_count: usize,
    pub latest_event_index: Option<usize>,
    pub profile_row_index: Option<usize>,
}

/// Group and order route-health event/profile rows through the Mojo authority.
pub fn runtime_doctor_route_health_groups<'a>(
    rows: impl IntoIterator<Item = (&'a str, &'a str, bool)>,
) -> Result<Vec<RuntimeDoctorRouteHealthGroup>, MojoError> {
    ensure_rich_abi()?;
    let mut profiles = Vec::new();
    let mut routes = Vec::new();
    let mut event_flags = Vec::new();
    for (profile, route, is_event) in rows {
        if profiles.len() >= RUNTIME_DOCTOR_ROUTE_HEALTH_GROUPS_MAX_ROWS {
            return Err(MojoError::Capacity);
        }
        profiles.push(view(profile));
        routes.push(view(route));
        event_flags.push(i64::from(is_event));
    }
    if profiles.is_empty() {
        return Ok(Vec::new());
    }
    let count = i64::try_from(profiles.len()).map_err(|_| MojoError::InvalidInput)?;
    let mut representative_indices = vec![0_i64; profiles.len()];
    let mut event_counts = vec![0_i64; profiles.len()];
    let mut latest_event_indices = vec![-1_i64; profiles.len()];
    let mut profile_row_indices = vec![-1_i64; profiles.len()];
    let mut output_count = 0_i64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_route_health_groups_v1(
            RUNTIME_DOCTOR_ROUTE_HEALTH_GROUPS_ABI_VERSION,
            mojo_pointer_address(profiles.as_ptr()),
            mojo_pointer_address(routes.as_ptr()),
            mojo_pointer_address(event_flags.as_ptr()),
            count,
            mojo_mut_pointer_address(representative_indices.as_mut_ptr()),
            mojo_mut_pointer_address(event_counts.as_mut_ptr()),
            mojo_mut_pointer_address(latest_event_indices.as_mut_ptr()),
            mojo_mut_pointer_address(profile_row_indices.as_mut_ptr()),
            mojo_mut_pointer_address(&mut output_count),
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            2 => MojoError::InvalidOutput,
            3 => MojoError::Capacity,
            4 => MojoError::AbiMismatch,
            5 => MojoError::InvalidOutput,
            _ => MojoError::InvalidOutput,
        });
    }
    let output_count = usize::try_from(output_count).map_err(|_| MojoError::InvalidOutput)?;
    if output_count > profiles.len() {
        return Err(MojoError::InvalidOutput);
    }
    let mut groups = Vec::with_capacity(output_count);
    for index in 0..output_count {
        let representative_index =
            usize::try_from(representative_indices[index]).map_err(|_| MojoError::InvalidOutput)?;
        if representative_index >= profiles.len() {
            return Err(MojoError::InvalidOutput);
        }
        let event_count =
            usize::try_from(event_counts[index]).map_err(|_| MojoError::InvalidOutput)?;
        let latest_event_index =
            route_health_source_index(latest_event_indices[index], &event_flags, 1)?;
        let profile_row_index =
            route_health_source_index(profile_row_indices[index], &event_flags, 0)?;
        groups.push(RuntimeDoctorRouteHealthGroup {
            representative_index,
            event_count,
            latest_event_index,
            profile_row_index,
        });
    }
    Ok(groups)
}

#[derive(Debug, Clone, Copy)]
pub struct RuntimeDoctorRouteReasonEvent<'a> {
    pub profile: &'a str,
    pub route: &'a str,
    pub detail: &'a str,
}

/// Mojo chooses the newest matching route event with a reason/outcome token,
/// then returns a borrowed UTF-8 range. No Rust token parser is retained.
pub fn runtime_doctor_route_latest_reason<'a>(
    events: &[RuntimeDoctorRouteReasonEvent<'a>],
    latest_index: usize,
    profile: &str,
    route: &str,
) -> Result<Option<&'a str>, MojoError> {
    ensure_rich_abi()?;
    if events.len() > RUNTIME_DOCTOR_ROUTE_HEALTH_GROUPS_MAX_ROWS {
        return Err(MojoError::Capacity);
    }
    if latest_index >= events.len() {
        return Err(MojoError::InvalidInput);
    }
    let profiles = events
        .iter()
        .map(|event| view(event.profile))
        .collect::<Vec<_>>();
    let routes = events
        .iter()
        .map(|event| view(event.route))
        .collect::<Vec<_>>();
    let details = events
        .iter()
        .map(|event| view(event.detail))
        .collect::<Vec<_>>();
    let requested_profile = view(profile);
    let requested_route = view(route);
    let mut output = [-1_i64; 3];
    let status = unsafe {
        prodex_mojo_runtime_doctor_route_latest_reason_v1(
            1,
            mojo_pointer_address(profiles.as_ptr()),
            mojo_pointer_address(routes.as_ptr()),
            mojo_pointer_address(details.as_ptr()),
            i64::try_from(events.len()).map_err(|_| MojoError::InvalidInput)?,
            i64::try_from(latest_index).map_err(|_| MojoError::InvalidInput)?,
            mojo_pointer_address(&requested_profile),
            mojo_pointer_address(&requested_route),
            mojo_mut_pointer_address(output.as_mut_ptr()),
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            2 => MojoError::InvalidOutput,
            3 => MojoError::Capacity,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    if output == [-1; 3] {
        return Ok(None);
    }
    let index = usize::try_from(output[0]).map_err(|_| MojoError::InvalidOutput)?;
    let start = usize::try_from(output[1]).map_err(|_| MojoError::InvalidOutput)?;
    let end = usize::try_from(output[2]).map_err(|_| MojoError::InvalidOutput)?;
    if index > latest_index {
        return Err(MojoError::InvalidOutput);
    }
    let detail = events.get(index).ok_or(MojoError::InvalidOutput)?.detail;
    detail
        .get(start..end)
        .map(Some)
        .ok_or(MojoError::InvalidOutput)
}

// Both optional indices carry the same ABI sentinel/range contract, with a
// different source row kind. Keep validation shared, not the Mojo reducer.
fn route_health_source_index(
    value: i64,
    event_flags: &[i64],
    expected_kind: i64,
) -> Result<Option<usize>, MojoError> {
    if value == -1 {
        return Ok(None);
    }
    let index = usize::try_from(value).map_err(|_| MojoError::InvalidOutput)?;
    if event_flags.get(index) != Some(&expected_kind) {
        return Err(MojoError::InvalidOutput);
    }
    Ok(Some(index))
}

#[cfg(test)]
#[path = "runtime_doctor_route_reason_tests.rs"]
mod route_reason_tests;

#[cfg(test)]
mod route_marker_fields_tests {
    use super::*;

    #[test]
    fn route_marker_scope_and_decimal_grammars_are_mojo_owned() {
        for (profile, route, target_profile, target_route, expected) in [
            (None, None, "東京", "responses", true),
            (Some("-"), Some("-"), "東京", "responses", true),
            (Some("東京"), Some("responses"), "東京", "responses", true),
            (Some("東京"), Some("websocket"), "東京", "responses", false),
            (Some("beta"), Some("responses"), "東京", "responses", false),
            (Some(""), Some(""), "", "", true),
            (Some(""), Some(""), "東京", "responses", false),
            (Some(" - "), Some("responses"), "東京", "responses", false),
        ] {
            assert_eq!(
                runtime_doctor_route_marker_matches(profile, route, target_profile, target_route)
                    .unwrap(),
                expected,
                "{profile:?}/{route:?}",
            );
        }
        for (text, expected) in [
            (None, None),
            (Some(""), None),
            (Some("+"), None),
            (Some("-0"), None),
            (Some("-1"), None),
            (Some("+0000000"), Some(0)),
            (Some("4294967295"), Some(u32::MAX as i64)),
            (Some("4294967296"), None),
            (Some("00000000000000000000042"), Some(42)),
            (Some(" 42"), None),
            (Some("４２"), None),
        ] {
            assert_eq!(
                runtime_doctor_route_marker_number(text, RUNTIME_DOCTOR_ROUTE_MARKER_U32).unwrap(),
                expected,
                "{text:?}"
            );
        }
        for (text, expected) in [
            (None, None),
            (Some("+0"), Some(0)),
            (Some("-0"), Some(0)),
            (Some("+00017"), Some(17)),
            (Some("-00017"), Some(-17)),
            (Some("9223372036854775807"), Some(i64::MAX)),
            (Some("-9223372036854775808"), Some(i64::MIN)),
            (Some("9223372036854775808"), None),
            (Some("-9223372036854775809"), None),
            (Some(""), None),
            (Some("--1"), None),
            (Some("- 1"), None),
            (Some("١"), None),
        ] {
            assert_eq!(
                runtime_doctor_route_marker_number(text, RUNTIME_DOCTOR_ROUTE_MARKER_I64).unwrap(),
                expected,
                "{text:?}"
            );
        }
    }

    #[test]
    fn route_marker_abis_reject_invalid_shapes_without_writing_outputs() {
        let text = view("東京");
        let route = view("responses");
        let mut matched = -1_i64;
        let target = mojo_pointer_address(&text);
        let route_ptr = mojo_pointer_address(&route);
        let matched_ptr = mojo_mut_pointer_address(&mut matched);
        unsafe {
            assert_eq!(
                prodex_mojo_runtime_doctor_route_marker_matches_v1(
                    2,
                    target,
                    1,
                    route_ptr,
                    1,
                    target,
                    route_ptr,
                    matched_ptr
                ),
                4
            );
            assert_eq!(
                prodex_mojo_runtime_doctor_route_marker_matches_v1(
                    1,
                    target,
                    2,
                    route_ptr,
                    1,
                    target,
                    route_ptr,
                    matched_ptr
                ),
                1
            );
            assert_eq!(
                prodex_mojo_runtime_doctor_route_marker_matches_v1(
                    1,
                    0,
                    1,
                    route_ptr,
                    1,
                    target,
                    route_ptr,
                    matched_ptr
                ),
                1
            );
        }
        assert_eq!(matched, -1);
        let number = view("42");
        let value_ptr = mojo_pointer_address(&number);
        let mut output = [-7_i64; 2];
        let output_ptr = mojo_mut_pointer_address(output.as_mut_ptr());
        unsafe {
            assert_eq!(
                prodex_mojo_runtime_doctor_route_marker_number_v1(2, value_ptr, 1, 0, output_ptr),
                4
            );
            assert_eq!(
                prodex_mojo_runtime_doctor_route_marker_number_v1(1, value_ptr, 2, 0, output_ptr),
                1
            );
            assert_eq!(
                prodex_mojo_runtime_doctor_route_marker_number_v1(1, value_ptr, 1, 2, output_ptr),
                1
            );
            assert_eq!(
                prodex_mojo_runtime_doctor_route_marker_number_v1(1, value_ptr, 1, 1, 0),
                1
            );
        }
        assert_eq!(output, [-7, -7]);
    }
}
