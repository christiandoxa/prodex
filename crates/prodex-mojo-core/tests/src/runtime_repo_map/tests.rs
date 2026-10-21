use super::*;

#[test]
fn repo_map_kernel_matches_module_helpers() {
    assert_eq!(
        repo_chunk_plan(0, 4, 10, 10, true).unwrap(),
        RepoChunkPlan::Accept
    );
    assert_eq!(
        repo_chunk_plan(4, 4, 10, 10, true).unwrap(),
        RepoChunkPlan::RejectCapacity
    );
    assert_eq!(
        repo_chunk_plan(0, 4, 10, 9, true).unwrap(),
        RepoChunkPlan::RejectInvalid
    );
    assert_eq!(repo_path_distance(10, 20, 15).unwrap(), 0);
    assert_eq!(repo_path_distance(10, 20, 25).unwrap(), 5);
    assert_eq!(
        repo_symbol_kind(Some("test_symbol"), "fn t() {}").unwrap(),
        RepoSymbolKind::Test
    );
    assert_eq!(
        repo_symbol_kind(Some("symbol"), "pub mod alpha {").unwrap(),
        RepoSymbolKind::Module
    );
    assert!(repo_entry_should_replace(2, 1, "b", "a").unwrap());
    assert!(repo_entry_should_replace(1, 1, "a", "b").unwrap());

    for (path, expected) in [
        ("src/foo/mod.rs", Some("foo".to_string())),
        ("src/lib.rs", Some("lib".to_string())),
        ("a/src/api/client.ts", Some("api::client".to_string())),
        ("b/app/index.js", Some("app".to_string())),
        ("./pkg/main.py", Some("pkg".to_string())),
        ("src\\api\\mod.rs", Some("api".to_string())),
        ("\"a/src/foo.rs\"", Some("a::src::foo".to_string())),
        ("dir.name/file", Some("dir".to_string())),
        (".", None),
        ("", None),
    ] {
        assert_eq!(repo_module_from_path(path).unwrap(), expected, "{path:?}");
    }
}
