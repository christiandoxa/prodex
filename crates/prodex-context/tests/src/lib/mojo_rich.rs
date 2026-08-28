use super::*;

#[test]
fn rich_context_counts_match_rust_oracle_for_20_000_generated_inputs() {
    let mut state = 0x7269_6368_5f74_6578_u64;
    for case in 0..20_000 {
        let mut input = String::new();
        let line_count = (next(&mut state) % 18) as usize;
        for line in 0..line_count {
            let bucket = next(&mut state) % 10;
            let value = match bucket {
                0 => format!("error: failure-{case}-{line} 火🙂"),
                1 => format!("warning: caution-{case}-{line}"),
                2 => format!("src/火.rs:{}:{}", line + 1, (next(&mut state) % 90) + 1),
                3 => format!("@@ -{},1 +{},1 @@", line + 1, line + 2),
                4 => format!("test tests::{case}_{line} ... FAILED"),
                5 => "stack backtrace:".to_string(),
                6 => format!("process exited with exit code {}", line + 1),
                7 => "{\"error\":{\"type\":\"server_error\"}}".to_string(),
                8 => format!("File \"src/火.py\", line {}, in main", line + 1),
                _ => format!("noise-{case}-{line} e\u{301}"),
            };
            input.push_str(if line % 3 == 0 { "\u{1b}[32m" } else { "" });
            input.push_str(&value);
            input.push_str(if line % 2 == 0 { "\u{1b}[0m\r\n" } else { "\n" });
        }
        let expected = rust_counts(&input);
        let actual = prodex_mojo_core::rich::analyze_context(&input)
            .expect("rich context generated input should parse")
            .counts;
        assert_eq!(actual, expected, "generated context case {case}: {input:?}");
    }
}

fn rust_counts(input: &str) -> [usize; 7] {
    let normalized = normalize_command_output(input);
    let mut counts = CriticalSignalCounts::default();
    for line in command_lines(&normalized) {
        counts.add_assign(critical_signal_counts_for_line_for_test(line));
    }
    [
        counts.errors,
        counts.file_locations,
        counts.diff_hunks,
        counts.test_failures,
        counts.exit_codes,
        counts.stack_markers,
        counts.rust_diagnostics,
    ]
}

fn next(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state
}
