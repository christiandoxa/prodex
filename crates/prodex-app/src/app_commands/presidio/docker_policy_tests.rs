use super::*;

#[test]
fn image_policy_preserves_exact_semver_and_digest_compatibility() {
    for (version, accepted) in [
        ("2.2.364", true),
        ("2.2.365+build.001", true),
        ("3.0.0", true),
        ("18446744073709551615.0.0", true),
        ("2.2.363", false),
        ("2.2.364-rc.1", false),
        ("3.0.0-alpha", false),
        ("2.2.0364", false),
        ("v2.2.364", false),
        (" 2.2.364", false),
        ("2.2.364\u{2003}", false),
        ("2.2.364+", false),
        ("2.2.364+build_1", false),
        ("18446744073709551616.0.0", false),
        ("2.2", false),
        ("", false),
    ] {
        let reference = semver::Version::parse(version).is_ok_and(|version| {
            version.pre.is_empty() && version >= semver::Version::new(2, 2, 364)
        });
        assert_eq!(reference, accepted, "test reference for {version:?}");
        for (digest, supported) in [
            (String::new(), true),
            (format!("@sha256:{}", "a".repeat(64)), true),
            (format!("@sha256:{}", "A".repeat(64)), true),
            (format!("@sha256:{}", "a".repeat(63)), false),
            (format!("@sha256:{}", "g".repeat(64)), false),
            (format!("@sha512:{}", "a".repeat(64)), false),
            ("@sha256:".to_owned(), false),
            ("@".to_owned(), false),
        ] {
            let image = format!("ghcr.io/data-privacy-stack/presidio-analyzer:{version}{digest}");
            assert_eq!(
                validate_presidio_image(&image, "analyzer").is_ok(),
                accepted && supported,
                "{image}"
            );
        }
    }
}

#[test]
fn image_policy_preserves_diagnostics_and_validation_precedence() {
    for (image, expected) in [
        (
            "example.com/presidio:3.0.0",
            "Presidio analyzer image must use the official ghcr.io/data-privacy-stack/presidio-analyzer:<version> image",
        ),
        (
            "ghcr.io/data-privacy-stack/presidio-analyzer:v3.0.0@wrong",
            "Presidio analyzer image has invalid version v3.0.0",
        ),
        (
            "ghcr.io/data-privacy-stack/presidio-analyzer:2.2.363@wrong",
            "Presidio analyzer 2.2.363 is incompatible; Prodex requires 2.2.364 or newer. Update Presidio to the latest stable release (release-qualified reference: 2.2.364)",
        ),
        (
            "ghcr.io/data-privacy-stack/presidio-analyzer:3.0.0@wrong",
            "Presidio analyzer image digest must use sha256",
        ),
        (
            "ghcr.io/data-privacy-stack/presidio-analyzer:3.0.0@sha256:123",
            "Presidio analyzer image digest must be a 64-character SHA-256",
        ),
    ] {
        assert_eq!(
            validate_presidio_image(image, "analyzer")
                .unwrap_err()
                .to_string(),
            expected
        );
    }
}

#[test]
fn container_policy_preserves_ownership_service_and_single_loopback_binding() {
    for (name, image) in [
        (PRESIDIO_ANALYZER_CONTAINER, PRESIDIO_ANALYZER_IMAGE),
        (PRESIDIO_ANONYMIZER_CONTAINER, PRESIDIO_ANONYMIZER_IMAGE),
    ] {
        let container = serde_json::json!({
            "Config": { "Image": image, "Labels": {
                "com.prodex.presidio.managed": "true",
                "com.prodex.presidio.service": name,
            }},
            "HostConfig": { "PortBindings": {
                "3000/tcp": [{ "HostIp": "127.0.0.1", "HostPort": "5002" }]
            }},
        });
        validate_presidio_container(&container, name, "5002").unwrap();
        for (path, value) in [
            ("/Config", serde_json::Value::Null),
            (
                "/Config/Image",
                serde_json::json!("example.com/foreign:3.0.0"),
            ),
            (
                "/Config/Labels/com.prodex.presidio.managed",
                serde_json::json!(true),
            ),
            (
                "/Config/Labels/com.prodex.presidio.service",
                serde_json::json!("other"),
            ),
            ("/HostConfig/PortBindings", serde_json::json!({})),
            (
                "/HostConfig/PortBindings",
                serde_json::json!({ "3000/tcp": [] }),
            ),
            (
                "/HostConfig/PortBindings",
                serde_json::json!({
                    "3000/tcp": [{ "HostIp": "127.0.0.1", "HostPort": "5002" }],
                    "4000/tcp": [],
                }),
            ),
            (
                "/HostConfig/PortBindings",
                serde_json::json!({ "3000/tcp": [
                { "HostIp": "127.0.0.1", "HostPort": "5002" },
                { "HostIp": "127.0.0.1", "HostPort": "5002" },
            ] }),
            ),
            (
                "/HostConfig/PortBindings/3000~1tcp/0/HostPort",
                serde_json::json!(5002),
            ),
            (
                "/HostConfig/PortBindings/3000~1tcp/0/HostIp",
                serde_json::json!("0.0.0.0"),
            ),
        ] {
            let mut invalid = container.clone();
            *invalid.pointer_mut(path).unwrap() = value;
            assert_eq!(
                validate_presidio_container(&invalid, name, "5002")
                    .unwrap_err()
                    .to_string(),
                format!(
                    "refusing existing Presidio container {name}: Prodex ownership, compatible official image version, or port configuration does not match"
                ),
                "{path}"
            );
        }
    }
    assert_eq!(
        validate_presidio_container(&serde_json::Value::Null, "other", "5002")
            .unwrap_err()
            .to_string(),
        "unknown Prodex-managed Presidio container other"
    );
}
