//! Bounded runtime-doctor degraded-route selection; Mojo owns the reducer.
use super::{MojoError, ensure_rich_abi, mojo_mut_pointer_address, mojo_pointer_address, view};

const RUNTIME_DOCTOR_DEGRADED_ROUTES_ABI_VERSION: i64 = 1;
pub const RUNTIME_DOCTOR_DEGRADED_ROUTES_MAX_ROWS: usize = 8;
const RUNTIME_DOCTOR_DEGRADED_ROUTES_MAX_INPUTS: usize = 65_536;

pub const RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_CIRCUIT: i64 = 0;
pub const RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_TRANSPORT: i64 = 1;
pub const RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_RETRY: i64 = 2;
pub const RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_BAD_PAIRING: i64 = 3;
pub const RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_HEALTH: i64 = 4;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeDoctorDegradedRouteRecord {
    pub kind: i64,
    pub source_index: i64,
    pub profile_start: i64,
    pub profile_len: i64,
    pub route_start: i64,
    pub route_len: i64,
    pub value: i64,
}

/// Mojo-selected rows and the borrowed map keys used to materialize them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeDoctorDegradedRoutePlan<'a> {
    pub records: Vec<RuntimeDoctorDegradedRouteRecord>,
    pub keys: Vec<&'a str>,
}

unsafe extern "C" {
    fn prodex_mojo_runtime_doctor_degraded_route_render_v1(
        abi_version: i64,
        kind: i64,
        profile: u64,
        route: u64,
        route_present: i64,
        value: i64,
        now: i64,
        output: u64,
        output_capacity: i64,
        written: u64,
    ) -> i64;
    fn prodex_mojo_runtime_doctor_degraded_routes_v1(
        abi_version: i64,
        now: i64,
        health_decay_seconds: i64,
        bad_pairing_decay_seconds: i64,
        keys: u64,
        values: u64,
        updated: u64,
        group_starts: u64,
        total_count: i64,
        output: u64,
        output_capacity: i64,
        output_count: u64,
    ) -> i64;
}

fn status_error(status: i64) -> MojoError {
    match status {
        1 => MojoError::InvalidInput,
        2 => MojoError::InvalidOutput,
        3 => MojoError::Capacity,
        4 => MojoError::AbiMismatch,
        _ => MojoError::InvalidOutput,
    }
}

fn record_is_valid(record: RuntimeDoctorDegradedRouteRecord, key_len: usize) -> bool {
    if !(RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_CIRCUIT..=RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_HEALTH)
        .contains(&record.kind)
        || record.source_index < 0
        || record.profile_start < 0
        || record.profile_len < 0
        || record.profile_start as usize > key_len
        || record.profile_len as usize > key_len - record.profile_start as usize
    {
        return false;
    }
    if record.route_start == -1 {
        return matches!(
            record.kind,
            RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_TRANSPORT | RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_RETRY
        ) && record.route_len == 0;
    }
    record.route_start >= 0
        && record.route_len >= 0
        && record.route_start as usize <= key_len
        && record.route_len as usize <= key_len - record.route_start as usize
        && matches!(
            record.kind,
            RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_CIRCUIT
                | RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_TRANSPORT
                | RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_BAD_PAIRING
                | RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_HEALTH
        )
}

/// Select, decay, sort, deduplicate, and truncate degraded route rows in Mojo.
pub fn runtime_doctor_degraded_routes<'a, RI, TI, CI, SI>(
    retry_backoffs: RI,
    transport_backoffs: TI,
    circuits: CI,
    scores: SI,
    now: i64,
    health_decay_seconds: i64,
    bad_pairing_decay_seconds: i64,
) -> Result<RuntimeDoctorDegradedRoutePlan<'a>, MojoError>
where
    RI: IntoIterator<Item = (&'a str, i64)>,
    TI: IntoIterator<Item = (&'a str, i64)>,
    CI: IntoIterator<Item = (&'a str, i64)>,
    SI: IntoIterator<Item = (&'a str, i64, i64)>,
{
    ensure_rich_abi()?;
    let retry_backoffs = retry_backoffs
        .into_iter()
        .map(|(key, value)| (key, value, 0_i64))
        .collect::<Vec<_>>();
    let transport_backoffs = transport_backoffs
        .into_iter()
        .map(|(key, value)| (key, value, 0_i64))
        .collect::<Vec<_>>();
    let circuits = circuits
        .into_iter()
        .map(|(key, value)| (key, value, 0_i64))
        .collect::<Vec<_>>();
    let scores = scores.into_iter().collect::<Vec<_>>();
    let groups = [
        retry_backoffs.as_slice(),
        transport_backoffs.as_slice(),
        circuits.as_slice(),
        scores.as_slice(),
    ];
    let total_count = groups.iter().map(|group| group.len()).sum::<usize>();
    if total_count > RUNTIME_DOCTOR_DEGRADED_ROUTES_MAX_INPUTS {
        return Err(MojoError::Capacity);
    }

    let mut keys = Vec::with_capacity(total_count);
    let mut key_views = Vec::with_capacity(total_count);
    let mut values = Vec::with_capacity(total_count);
    let mut updated = Vec::with_capacity(total_count);
    let mut group_starts = [0_i64; 5];
    for (group_index, group) in groups.iter().enumerate() {
        for &(key, value, updated_at) in *group {
            keys.push(key);
            key_views.push(view(key));
            values.push(value);
            updated.push(updated_at);
        }
        group_starts[group_index + 1] =
            i64::try_from(keys.len()).map_err(|_| MojoError::InvalidInput)?;
    }

    let total_count = i64::try_from(total_count).map_err(|_| MojoError::InvalidInput)?;
    let mut records =
        vec![RuntimeDoctorDegradedRouteRecord::default(); RUNTIME_DOCTOR_DEGRADED_ROUTES_MAX_ROWS];
    let mut output_count = 0_i64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_degraded_routes_v1(
            RUNTIME_DOCTOR_DEGRADED_ROUTES_ABI_VERSION,
            now,
            health_decay_seconds.max(1),
            bad_pairing_decay_seconds.max(1),
            mojo_pointer_address(key_views.as_ptr()),
            mojo_pointer_address(values.as_ptr()),
            mojo_pointer_address(updated.as_ptr()),
            mojo_pointer_address(group_starts.as_ptr()),
            total_count,
            mojo_mut_pointer_address(records.as_mut_ptr()),
            RUNTIME_DOCTOR_DEGRADED_ROUTES_MAX_ROWS as i64,
            mojo_mut_pointer_address(&mut output_count),
        )
    };
    if status != 0 {
        return Err(status_error(status));
    }
    let output_count = usize::try_from(output_count).map_err(|_| MojoError::InvalidOutput)?;
    if output_count > RUNTIME_DOCTOR_DEGRADED_ROUTES_MAX_ROWS {
        return Err(MojoError::InvalidOutput);
    }
    records.truncate(output_count);
    for record in &records {
        let source_index =
            usize::try_from(record.source_index).map_err(|_| MojoError::InvalidOutput)?;
        let key = keys.get(source_index).ok_or(MojoError::InvalidOutput)?;
        if !record_is_valid(*record, key.len()) {
            return Err(MojoError::InvalidOutput);
        }
    }
    Ok(RuntimeDoctorDegradedRoutePlan { records, keys })
}

/// Materialize one Mojo-selected diagnostic row through the compiled Mojo
/// renderer. The host passes borrowed profile/route text; there is no
/// equivalent Rust formatter for the route kind or signed count.
pub fn runtime_doctor_degraded_route_render(
    kind: i64,
    profile: &str,
    route: Option<&str>,
    value: i64,
    now: i64,
) -> Result<String, MojoError> {
    ensure_rich_abi()?;
    if !(RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_CIRCUIT..=RUNTIME_DOCTOR_DEGRADED_ROUTE_KIND_HEALTH)
        .contains(&kind)
    {
        return Err(MojoError::InvalidInput);
    }
    let capacity = profile
        .len()
        .checked_add(route.map_or(0, str::len))
        .and_then(|value| value.checked_add(100))
        .ok_or(MojoError::InvalidInput)?;
    let profile = view(profile);
    let route_present = i64::from(route.is_some());
    let route = view(route.unwrap_or(""));
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_degraded_route_render_v1(
            RUNTIME_DOCTOR_DEGRADED_ROUTES_ABI_VERSION,
            kind,
            mojo_pointer_address(&profile),
            mojo_pointer_address(&route),
            route_present,
            value,
            now,
            mojo_mut_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut written),
        )
    };
    if status != 0 {
        return Err(status_error(status));
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    String::from_utf8(output).map_err(|_| MojoError::InvalidOutput)
}

#[cfg(test)]
mod route_render_tests {
    use super::*;

    #[test]
    fn mojo_route_renderer_owns_kind_state_and_signed_decimal() {
        for (kind, route, value, now, expected) in [
            (
                0,
                Some("responses"),
                11,
                10,
                "東京/responses circuit=open until=11",
            ),
            (
                0,
                Some("responses"),
                10,
                10,
                "東京/responses circuit=half-open until=10",
            ),
            (
                0,
                Some("websocket"),
                i64::MIN,
                0,
                "東京/websocket circuit=half-open until=-9223372036854775808",
            ),
            (
                1,
                None,
                i64::MAX,
                0,
                "東京/transport transport_backoff until=9223372036854775807",
            ),
            (
                1,
                Some("compact"),
                0,
                0,
                "東京/compact transport_backoff until=0",
            ),
            (2, None, -1, 0, "東京/retry retry_backoff until=-1"),
            (3, Some("compact"), 3, 0, "東京/compact bad_pairing=3"),
            (4, Some("responses"), 0, 0, "東京/responses health=0"),
        ] {
            assert_eq!(
                runtime_doctor_degraded_route_render(kind, "東京", route, value, now).unwrap(),
                expected,
            );
        }
    }

    #[test]
    fn mojo_route_renderer_rejects_malformed_input_before_mutation() {
        let profile = view("alpha");
        let route = view("responses");
        let mut output = [99_u8; 128];
        let mut written = 77_i64;
        unsafe {
            assert_eq!(
                prodex_mojo_runtime_doctor_degraded_route_render_v1(
                    2,
                    0,
                    mojo_pointer_address(&profile),
                    mojo_pointer_address(&route),
                    1,
                    11,
                    10,
                    mojo_mut_pointer_address(output.as_mut_ptr()),
                    128,
                    mojo_mut_pointer_address(&mut written),
                ),
                4,
            );
            assert_eq!(
                prodex_mojo_runtime_doctor_degraded_route_render_v1(
                    1,
                    0,
                    mojo_pointer_address(&profile),
                    mojo_pointer_address(&route),
                    0,
                    11,
                    10,
                    mojo_mut_pointer_address(output.as_mut_ptr()),
                    128,
                    mojo_mut_pointer_address(&mut written),
                ),
                1,
            );
            assert_eq!(
                prodex_mojo_runtime_doctor_degraded_route_render_v1(
                    1,
                    5,
                    mojo_pointer_address(&profile),
                    mojo_pointer_address(&route),
                    1,
                    11,
                    10,
                    mojo_mut_pointer_address(output.as_mut_ptr()),
                    128,
                    mojo_mut_pointer_address(&mut written),
                ),
                1,
            );
        }
        assert_eq!(output, [99; 128]);
        assert_eq!(written, 77);
    }
}
