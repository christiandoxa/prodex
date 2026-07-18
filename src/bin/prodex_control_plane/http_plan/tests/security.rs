#![cfg(test)]

use super::*;

#[test]
fn control_plane_plan_header_debug_redacts_nested_values() {
    let headers = vec![ControlPlaneHttpHeaderFile {
        name: "Authorization".to_string(),
        value: "Bearer control-plane-plan-secret".to_string(),
    }];

    let rendered = format!("{headers:?}");
    assert!(rendered.contains("<redacted>"));
    assert!(!rendered.contains("control-plane-plan-secret"));
}

#[test]
fn control_plane_plan_rejects_credential_headers_without_echoing_values() {
    for name in [
        "Authorization",
        "Proxy-Authorization",
        "X-Api-Key",
        "Api-Key",
    ] {
        let error = control_plane_plan_headers(vec![ControlPlaneHttpHeaderFile {
            name: name.to_string(),
            value: "Bearer control-plane-plan-secret".to_string(),
        }])
        .expect_err("separate principal evidence makes credential headers invalid here");

        assert_eq!(
            error,
            "control-plane plan input must not include credential headers"
        );
        assert!(!error.contains("control-plane-plan-secret"));
    }
}

#[test]
fn control_plane_plan_header_zeroize_clears_owned_value() {
    let mut header = ControlPlaneHttpHeaderFile {
        name: "X-Trace".to_string(),
        value: "control-plane-plan-secret".to_string(),
    };

    header.zeroize();
    assert!(header.value.is_empty());
}
