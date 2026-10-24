use super::*;

#[test]
fn profile_panel_defaults_preserve_each_provider_schema() {
    for (kind, expected) in [
        (
            QuotaPanelKind::OpenAi,
            r#"["Quota main",[["Profile","main"],["Account","-"],["Plan","-"],["Status","Ready"],["Main","full"]]]"#,
        ),
        (
            QuotaPanelKind::Copilot,
            r#"["Quota main",[["Profile","main"],["Account","-"],["Plan","-"],["Status","Ready"],["Main","full"]]]"#,
        ),
        (
            QuotaPanelKind::Gemini,
            r#"["Quota main",[["Profile","main"],["Account","-"],["Plan","-"],["Project","-"],["Status","Ready"],["Main","full"]]]"#,
        ),
        (
            QuotaPanelKind::External,
            r#"["Quota main",[["Profile","main"],["Provider","-"],["Account","-"],["Plan","-"],["Status","Ready"],["Main","full"]]]"#,
        ),
    ] {
        assert_eq!(
            quota_profile_panel_json(
                kind,
                QuotaPanelInput {
                    profile: "main",
                    status: "Ready",
                    main: "full",
                    ..Default::default()
                },
                false
            )
            .unwrap(),
            expected
        );
    }
}

#[test]
fn profile_panel_copilot_plan_presence_and_access_equality_are_exact() {
    for (plan, expected) in [
        (
            None,
            r#"["Quota x",[["Profile","x"],["Account",""],["Plan","sku"],["Access","sku"],["Status",""],["Main",""]]]"#,
        ),
        (
            Some("sku"),
            r#"["Quota x",[["Profile","x"],["Account",""],["Plan","sku"],["Status",""],["Main",""]]]"#,
        ),
        (
            Some(""),
            r#"["Quota x",[["Profile","x"],["Account",""],["Plan",""],["Access","sku"],["Status",""],["Main",""]]]"#,
        ),
    ] {
        assert_eq!(
            quota_profile_panel_json(
                QuotaPanelKind::Copilot,
                QuotaPanelInput {
                    profile: "x",
                    account: Some(""),
                    plan,
                    access: Some("sku"),
                    ..Default::default()
                },
                false
            )
            .unwrap(),
            expected
        );
    }
}

#[test]
fn profile_panel_abi_rejects_bad_views_flags_and_capacity_before_output() {
    let mut fields = [StringView::from(""); 11];
    let mut output = [77_u8; 16];
    let mut metadata = [71_i64; 2];
    let mut call = |abi, kind, detail, fields: &[StringView; 11], presence, capacity| unsafe {
        prodex_quota_profile_panel_v1(
            abi,
            kind,
            detail,
            fields.as_ptr() as u64,
            presence,
            0,
            0,
            0,
            output.as_mut_ptr() as u64,
            capacity,
            metadata.as_mut_ptr() as u64,
        )
    };
    assert_eq!(call(2, 0, 0, &fields, 193, 16), 4);
    assert_eq!(call(1, 4, 0, &fields, 193, 16), 1);
    assert_eq!(call(1, 0, 2, &fields, 193, 16), 1);
    assert_eq!(call(1, 0, 0, &fields, 0, 16), 1);
    assert_eq!(call(1, 0, 0, &fields, 4095, 16), 1);
    fields[2] = StringView::from("absent value");
    assert_eq!(call(1, 0, 0, &fields, 193, 16), 1);
    let invalid = [255_u8];
    fields[2] = StringView {
        address: invalid.as_ptr() as u64,
        length: 1,
    };
    assert_eq!(call(1, 0, 0, &fields, 197, 16), 1);
    fields[2] = StringView::from("");
    assert_eq!(call(1, 0, 0, &fields, 193, 16), 3);
    assert_eq!(output, [77; 16]);
    assert_eq!(metadata, [71; 2]);
}

#[test]
fn profile_panel_openai_extras_keep_presence_precedence_and_empty_labels() {
    let extras = [
        QuotaPanelExtra {
            names: [Some(""), Some("ignored"), None],
            window_label: "60s",
            value: "zero",
        },
        QuotaPanelExtra {
            names: [None, Some("feature"), Some("ignored")],
            window_label: "60s",
            value: "one",
        },
        QuotaPanelExtra {
            names: [None, None, Some("id")],
            window_label: "60s",
            value: "two",
        },
        QuotaPanelExtra {
            names: [None; 3],
            window_label: "60s",
            value: "three",
        },
    ];
    assert_eq!(
        quota_profile_panel_json(
            QuotaPanelKind::OpenAi,
            QuotaPanelInput {
                profile: "main",
                reset: Some(""),
                code_review: Some(""),
                reset_credits: Some(""),
                extras: &extras,
                ..Default::default()
            },
            true
        )
        .unwrap(),
        r#"["Quota main",[["Profile","main"],["Account","-"],["Plan","-"],["Status",""],["Main",""],["Reset",""],["Code review",""],["Reset credits",""],[" 60s","zero"],["feature 60s","one"],["id 60s","two"],["Additional 60s","three"]]]"#
    );
}

#[test]
fn profile_panel_gemini_buckets_keep_large_input_order_and_exact_numbers() {
    let values = (0..2_049)
        .map(|index| format!("value-{index}"))
        .collect::<Vec<_>>();
    let extras = values
        .iter()
        .map(|value| QuotaPanelExtra {
            value,
            ..Default::default()
        })
        .collect::<Vec<_>>();
    let expected = values
        .iter()
        .enumerate()
        .map(|(index, value)| format!(r#",["Bucket {}","{value}"]"#, index + 1))
        .collect::<String>();
    assert_eq!(
        quota_profile_panel_json(
            QuotaPanelKind::Gemini,
            QuotaPanelInput {
                profile: "main",
                extras: &extras,
                ..Default::default()
            },
            false
        )
        .unwrap(),
        format!(
            r#"["Quota main",[["Profile","main"],["Account","-"],["Plan","-"],["Project","-"],["Status",""],["Main",""]{expected}]]"#
        )
    );
}
