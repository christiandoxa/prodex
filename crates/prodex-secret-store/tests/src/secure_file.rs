use super::*;

#[test]
fn private_file_facade_accepts_search_only_ancestors() {
    use std::os::unix::fs::PermissionsExt as _;

    let root = temp_dir("search-only-ancestor");
    let ancestor = root.join("home");
    let private = ancestor.join(".prodex");
    fs::create_dir(&ancestor).unwrap();
    fs::create_dir(&private).unwrap();
    fs::set_permissions(&ancestor, fs::Permissions::from_mode(0o111)).unwrap();
    ensure_private_directory(&private).unwrap();
    assert_eq!(
        fs::metadata(&private).unwrap().permissions().mode() & 0o777,
        0o700
    );
    let path = private.join("auth.json");

    write_private_file_atomic(&path, b"secret").unwrap();
    assert_eq!(
        read_private_file_bounded(&path, 6)
            .unwrap()
            .unwrap()
            .as_slice(),
        b"secret"
    );

    fs::set_permissions(&ancestor, fs::Permissions::from_mode(0o700)).unwrap();
    let _ = fs::remove_dir_all(root);
}
