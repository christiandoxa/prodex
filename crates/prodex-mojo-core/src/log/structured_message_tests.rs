use super::*;

#[test]
fn compiled_structured_message_keeps_prepared_values_order_and_spacing() {
    let fields = vec![
        ("clé".into(), "\"one two\"".into()),
        ("clé".into(), "two".into()),
        ("empty".into(), "\"\"".into()),
    ];
    assert_eq!(
        render_structured_log_message("", &fields),
        Ok("clé=\"one two\" clé=two empty=\"\"".into())
    );
    assert_eq!(render_structured_log_message("", &[]), Ok(String::new()));
    assert_eq!(render_structured_log_message(" ", &[]), Ok(" ".into()));
    assert_eq!(
        render_structured_log_message("évent ", &fields[..1]),
        Ok("évent  clé=\"one two\"".into())
    );
    assert_eq!(
        render_structured_log_message("", &vec![(String::new(), String::new()); 65_537]),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn message_policy_adds_empty_key_suppression_without_changing_existing_facade() {
    assert!(
        !super::super::structured_log_field_policy("", "value")
            .unwrap()
            .skip
    );
    assert!(
        structured_log_message_field_policy("", "value")
            .unwrap()
            .skip
    );
    for (key, value) in [
        ("request", "7"),
        ("bad key", "value"),
        ("a=b", "value"),
        ("ERROR", "upstream_timeout"),
        ("error", "arbitrary prose"),
        ("upstream_url", "https://example.com/v1?key=fixture"),
        ("clé", "a\u{a0}b"),
    ] {
        assert_eq!(
            structured_log_message_field_policy(key, value),
            super::super::structured_log_field_policy(key, value)
        );
    }
}

#[test]
fn message_abi_rejects_invalid_views_flags_and_short_capacity_before_writes() {
    let fields = [
        LogStringView {
            ptr: "key".as_ptr() as u64,
            len: 3,
        },
        LogStringView {
            ptr: "value".as_ptr() as u64,
            len: 5,
        },
    ];
    let mut output = [73_u8; 64];
    let mut written = 83_i64;
    let mut call = |version, event, length, views, count, phase, capacity| unsafe {
        prodex_mojo_structured_log_message_v1(
            version,
            event,
            length,
            views,
            count,
            phase,
            output.as_mut_ptr() as u64,
            capacity,
            &mut written as *mut i64 as u64,
        )
    };
    let table = fields.as_ptr() as u64;
    assert_eq!(call(2, 0, 0, 0, 0, 0, 64), 4);
    assert_eq!(call(1, 0, 1, 0, 0, 0, 64), 1);
    assert_eq!(call(1, 0, -1, 0, 0, 0, 64), 1);
    assert_eq!(call(1, 0, 67_108_865, 0, 0, 0, 64), 1);
    assert_eq!(call(1, "é".as_ptr() as u64, 1, 0, 0, 0, 64), 1);
    assert_eq!(call(1, 0, 0, 0, 1, 0, 64), 1);
    assert_eq!(call(1, 0, 0, table + 1, 1, 0, 64), 1);
    assert_eq!(call(1, 0, 0, table, -1, 0, 64), 1);
    assert_eq!(call(1, 0, 0, table, 65_537, 0, 64), 1);
    assert_eq!(call(1, 0, 0, 0, 0, 2, 64), 1);
    assert_eq!(call(1, 0, 0, 0, 0, 1, 64), 1);
    assert_eq!(call(1, 0, 0, 0, 0, 0, -1), 1);
    assert_eq!(call(1, 0, 0, 0, 0, 0, 67_108_865), 1);
    for capacity in 0..9 {
        assert_eq!(call(1, 0, 0, table, 1, 0, capacity), 2);
    }
    let invalid = [
        fields[0],
        LogStringView {
            ptr: "é".as_ptr() as u64,
            len: 1,
        },
    ];
    assert_eq!(call(1, 0, 0, invalid.as_ptr() as u64, 1, 0, 64), 1);
    let oversized = [
        fields[0],
        LogStringView {
            ptr: 0,
            len: 67_108_865,
        },
    ];
    assert_eq!(call(1, 0, 0, oversized.as_ptr() as u64, 1, 0, 64), 1);
    assert_eq!(output, [73; 64]);
    assert_eq!(written, 83);
    assert_eq!(
        unsafe {
            prodex_mojo_structured_log_message_v1(
                1,
                0,
                0,
                table,
                1,
                0,
                output.as_mut_ptr() as u64,
                9,
                &mut written as *mut i64 as u64,
            )
        },
        0
    );
    assert_eq!(&output[..9], b"key=value");
    assert_eq!(&output[9..], &[73; 55]);
    assert_eq!(written, 9);
}

#[test]
fn message_field_policy_rejects_invalid_wire_inputs_without_touching_flags() {
    let mut flags = [83_i64; 5];
    let mut call = |version, key, key_len, value, value_len| unsafe {
        prodex_mojo_structured_log_message_field_policy_v1(
            version,
            key,
            key_len,
            value,
            value_len,
            flags.as_mut_ptr() as u64,
        )
    };
    assert_eq!(call(2, 0, 0, 0, 0), 4);
    assert_eq!(call(1, 0, 1, 0, 0), 1);
    assert_eq!(call(1, 0, -1, 0, 0), 1);
    assert_eq!(call(1, 0, 0, 0, 1), 1);
    assert_eq!(call(1, 0, 0, 0, -1), 1);
    assert_eq!(call(1, 0, 67_108_865, 0, 0), 1);
    assert_eq!(call(1, "é".as_ptr() as u64, 1, 0, 0), 1);
    assert_eq!(flags, [83; 5]);
}
