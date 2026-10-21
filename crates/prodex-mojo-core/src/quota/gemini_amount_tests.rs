use super::*;

#[test]
fn compiled_gemini_records_preserve_presence_percent_and_total_fields() {
    let inputs = [
        GeminiBucketTextInput {
            remaining_amount: None,
            remaining_fraction: Some(0.5),
        },
        GeminiBucketTextInput {
            remaining_amount: Some("bad"),
            remaining_fraction: Some(0.5),
        },
        GeminiBucketTextInput {
            remaining_amount: Some("　+42\t"),
            remaining_fraction: Some(0.5),
        },
    ];
    assert_eq!(
        gemini_bucket_text_numeric_batch(&inputs).unwrap(),
        [
            GeminiBucketNumericOutput {
                remaining: Some(50),
                total: Some(100),
                remaining_percent: Some(50),
                exhausted: false
            },
            GeminiBucketNumericOutput {
                remaining: None,
                total: None,
                remaining_percent: Some(50),
                exhausted: false
            },
            GeminiBucketNumericOutput {
                remaining: Some(42),
                total: Some(84),
                remaining_percent: Some(50),
                exhausted: false
            },
        ]
    );
}

#[test]
fn compiled_gemini_text_matches_parsed_counter_api() {
    let text = [
        None,
        Some(""),
        Some(" \t\u{3000}+42\u{85}"),
        Some("-9223372036854775808"),
        Some("9223372036854775807"),
        Some("9223372036854775808"),
        Some("-9223372036854775809"),
        Some("4 2"),
        Some("+"),
        Some("+0000"),
        Some("\u{200b}42"),
        Some("1.0"),
    ];
    for fraction in [
        None,
        Some(0.5),
        Some(0.0),
        Some(-1.0),
        Some(f64::INFINITY),
        Some(f64::NAN),
    ] {
        let raw = text
            .iter()
            .map(|value| GeminiBucketTextInput {
                remaining_amount: *value,
                remaining_fraction: fraction,
            })
            .collect::<Vec<_>>();
        let parsed = text
            .iter()
            .map(|value| GeminiBucketNumericInput {
                remaining_amount: match value {
                    None => GeminiRemainingAmount::Absent,
                    Some(value) => value
                        .trim()
                        .parse::<i64>()
                        .map(GeminiRemainingAmount::Parsed)
                        .unwrap_or(GeminiRemainingAmount::Invalid),
                },
                remaining_fraction: fraction,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            gemini_bucket_text_numeric_batch(&raw).unwrap(),
            gemini_bucket_numeric_batch(&parsed).unwrap()
        );
    }
    assert_eq!(gemini_bucket_text_numeric_batch(&[]).unwrap(), []);
    assert_eq!(
        gemini_bucket_text_numeric_batch(&vec![
            GeminiBucketTextInput {
                remaining_amount: None,
                remaining_fraction: None
            };
            QUOTA_GEMINI_BUCKET_BATCH_MAX_COUNT + 1
        ]),
        Err(MojoError::InvalidInput)
    );
    let mut full = vec![
        GeminiBucketTextInput {
            remaining_amount: None,
            remaining_fraction: None
        };
        QUOTA_GEMINI_BUCKET_BATCH_MAX_COUNT
    ];
    full[QUOTA_GEMINI_BUCKET_BATCH_MAX_COUNT - 1].remaining_amount = Some("-1");
    let values = gemini_bucket_text_numeric_batch(&full).unwrap();
    assert_eq!(values.last().unwrap().remaining, Some(-1));
    assert!(
        values[..QUOTA_GEMINI_BUCKET_BATCH_MAX_COUNT - 1]
            .iter()
            .all(|value| value.remaining.is_none())
    );
}

#[test]
fn compiled_gemini_records_validate_all_rows_before_writing() {
    let text = "42";
    let mut rows = [
        InputRow {
            amount_address: text.as_ptr() as u64,
            amount_length: 2,
            amount_source: 3,
            ..InputRow::default()
        },
        InputRow {
            amount_source: 4,
            ..InputRow::default()
        },
    ];
    let sentinel = OutputRow {
        remaining: 77,
        remaining_present: 77,
        total: 77,
        total_present: 77,
        remaining_percent: 77,
        remaining_percent_present: 77,
        exhausted: 77,
    };
    let mut output = [sentinel; 2];
    let mut call = |version, count, capacity| unsafe {
        prodex_quota_gemini_bucket_records_v1(
            version,
            rows.as_ptr() as u64,
            count,
            output.as_mut_ptr() as u64,
            capacity,
        )
    };
    assert_eq!(call(2, 2, 2), 4);
    assert_eq!(call(1, 2, 1), 3);
    assert_eq!(call(1, 2, 2), 1);
    assert_eq!(call(1, 1025, 2), 1);
    for value in output {
        assert_eq!(
            [
                value.remaining,
                value.remaining_present,
                value.total,
                value.total_present,
                value.remaining_percent,
                value.remaining_percent_present,
                value.exhausted
            ],
            [77; 7]
        );
    }
    let malformed = [255_u8];
    rows[1] = InputRow {
        amount_source: 3,
        amount_address: malformed.as_ptr() as u64,
        amount_length: 1,
        ..InputRow::default()
    };
    assert_eq!(
        unsafe {
            prodex_quota_gemini_bucket_records_v1(
                1,
                rows.as_ptr() as u64,
                2,
                output.as_mut_ptr() as u64,
                2,
            )
        },
        1
    );
    for value in output {
        assert_eq!(
            [
                value.remaining,
                value.remaining_present,
                value.total,
                value.total_present,
                value.remaining_percent,
                value.remaining_percent_present,
                value.exhausted
            ],
            [77; 7]
        );
    }
}
