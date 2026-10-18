use super::*;
use crate::json::{JsonKind, StringView, ffi_nodes};

fn empty() -> JsonNode<'static> {
    JsonNode {
        kind: JsonKind::Object,
        first_child: None,
        next_sibling: None,
        parent: None,
        key: "",
        text: "",
        raw_start: 0,
        raw_length: 2,
    }
}

#[test]
fn compiled_external_quota_projects_empty_rate_limits_and_absent_agy_accounts() {
    assert_eq!(external_quota_projection(&[empty()], "{}", ExternalQuotaProjection::Anthropic).unwrap().unwrap(),
        br#"{"main":"0 rate group(s)","details":[{"label":"Rate groups","value":"0"},{"label":"Model groups","value":"0"}]}"#);
    let raw = r#"{"value":[]}"#;
    let mut root = empty();
    root.first_child = Some(1);
    root.raw_length = raw.len();
    let child = JsonNode {
        kind: JsonKind::Array,
        parent: Some(0),
        key: "value",
        raw_start: raw.find('[').unwrap(),
        ..empty()
    };
    assert_eq!(
        external_quota_projection(&[root, child], raw, ExternalQuotaProjection::Agy).unwrap(),
        None
    );
    assert_eq!(
        external_quota_projection(&[empty()], "{}", ExternalQuotaProjection::Agy),
        Err(MojoError::InvalidInput)
    );
    let raw = r#"{"value":null,"preferred":1}"#;
    let root = JsonNode {
        first_child: Some(1),
        raw_length: raw.len(),
        ..empty()
    };
    let value = JsonNode {
        kind: JsonKind::Null,
        parent: Some(0),
        next_sibling: Some(2),
        key: "value",
        raw_start: raw.find("null").unwrap(),
        raw_length: 4,
        ..empty()
    };
    let preferred = JsonNode {
        kind: JsonKind::Number,
        parent: Some(0),
        key: "preferred",
        raw_start: raw.find('1').unwrap(),
        raw_length: 1,
        ..empty()
    };
    assert_eq!(
        external_quota_projection(&[root, value, preferred], raw, ExternalQuotaProjection::Agy),
        Err(MojoError::InvalidInput)
    );
}

#[test]
fn compiled_external_quota_abi_rejects_invalid_input_before_output_or_metadata_writes() {
    let raw = "{}";
    let input = ffi_nodes(&[empty()], raw).unwrap();
    let mut scratch = [StringView::default(); 1];
    let mut output = [57_u8; 512];
    let mut metadata = [89_i64; 2];
    let valid = [
        1,
        1,
        0,
        input.as_ptr() as usize as i64,
        1,
        raw.as_ptr() as usize as i64,
        2,
        scratch.as_mut_ptr() as usize as i64,
        1,
        0,
        output.as_mut_ptr() as usize as i64,
        512,
        metadata.as_mut_ptr() as usize as i64,
    ];
    let call = |v: [i64; 13]| unsafe {
        prodex_external_quota_projection_v1(
            v[0],
            v[1],
            v[2],
            v[3] as u64,
            v[4],
            v[5] as u64,
            v[6],
            v[7] as u64,
            v[8],
            v[9],
            v[10] as u64,
            v[11],
            v[12] as u64,
        )
    };
    for (index, value, expected) in [
        (0, 2, 4),
        (1, 2, 1),
        (2, 1, 1),
        (3, 0, 1),
        (4, 0, 1),
        (4, -1, 1),
        (5, 0, 1),
        (6, -1, 1),
        (6, 0, 1),
        (6, 67_108_865, 1),
        (7, 0, 1),
        (8, 0, 1),
        (8, 2, 1),
        (9, 1, 1),
        (9, 2, 1),
        (10, 0, 1),
        (11, -1, 1),
        (11, 1, 3),
        (12, 0, 1),
    ] {
        let mut bad = valid;
        bad[index] = value;
        assert_eq!(call(bad), expected, "argument {index}");
        assert_eq!(output, [57; 512]);
        assert_eq!(metadata, [89; 2]);
    }
    let invalid = [0xff_u8];
    let mut malformed = ffi_nodes(&[empty()], raw).unwrap();
    malformed[0].text = StringView {
        address: invalid.as_ptr() as usize as u64,
        length: 1,
    };
    let mut bad = valid;
    bad[3] = malformed.as_ptr() as usize as i64;
    assert_eq!(call(bad), 1);
    malformed[0].text.length = 67_108_865;
    assert_eq!(call(bad), 1);
    assert_eq!(output, [57; 512]);
    assert_eq!(metadata, [89; 2]);
    assert_eq!(call(valid), 0);
    assert_eq!(metadata[0], 1);
    let length = metadata[1] as usize;
    assert_eq!(
        &output[..length],
        external_quota_projection(&[empty()], raw, ExternalQuotaProjection::Anthropic)
            .unwrap()
            .unwrap()
    );
    assert!(output[length..].iter().all(|byte| *byte == 57));
}
