use super::*;

#[test]
fn token_numbers_preserve_required_fields_defaults_and_counter_boundaries() {
    let fields = InfoTokenUsageFields {
        input_tokens: Some("+18446744073709551615"),
        cached_input_tokens: Some("18446744073709551616"),
        output_tokens: Some("+15"),
        reasoning_tokens: Some("-1"),
        request: Some("0"),
        generation_ms: Some("1000"),
        output_tokens_per_second: Some(".1e+2"),
    };
    assert_eq!(
        info_token_usage_numbers(InfoTokenUsageKind::Completed, fields),
        Ok(Some(InfoTokenUsageNumbers {
            input_tokens: u64::MAX,
            cached_input_tokens: 0,
            output_tokens: 15,
            reasoning_tokens: 0,
            request: Some(0),
            generation_ms: Some(1000),
            output_tokens_per_second: Some(15.0),
        }))
    );
    assert_eq!(
        info_token_usage_numbers(
            InfoTokenUsageKind::Completed,
            InfoTokenUsageFields::default()
        ),
        Ok(None)
    );
    assert_eq!(
        info_token_usage_numbers(
            InfoTokenUsageKind::Progress,
            InfoTokenUsageFields {
                input_tokens: Some("invalid"),
                output_tokens: Some("0"),
                generation_ms: Some("1"),
                ..InfoTokenUsageFields::default()
            }
        ),
        Ok(Some(InfoTokenUsageNumbers {
            input_tokens: 0,
            cached_input_tokens: 0,
            output_tokens: 0,
            reasoning_tokens: 0,
            request: None,
            generation_ms: Some(1),
            output_tokens_per_second: None,
        }))
    );
}

#[test]
fn token_number_abi_rejects_invalid_facts_without_writing_output() {
    let texts = [TokenFieldView::default(); 7];
    let mut output = [71_u64; 9];
    for (abi, kind, count, presence, words, expected) in [
        (2, 0, 7, 0, 9, 4),
        (1, 2, 7, 0, 9, 1),
        (1, 0, 6, 0, 9, 1),
        (1, 0, 7, 128, 9, 1),
        (1, 0, 7, 0, 8, 1),
    ] {
        assert_eq!(
            unsafe {
                prodex_info_token_usage_numbers_v1(
                    abi,
                    kind,
                    texts.as_ptr() as u64,
                    count,
                    presence,
                    output.as_mut_ptr() as u64,
                    words,
                )
            },
            expected
        );
        assert_eq!(output, [71; 9]);
    }
    let invalid = [TokenFieldView {
        address: 0,
        length: 1,
    }; 7];
    assert_eq!(
        unsafe {
            prodex_info_token_usage_numbers_v1(
                1,
                0,
                invalid.as_ptr() as u64,
                7,
                127,
                output.as_mut_ptr() as u64,
                9,
            )
        },
        1
    );
    assert_eq!(output, [71; 9]);
}

#[test]
fn token_number_rate_validation_keeps_decimal_rounding_and_grammar_boundaries() {
    for (text, valid) in [
        (".1", true),
        ("+1.", true),
        ("1e+308", true),
        ("5e-324", true),
        ("2.4703282292062327e-324", false),
        ("2.4703282292062328e-324", true),
        ("1.7976931348623158e308", true),
        ("1.7976931348623159e308", false),
        ("1e-99999999999999999", false),
        ("1e99999999999999999", false),
        ("0e99999", false),
        ("+0", false),
        ("-0", false),
        ("-1", false),
        ("1_0", false),
        (" 1", false),
        ("1 ", false),
        ("1\0", false),
        ("NaN", false),
        ("infinity", false),
        ("0x1p0", false),
        ("1e+", false),
        ("1..0", false),
    ] {
        let actual = info_token_usage_numbers(
            InfoTokenUsageKind::Completed,
            InfoTokenUsageFields {
                input_tokens: Some("1"),
                output_tokens: Some("15"),
                generation_ms: Some("1000"),
                output_tokens_per_second: Some(text),
                ..InfoTokenUsageFields::default()
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(actual.generation_ms, valid.then_some(1000), "{text:?}");
        assert_eq!(
            actual.output_tokens_per_second,
            valid.then_some(15.0),
            "{text:?}"
        );
    }
    let zeros = "0".repeat(2048);
    for text in [format!("1{zeros}e-2048"), format!(".{zeros}1e2049")] {
        let actual = info_token_usage_numbers(
            InfoTokenUsageKind::Completed,
            InfoTokenUsageFields {
                input_tokens: Some("1"),
                output_tokens: Some("15"),
                generation_ms: Some("1000"),
                output_tokens_per_second: Some(&text),
                ..InfoTokenUsageFields::default()
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(actual.output_tokens_per_second, Some(15.0));
    }
}

#[test]
fn token_number_decimal_ties_match_zero_and_infinity_rounding() {
    for (text, valid) in [
        (
            "24703282292062327208828439643411068618252990130716238221279284125033775363510437593264991818081799618989828234772285886546332835517796989819938739800539093906315035659515570226392290858392449105184435931802849936536152500319370457678249219365623669863658480757001585769269903706311928279558551332927834338409351978015531246597263579574622766465272827220056374006485499977096599470454020828166226237857393450736339007967761930577506740176324673600968951340535537458516661134223766678604162159680461914467291840300530057530849048765391711386591646239524912623653881879636239373280423891018672348497668235089863388587925628302755995657524455507255189313690836254779186948667994968324049705821028513185451396213837722826145437693412532098591327667236328124e-1075",
            false,
        ),
        (
            "24703282292062327208828439643411068618252990130716238221279284125033775363510437593264991818081799618989828234772285886546332835517796989819938739800539093906315035659515570226392290858392449105184435931802849936536152500319370457678249219365623669863658480757001585769269903706311928279558551332927834338409351978015531246597263579574622766465272827220056374006485499977096599470454020828166226237857393450736339007967761930577506740176324673600968951340535537458516661134223766678604162159680461914467291840300530057530849048765391711386591646239524912623653881879636239373280423891018672348497668235089863388587925628302755995657524455507255189313690836254779186948667994968324049705821028513185451396213837722826145437693412532098591327667236328125e-1075",
            false,
        ),
        (
            "24703282292062327208828439643411068618252990130716238221279284125033775363510437593264991818081799618989828234772285886546332835517796989819938739800539093906315035659515570226392290858392449105184435931802849936536152500319370457678249219365623669863658480757001585769269903706311928279558551332927834338409351978015531246597263579574622766465272827220056374006485499977096599470454020828166226237857393450736339007967761930577506740176324673600968951340535537458516661134223766678604162159680461914467291840300530057530849048765391711386591646239524912623653881879636239373280423891018672348497668235089863388587925628302755995657524455507255189313690836254779186948667994968324049705821028513185451396213837722826145437693412532098591327667236328126e-1075",
            true,
        ),
        (
            "179769313486231580793728971405303415079934132710037826936173778980444968292764750946649017977587207096330286416692887910946555547851940402630657488671505820681908902000708383676273854845817711531764475730270069855571366959622842914819860834936475292719074168444365510704342711559699508093042880177904174497791e0",
            true,
        ),
        (
            "179769313486231580793728971405303415079934132710037826936173778980444968292764750946649017977587207096330286416692887910946555547851940402630657488671505820681908902000708383676273854845817711531764475730270069855571366959622842914819860834936475292719074168444365510704342711559699508093042880177904174497792e0",
            false,
        ),
        (
            "179769313486231580793728971405303415079934132710037826936173778980444968292764750946649017977587207096330286416692887910946555547851940402630657488671505820681908902000708383676273854845817711531764475730270069855571366959622842914819860834936475292719074168444365510704342711559699508093042880177904174497793e0",
            false,
        ),
    ] {
        let actual = info_token_usage_numbers(
            InfoTokenUsageKind::Completed,
            InfoTokenUsageFields {
                input_tokens: Some("1"),
                output_tokens: Some("1"),
                generation_ms: Some("1000"),
                output_tokens_per_second: Some(text),
                ..InfoTokenUsageFields::default()
            },
        )
        .unwrap()
        .unwrap();
        assert_eq!(actual.output_tokens_per_second, valid.then_some(1.0));
    }
}
