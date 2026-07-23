use super::*;
use filetime::FileTime;
use std::ffi::OsString;
use std::sync::atomic::{AtomicU64, Ordering};

static SHARED_CODEX_ATOMIC_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(super) fn ensure_shared_codex_parent_dir(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    Ok(())
}

pub(super) fn load_shared_codex_entry_metadata(path: &Path) -> Result<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => Ok(Some(metadata)),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).with_context(|| format!("failed to inspect {}", path.display())),
    }
}

fn ensure_shared_codex_directory(path: &Path, metadata: &fs::Metadata) -> Result<()> {
    if metadata.is_dir() {
        return Ok(());
    }
    bail!(
        "expected {} to be a directory for shared Codex state",
        path.display()
    );
}

fn ensure_shared_codex_file(path: &Path, metadata: &fs::Metadata) -> Result<()> {
    if metadata.is_file() {
        return Ok(());
    }
    bail!(
        "expected {} to be a file for shared Codex state",
        path.display()
    );
}

fn ensure_shared_codex_path_is_directory(path: &Path) -> Result<()> {
    let Some(metadata) = load_shared_codex_entry_metadata(path)? else {
        bail!(
            "expected {} to be a directory for shared Codex state",
            path.display()
        );
    };
    ensure_shared_codex_directory(path, &metadata)
}

fn ensure_shared_codex_path_is_file(path: &Path) -> Result<()> {
    let Some(metadata) = load_shared_codex_entry_metadata(path)? else {
        bail!(
            "expected {} to be a file for shared Codex state",
            path.display()
        );
    };
    ensure_shared_codex_file(path, &metadata)
}

fn ensure_shared_codex_target_is_directory(path: &Path) -> Result<()> {
    if path.is_dir() {
        return Ok(());
    }
    bail!(
        "expected {} to be a directory for shared Codex state",
        path.display()
    );
}

fn ensure_shared_codex_target_is_file(path: &Path) -> Result<()> {
    if path.is_file() {
        return Ok(());
    }
    bail!(
        "expected {} to be a file for shared Codex state",
        path.display()
    );
}

pub(super) fn copy_shared_codex_file(source: &Path, destination: &Path) -> Result<()> {
    copy_shared_codex_file_replacing_existing(source, destination, "failed to copy")
}

pub(super) fn copy_shared_codex_file_replacing_existing(
    source: &Path,
    destination: &Path,
    context: &str,
) -> Result<()> {
    ensure_shared_codex_parent_dir(destination)?;
    let source_metadata = fs::symlink_metadata(source)
        .with_context(|| format!("failed to inspect {}", source.display()))?;
    ensure_shared_codex_file(source, &source_metadata)?;
    let mut source_file =
        fs::File::open(source).with_context(|| format!("failed to read {}", source.display()))?;
    if !shared_codex_same_file_metadata(&source_metadata, &source_file.metadata()?) {
        bail!("source file changed while opening {}", source.display());
    }

    replace_shared_codex_file_atomic(
        destination,
        source_metadata.permissions(),
        Some((
            FileTime::from_last_access_time(&source_metadata),
            FileTime::from_last_modification_time(&source_metadata),
        )),
        context,
        |destination_file| {
            io::copy(&mut source_file, destination_file)?;
            Ok(())
        },
    )
    .with_context(|| {
        format!(
            "{context} {} to {}",
            source.display(),
            destination.display()
        )
    })
}

pub(super) fn replace_shared_codex_file_atomic<F>(
    destination: &Path,
    permissions: fs::Permissions,
    times: Option<(FileTime, FileTime)>,
    context: &str,
    write: F,
) -> Result<()>
where
    F: FnOnce(&mut fs::File) -> io::Result<()>,
{
    ensure_shared_codex_parent_dir(destination)?;
    if let Some(metadata) = load_shared_codex_entry_metadata(destination)?
        && metadata.is_dir()
        && !metadata.file_type().is_symlink()
    {
        bail!(
            "expected {} to be a file for shared Codex state",
            destination.display()
        );
    }

    let (temporary_path, mut temporary_file) =
        create_shared_codex_atomic_file(destination, &permissions, context)?;
    let prepare_result = (|| -> Result<()> {
        write(&mut temporary_file)
            .with_context(|| format!("{context} temporary file for {}", destination.display()))?;
        temporary_file
            .set_permissions(permissions)
            .with_context(|| format!("failed to preserve mode for {}", destination.display()))?;
        if let Some((accessed, modified)) = times {
            filetime::set_file_handle_times(&temporary_file, Some(accessed), Some(modified))
                .with_context(|| {
                    format!(
                        "failed to preserve modified time for copied file {}",
                        destination.display()
                    )
                })?;
        }
        temporary_file
            .sync_all()
            .with_context(|| format!("failed to sync {}", destination.display()))?;
        Ok(())
    })();
    drop(temporary_file);

    if let Err(error) = prepare_result {
        let _ = fs::remove_file(&temporary_path);
        return Err(error);
    }

    #[cfg(windows)]
    let original_permissions = load_shared_codex_entry_metadata(destination)?
        .filter(|metadata| !metadata.file_type().is_symlink())
        .map(|metadata| metadata.permissions());
    #[cfg(windows)]
    if let Some(metadata) = load_shared_codex_entry_metadata(destination)? {
        if let Err(error) = make_shared_codex_path_writable_for_removal(destination, &metadata) {
            let _ = fs::remove_file(&temporary_path);
            return Err(error);
        }
    }

    if let Err(error) = fs::rename(&temporary_path, destination) {
        let _ = fs::remove_file(&temporary_path);
        #[cfg(windows)]
        if let Some(permissions) = original_permissions {
            let _ = fs::set_permissions(destination, permissions);
        }
        return Err(error).with_context(|| format!("{context} {}", destination.display()));
    }

    #[cfg(unix)]
    if let Some(parent) = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
    {
        fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .with_context(|| format!("failed to sync {}", parent.display()))?;
    }
    Ok(())
}

fn create_shared_codex_atomic_file(
    destination: &Path,
    permissions: &fs::Permissions,
    context: &str,
) -> Result<(PathBuf, fs::File)> {
    let parent = destination
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file_name = destination
        .file_name()
        .context("shared Codex destination must have a file name")?;

    for _ in 0..64 {
        let counter = SHARED_CODEX_ATOMIC_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
        let mut temporary_name = OsString::from(".");
        temporary_name.push(file_name);
        temporary_name.push(format!(".prodex-tmp-{}-{counter}", std::process::id()));
        let temporary_path = parent.join(temporary_name);
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
            options.mode(permissions.mode());
        }
        #[cfg(not(unix))]
        let _ = permissions;
        match options.open(&temporary_path) {
            Ok(file) => return Ok((temporary_path, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("{context} temporary file for {}", destination.display())
                });
            }
        }
    }

    bail!(
        "{context} temporary file for {} after repeated name collisions",
        destination.display()
    )
}

#[cfg(unix)]
fn shared_codex_same_file_metadata(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn shared_codex_same_file_metadata(_left: &fs::Metadata, _right: &fs::Metadata) -> bool {
    true
}

pub(super) fn move_directory(source: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(_) => {
            create_codex_home_if_missing(destination)?;
            copy_directory_contents(source, destination)?;
            fs::remove_dir_all(source)
                .with_context(|| format!("failed to remove {}", source.display()))
        }
    }
}

pub(super) fn move_file(source: &Path, destination: &Path) -> Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    match fs::rename(source, destination) {
        Ok(()) => Ok(()),
        Err(_) => {
            copy_shared_codex_file_replacing_existing(source, destination, "failed to copy")?;
            fs::remove_file(source)
                .with_context(|| format!("failed to remove {}", source.display()))
        }
    }
}

pub(super) fn ensure_symlink_to_shared(
    local_path: &Path,
    shared_path: &Path,
    kind: SharedCodexEntryKind,
) -> Result<()> {
    if let Some(parent) = local_path.parent() {
        create_codex_home_if_missing(parent)?;
    }
    if local_path.exists() || fs::symlink_metadata(local_path).is_ok() {
        remove_path(local_path)?;
    }

    create_symlink(shared_path, local_path, kind)
}

fn create_symlink(target: &Path, link: &Path, kind: SharedCodexEntryKind) -> Result<()> {
    #[cfg(unix)]
    {
        let _ = kind;
        std::os::unix::fs::symlink(target, link).with_context(|| {
            format!(
                "failed to link shared Codex state {} -> {}",
                link.display(),
                target.display()
            )
        })?;
    }

    #[cfg(windows)]
    {
        match kind {
            SharedCodexEntryKind::Directory => std::os::windows::fs::symlink_dir(target, link),
            SharedCodexEntryKind::File => std::os::windows::fs::symlink_file(target, link),
        }
        .with_context(|| {
            format!(
                "failed to link shared Codex state {} -> {}",
                link.display(),
                target.display()
            )
        })?;
    }

    #[cfg(not(any(unix, windows)))]
    {
        let _ = kind;
        bail!("shared Codex session links are not supported on this platform");
    }

    Ok(())
}

fn make_shared_codex_path_writable_for_removal(path: &Path, metadata: &fs::Metadata) -> Result<()> {
    if metadata.file_type().is_symlink() || !metadata.permissions().readonly() {
        return Ok(());
    }

    let mut permissions = metadata.permissions();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        permissions.set_mode(permissions.mode() | 0o200);
    }
    #[cfg(windows)]
    {
        permissions.set_readonly(false);
    }
    #[cfg(not(any(unix, windows)))]
    {
        permissions.set_readonly(false);
    }
    fs::set_permissions(path, permissions)
        .with_context(|| format!("failed to make {} writable", path.display()))?;
    Ok(())
}

pub(super) fn remove_path(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect {}", path.display()))?;
    let file_type = metadata.file_type();

    if file_type.is_symlink() {
        fs::remove_file(path)
            .or_else(|_| fs::remove_dir(path))
            .with_context(|| format!("failed to remove symbolic link {}", path.display()))?;
        return Ok(());
    }

    if metadata.is_dir() {
        fs::remove_dir_all(path).with_context(|| format!("failed to remove {}", path.display()))?;
    } else {
        make_shared_codex_path_writable_for_removal(path, &metadata)?;
        fs::remove_file(path).with_context(|| format!("failed to remove {}", path.display()))?;
    }

    Ok(())
}

pub fn create_codex_home_if_missing(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("failed to create {}", path.display()))?;
    secure_private_codex_directory(path)
}

pub fn ensure_managed_profiles_root(paths: &AppPaths) -> Result<()> {
    let root = &paths.managed_profiles_root;
    match fs::symlink_metadata(root) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                bail!(
                    "managed profile root {} must not be a symbolic link",
                    root.display()
                );
            }
            if !metadata.is_dir() {
                bail!(
                    "managed profile root {} must be a directory",
                    root.display()
                );
            }
        }
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            fs::create_dir_all(root)
                .with_context(|| format!("failed to create {}", root.display()))?;
        }
        Err(err) => {
            return Err(err).with_context(|| format!("failed to inspect {}", root.display()));
        }
    }

    secure_private_codex_directory(root)
}

fn secure_private_codex_directory(_path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        fs::set_permissions(_path, fs::Permissions::from_mode(0o700))
            .with_context(|| format!("failed to secure {}", _path.display()))?;
    }
    Ok(())
}

pub(super) fn dir_is_empty(path: &Path) -> Result<bool> {
    if !path.exists() {
        return Ok(true);
    }
    let mut entries =
        fs::read_dir(path).with_context(|| format!("failed to read {}", path.display()))?;
    Ok(entries.next().is_none())
}

pub(super) fn ensure_shared_codex_directory_public(
    path: &Path,
    metadata: &fs::Metadata,
) -> Result<()> {
    ensure_shared_codex_directory(path, metadata)
}

pub(super) fn ensure_shared_codex_file_public(path: &Path, metadata: &fs::Metadata) -> Result<()> {
    ensure_shared_codex_file(path, metadata)
}

pub(super) fn ensure_shared_codex_path_is_directory_public(path: &Path) -> Result<()> {
    ensure_shared_codex_path_is_directory(path)
}

pub(super) fn ensure_shared_codex_path_is_file_public(path: &Path) -> Result<()> {
    ensure_shared_codex_path_is_file(path)
}

pub(super) fn ensure_shared_codex_target_is_directory_public(path: &Path) -> Result<()> {
    ensure_shared_codex_target_is_directory(path)
}

pub(super) fn ensure_shared_codex_target_is_file_public(path: &Path) -> Result<()> {
    ensure_shared_codex_target_is_file(path)
}

#[cfg(all(test, unix))]
#[test]
fn create_codex_home_repairs_legacy_directory_permissions() {
    use std::os::unix::fs::PermissionsExt as _;

    let path = env::temp_dir().join(format!("prodex-private-home-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o775)).unwrap();

    create_codex_home_if_missing(&path).unwrap();

    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o700
    );
    fs::remove_dir(&path).unwrap();
}

#[cfg(test)]
fn shared_codex_ops_test_dir(name: &str) -> PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = env::temp_dir().join(format!(
        "prodex-shared-ops-{name}-{}-{unique}",
        std::process::id()
    ));
    fs::create_dir_all(&path).unwrap();
    path
}

#[test]
fn atomic_replace_failure_keeps_existing_destination() {
    use std::io::Write as _;

    let root = shared_codex_ops_test_dir("atomic-failure");
    let destination = root.join("state.json");
    fs::write(&destination, b"previous").unwrap();
    let permissions = fs::metadata(&destination).unwrap().permissions();

    let error = replace_shared_codex_file_atomic(
        &destination,
        permissions,
        None,
        "injected write",
        |file| {
            file.write_all(b"partial")?;
            Err(io::Error::other("injected failure"))
        },
    )
    .expect_err("injected write should fail");

    assert!(format!("{error:#}").contains("injected failure"));
    assert_eq!(fs::read(&destination).unwrap(), b"previous");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn copy_failure_keeps_existing_destination() {
    let root = shared_codex_ops_test_dir("copy-failure");
    let source = root.join("source-directory");
    let destination = root.join("destination.txt");
    fs::create_dir(&source).unwrap();
    fs::write(&destination, b"previous").unwrap();

    copy_shared_codex_file_replacing_existing(&source, &destination, "failed to copy")
        .expect_err("directory source should fail");

    assert_eq!(fs::read(&destination).unwrap(), b"previous");
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn atomic_copy_preserves_source_mode() {
    use std::os::unix::fs::PermissionsExt as _;

    let root = shared_codex_ops_test_dir("copy-mode");
    let source = root.join("source.txt");
    let destination = root.join("destination.txt");
    fs::write(&source, b"fresh").unwrap();
    fs::write(&destination, b"previous").unwrap();
    fs::set_permissions(&source, fs::Permissions::from_mode(0o640)).unwrap();
    fs::set_permissions(&destination, fs::Permissions::from_mode(0o400)).unwrap();

    copy_shared_codex_file_replacing_existing(&source, &destination, "failed to copy").unwrap();

    assert_eq!(fs::read(&destination).unwrap(), b"fresh");
    assert_eq!(
        fs::metadata(&destination).unwrap().permissions().mode() & 0o777,
        0o640
    );
    fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
fn private_directory_permission_failures_are_reported() {
    let protected = Path::new("/sys");
    let create_error = create_codex_home_if_missing(protected)
        .expect_err("virtual proc directory mode must not be mutable");
    assert!(format!("{create_error:#}").contains("failed to secure"));

    let paths = AppPaths {
        root: PathBuf::from("/unused"),
        state_file: PathBuf::from("/unused/state.json"),
        managed_profiles_root: protected.to_path_buf(),
        shared_codex_root: PathBuf::from("/unused/shared"),
        legacy_shared_codex_root: PathBuf::from("/unused/legacy"),
    };
    let managed_error = ensure_managed_profiles_root(&paths)
        .expect_err("virtual proc directory mode must not be mutable");
    assert!(format!("{managed_error:#}").contains("failed to secure"));
}
