use super::*;
use windows_sys::Win32::Security::{
    CreateWellKnownSid, INHERITED_ACE, SECURITY_MAX_SID_SIZE, WinInteractiveSid,
};

#[test]
fn handle_relative_replace_works() {
    let root = std::env::temp_dir().join(format!(
        "prodex-secret-windows-rename-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let source = root.join("source.tmp");
    let target = root.join("target.json");
    fs::write(&source, b"new").unwrap();
    fs::write(&target, b"old").unwrap();
    let parent = open_directory(&root, false).unwrap();
    let file = OpenOptions::new()
        .access_mode(FILE_GENERIC_READ | FILE_GENERIC_WRITE | DELETE)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .open(&source)
        .unwrap();

    rename_opened_file(&parent, OsStr::new("target.json"), &file).unwrap();

    assert_eq!(fs::read(&target).unwrap(), b"new");
    drop(file);
    drop(parent);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn private_acl_round_trip_is_handle_verified() {
    let root =
        std::env::temp_dir().join(format!("prodex-secret-windows-acl-{}", std::process::id()));
    fs::create_dir_all(&root).unwrap();
    let directory = Directory::open_path(&root, false).unwrap();
    let file = directory
        .create_private_file(OsStr::new("private.tmp"))
        .unwrap();
    validate_acl(&file, AclUse::PrivateFile).unwrap();
    drop(file);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn private_acl_rejects_sensitive_access_for_a_process_group() {
    let root = std::env::temp_dir().join(format!(
        "prodex-secret-windows-group-acl-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    let directory = Directory::open_path(&root, false).unwrap();
    let file = directory
        .create_private_file(OsStr::new("private.tmp"))
        .unwrap();
    let user = CurrentUserSid::load().unwrap();
    let mut group_storage =
        vec![0usize; (SECURITY_MAX_SID_SIZE as usize).div_ceil(size_of::<usize>())];
    let mut group_len = SECURITY_MAX_SID_SIZE;
    // SAFETY: the aligned buffer contains SECURITY_MAX_SID_SIZE writable
    // bytes and remains live while its SID is added to the ACL.
    assert_ne!(
        unsafe {
            CreateWellKnownSid(
                WinInteractiveSid,
                std::ptr::null_mut(),
                group_storage.as_mut_ptr().cast(),
                &mut group_len,
            )
        },
        0
    );
    let group = group_storage.as_mut_ptr().cast();
    set_test_acl_with_group(&file, user.sid(), group, FILE_GENERIC_READ);

    let error = validate_acl(&file, AclUse::PrivateFile).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);

    drop(file);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn external_acl_allows_inherited_read_only_group_but_rejects_write() {
    let root = std::env::temp_dir().join(format!(
        "prodex-secret-windows-external-acl-{}",
        std::process::id()
    ));
    fs::create_dir_all(&root).unwrap();
    let directory = Directory::open_path(&root, false).unwrap();
    let file = directory
        .create_private_file(OsStr::new("external.tmp"))
        .unwrap();
    let user = CurrentUserSid::load().unwrap();
    let mut group_storage =
        vec![0usize; (SECURITY_MAX_SID_SIZE as usize).div_ceil(size_of::<usize>())];
    let mut group_len = SECURITY_MAX_SID_SIZE;
    // SAFETY: the aligned buffer contains SECURITY_MAX_SID_SIZE writable
    // bytes and remains live while its SID is added to the ACL.
    assert_ne!(
        unsafe {
            CreateWellKnownSid(
                WinInteractiveSid,
                std::ptr::null_mut(),
                group_storage.as_mut_ptr().cast(),
                &mut group_len,
            )
        },
        0
    );
    let group = group_storage.as_mut_ptr().cast();

    set_test_acl_with_group(&file, user.sid(), group, FILE_GENERIC_READ);
    validate_acl(&file, AclUse::ExternalFile).unwrap();

    set_test_acl_with_group(&file, user.sid(), group, FILE_GENERIC_WRITE);
    let error = validate_acl(&file, AclUse::ExternalFile).unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);

    drop(file);
    fs::remove_dir_all(root).unwrap();
}

fn set_test_acl_with_group(file: &File, user: PSID, group: PSID, group_access: u32) {
    // SAFETY: both SIDs point into live aligned buffers owned by the caller.
    let user_len = unsafe { GetLengthSid(user) };
    // SAFETY: same as above.
    let group_len = unsafe { GetLengthSid(group) };
    let acl_len = size_of::<ACL>()
        + 2 * (size_of::<ACCESS_ALLOWED_ACE>() - size_of::<u32>())
        + usize::try_from(user_len).unwrap_or(0)
        + usize::try_from(group_len).unwrap_or(0);
    let mut storage = vec![0usize; acl_len.div_ceil(size_of::<usize>())];
    let acl = storage.as_mut_ptr().cast::<ACL>();
    // SAFETY: storage is aligned and large enough for both allow ACEs;
    // the SIDs and file handle remain live for the complete operation.
    unsafe {
        assert_ne!(
            InitializeAcl(
                acl,
                u32::try_from(acl_len).unwrap_or(u32::MAX),
                ACL_REVISION,
            ),
            0
        );
        assert_ne!(
            AddAccessAllowedAceEx(acl, ACL_REVISION, 0, FILE_ALL_ACCESS, user),
            0
        );
        assert_ne!(
            AddAccessAllowedAceEx(acl, ACL_REVISION, INHERITED_ACE, group_access, group),
            0
        );
        assert_eq!(
            SetSecurityInfo(
                file.as_raw_handle().cast(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                acl,
                std::ptr::null_mut(),
            ),
            ERROR_SUCCESS
        );
    }
}
