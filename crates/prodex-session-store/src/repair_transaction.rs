use anyhow::{Context, Result, bail};
use std::ffi::OsString;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

// Separate from shared-fs' maintenance lock: some callers already hold that lock.
const REPAIR_LOCK_FILE: &str = ".prodex-session-repair.lock";
#[cfg(unix)]
const PRIVATE_FILE_MODE: u32 = 0o600;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) struct SessionRepairTransaction {
    path: PathBuf,
    parent: PathBuf,
    source: File,
    source_revision: SourceRevision,
    contents: String,
    _maintenance_lock: File,
}

impl SessionRepairTransaction {
    pub(super) fn begin(repository_root: &Path, path: &Path, max_bytes: u64) -> Result<Self> {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        require_directory(&parent)?;
        let maintenance_lock = acquire_repository_lock(repository_root)?;
        let (source, metadata) = open_regular_file(path, false)?;
        if metadata.len() > max_bytes {
            bail!(
                "session {} exceeds safe size limit ({} bytes)",
                path.display(),
                max_bytes
            );
        }

        let source_revision = SourceRevision::from_metadata(&metadata);
        let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
        (&source)
            .take(max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)
            .with_context(|| format!("failed to read session {}", path.display()))?;
        if bytes.len() as u64 > max_bytes {
            bail!(
                "session {} exceeds safe size limit ({} bytes)",
                path.display(),
                max_bytes
            );
        }
        verify_source(path, &source, &source_revision)?;
        let contents = String::from_utf8(bytes)
            .with_context(|| format!("failed to decode session {}", path.display()))?;

        Ok(Self {
            path: path.to_path_buf(),
            parent,
            source,
            source_revision,
            contents,
            _maintenance_lock: maintenance_lock,
        })
    }

    pub(super) fn contents(&self) -> &str {
        &self.contents
    }

    pub(super) fn commit(self, repaired: &[u8]) -> Result<()> {
        self.commit_inner(repaired, || {})
    }

    #[cfg(test)]
    pub(super) fn commit_after_prepare(
        self,
        repaired: &[u8],
        before_source_verification: impl FnOnce(),
    ) -> Result<()> {
        self.commit_inner(repaired, before_source_verification)
    }

    fn commit_inner(
        self,
        repaired: &[u8],
        before_source_verification: impl FnOnce(),
    ) -> Result<()> {
        verify_source(&self.path, &self.source, &self.source_revision)?;
        let mut backup = ensure_backup(&self.path, self.contents.as_bytes())?;

        let result = (|| {
            let mut temporary = create_temporary_file(&self.path)?;
            temporary.file_mut().write_all(repaired).with_context(|| {
                format!(
                    "failed to write repaired session {}",
                    temporary.path().display()
                )
            })?;
            temporary.file_mut().sync_all().with_context(|| {
                format!(
                    "failed to sync repaired session {}",
                    temporary.path().display()
                )
            })?;
            sync_directory(&self.parent)?;

            before_source_verification();
            verify_source(&self.path, &self.source, &self.source_revision)?;
            temporary.close();
            fs::rename(temporary.path(), &self.path).with_context(|| {
                format!("failed to replace repaired session {}", self.path.display())
            })?;
            temporary.disarm();
            if let Some(backup) = backup.as_mut() {
                backup.disarm();
            }
            temporary.verify_replaced(&self.path)?;
            sync_directory(&self.parent)
        })();

        if result.is_err() {
            drop(backup);
            let _ = sync_directory(&self.parent);
        }
        result
    }
}

pub(super) fn repository_root(path: &Path) -> &Path {
    let fallback = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    path.ancestors()
        .skip(1)
        .find(|ancestor| {
            ancestor
                .file_name()
                .is_some_and(|name| name == "sessions" || name == "archived_sessions")
        })
        .and_then(Path::parent)
        .unwrap_or(fallback)
}

fn acquire_repository_lock(root: &Path) -> Result<File> {
    require_directory(root)?;
    let path = root.join(REPAIR_LOCK_FILE);
    let (file, created) = match CreatedPrivateFile::create(&path) {
        Ok(created) => (created.into_file(), true),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let (file, _) = open_regular_file(&path, true)?;
            set_private_mode(&file, &path)?;
            (file, false)
        }
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to create repair lock {}", path.display()));
        }
    };
    file.lock()
        .with_context(|| format!("failed to lock session repository {}", root.display()))?;
    verify_named_file(&path, &file.metadata()?)?;
    if created {
        file.sync_all()
            .with_context(|| format!("failed to sync repair lock {}", path.display()))?;
        sync_directory(root)?;
    }
    Ok(file)
}

fn ensure_backup(path: &Path, contents: &[u8]) -> Result<Option<CreatedPrivateFile>> {
    let backup_path = backup_path(path);
    match CreatedPrivateFile::create(&backup_path) {
        Ok(mut backup) => {
            backup
                .file_mut()
                .write_all(contents)
                .with_context(|| format!("failed to backup session {}", path.display()))?;
            backup
                .file_mut()
                .sync_all()
                .with_context(|| format!("failed to sync backup {}", backup_path.display()))?;
            sync_directory(
                path.parent()
                    .filter(|parent| !parent.as_os_str().is_empty())
                    .unwrap_or_else(|| Path::new(".")),
            )?;
            Ok(Some(backup))
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            let (backup, _) = open_regular_file(&backup_path, false)?;
            set_private_mode(&backup, &backup_path)?;
            backup
                .sync_all()
                .with_context(|| format!("failed to sync backup {}", backup_path.display()))?;
            verify_named_file(&backup_path, &backup.metadata()?)?;
            Ok(None)
        }
        Err(error) => {
            Err(error).with_context(|| format!("failed to create backup {}", backup_path.display()))
        }
    }
}

fn create_temporary_file(path: &Path) -> Result<CreatedPrivateFile> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .context("session path must have a file name")?;
    let mut last_collision = None;
    for _ in 0..64 {
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let mut temporary_name = OsString::from(".");
        temporary_name.push(name);
        temporary_name.push(format!(
            ".prodex-repair-tmp-{}-{sequence}",
            std::process::id()
        ));
        let temporary_path = parent.join(temporary_name);
        match CreatedPrivateFile::create(&temporary_path) {
            Ok(file) => return Ok(file),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                last_collision = Some(error);
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "failed to create repaired session temporary file for {}",
                        path.display()
                    )
                });
            }
        }
    }
    Err(last_collision.unwrap_or_else(|| {
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate repaired session temporary file",
        )
    }))
    .with_context(|| {
        format!(
            "failed to create repaired session temporary file for {}",
            path.display()
        )
    })
}

fn backup_path(path: &Path) -> PathBuf {
    path.with_extension(format!(
        "{}.prodex-repair-bak",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("session")
    ))
}

fn require_directory(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect directory {}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        bail!(
            "repair directory {} is not a regular directory",
            path.display()
        );
    }
    Ok(())
}

fn open_regular_file(path: &Path, writable: bool) -> Result<(File, Metadata)> {
    let named = fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect file {}", path.display()))?;
    if named.file_type().is_symlink() {
        bail!(
            "refusing to use repair file through symlink {}",
            path.display()
        );
    }
    if !named.is_file() {
        bail!("repair path {} is not a regular file", path.display());
    }
    let mut options = OpenOptions::new();
    options.read(true).write(writable);
    let file = options
        .open(path)
        .with_context(|| format!("failed to open repair file {}", path.display()))?;
    let descriptor = file.metadata()?;
    if !same_file_identity(&named, &descriptor) {
        bail!("repair file changed while opening {}", path.display());
    }
    Ok((file, descriptor))
}

fn verify_source(path: &Path, source: &File, expected: &SourceRevision) -> Result<()> {
    let descriptor = source
        .metadata()
        .with_context(|| format!("failed to inspect open session {}", path.display()))?;
    if SourceRevision::from_metadata(&descriptor) != *expected {
        bail!("session changed during repair: {}", path.display());
    }
    let named = fs::symlink_metadata(path)
        .with_context(|| format!("failed to re-inspect session {}", path.display()))?;
    if named.file_type().is_symlink()
        || !named.is_file()
        || SourceRevision::from_metadata(&named) != *expected
    {
        bail!("session changed during repair: {}", path.display());
    }
    Ok(())
}

fn verify_named_file(path: &Path, expected: &Metadata) -> Result<()> {
    let named = fs::symlink_metadata(path)
        .with_context(|| format!("failed to verify repair file {}", path.display()))?;
    if named.file_type().is_symlink() || !named.is_file() || !same_file_identity(&named, expected) {
        bail!("repair file changed while in use: {}", path.display());
    }
    Ok(())
}

fn set_private_mode(file: &File, path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let mut permissions = file.metadata()?.permissions();
        permissions.set_mode(PRIVATE_FILE_MODE);
        file.set_permissions(permissions)
            .with_context(|| format!("failed to make repair file private {}", path.display()))?;
        if file.metadata()?.mode() & 0o777 != PRIVATE_FILE_MODE {
            bail!("repair file is not private: {}", path.display());
        }
    }
    #[cfg(not(unix))]
    let _ = (file, path);
    Ok(())
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .with_context(|| format!("failed to sync directory {}", path.display()))
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<()> {
    Ok(())
}

struct CreatedPrivateFile {
    path: PathBuf,
    file: Option<File>,
    identity: FileIdentity,
    armed: bool,
}

impl CreatedPrivateFile {
    fn create(path: &Path) -> io::Result<Self> {
        let mut options = OpenOptions::new();
        // `create_new` is atomic and rejects an existing final symlink.
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(PRIVATE_FILE_MODE);
        }
        let file = options.open(path)?;
        let identity = FileIdentity::from_metadata(&file.metadata()?);
        let created = Self {
            path: path.to_path_buf(),
            file: Some(file),
            identity,
            armed: true,
        };
        set_private_mode(created.file(), path).map_err(anyhow_to_io)?;
        verify_named_file(path, &created.file().metadata()?).map_err(anyhow_to_io)?;
        Ok(created)
    }

    fn file(&self) -> &File {
        self.file.as_ref().expect("created file should remain open")
    }

    fn file_mut(&mut self) -> &mut File {
        self.file.as_mut().expect("created file should remain open")
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn close(&mut self) {
        drop(self.file.take());
    }

    fn verify_replaced(&self, destination: &Path) -> Result<()> {
        let metadata = fs::symlink_metadata(destination)
            .with_context(|| format!("failed to verify replacement {}", destination.display()))?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || FileIdentity::from_metadata(&metadata) != self.identity
        {
            bail!(
                "repaired session replacement changed: {}",
                destination.display()
            );
        }
        Ok(())
    }

    fn disarm(&mut self) {
        self.armed = false;
    }

    fn into_file(mut self) -> File {
        self.armed = false;
        self.file.take().expect("created file should remain open")
    }
}

impl Drop for CreatedPrivateFile {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        drop(self.file.take());
        if fs::symlink_metadata(&self.path)
            .ok()
            .is_some_and(|metadata| FileIdentity::from_metadata(&metadata) == self.identity)
        {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn anyhow_to_io(error: anyhow::Error) -> io::Error {
    io::Error::other(error.to_string())
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileIdentity {
    device: u64,
    inode: u64,
}

#[cfg(unix)]
impl FileIdentity {
    fn from_metadata(metadata: &Metadata) -> Self {
        use std::os::unix::fs::MetadataExt;

        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
        }
    }
}

#[cfg(not(unix))]
#[derive(Clone, Debug, PartialEq, Eq)]
struct FileIdentity {
    length: u64,
    modified: Option<std::time::SystemTime>,
    created: Option<std::time::SystemTime>,
}

#[cfg(not(unix))]
impl FileIdentity {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            length: metadata.len(),
            modified: metadata.modified().ok(),
            created: metadata.created().ok(),
        }
    }
}

fn same_file_identity(left: &Metadata, right: &Metadata) -> bool {
    FileIdentity::from_metadata(left) == FileIdentity::from_metadata(right)
}

#[cfg(unix)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SourceRevision {
    identity: FileIdentity,
    length: u64,
    modified_seconds: i64,
    modified_nanoseconds: i64,
    changed_seconds: i64,
    changed_nanoseconds: i64,
    mode: u32,
}

#[cfg(unix)]
impl SourceRevision {
    fn from_metadata(metadata: &Metadata) -> Self {
        use std::os::unix::fs::MetadataExt;

        Self {
            identity: FileIdentity::from_metadata(metadata),
            length: metadata.len(),
            modified_seconds: metadata.mtime(),
            modified_nanoseconds: metadata.mtime_nsec(),
            changed_seconds: metadata.ctime(),
            changed_nanoseconds: metadata.ctime_nsec(),
            mode: metadata.mode(),
        }
    }
}

#[cfg(not(unix))]
#[derive(Clone, Debug, PartialEq, Eq)]
struct SourceRevision {
    identity: FileIdentity,
    length: u64,
    modified: Option<std::time::SystemTime>,
    created: Option<std::time::SystemTime>,
    readonly: bool,
}

#[cfg(not(unix))]
impl SourceRevision {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            identity: FileIdentity::from_metadata(metadata),
            length: metadata.len(),
            modified: metadata.modified().ok(),
            created: metadata.created().ok(),
            readonly: metadata.permissions().readonly(),
        }
    }
}
