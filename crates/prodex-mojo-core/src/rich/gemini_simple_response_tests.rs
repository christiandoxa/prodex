use super::*;

fn part<'a>(keys: &'a [&'a str]) -> GeminiSimplePartInput<'a> {
    GeminiSimplePartInput {
        keys: Some(keys),
        text_present: false,
        text_is_string: false,
        thought: false,
        function_call_present: false,
        function_call_is_object: false,
        function_name: None,
    }
}

#[test]
fn simple_response_plan_distinguishes_shortcuts_ordered_calls_and_late_rejection() {
    assert_eq!(
        plan_gemini_simple_response(&[]),
        Ok(GeminiSimpleResponsePlan::MessageOnly)
    );
    let keys = ["text", "thought"];
    let mut text = part(&keys);
    text.text_present = true;
    text.text_is_string = true;
    assert_eq!(
        plan_gemini_simple_response(&[text]),
        Ok(GeminiSimpleResponsePlan::MessageOnly)
    );
    let call_keys = ["functionCall"];
    let extra_keys = ["functionCall", "extra"];
    let mut call = part(&call_keys);
    call.function_call_present = true;
    call.function_call_is_object = true;
    call.function_name = Some("\u{2003}tool\u{a0}");
    let mut extra = part(&extra_keys);
    extra.function_call_present = true;
    extra.function_call_is_object = true;
    extra.function_name = Some("other");
    assert_eq!(
        plan_gemini_simple_response(&[call, extra]),
        Ok(GeminiSimpleResponsePlan::CheckParts(vec![
            GeminiSimplePartAction::CheckCall,
            GeminiSimplePartAction::CheckCallThenReject,
        ]))
    );
    let mut empty_name = part(&call_keys);
    empty_name.function_call_present = true;
    empty_name.function_call_is_object = true;
    empty_name.function_name = Some("\u{2003}\u{a0}");
    assert_eq!(
        plan_gemini_simple_response(&[empty_name]),
        Ok(GeminiSimpleResponsePlan::CheckParts(vec![
            GeminiSimplePartAction::Reject
        ]))
    );
}

#[test]
fn simple_response_abi_rejects_invalid_records_before_any_write() {
    let valid_keys = [view("functionCall")];
    let valid = Part {
        keys_address: valid_keys.as_ptr() as u64,
        key_count: 1,
        name: view("tool"),
        flags: 49,
    };
    let mut rows = [valid, valid];
    let mut output = [71_i64; 3];
    let destination = output.as_mut_ptr() as u64;
    let source = rows.as_ptr() as u64;
    let call = |version, input, count, out, capacity| unsafe {
        prodex_mojo_gemini_simple_response_plan_v1(version, input, count, out, capacity)
    };
    assert_eq!(call(2, source, 2, destination, 3), 4);
    assert_eq!(call(1, source, -1, destination, 3), 1);
    assert_eq!(call(1, source, i64::MAX, destination, 3), 1);
    assert_eq!(call(1, 0, 2, destination, 3), 1);
    assert_eq!(call(1, source, 2, 0, 3), 1);
    assert_eq!(call(1, source, 2, destination, 2), 3);
    for invalid in [
        Part {
            flags: 128,
            ..valid
        },
        Part {
            keys_address: 0,
            ..valid
        },
        Part {
            key_count: -1,
            ..valid
        },
        Part { flags: 0, ..valid },
        Part { flags: 5, ..valid },
        Part { flags: 33, ..valid },
    ] {
        rows[1] = invalid;
        assert_eq!(call(1, rows.as_ptr() as u64, 2, destination, 3), 1);
    }
    let invalid_utf8 = [0xff_u8];
    rows[1] = Part {
        name: RichStringView {
            ptr: invalid_utf8.as_ptr() as u64,
            len: 1,
        },
        ..valid
    };
    assert_eq!(call(1, rows.as_ptr() as u64, 2, destination, 3), 2);
    let invalid_keys = [RichStringView {
        ptr: invalid_utf8.as_ptr() as u64,
        len: 1,
    }];
    rows[1] = Part {
        keys_address: invalid_keys.as_ptr() as u64,
        ..valid
    };
    assert_eq!(call(1, rows.as_ptr() as u64, 2, destination, 3), 2);
    assert_eq!(output, [71; 3]);
    assert_eq!(call(1, 0, 0, destination, 1), 0);
    assert_eq!(output[0], 0);
}

#[test]
fn simple_response_plan_handles_large_part_batches_without_a_new_count_cap() {
    let keys = ["functionCall"];
    let inputs = (0..65_537)
        .map(|_| {
            let mut call = part(&keys);
            call.function_call_present = true;
            call.function_call_is_object = true;
            call.function_name = Some("tool");
            call
        })
        .collect::<Vec<_>>();
    assert_eq!(
        plan_gemini_simple_response(&inputs),
        Ok(GeminiSimpleResponsePlan::CheckParts(
            vec![GeminiSimplePartAction::CheckCall; 65_537]
        ))
    );
}
