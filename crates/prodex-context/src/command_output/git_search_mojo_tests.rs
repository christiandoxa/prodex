
use super::*;

fn oracle(input: &str, options: &CommandOutputCompactOptions) -> String {
    let normalized = normalize_command_output(input);
    let output = compact_git_status_output_rust(&normalized, options);
    if options.max_lines <= 12 {
        output
    } else {
        canonicalize_compacted_command_paths(&normalized, &output, CommandOutputKind::GitStatus)
    }
}

#[test]
fn public_mojo_git_status_formatter_matches_860_case_rust_oracle() {
    let mut cases = Vec::new();
    for first in [' ', 'M', 'A', 'D', 'R', 'C', 'U', '?', '!'] {
        for second in [' ', 'M', 'A', 'D', 'R', 'C', 'U', '?', '!'] {
            cases.push(format!(
                "## main\n{first}{second} src/one.rs\n{first}{second} src/one.rs\n"
            ));
        }
    }
    cases.extend([
            "\n\n\n\n".to_string(),
            "## main\n M café/文件.rs\n??  alpha\u{00a0}\n".to_string(),
            "## first\n M one\n## second\n M two\n".to_string(),
            "On branch main\nChanges not staged for commit:\n  deleted : old.rs\nUntracked files:\n a\n a\n b\n".to_string(),
            format!(
                "## main\n{}",
                (0..48)
                    .map(|index| format!("?? src/file_{index:03}.rs\n"))
                    .collect::<String>()
            ),
        ]);
    for max_path_entries in [
        0,
        1,
        6,
        23,
        24,
        25,
        120,
        i64::MAX as usize - 5,
        i64::MAX as usize,
        usize::MAX,
    ] {
        let options = CommandOutputCompactOptions {
            kind: CommandOutputKind::GitStatus,
            max_lines: 1_000,
            max_line_chars: 20_000,
            max_path_entries,
            ..CommandOutputCompactOptions::default()
        };
        for input in &cases {
            assert_eq!(
                compact_command_output_with_options(input, &options).output,
                oracle(input, &options),
                "max_path_entries={max_path_entries}, input={input:?}",
            );
        }
    }
}

#[test]
fn mojo_git_status_handles_blank_heavy_and_long_inputs() {
    let options = CommandOutputCompactOptions {
        kind: CommandOutputKind::GitStatus,
        max_lines: 1_000,
        max_line_chars: 20_000,
        ..CommandOutputCompactOptions::default()
    };
    for input in [
        "\n".repeat(4 * 1024 * 1024 - 1) + "x",
        format!("## main\n?? {}\n", "界".repeat(1_500_000)),
        "## main\n?? same\n".repeat(200_000),
    ] {
        assert_eq!(
            compact_command_output_with_options(&input, &options).output,
            oracle(&input, &options)
        );
    }
}

fn file_list_oracle(input: &str, options: &CommandOutputCompactOptions) -> String {
    let normalized = normalize_command_output(input);
    let output = compact_file_list_output_rust(&normalized, options);
    if options.max_lines <= 12 {
        output
    } else {
        canonicalize_compacted_command_paths(&normalized, &output, CommandOutputKind::FileList)
    }
}

#[test]
fn public_mojo_file_list_formatter_matches_rust_oracle() {
    let cases = [
        "",
        "ordinary output\n",
        "src/lib.rs\nsrc/main.rs\nsrc/bin/tool.rs\nREADME.md\n",
        "./src/lib.rs\nC:\\workspace\\src\\main.rs\n/home/test-user/file.txt\n",
        "root\n├── src\n│   ├── lib.rs\n│   └── main.rs\n└── README.md\n",
        "-rw-r--r-- 1 user group 12 Sep 14 12:00 file one.rs\ndrwxr-xr-x 2 user group 12 Sep 14 12:00 src\n",
        "a/A.RS\na/B.rs\nb/no_extension\nc/archive.tar.gz\nCargo.toml\n",
        "a/x.rs\na/x.rs\na/y.rs\nb/z.md\nc/z.md\nd/z.md\ne/z.md\nf/z.md\ng/z.md\nh/z.md\ni/z.md\n",
        "café/文件.rs\n資料/長い名前.TXT\n",
    ];
    for max_path_entries in [0, 1, 4, 5, 120, usize::MAX] {
        for max_line_chars in [0, 24, 25, 64, usize::MAX] {
            let options = CommandOutputCompactOptions {
                kind: CommandOutputKind::FileList,
                max_lines: 1_000,
                max_line_chars,
                max_path_entries,
                ..CommandOutputCompactOptions::default()
            };
            for input in cases {
                assert_eq!(
                    compact_command_output_with_options(input, &options).output,
                    file_list_oracle(input, &options),
                    "max_path_entries={max_path_entries}, max_line_chars={max_line_chars}, input={input:?}",
                );
            }
        }
    }
}

#[test]
fn public_mojo_file_list_generated_and_large_corpus_matches_rust_oracle() {
    let mut cases = Vec::new();
    for prefix in ["", "./", "/", "|-- ", "`-- ", "├── ", "│   ├── ", "└── "] {
        for path in [
            "src/lib.rs",
            "README.md",
            "café/文件.RS",
            "no_extension",
            "a b/file.txt",
            "archive.tar.gz",
        ] {
            cases.push(format!("{prefix}{path}\n"));
        }
    }
    cases.extend([
        "-rw-r--r-- 1 user group 12 Sep 14 12:00 file one.rs\n".to_string(),
        "total 12\n".to_string(),
        "https://example.com/file.rs\n".to_string(),
        "[...]\n".to_string(),
        "same/x.rs\nsame/x.rs\nsame/x.rs\n".to_string(),
    ]);
    for (max_lines, max_path_entries, max_line_chars) in [
        (1, 0, 0),
        (12, 1, 24),
        (13, 2, 25),
        (1_000, 5, 40),
        (1_000, usize::MAX, usize::MAX),
    ] {
        let options = CommandOutputCompactOptions {
            kind: CommandOutputKind::FileList,
            max_lines,
            max_line_chars,
            max_path_entries,
            ..CommandOutputCompactOptions::default()
        };
        for input in &cases {
            assert_eq!(
                compact_command_output_with_options(input, &options).output,
                file_list_oracle(input, &options),
                "max_lines={max_lines}, max_path_entries={max_path_entries}, max_line_chars={max_line_chars}, input_bytes={}",
                input.len(),
            );
        }
    }
}

fn assert_large_file_list_parity(input: String) {
    let options = CommandOutputCompactOptions {
        kind: CommandOutputKind::FileList,
        max_lines: 1_000,
        ..CommandOutputCompactOptions::default()
    };
    assert_eq!(
        compact_command_output_with_options(&input, &options).output,
        file_list_oracle(&input, &options),
        "large input bytes={}",
        input.len(),
    );
}

#[test]
fn public_mojo_file_list_long_unicode_path_matches_rust_oracle() {
    assert_large_file_list_parity(format!("root/{}.rs\n", "界".repeat(1_500_000)));
}
