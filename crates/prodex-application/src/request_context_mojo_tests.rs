use super::*;

#[test]
fn mojo_metadata_normalization_matches_rust_presence_oracle() {
    let mut headers = vec![
        GatewayHttpHeader::new(" TraceParent ", "ignored"),
        GatewayHttpHeader::new("AUTHORIZATION", "ignored"),
        GatewayHttpHeader::new("x-codex-turn-state", "ignored"),
        GatewayHttpHeader::new(" X-CODEX-BETA-FEATURES ", "ignored"),
        GatewayHttpHeader::new("User-Agent", "ignored"),
        GatewayHttpHeader::new("x-private", "ignored"),
    ];
    headers.extend(
        (headers.len()..70)
            .map(|index| GatewayHttpHeader::new(format!("x-padding-{index}"), "ignored")),
    );

    assert_eq!(
        ApplicationRequestMetadata::from_headers(&headers),
        ApplicationRequestMetadata::from_headers_rust(&headers),
    );
}
