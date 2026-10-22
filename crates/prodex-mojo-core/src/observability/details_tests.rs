use super::*;

fn text<'a>(
    format: OperationalDetailFormat,
    original: &'a str,
    prepared: &'a str,
) -> OperationalDetailText<'a> {
    OperationalDetailText {
        label: "value",
        format,
        original,
        prepared,
    }
}

#[test]
fn compiled_detail_renderer_keeps_order_raw_visibility_and_format_specific_sentinels() {
    use OperationalDetailFormat::{Endpoint, Percent, Plain};
    let fields = [
        text(Plain, "raw", "-"),
        text(Percent, "-", "-"),
        text(Endpoint, "raw", "/responses"),
        text(Plain, "a\u{85}b", "hidden"),
        text(Percent, " ", ""),
        text(Plain, "é", "é"),
    ];
    assert_eq!(
        render_operational_details("Event", &fields),
        Ok("Event  value=-% · value=/responses · value=% · value=é".into())
    );
    assert_eq!(
        render_operational_details("Event", &[text(Plain, "", "unused")]),
        Ok("Event".into())
    );
    assert_eq!(render_operational_details("", &[]), Ok(String::new()));
}

#[test]
fn compiled_detail_renderer_applies_scalar_limits_without_changing_prepared_text() {
    let unicode = format!("{}e\u{301}", "é".repeat(191));
    let number = "7".repeat(33);
    assert_eq!(
        render_operational_details(
            "Event",
            &[
                text(OperationalDetailFormat::Plain, "raw", &unicode),
                text(OperationalDetailFormat::Percent, "raw", &number),
            ]
        ),
        Ok(format!(
            "Event  value={}e… · value={}…%",
            "é".repeat(191),
            "7".repeat(32)
        ))
    );
    assert_eq!(
        render_operational_details("Event", &[text(OperationalDetailFormat::Plain, " ", " ")]),
        Ok("Event  value= ".into())
    );
}

#[test]
fn compiled_detail_renderer_accepts_the_plan_bound_and_rejects_excess_records() {
    let fields: Vec<_> = (0..64)
        .map(|_| text(OperationalDetailFormat::Plain, "", ""))
        .collect();
    assert_eq!(
        render_operational_details("Event", &fields),
        Ok("Event".into())
    );
    let fields: Vec<_> = (0..65)
        .map(|_| text(OperationalDetailFormat::Plain, "", ""))
        .collect();
    assert_eq!(
        render_operational_details("Event", &fields),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn compiled_detail_abi_rejects_invalid_modes_views_and_capacity_without_writes() {
    let mut fields = [DetailView {
        label: "value".into(),
        original: "raw".into(),
        prepared: "visible".into(),
        format: 0,
    }];
    let mut output = [73_u8; 64];
    let mut written = 83_i64;
    let mut call = |version, address, count, measuring, capacity| unsafe {
        prodex_mojo_operational_log_details_v1(
            version,
            "Event".as_ptr() as u64,
            5,
            address,
            count,
            measuring,
            output.as_mut_ptr() as u64,
            capacity,
            &mut written as *mut i64 as u64,
        )
    };
    assert_eq!(call(2, fields.as_ptr() as u64, 1, 0, 64), 4);
    assert_eq!(call(1, 0, 1, 0, 64), 1);
    assert_eq!(call(1, fields.as_ptr() as u64 + 1, 1, 0, 64), 1);
    assert_eq!(call(1, fields.as_ptr() as u64, 65, 0, 64), 1);
    assert_eq!(call(1, fields.as_ptr() as u64, 1, 2, 64), 1);
    assert_eq!(call(1, fields.as_ptr() as u64, 1, 0, 1), 2);
    fields[0].format = 3;
    assert_eq!(call(1, fields.as_ptr() as u64, 1, 0, 64), 1);
    fields[0].format = 0;
    fields[0].original = TextView {
        address: "é".as_ptr() as u64,
        length: 1,
    };
    assert_eq!(call(1, fields.as_ptr() as u64, 1, 0, 64), 1);
    assert_eq!(output, [73; 64]);
    assert_eq!(written, 83);
}
