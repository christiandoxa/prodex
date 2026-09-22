#![cfg(feature = "mojo-runtime")]
#![allow(unsafe_code)]

use prodex_mojo_core::launch_config::{LaunchConfigArgument as Arg, plan_launch_config};

#[repr(C)]
struct View {
    address: u64,
    length: u64,
    valid_utf8: i64,
}
unsafe extern "C" {
    fn prodex_mojo_launch_config_v1(
        version: i64,
        args: u64,
        count: i64,
        keys: u64,
        key_count: i64,
        output: u64,
        output_count: i64,
        replaced: u64,
        replaced_count: i64,
    ) -> i64;
}

fn oracle(args: &[Option<&str>], keys: &[&str]) -> (Vec<Arg>, Vec<bool>) {
    let mut result = vec![Arg::Original; args.len()];
    let mut replaced = vec![false; keys.len()];
    let mut index = 0;
    while index < args.len() {
        let Some(arg) = args[index] else {
            index += 1;
            continue;
        };
        if arg == "--" {
            break;
        }
        let (value, kind): (_, fn(usize) -> Arg) =
            if matches!(arg, "-c" | "--config") && index + 1 < args.len() {
                index += 1;
                (args[index], Arg::Separate)
            } else if let Some(value) = arg.strip_prefix("--config=") {
                (Some(value), Arg::LongInline)
            } else if let Some(value) = arg
                .strip_prefix("-c")
                .filter(|s| !s.is_empty() && s.contains('='))
            {
                (Some(value), Arg::ShortInline)
            } else {
                (None, Arg::Separate)
            };
        if let Some((key, _)) = value.and_then(|v| v.split_once('=')) {
            let key = key.trim();
            if !key.is_empty()
                && let Some(key) = keys.iter().position(|&candidate| candidate == key)
            {
                result[index] = kind(key);
                replaced[key] = true;
            }
        }
        index += 1;
    }
    (result, replaced)
}

#[test]
fn config_plan_matches_ten_thousand_seeded_vectors() {
    const { assert!(prodex_mojo_core::MOJO_ACTIVE) };
    let tokens = [
        None,
        Some(""),
        Some("-c"),
        Some("--config"),
        Some("--"),
        Some("exec"),
        Some("resume"),
        Some("x=old"),
        Some("-cx=old"),
        Some("--config=x=old"),
        Some("--config=x"),
        Some("-cx"),
        Some("=empty"),
        Some(" x \t=old"),
        Some("--config=\u{2003}x\u{00a0}=old"),
        Some("--config=\u{001c}x=old"),
        Some("-c--=separator-value"),
        Some("--=separator-value"),
        Some("--config=東京=value=tail"),
        Some("-c\u{2003}=blank"),
        Some("--config==empty"),
        Some("--config=--config=x=nested"),
        Some("--model=x"),
        Some("--config=key\0tail=value"),
    ];
    let keys = [
        "x",
        "x",
        "東京",
        "--",
        "--config",
        "key\0tail",
        "",
        "\u{001c}x",
    ];
    let mut seed = 0xbc83_f05c_4389_8901_u64;
    for _ in 0..10_000 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let size = (seed >> 32) as usize % 60;
        let args = (0..size)
            .map(|_| {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                tokens[(seed >> 32) as usize % tokens.len()]
            })
            .collect::<Vec<_>>();
        let plan = plan_launch_config(&args, &keys).unwrap();
        assert_eq!(
            (plan.arguments, plan.replaced),
            oracle(&args, &keys),
            "{args:?}"
        );
    }
}

#[test]
fn config_boundary_validates_lengths_and_utf8_before_writing() {
    let invalid_bytes = [0xff_u8];
    let invalid = View {
        address: invalid_bytes.as_ptr() as u64,
        length: 1,
        valid_utf8: 1,
    };
    let mut output = [77_i64; 2];
    let mut replaced = [77_i64; 1];
    let calls = [
        (2, 0, 0, 0, 0, 0, 0, 4),
        (1, 0, -1, 0, 0, 0, 0, 1),
        (1, 0, 1, 0, 0, 1, 0, 1),
        (1, 0, 0, 0, 1, 0, 1, 1),
        (1, 0, 0, 0, 0, 1, 0, 3),
        (1, 0, 0, 0, 0, 0, 1, 3),
        (1, &invalid as *const View as u64, 1, 0, 0, 1, 0, 2),
        (1, 0, 0, &invalid as *const View as u64, 1, 0, 1, 2),
    ];
    for (version, args, count, keys, key_count, output_count, replaced_count, expected) in calls {
        let result = unsafe {
            prodex_mojo_launch_config_v1(
                version,
                args,
                count,
                keys,
                key_count,
                output.as_mut_ptr() as u64,
                output_count,
                replaced.as_mut_ptr() as u64,
                replaced_count,
            )
        };
        assert_eq!(result, expected);
        assert_eq!(output, [77; 2]);
        assert_eq!(replaced, [77]);
    }
}

#[test]
fn config_plan_preserves_first_duplicate_override_and_separator_value_rules() {
    let args = [
        Some("-c"),
        Some("--"),
        Some("--config=x=old"),
        Some("--"),
        Some("-cx=literal"),
    ];
    let plan = plan_launch_config(&args, &["x", "x"]).unwrap();
    assert_eq!(
        plan.arguments,
        vec![
            Arg::Original,
            Arg::Original,
            Arg::LongInline(0),
            Arg::Original,
            Arg::Original
        ]
    );
    assert_eq!(plan.replaced, vec![true, false]);
    let no_keys = plan_launch_config(&args, &[]).unwrap();
    assert_eq!(no_keys.arguments, vec![Arg::Original; args.len()]);
    assert!(no_keys.replaced.is_empty());
}
