use super::{
    MojoError, ensure_rich_abi, mojo_mut_pointer_address, mojo_pointer_address, status_error, view,
};

pub const RUNTIME_DOCTOR_RENDER_ABI_VERSION: i64 = 1;
/// Mojo renderer operation for request-timeline detail fields.
pub const RUNTIME_DOCTOR_RENDER_REQUEST_TIMELINE_DETAIL: i64 = 22;
/// Mojo renderer operation for the bounded last-marker log line.
pub const RUNTIME_DOCTOR_RENDER_LAST_MARKER_LINE_TRUNCATION: i64 = 23;
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
    fn prodex_mojo_runtime_doctor_render_v1(
        abi_version: i64,
        input: u64,
        output: u64,
        output_capacity: i64,
        written: u64,
    ) -> i64;
}

pub fn runtime_doctor_render(input: RuntimeDoctorRenderInput<'_>) -> Result<String, MojoError> {
    ensure_rich_abi()?;
    if !(0..=RUNTIME_DOCTOR_RENDER_LAST_MARKER_LINE_TRUNCATION).contains(&input.operation)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rich::RichStringView;

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
