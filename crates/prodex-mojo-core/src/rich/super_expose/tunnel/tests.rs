use super::*;

fn official_version(version: &str) -> String {
    format!("{version}+{} (git sha: {})", "1".repeat(40), "1".repeat(40))
}

#[test]
fn tunnel_supported_version_selects_first_compatible_line_and_canonical_label() {
    let input = format!(
        "probe notice\r\n{}\r\n \t{} \r\n{}\n",
        official_version("0.0.12"),
        official_version("0.0.15"),
        official_version("0.0.17"),
    );
    assert_eq!(
        super_expose_tunnel_supported_version(&input, "0.0.13"),
        Ok(Some("0.0.15"))
    );
    assert_eq!(
        super_expose_tunnel_supported_version(&input, "0.0.16"),
        Ok(Some("0.0.17"))
    );
}

#[test]
fn tunnel_supported_version_uses_numeric_semver_order_and_u64_limits() {
    for (version, minimum, expected) in [
        ("0.0.13", "0.0.13", Some("0.0.13")),
        ("0.0.9", "0.0.13", None),
        ("0.1.0", "0.0.13", Some("0.1.0")),
        ("1.0.0", "0.9.99", Some("1.0.0")),
        ("1.9.99", "1.10.0", None),
        ("1.2.13", "1.2.14", None),
        (
            "18446744073709551615.18446744073709551615.18446744073709551615",
            "18446744073709551615.18446744073709551615.18446744073709551615",
            Some("18446744073709551615.18446744073709551615.18446744073709551615"),
        ),
    ] {
        assert_eq!(
            super_expose_tunnel_supported_version(&official_version(version), minimum),
            Ok(expected),
            "{version} >= {minimum}"
        );
    }
    for version in [
        "00.0.15",
        "0.00.15",
        "0.0.015",
        "18446744073709551616.0.0",
        "0.18446744073709551616.0",
        "0.0.18446744073709551616",
        "0.0.15-rc.1",
        "v0.0.15",
        "0.0.15+extra",
        "0.0",
        "0.0.15.0",
    ] {
        assert_eq!(
            super_expose_tunnel_supported_version(&official_version(version), "0.0.13"),
            Ok(None),
            "{version}"
        );
    }
}

#[test]
fn tunnel_supported_version_preserves_independent_ascii_format_gate() {
    let input = format!("\u{2003}{}\u{a0}", official_version("0.0.15"));
    assert_eq!(
        super_expose_tunnel_supported_version(&input, "0.0.13"),
        Ok(None)
    );
    let input = format!("{input}\n{}", official_version("0.0.12"));
    assert_eq!(
        super_expose_tunnel_supported_version(&input, "0.0.13"),
        Ok(Some("0.0.15"))
    );
}

#[test]
fn tunnel_supported_version_rejects_truncation_and_mismatched_sha() {
    let input = official_version("0.0.15").replace(&"1".repeat(40), &"A".repeat(40));
    assert_eq!(
        super_expose_tunnel_supported_version(&input, "0.0.13"),
        Ok(Some("0.0.15"))
    );
    let input = official_version("0.0.15");
    for end in 0..input.len() {
        assert_eq!(
            super_expose_tunnel_supported_version(&input[..end], "0.0.13"),
            Ok(None),
            "truncated at {end}"
        );
    }
    for input in [
        format!("{input} unexpected"),
        input.replacen("111", "112", 1),
        input.replace("111", "11g"),
    ] {
        assert_eq!(
            super_expose_tunnel_supported_version(&input, "0.0.13"),
            Ok(None)
        );
    }
}

#[test]
fn tunnel_supported_version_rejects_invalid_minimum_and_oversized_input() {
    for minimum in ["", "0.0.013", "0.0.13-rc.1", "0.0.13+build", "v0.0.13"] {
        assert_eq!(
            super_expose_tunnel_supported_version("", minimum),
            Err(MojoError::InvalidInput)
        );
    }
    assert_eq!(
        super_expose_tunnel_supported_version(&" ".repeat(16_385), "0.0.13"),
        Err(MojoError::InvalidInput)
    );
    assert_eq!(
        super_expose_tunnel_supported_version("", &"1".repeat(65)),
        Err(MojoError::InvalidInput)
    );
    assert_eq!(
        super_expose_tunnel_supported_version(&" ".repeat(16_384), "0.0.13"),
        Ok(None)
    );
}

#[test]
fn tunnel_supported_version_abi_rejects_invalid_views() {
    let minimum = "0.0.13";
    let invalid_utf8 = [0xff_u8];
    let mut output = [-1_i64; 3];
    for (abi, address, length, expected) in [
        (2, 0, 0, 4),
        (1, 0, -1, 1),
        (1, 0, 1, 1),
        (1, 0, 16_385, 1),
        (1, invalid_utf8.as_ptr() as u64, 1, 1),
    ] {
        let status = unsafe {
            prodex_super_expose_tunnel_supported_version_v1(
                abi,
                address,
                length,
                minimum.as_ptr() as u64,
                minimum.len() as i64,
                output.as_mut_ptr() as u64,
            )
        };
        assert_eq!(status, expected);
        assert_eq!(output, [-1; 3]);
    }
}
