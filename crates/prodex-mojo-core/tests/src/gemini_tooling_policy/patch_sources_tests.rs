use super::*;

fn empty<'a>() -> [Option<&'a str>; GEMINI_PATCH_SOURCE_FIELD_COUNT] {
    [None; GEMINI_PATCH_SOURCE_FIELD_COUNT]
}

#[test]
fn compiled_gemini_patch_sources_preserve_alias_priority_empty_values_and_unicode() {
    let mut fields = empty();
    assert_eq!(
        gemini_apply_patch_sources(&fields),
        Ok(GeminiApplyPatchSources {
            explicit: None,
            path: None,
            old: None,
            new: None,
            content: None,
        })
    );
    fields[1] = Some("");
    fields[2] = Some("second");
    assert_eq!(
        gemini_apply_patch_sources(&fields).unwrap().explicit,
        Some("")
    );
    fields[0] = Some("direct");
    assert_eq!(
        gemini_apply_patch_sources(&fields).unwrap().explicit,
        Some("direct")
    );
    fields[0] = None;
    fields[1] = None;
    fields[5] = Some(" \u{3000} ");
    fields[6] = Some("fallback/path.rs");
    assert_eq!(gemini_apply_patch_sources(&fields).unwrap().path, None);
    fields[5] = Some(" \u{2003}docs/雪.rs\t ");
    fields[8] = Some("old");
    fields[9] = Some("ignored old");
    fields[12] = Some("");
    fields[13] = Some("ignored new");
    fields[16] = Some("content fallback");
    let selected = gemini_apply_patch_sources(&fields).unwrap();
    assert_eq!(selected.path, Some("docs/雪.rs"));
    assert_eq!(selected.old, Some("old"));
    assert_eq!(selected.new, Some(""));
    assert_eq!(selected.content, Some("content fallback"));
    assert_eq!(selected.explicit, Some("second"));
    fields[2] = None;
    assert_eq!(gemini_apply_patch_sources(&fields).unwrap().explicit, None);
}

#[test]
fn compiled_gemini_patch_sources_abi_rejects_bad_late_rows_before_output_write() {
    let mut words = [0_u64; GEMINI_PATCH_SOURCE_FIELD_COUNT * 3];
    let mut output = [79_i64; 7];
    let fields = words.as_ptr() as usize as u64;
    let result = output.as_mut_ptr() as usize as u64;
    for (version, data, count, destination, expected) in [
        (2, fields, 17, result, 4),
        (1, 0, 17, result, 1),
        (1, fields, 16, result, 1),
        (1, fields, 18, result, 1),
        (1, fields, 17, 0, 1),
    ] {
        assert_eq!(
            unsafe { prodex_gemini_apply_patch_sources_v1(version, data, count, destination) },
            expected
        );
        assert_eq!(output, [79; 7]);
    }
    words[16 * 3] = 2;
    assert_eq!(
        unsafe {
            prodex_gemini_apply_patch_sources_v1(1, words.as_ptr() as usize as u64, 17, result)
        },
        1
    );
    assert_eq!(output, [79; 7]);
    words[16 * 3] = 1;
    words[16 * 3 + 1] = 0;
    words[16 * 3 + 2] = 1;
    assert_eq!(
        unsafe {
            prodex_gemini_apply_patch_sources_v1(1, words.as_ptr() as usize as u64, 17, result)
        },
        1
    );
    assert_eq!(output, [79; 7]);
}
