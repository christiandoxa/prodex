#![cfg(all(test, feature = "mojo"))]

use super::*;

#[test]
fn mojo_text_rows_match_rust_oracle_for_utf8_duplicates_and_generated_inputs() {
    let long = format!("error: {}🔥", "x".repeat(64 * 1024));
    let fixed = [
        (String::new(), String::new()),
        (
            "error: duplicate\nnoise\nerror: duplicate\n".into(),
            "error: duplicate\n".into(),
        ),
        (
            "  error: 账户🙂e\u{301}\0  \n警告: 東京\n".into(),
            "error: 账户🙂e\u{301}\0\n".into(),
        ),
        (format!("{long}\n"), String::new()),
    ];
    for (case, (before, after)) in fixed.into_iter().enumerate() {
        assert_eq!(
            critical_signal_normalized_rows(&before, &after),
            critical_signal_normalized_rows_rust(&before, &after),
            "fixed case {case}"
        );
    }

    let mut state = 0x7465_7874_5f72_6f77_u64;
    for case in 0..512 {
        let before_count = (next_random(&mut state) % 33) as usize;
        let before = generated_output(&mut state, before_count);
        let after_count = (next_random(&mut state) % 33) as usize;
        let after = generated_output(&mut state, after_count);
        assert_eq!(
            critical_signal_normalized_rows(&before, &after),
            critical_signal_normalized_rows_rust(&before, &after),
            "generated case {case}: before={before:?} after={after:?}"
        );
    }
}

fn generated_output(state: &mut u64, count: usize) -> String {
    let mut output = String::new();
    for index in 0..count {
        let bucket = next_random(state) % 8;
        let line = match next_random(state) % 6 {
            0 => format!("error: duplicate-{bucket} 账户🙂"),
            1 => format!("fatal: e\u{301}-{bucket}"),
            2 => format!("noise-\0-{bucket}"),
            3 => format!("   warning: 東京-{bucket}   "),
            4 => format!("src/火.rs:{}:2", bucket + 1),
            _ => format!("@@ -{index},1 +{bucket},1 @@"),
        };
        output.push_str(&line);
        output.push(if next_random(state).is_multiple_of(5) {
            '\r'
        } else {
            '\n'
        });
        if output.ends_with('\r') {
            output.push('\n');
        }
    }
    output
}

fn next_random(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    *state
}
