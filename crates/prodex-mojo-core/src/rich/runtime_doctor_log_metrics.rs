//! Versioned Mojo planner for runtime-doctor log counter precedence and
//! decimal value decoding. Rust only marshals borrowed marker fields.

use super::{ensure_rich_abi, mojo_mut_pointer_address, mojo_pointer_address, view};
use crate::MojoError;

const SOURCE_COUNT: usize = 13;
const RESULT_COUNT: usize = 6;

unsafe extern "C" {
    fn prodex_mojo_runtime_doctor_unsigned_field_v1(
        abi_version: i64,
        value_view: u64,
        input_present: i64,
        maximum: u64,
        output_value: u64,
        output_present: u64,
    ) -> i64;
    fn prodex_mojo_runtime_doctor_log_metric_plan_v1(
        abi_version: i64,
        views: u64,
        present: u64,
        source_count: i64,
        max_backlog: u64,
        values: u64,
        output_present: u64,
        capacity: i64,
    ) -> i64;
}

/// Each input is one borrowed marker field in documented priority order.
/// No Rust numeric parser or alternate source selector is used.
pub fn runtime_doctor_log_metric_plan(
    fields: [Option<&str>; SOURCE_COUNT],
) -> Result<[Option<u64>; RESULT_COUNT], MojoError> {
    ensure_rich_abi()?;
    let views = fields.map(|field| view(field.unwrap_or("")));
    let present = fields.map(|field| i64::from(field.is_some()));
    let mut values = [0_u64; RESULT_COUNT];
    let mut output_present = [-1_i64; RESULT_COUNT];
    let status = unsafe {
        prodex_mojo_runtime_doctor_log_metric_plan_v1(
            1,
            mojo_pointer_address(views.as_ptr()),
            mojo_pointer_address(present.as_ptr()),
            SOURCE_COUNT as i64,
            usize::MAX as u64,
            mojo_mut_pointer_address(values.as_mut_ptr()),
            mojo_mut_pointer_address(output_present.as_mut_ptr()),
            RESULT_COUNT as i64,
        )
    };
    if status != 0 {
        return Err(match status {
            1 => MojoError::InvalidInput,
            4 => MojoError::AbiMismatch,
            _ => MojoError::InvalidOutput,
        });
    }
    let mut result = [None; RESULT_COUNT];
    for index in 0..RESULT_COUNT {
        result[index] = match output_present[index] {
            0 => None,
            1 => Some(values[index]),
            _ => return Err(MojoError::InvalidOutput),
        };
    }
    Ok(result)
}

/// Decode an optional unsigned marker field only through compiled Mojo.
/// Rust keeps the target-size projection and never re-parses invalid input.
pub fn runtime_doctor_unsigned_field(value: Option<&str>) -> Result<Option<usize>, MojoError> {
    ensure_rich_abi()?;
    let source = view(value.unwrap_or(""));
    let mut parsed = 0_u64;
    let mut present = -1_i64;
    let status = unsafe {
        prodex_mojo_runtime_doctor_unsigned_field_v1(
            1,
            mojo_pointer_address(&source),
            i64::from(value.is_some()),
            usize::MAX as u64,
            mojo_mut_pointer_address(&mut parsed),
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
    match present {
        0 => Ok(None),
        1 => usize::try_from(parsed)
            .map(Some)
            .map_err(|_| MojoError::InvalidOutput),
        _ => Err(MojoError::InvalidOutput),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mojo_optional_unsigned_field_has_no_rust_parser_fallback() {
        for (source, expected) in [
            (None, None),
            (Some(""), None),
            (Some("+"), None),
            (Some("+00017"), Some(17)),
            (Some("0"), Some(0)),
            (Some("-4"), None),
            (Some(" 7"), None),
            (Some("7 "), None),
            (Some("🧭"), None),
        ] {
            assert_eq!(
                runtime_doctor_unsigned_field(source).unwrap(),
                expected,
                "{source:?}"
            );
        }
        assert_eq!(
            runtime_doctor_unsigned_field(Some(&usize::MAX.to_string())).unwrap(),
            Some(usize::MAX),
        );
        assert_eq!(
            runtime_doctor_unsigned_field(Some(&format!("{}0", usize::MAX))).unwrap(),
            None,
        );
    }

    #[test]
    fn mojo_unsigned_field_rejects_malformed_abi_before_touching_output() {
        let view = view("123");
        let mut value = 77_u64;
        let mut present = 77_i64;
        unsafe {
            assert_eq!(
                prodex_mojo_runtime_doctor_unsigned_field_v1(
                    2,
                    mojo_pointer_address(&view),
                    1,
                    100,
                    mojo_mut_pointer_address(&mut value),
                    mojo_mut_pointer_address(&mut present),
                ),
                4,
            );
            assert_eq!(
                prodex_mojo_runtime_doctor_unsigned_field_v1(
                    1,
                    mojo_pointer_address(&view),
                    2,
                    100,
                    mojo_mut_pointer_address(&mut value),
                    mojo_mut_pointer_address(&mut present),
                ),
                1,
            );
        }
        assert_eq!(value, 77);
        assert_eq!(present, 77);
    }

    #[test]
    fn mojo_metric_planner_owns_priority_and_decimal_parsing() {
        // Groups: state backlog(2), state lag(3), journal backlog(2),
        // journal lag(2), probe backlog(2), probe lag(2).
        let fields = [
            Some("-1"),
            Some("+0007"),
            Some("18446744073709551616"),
            Some("18446744073709551615"),
            Some("9"),
            Some(""),
            Some("003"),
            Some("18 "),
            Some("17"),
            Some("2"),
            Some("500"),
            Some("🧭"),
            Some("0"),
        ];
        let result = runtime_doctor_log_metric_plan(fields).unwrap();
        assert_eq!(
            result,
            [Some(7), Some(u64::MAX), Some(3), Some(17), Some(2), Some(0)]
        );

        let absent = runtime_doctor_log_metric_plan([None; SOURCE_COUNT]).unwrap();
        assert_eq!(absent, [None; RESULT_COUNT]);
        assert_eq!(
            runtime_doctor_log_metric_plan([
                Some(" 9"),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ])
            .unwrap(),
            [None; RESULT_COUNT],
        );
    }

    #[test]
    fn mojo_metric_abi_rejects_invalid_inputs_without_touching_output() {
        let views = [view("123"); SOURCE_COUNT];
        let mut present = [0_i64; SOURCE_COUNT];
        present[0] = 2;
        let mut values = [99_u64; RESULT_COUNT];
        let mut found = [77_i64; RESULT_COUNT];
        let mut call = |abi, input_count, capacity, flags: &[i64; SOURCE_COUNT]| unsafe {
            prodex_mojo_runtime_doctor_log_metric_plan_v1(
                abi,
                mojo_pointer_address(views.as_ptr()),
                mojo_pointer_address(flags.as_ptr()),
                input_count,
                100,
                mojo_mut_pointer_address(values.as_mut_ptr()),
                mojo_mut_pointer_address(found.as_mut_ptr()),
                capacity,
            )
        };
        assert_eq!(call(2, 13, 6, &present), 4);
        assert_eq!(call(1, 13, 6, &present), 1);
        present[0] = 1;
        assert_eq!(call(1, 12, 6, &present), 1);
        assert_eq!(call(1, 13, 5, &present), 1);
        assert_eq!(values, [99; RESULT_COUNT]);
        assert_eq!(found, [77; RESULT_COUNT]);
    }

    #[test]
    fn mojo_metric_abi_respects_platform_backlog_bounds() {
        let views = [view("4294967296"); SOURCE_COUNT];
        let present = [1_i64; SOURCE_COUNT];
        let mut values = [0_u64; RESULT_COUNT];
        let mut flags = [-1_i64; RESULT_COUNT];
        let status = unsafe {
            prodex_mojo_runtime_doctor_log_metric_plan_v1(
                1,
                mojo_pointer_address(views.as_ptr()),
                mojo_pointer_address(present.as_ptr()),
                13,
                u32::MAX as u64,
                mojo_mut_pointer_address(values.as_mut_ptr()),
                mojo_mut_pointer_address(flags.as_mut_ptr()),
                6,
            )
        };
        assert_eq!(status, 0);
        assert_eq!(flags, [0, 1, 0, 1, 0, 1]);
        assert_eq!(values, [0, 4294967296, 0, 4294967296, 0, 4294967296]);
    }
}
