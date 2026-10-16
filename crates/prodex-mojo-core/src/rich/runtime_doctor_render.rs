use super::{
    MojoError, ensure_rich_abi, mojo_mut_pointer_address, mojo_pointer_address, status_error, view,
};

pub const RUNTIME_DOCTOR_RENDER_ABI_VERSION: i64 = 1;
/// Mojo renderer operation for request-timeline detail fields.
pub const RUNTIME_DOCTOR_RENDER_REQUEST_TIMELINE_DETAIL: i64 = 22;
/// Mojo renderer operation for the bounded last-marker log line.
pub const RUNTIME_DOCTOR_RENDER_LAST_MARKER_LINE_TRUNCATION: i64 = 23;
/// Mojo renderer operation for compact chain-event summaries.
pub const RUNTIME_DOCTOR_RENDER_CHAIN_EVENT_SUMMARY: i64 = 24;
/// Mojo renderer operation for doctor configuration status.
pub const RUNTIME_DOCTOR_RENDER_CONFIGURATION_STATUS: i64 = 25;
/// Mojo renderer operation for the latest human-readable selection decision.
pub const RUNTIME_DOCTOR_RENDER_SELECTION_LATEST_DECISION: i64 = 26;
const RUNTIME_DOCTOR_RENDER_VALUE_COUNT: usize = 16;

pub struct RuntimeDoctorRenderInput<'a> {
    pub operation: i64,
    pub detail: i64,
    pub values: &'a [Option<&'a str>],
}

#[repr(C)]
struct RuntimeDoctorRenderFfiInput {
    operation: i64,
    detail: i64,
    presence: u64,
    values: u64,
}

const _: () = assert!(std::mem::size_of::<RuntimeDoctorRenderFfiInput>() == 4 * 8);

unsafe extern "C" {
    fn prodex_mojo_runtime_doctor_scope_v1(
        abi_version: i64,
        views: u64,
        presence_bits: i64,
        output: u64,
        output_capacity: i64,
        written: u64,
        present: u64,
    ) -> i64;
    fn prodex_mojo_runtime_doctor_count_breakdown_v1(
        abi_version: i64,
        keys: u64,
        counts: u64,
        count: i64,
        output: u64,
        output_capacity: i64,
        written: u64,
    ) -> i64;
    fn prodex_mojo_runtime_doctor_timeline_append_v1(
        abi_version: i64,
        existing_count: u64,
        evict_oldest: u64,
    ) -> i64;
    fn prodex_mojo_runtime_doctor_timeline_latest_v1(
        abi_version: i64,
        indices: u64,
        count: i64,
        selected: u64,
    ) -> i64;
    fn prodex_mojo_runtime_doctor_render_v1(
        abi_version: i64,
        input: u64,
        output: u64,
        output_capacity: i64,
        written: u64,
    ) -> i64;
}

/// Plan whether adding one request event removes the oldest stored event.
pub fn runtime_doctor_timeline_evict_oldest(existing_events: usize) -> Result<bool, MojoError> {
    ensure_rich_abi()?;
    let mut evict = -1_i64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_timeline_append_v1(
            RUNTIME_DOCTOR_RENDER_ABI_VERSION,
            u64::try_from(existing_events).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut evict),
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    match evict {
        0 => Ok(false),
        1 => Ok(true),
        _ => Err(MojoError::InvalidOutput),
    }
}

/// Choose the latest request in caller-provided BTreeMap order. For equal
/// last-log-line indices, the lexically greatest request ID wins.
pub fn runtime_doctor_latest_timeline_index(
    last_indices: &[usize],
) -> Result<Option<usize>, MojoError> {
    ensure_rich_abi()?;
    let indices = last_indices
        .iter()
        .map(|index| u64::try_from(*index).map_err(|_| MojoError::InvalidInput))
        .collect::<Result<Vec<_>, _>>()?;
    let mut selected = -2_i64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_timeline_latest_v1(
            RUNTIME_DOCTOR_RENDER_ABI_VERSION,
            mojo_pointer_address(indices.as_ptr()),
            i64::try_from(indices.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut selected),
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    match selected {
        -1 if indices.is_empty() => Ok(None),
        value if value >= 0 => usize::try_from(value)
            .ok()
            .filter(|index| *index < indices.len())
            .map(Some)
            .ok_or(MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
}

pub fn runtime_doctor_render(input: RuntimeDoctorRenderInput<'_>) -> Result<String, MojoError> {
    ensure_rich_abi()?;
    if !(0..=RUNTIME_DOCTOR_RENDER_SELECTION_LATEST_DECISION).contains(&input.operation)
        || !(0..=799).contains(&input.detail)
        || input.values.len() > RUNTIME_DOCTOR_RENDER_VALUE_COUNT
        || input
            .values
            .iter()
            .flatten()
            .any(|value| value.len() > i64::MAX as usize)
    {
        return Err(MojoError::InvalidInput);
    }
    let capacity = input
        .values
        .iter()
        .flatten()
        .try_fold(2048_usize, |capacity, value| {
            capacity.checked_add(value.len().checked_mul(2)?)
        })
        .ok_or(MojoError::InvalidInput)?;
    let mut values = [view(""); RUNTIME_DOCTOR_RENDER_VALUE_COUNT];
    let mut presence = 0_u64;
    for (index, value) in input.values.iter().enumerate() {
        if let Some(value) = value {
            values[index] = view(value);
            presence |= 1 << index;
        }
    }
    let ffi_input = RuntimeDoctorRenderFfiInput {
        operation: input.operation,
        detail: input.detail,
        presence,
        values: mojo_pointer_address(values.as_ptr()),
    };
    let mut output = vec![0_u8; capacity];
    let mut written = 0_i64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_render_v1(
            RUNTIME_DOCTOR_RENDER_ABI_VERSION,
            mojo_pointer_address(&ffi_input),
            mojo_mut_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut written),
        )
    };
    if status != 0 {
        return Err(status_error(status, 12, input.operation, 0, 0));
    }
    let written = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if written > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(written);
    String::from_utf8(output).map_err(|_| MojoError::InvalidOutput)
}

/// Materialize a profile/route scope strictly from the compiled Mojo policy.
pub fn runtime_doctor_marker_scope_text(
    profile: Option<&str>,
    route: Option<&str>,
) -> Result<Option<String>, MojoError> {
    ensure_rich_abi()?;
    let views = [view(profile.unwrap_or("")), view(route.unwrap_or(""))];
    let presence = i64::from(profile.is_some()) | (i64::from(route.is_some()) << 1);
    let capacity = profile
        .map_or(0, str::len)
        .checked_add(route.map_or(0, str::len))
        .and_then(|length| length.checked_add(1))
        .ok_or(MojoError::InvalidInput)?;
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    let mut present = -1_i64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_scope_v1(
            RUNTIME_DOCTOR_RENDER_ABI_VERSION,
            mojo_pointer_address(views.as_ptr()),
            presence,
            mojo_mut_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut written),
            mojo_mut_pointer_address(&mut present),
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    let length = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if length > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    match present {
        0 if length == 0 => Ok(None),
        1 => {
            output.truncate(length);
            String::from_utf8(output)
                .map(Some)
                .map_err(|_| MojoError::InvalidOutput)
        }
        _ => Err(MojoError::InvalidOutput),
    }
}

/// The host supplies BTreeMap-ordered borrowed names and counts; Mojo emits
/// the exact label=count list including zero and u64::MAX, without a Rust
/// formatter, alternate separator builder or numeric display fallback.
pub fn runtime_doctor_count_breakdown_text<'a>(
    entries: impl IntoIterator<Item = (&'a str, usize)>,
) -> Result<String, MojoError> {
    ensure_rich_abi()?;
    let entries = entries.into_iter().collect::<Vec<_>>();
    if entries.len() > 65536 {
        return Err(MojoError::Capacity);
    }
    let capacity = entries.iter().try_fold(1usize, |capacity, (key, _)| {
        capacity.checked_add(key.len().checked_add(23)?)
    });
    let capacity = capacity.ok_or(MojoError::InvalidInput)?;
    let keys = entries.iter().map(|(key, _)| view(key)).collect::<Vec<_>>();
    let counts = entries
        .iter()
        .map(|(_, count)| *count as u64)
        .collect::<Vec<_>>();
    let mut output = vec![0_u8; capacity];
    let mut written = -1_i64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_count_breakdown_v1(
            RUNTIME_DOCTOR_RENDER_ABI_VERSION,
            mojo_pointer_address(keys.as_ptr()),
            mojo_pointer_address(counts.as_ptr()),
            i64::try_from(entries.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(output.as_mut_ptr()),
            i64::try_from(output.len()).map_err(|_| MojoError::InvalidInput)?,
            mojo_mut_pointer_address(&mut written),
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            3 => MojoError::Capacity,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    let length = usize::try_from(written).map_err(|_| MojoError::InvalidOutput)?;
    if length > output.len() {
        return Err(MojoError::InvalidOutput);
    }
    output.truncate(length);
    String::from_utf8(output).map_err(|_| MojoError::InvalidOutput)
}

/// Render the redaction-safe human status for runtime configuration parsing.
pub fn runtime_doctor_configuration_status(error: Option<&str>) -> Result<String, MojoError> {
    runtime_doctor_render(RuntimeDoctorRenderInput {
        operation: RUNTIME_DOCTOR_RENDER_CONFIGURATION_STATUS,
        detail: 0,
        values: &[error],
    })
}

#[cfg(test)]
mod selection_latest_render_tests {
    use super::*;

    #[test]
    fn mojo_selection_latest_decision_renders_marker_and_optional_details() {
        let render = |marker: &str, detail: Option<&str>| {
            runtime_doctor_render(RuntimeDoctorRenderInput {
                operation: RUNTIME_DOCTOR_RENDER_SELECTION_LATEST_DECISION,
                detail: 0,
                values: &[Some(marker), detail],
            })
            .unwrap()
        };
        assert_eq!(render("selection_pick", None), "selection_pick");
        assert_eq!(render("selection_pick", Some("")), "selection_pick");
        assert_eq!(
            render("selection_skip_affinity", Some("profile=東京 reason=🧭")),
            "selection_skip_affinity profile=東京 reason=🧭"
        );
    }
}

#[cfg(test)]
mod timeline_policy_tests {
    use super::*;

    #[test]
    fn compiled_timeline_retention_and_latest_index_keep_oldest_and_last_tie() {
        for (existing, evict) in [(0, false), (11, false), (12, true), (13, true)] {
            assert_eq!(runtime_doctor_timeline_evict_oldest(existing), Ok(evict));
        }
        assert_eq!(runtime_doctor_timeline_evict_oldest(usize::MAX), Ok(true));
        assert_eq!(runtime_doctor_latest_timeline_index(&[]), Ok(None));
        assert_eq!(runtime_doctor_latest_timeline_index(&[0]), Ok(Some(0)));
        assert_eq!(
            runtime_doctor_latest_timeline_index(&[7, 9, 8]),
            Ok(Some(1))
        );
        assert_eq!(
            runtime_doctor_latest_timeline_index(&[9, 9, 7, 9]),
            Ok(Some(3))
        );
        assert_eq!(
            runtime_doctor_latest_timeline_index(&[usize::MAX, 0, usize::MAX]),
            Ok(Some(2))
        );
    }

    #[test]
    fn compiled_timeline_rejects_invalid_abi_without_writing() {
        let mut out = 77_i64;
        let values = [4_u64, 12];
        unsafe {
            assert_eq!(
                prodex_mojo_runtime_doctor_timeline_append_v1(
                    2,
                    12,
                    mojo_mut_pointer_address(&mut out)
                ),
                4
            );
            assert_eq!(prodex_mojo_runtime_doctor_timeline_append_v1(1, 12, 0), 1);
            assert_eq!(
                prodex_mojo_runtime_doctor_timeline_latest_v1(
                    2,
                    mojo_pointer_address(values.as_ptr()),
                    2,
                    mojo_mut_pointer_address(&mut out),
                ),
                4
            );
            assert_eq!(
                prodex_mojo_runtime_doctor_timeline_latest_v1(
                    1,
                    0,
                    2,
                    mojo_mut_pointer_address(&mut out)
                ),
                1
            );
            assert_eq!(
                prodex_mojo_runtime_doctor_timeline_latest_v1(
                    1,
                    mojo_pointer_address(values.as_ptr()),
                    -1,
                    mojo_mut_pointer_address(&mut out)
                ),
                1
            );
        }
        assert_eq!(out, 77);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rich::RichStringView;

    #[test]
    fn mojo_marker_scope_preserves_all_presence_and_utf8_cases() {
        for (profile, route, expected) in [
            (None, None, None),
            (Some(""), None, Some("")),
            (None, Some(""), Some("")),
            (Some("🧭"), None, Some("🧭")),
            (None, Some("responses"), Some("responses")),
            (Some("東京"), Some("compact"), Some("東京/compact")),
            (Some(""), Some(""), Some("/")),
        ] {
            assert_eq!(
                runtime_doctor_marker_scope_text(profile, route)
                    .unwrap()
                    .as_deref(),
                expected,
            );
        }
    }

    #[test]
    fn mojo_count_breakdown_writes_ordered_labels_zero_and_maximum() {
        assert_eq!(runtime_doctor_count_breakdown_text([]).unwrap(), "-");
        let entries = [("alpha", 0), ("service 🧭", 12), ("z", usize::MAX)];
        assert_eq!(
            runtime_doctor_count_breakdown_text(entries).unwrap(),
            format!("alpha=0, service 🧭=12, z={}", usize::MAX),
        );
    }

    #[test]
    fn mojo_text_summary_abi_rejects_unknown_presence_and_short_output() {
        let views = [view("alpha"), view("responses")];
        let mut output = [55_u8; 8];
        let mut written = [77_i64; 2];
        unsafe {
            assert_eq!(
                prodex_mojo_runtime_doctor_scope_v1(
                    2,
                    mojo_pointer_address(views.as_ptr()),
                    3,
                    mojo_mut_pointer_address(output.as_mut_ptr()),
                    8,
                    mojo_mut_pointer_address(&mut written[0]),
                    mojo_mut_pointer_address(&mut written[1]),
                ),
                4,
            );
            assert_eq!(
                prodex_mojo_runtime_doctor_scope_v1(
                    1,
                    mojo_pointer_address(views.as_ptr()),
                    4,
                    mojo_mut_pointer_address(output.as_mut_ptr()),
                    8,
                    mojo_mut_pointer_address(&mut written[0]),
                    mojo_mut_pointer_address(&mut written[1]),
                ),
                1,
            );
        }
        assert_eq!(output, [55; 8]);
        assert_eq!(written, [77; 2]);
    }

    #[test]
    fn renderer_preserves_unicode_and_repeated_values() {
        assert_eq!(
            runtime_doctor_render(RuntimeDoctorRenderInput {
                operation: 7,
                detail: 12,
                values: &[Some("配置"), Some("responses"), Some("expired")],
            })
            .unwrap(),
            "Refresh credentials for profile 配置 with `prodex login --profile 配置` and retry route responses; latest recovery error: expired."
        );
    }

    #[test]
    fn configuration_status_uses_mojo_precedence() {
        assert_eq!(runtime_doctor_configuration_status(None).unwrap(), "Valid");
        assert_eq!(
            runtime_doctor_configuration_status(Some("worker count must be greater than zero"))
                .unwrap(),
            "Invalid: worker count must be greater than zero"
        );
    }

    #[test]
    fn timeline_detail_preserves_order_caps_fields_and_truncates_unicode() {
        let route = "🧭".repeat(49);
        assert_eq!(
            runtime_doctor_render(RuntimeDoctorRenderInput {
                operation: RUNTIME_DOCTOR_RENDER_REQUEST_TIMELINE_DETAIL,
                detail: 0,
                values: &[
                    Some("東京"),
                    Some(&route),
                    None,
                    Some("timeout"),
                    Some("429"),
                    Some("ignored"),
                ],
            })
            .unwrap(),
            format!(
                "profile=東京 route={}... reason=timeout status=429 code=ignored",
                "🧭".repeat(45)
            )
        );
        assert_eq!(
            runtime_doctor_render(RuntimeDoctorRenderInput {
                operation: RUNTIME_DOCTOR_RENDER_REQUEST_TIMELINE_DETAIL,
                detail: 0,
                values: &[Some("profile"), Some(&"🧭".repeat(48))],
            })
            .unwrap(),
            format!("profile=profile route={}", "🧭".repeat(48))
        );
    }

    #[test]
    fn chain_event_summary_preserves_field_order_and_omits_absent_values() {
        assert_eq!(
            runtime_doctor_render(RuntimeDoctorRenderInput {
                operation: RUNTIME_DOCTOR_RENDER_CHAIN_EVENT_SUMMARY,
                detail: 0,
                values: &[
                    Some("chain_retried_owner"),
                    Some("quota"),
                    Some("alpha"),
                    Some("http"),
                    None,
                    Some("42"),
                    None,
                    Some("retry"),
                    Some("rotation"),
                ],
            })
            .unwrap(),
            "chain_retried_owner reason=quota profile=alpha transport=http websocket_session=42 event=retry via=rotation"
        );
    }

    #[test]
    fn last_marker_line_preserves_160_unicode_scalars_and_truncates_at_boundary() {
        let at_limit = "🧭".repeat(160);
        let over_limit = "🧭".repeat(161);
        assert_eq!(
            runtime_doctor_render(RuntimeDoctorRenderInput {
                operation: RUNTIME_DOCTOR_RENDER_LAST_MARKER_LINE_TRUNCATION,
                detail: 0,
                values: &[Some(&at_limit)],
            })
            .unwrap(),
            at_limit
        );
        assert_eq!(
            runtime_doctor_render(RuntimeDoctorRenderInput {
                operation: RUNTIME_DOCTOR_RENDER_LAST_MARKER_LINE_TRUNCATION,
                detail: 0,
                values: &[Some(&over_limit)],
            })
            .unwrap(),
            format!("{}…", "🧭".repeat(159))
        );
    }

    #[test]
    fn renderer_rejects_invalid_abi_pointer_utf8_and_capacity() {
        let invalid = [0xff_u8];
        let mut values = [view(""); RUNTIME_DOCTOR_RENDER_VALUE_COUNT];
        values[0] = RichStringView {
            ptr: mojo_pointer_address(invalid.as_ptr()),
            len: 1,
        };
        let input = RuntimeDoctorRenderFfiInput {
            operation: 0,
            detail: 0,
            presence: 1,
            values: mojo_pointer_address(values.as_ptr()),
        };
        let mut output = [0_u8; 1];
        let mut written = 0_i64;
        let mut call = |abi, input_address, capacity| unsafe {
            prodex_mojo_runtime_doctor_render_v1(
                abi,
                input_address,
                mojo_mut_pointer_address(output.as_mut_ptr()),
                capacity,
                mojo_mut_pointer_address(&mut written),
            )
        };
        assert_eq!(
            call(
                RUNTIME_DOCTOR_RENDER_ABI_VERSION + 1,
                mojo_pointer_address(&input),
                1
            ),
            4
        );
        assert_eq!(call(RUNTIME_DOCTOR_RENDER_ABI_VERSION, 0, 1), 1);
        assert_eq!(
            call(
                RUNTIME_DOCTOR_RENDER_ABI_VERSION,
                mojo_pointer_address(&input),
                1
            ),
            2
        );

        values[0] = view("");
        let input = RuntimeDoctorRenderFfiInput {
            operation: 0,
            detail: 0,
            presence: 1,
            values: mojo_pointer_address(values.as_ptr()),
        };
        assert_eq!(
            call(
                RUNTIME_DOCTOR_RENDER_ABI_VERSION,
                mojo_pointer_address(&input),
                1
            ),
            3
        );
    }
}
