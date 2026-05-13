use super::*;

#[test]
fn path_aliases_extract_repeated_marker_prefixes_when_savings_are_positive() {
    let aliases = smart_context_path_aliases(
        "psc art /home/doxa/IdeaProjects/prodex/crates/prodex-app/src/lib.rs \
         and /home/doxa/IdeaProjects/prodex/crates/prodex-runtime-proxy/src/lib.rs",
    );

    assert_eq!(
        aliases,
        vec![(
            "$R".to_string(),
            "/home/doxa/IdeaProjects/prodex".to_string()
        )]
    );
}

#[test]
fn path_aliases_trim_outer_punctuation_and_use_generic_directory_prefixes() {
    let aliases = smart_context_path_aliases(
        "see (/home/doxa/IdeaProjects/prodex/foo/main.rs), \
         /home/doxa/IdeaProjects/prodex/foo/lib.rs.",
    );

    assert_eq!(
        aliases,
        vec![(
            "$R".to_string(),
            "/home/doxa/IdeaProjects/prodex/foo".to_string()
        )]
    );
}

#[test]
fn path_aliases_ignore_relative_single_or_too_short_prefixes() {
    assert!(smart_context_path_aliases("src/main.rs src/lib.rs").is_empty());
    assert!(
        smart_context_path_aliases("/home/doxa/IdeaProjects/prodex/src/main.rs only-once")
            .is_empty()
    );
    assert!(smart_context_path_aliases("/a/b/src/main.rs /a/b/src/lib.rs").is_empty());
}
