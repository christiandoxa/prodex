use super::*;

fn assert_normalized(input: &str, command_output: &str, static_context: &str) {
    assert_eq!(
        smart_context_normalize_volatile_command_output(input).as_ref(),
        command_output,
        "command-output normalization for {input:?}"
    );
    assert_eq!(
        smart_context_normalize_volatile_static_context(input).as_ref(),
        static_context,
        "static-context normalization for {input:?}"
    );
}

#[test]
fn removes_standard_and_truncated_ansi_sequences() {
    let input = "\x1b[32mred\x1b[0m \x1b]0;title\x07green\x1b]2;title\x1b\\ blue\x1b7 done";
    assert_normalized(input, "red green blue done", "red green blue done");
    assert_normalized("unfinished \x1b[31", "unfinished ", "unfinished ");
}

#[test]
fn preserves_unicode_after_a_bare_escape() {
    assert_normalized("A\x1béB", "AéB", "AéB");
}

#[test]
fn bounds_temporary_paths_at_unicode_whitespace() {
    let input = "paths /tmp/prodex-a/run.log C:\\Users\\test\\AppData\\Local\\Temp\\tmp.txt /var/tmp/cache\u{00a0}visible /tmp/cache\u{2028}visible /tmp/cache\u{0086}visible";
    let expected = "paths <tmp-path> <tmp-path> <tmp-path>\u{00a0}visible <tmp-path>\u{2028}visible <tmp-path>\u{0086}visible";
    assert_normalized(input, expected, expected);
}

#[test]
fn normalizes_timestamps_and_leaves_malformed_near_matches() {
    assert_normalized(
        "at 2026-05-04T01:02:03.123456+07:00, 2026-05-04 01:02Z! and x2026-05-04T01:02:03Z",
        "at <timestamp>, <timestamp>! and x2026-05-04T01:02:03Z",
        "at <timestamp>, <timestamp>! and x2026-05-04T01:02:03Z",
    );
    let malformed = "bad 2026-05-04T01:02:03.Z 2026-05-04T01:02:03+7:00";
    assert_normalized(malformed, malformed, malformed);
}

#[test]
fn normalizes_progress_and_durations_only_in_command_output() {
    assert_normalized(
        "build 50% 3/9 2 of 4 11/10 2of4 100.% 3%done",
        "build <progress> <progress> <progress> 11/10 2of4 100.% 3%done",
        "build 50% 3/9 2 of 4 11/10 2of4 100.% 3%done",
    );
    assert_normalized(
        "elapsed 1% 1s 1.25ms 120 milliseconds 0.5SEC 2 hrs 1s2 1.s",
        "elapsed <progress> <duration> <duration> <duration> <duration> <duration> 1s2 1.s",
        "elapsed 1% 1s 1.25ms 120 milliseconds 0.5SEC 2 hrs 1s2 1.s",
    );
}

#[test]
fn normalizes_labeled_random_ids_and_exact_uuids() {
    assert_normalized(
        "Request_ID=123e4567-e89b-12d3-a456-426614174000; Prodex conversation-id: 'c0ffee00-1234-abcd-9876-123456789abc'; x-request-id=0123456789abcdef0123; id=0123456789abcdef; message_id=0123456789abcdef0123456789",
        "Request_ID=<id>; Prodex conversation-id: '<id>'; x-request-id=<id>; id=0123456789abcdef; message_id=0123456789abcdef0123456789",
        "Request_ID=<id>; Prodex conversation-id: '<id>'; x-request-id=<id>; id=0123456789abcdef; message_id=0123456789abcdef0123456789",
    );
    assert_normalized(
        "é 123e4567-e89b-12d3-a456-426614174000界 x123e4567-e89b-12d3-a456-426614174000 123e4567-e89b-12d3-a456-42661417400x",
        "é <id>界 x123e4567-e89b-12d3-a456-426614174000 123e4567-e89b-12d3-a456-42661417400x",
        "é <id>界 x123e4567-e89b-12d3-a456-426614174000 123e4567-e89b-12d3-a456-42661417400x",
    );
    let unicode_key = "request\u{00a0}id=123e4567-e89b-12d3-a456-426614174000";
    let unicode_key_with_uuid_normalized = "request\u{00a0}id=<id>";
    assert_normalized(
        unicode_key,
        unicode_key_with_uuid_normalized,
        unicode_key_with_uuid_normalized,
    );
}

#[test]
fn expands_long_progress_output_within_the_abi_limit() {
    let count = 800_000;
    let input = "1% ".repeat(count);
    let expected = "<progress> ".repeat(count);
    let normalized = smart_context_normalize_volatile_command_output(&input);
    assert!(
        normalized.as_ref() == expected,
        "long normalized output mismatch ({} bytes, expected {} bytes)",
        normalized.len(),
        expected.len()
    );
    assert_eq!(
        smart_context_normalize_volatile_static_context(&input).as_ref(),
        input
    );
}
