use super::*;

#[test]
fn compiled_quota_duration_preserves_compound_units_whitespace_and_rounding() {
    for (text, expected) in [
        ("", None),
        (" \t\u{2003}", None),
        ("0", Some(0)),
        ("0ms", Some(0)),
        (".5", Some(1)),
        ("1d2h3m4s5ms6", Some(93_791)),
        ("1h30m", Some(5_400)),
        ("500ms500ms", Some(1)),
        ("500ms501ms", Some(2)),
        ("1 2 . 5s", Some(13)),
        ("\u{2003}1\t.\n5h\u{3000}", Some(5_400)),
        ("\u{b}1s\u{b}", Some(1)),
        ("1\u{b}s", None),
        ("1\u{2003}s", None),
        ("1m s", None),
        ("1M", None),
        ("1e3s", None),
        ("-1s", None),
        ("+1s", None),
        ("NaNs", None),
        ("infs", None),
        (".s", None),
        ("1s..s", None),
        ("1s2ss", None),
        ("1s\0", None),
        ("1s東京", None),
        ("9223372036854775808s", Some(i64::MAX)),
    ] {
        assert_eq!(
            provider_quota_duration_seconds(text),
            Ok(expected),
            "{text:?}"
        );
    }
    assert_eq!(
        provider_quota_duration_seconds(&format!("{}d", "9".repeat(400))),
        Ok(Some(i64::MAX))
    );
    assert_eq!(
        provider_quota_duration_seconds(&format!("0.{}1ms", "0".repeat(330))),
        Ok(Some(0))
    );
}

#[test]
fn compiled_quota_duration_uses_probed_storage_without_a_new_segment_cap() {
    assert_eq!(
        provider_quota_duration_seconds(&"0s".repeat(65_537)),
        Ok(Some(0))
    );
}

#[test]
fn quota_duration_plan_checks_capacity_and_invalid_utf8_before_writes() {
    let text = "1h2ms3";
    let mut numbers = [77_u8; 3];
    let mut rows = [71_i64; 9];
    let mut result = [-1_i64; 3];
    let number_address = numbers.as_mut_ptr() as u64;
    let row_address = rows.as_mut_ptr() as u64;
    let call = |version, source, length, bytes, capacity, rows, count, result: &mut [i64; 3]| unsafe {
        prodex_provider_quota_duration_plan_v1(
            version,
            source,
            length,
            bytes,
            capacity,
            rows,
            count,
            result.as_mut_ptr() as u64,
        )
    };
    let source = text.as_ptr() as u64;
    assert_eq!(call(2, source, 6, 0, 0, 0, 0, &mut result), 4);
    assert_eq!(call(1, source, -1, 0, 0, 0, 0, &mut result), 1);
    assert_eq!(call(1, source, i64::MAX, 0, 0, 0, 0, &mut result), 1);
    assert_eq!(
        call(1, [0xff_u8].as_ptr() as u64, 1, 0, 0, 0, 0, &mut result),
        1
    );
    assert_eq!(call(1, source, 6, 0, 0, 0, 0, &mut result), 0);
    assert_eq!(result, [1, 3, 3]);
    assert_eq!(
        call(1, source, 6, number_address, 2, row_address, 3, &mut result),
        3
    );
    assert_eq!(
        call(1, source, 6, number_address, 3, row_address, 2, &mut result),
        3
    );
    assert_eq!(call(1, source, 6, number_address, 3, 1, 3, &mut result), 1);
    assert_eq!(numbers, [77; 3]);
    assert_eq!(rows, [71; 9]);
    assert_eq!(
        call(1, source, 6, number_address, 3, row_address, 3, &mut result),
        0
    );
    assert_eq!(&numbers, b"123");
    assert_eq!(rows, [0, 1, 3, 1, 2, 1, 2, 3, 0]);
    let invalid = "1h2ms3x";
    assert_eq!(
        call(
            1,
            invalid.as_ptr() as u64,
            invalid.len() as i64,
            number_address,
            3,
            row_address,
            3,
            &mut result
        ),
        0
    );
    assert_eq!(result, [0; 3]);
    assert_eq!(&numbers, b"123");
    assert_eq!(rows, [0, 1, 3, 1, 2, 1, 2, 3, 0]);
}

#[test]
fn quota_duration_finish_rejects_bad_facts_and_keeps_parse_failure_distinct_from_zero() {
    let mut output = [-1_i64; 2];
    let call = |version, values: &[AcquiredNumber], capacity, output: &mut [i64; 2]| unsafe {
        prodex_provider_quota_duration_finish_v1(
            version,
            values.as_ptr() as u64,
            values.len() as i64,
            output.as_mut_ptr() as u64,
            capacity,
        )
    };
    let mut values = [AcquiredNumber {
        unit: 0,
        parsed: 1,
        amount: 0.0,
    }];
    assert_eq!(call(2, &values, 2, &mut output), 4);
    assert_eq!(call(1, &[], 2, &mut output), 1);
    assert_eq!(call(1, &values, 1, &mut output), 3);
    assert_eq!(call(1, &values, 2, &mut output), 0);
    assert_eq!(output, [1, 0]);
    values[0].parsed = 0;
    assert_eq!(call(1, &values, 2, &mut output), 0);
    assert_eq!(output, [0, 0]);
    for (unit, parsed, amount) in [
        (5, 1, 1.0),
        (0, 2, 1.0),
        (0, 1, -1.0),
        (0, 1, f64::NAN),
        (0, 0, 1.0),
    ] {
        values[0] = AcquiredNumber {
            unit,
            parsed,
            amount,
        };
        assert_eq!(call(1, &values, 2, &mut output), 1);
        assert_eq!(output, [0, 0]);
    }
}
