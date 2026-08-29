#![cfg(all(test, feature = "mojo"))]

use super::*;

fn critical_signal_normalized_rows_rust(
    before: &str,
    after: &str,
) -> Result<(Vec<i64>, Vec<i64>, usize), prodex_mojo_core::MojoError> {
    let before = normalize_command_output(before);
    let after = normalize_command_output(after);
    let before_lines = command_lines(&before);
    let mut key_ids = BTreeMap::<String, usize>::new();
    let mut after_available = Vec::<i64>::new();

    for line in command_lines(&after) {
        let counts = rust_critical_signal_counts_for_line(line);
        if counts.is_empty() {
            continue;
        }
        let next_id = key_ids.len();
        let key_id = *key_ids
            .entry(critical_signal_line_key(line))
            .or_insert(next_id);
        if key_id == after_available.len() {
            after_available.push(0);
        }
        after_available[key_id] = after_available[key_id]
            .checked_add(1_i64)
            .ok_or(prodex_mojo_core::MojoError::InvalidInput)?;
    }

    let mut before_rows = Vec::with_capacity(
        before_lines
            .len()
            .checked_mul(8)
            .ok_or(prodex_mojo_core::MojoError::InvalidInput)?,
    );
    for line in &before_lines {
        let counts = rust_critical_signal_counts_for_line(line);
        let key_id = if counts.is_empty() {
            -1
        } else {
            let next_id = key_ids.len();
            let key_id = *key_ids
                .entry(critical_signal_line_key(line))
                .or_insert(next_id);
            if key_id == after_available.len() {
                after_available.push(0);
            }
            i64::try_from(key_id).map_err(|_| prodex_mojo_core::MojoError::InvalidInput)?
        };
        before_rows.push(key_id);
        for value in counts.values() {
            before_rows
                .push(i64::try_from(value).map_err(|_| prodex_mojo_core::MojoError::InvalidInput)?);
        }
    }

    Ok((before_rows, after_available, before_lines.len()))
}

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
