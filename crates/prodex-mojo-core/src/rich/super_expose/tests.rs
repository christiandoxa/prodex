use super::*;

#[test]
fn tunnel_client_version_policy_accepts_official_format_across_versions() {
    for value in [
        "0.0.13+4b5267f823be0b046bb883aacb51603cfde3a0ea (git sha: 4b5267f823be0b046bb883aacb51603cfde3a0ea)",
        "0.0.15+a390c168ff1b2d14e73a95991c186c6aba3ff5a0 (git sha: a390c168ff1b2d14e73a95991c186c6aba3ff5a0)",
        "0.0.16+1111111111111111111111111111111111111111 (git sha: 1111111111111111111111111111111111111111)",
    ] {
        assert!(super_expose_tunnel_client_version_output_valid(value).unwrap());
    }
}

#[test]
fn tunnel_client_version_policy_rejects_malformed_official_format() {
    for value in [
        "0.0.15+1111111111111111111111111111111111111111 (git sha: 2222222222222222222222222222222222222222)",
        "0.0.15+not-a-git-sha (git sha: not-a-git-sha)",
        "0.0.15",
    ] {
        assert!(!super_expose_tunnel_client_version_output_valid(value).unwrap());
    }
}

#[test]
fn concurrency_plan_preserves_menu_order_and_selected_choice_mapping() {
    use SuperExposeConcurrencyChoice::*;

    let presets = [4, 8, 16, 32];
    let (choices, selected) = super_expose_concurrency_plan(4, &presets, 64, None).unwrap();
    assert_eq!(
        choices,
        [
            Default,
            Preset(4),
            Preset(8),
            Preset(16),
            Preset(32),
            Custom
        ]
    );
    assert_eq!(selected, None);

    for (index, expected) in [
        Default,
        Preset(4),
        Preset(8),
        Preset(16),
        Preset(32),
        Custom,
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            super_expose_concurrency_plan(4, &presets, 64, Some(index))
                .unwrap()
                .1,
            Some(expected)
        );
    }
}

#[test]
fn concurrency_plan_handles_empty_presets_and_rejects_invalid_bounds() {
    use SuperExposeConcurrencyChoice::*;

    assert_eq!(
        super_expose_concurrency_plan(4, &[], 64, Some(1)).unwrap(),
        (vec![Default, Custom], Some(Custom))
    );
    for (default, presets, hard_max, selected) in [
        (0, &[][..], 64, None),
        (8, &[][..], 4, None),
        (4, &[65][..], 64, None),
        (4, &[8, 8][..], 64, None),
        (4, &[][..], 64, Some(2)),
        (4, &[][..], 64, Some(usize::MAX)),
    ] {
        assert_eq!(
            super_expose_concurrency_plan(default, presets, hard_max, selected),
            Err(MojoError::InvalidInput)
        );
    }
    assert_eq!(
        super_expose_concurrency_plan(4, &[1; 33], 64, None),
        Err(MojoError::InvalidInput)
    );
}
