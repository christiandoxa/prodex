use std::ffi::{OsStr, OsString};
use std::fs::{File, Metadata};
use std::io::{self, Read as _, Write as _};
use std::path::{Component, Path};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};
use zeroize::Zeroizing;

#[cfg(unix)]
#[path = "secure_file/unix.rs"]
mod platform;
#[cfg(windows)]
#[path = "secure_file/windows.rs"]
mod platform;
#[cfg(not(any(unix, windows)))]
#[path = "secure_file/unsupported.rs"]
mod platform;

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FileSecurity {
    Private,
    Projected,
}

pub(crate) struct OpenedFile {
    file: File,
    metadata: Metadata,
}

impl OpenedFile {
    pub(crate) fn metadata(&self) -> &Metadata {
        &self.metadata
    }

    pub(crate) fn read_bounded(self, max_bytes: u64) -> io::Result<Vec<u8>> {
        if self.metadata.len() > max_bytes {
            return Err(size_error(max_bytes));
        }
        let capacity = usize::try_from(self.metadata.len().min(max_bytes)).unwrap_or(0);
        let mut bytes = Zeroizing::new(Vec::with_capacity(capacity));
        self.file
            .take(max_bytes.saturating_add(1))
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > max_bytes {
            return Err(size_error(max_bytes));
        }
        Ok(std::mem::take(&mut *bytes))
    }
}

pub(crate) struct SecureDirectory(platform::Directory);

impl SecureDirectory {
    pub(crate) fn open(path: &Path, create: bool) -> io::Result<Self> {
        platform::Directory::open_path(path, create).map(Self)
    }

    pub(crate) fn open_directory_beneath(&self, relative: &Path) -> io::Result<SecureDirectory> {
        let components = normal_components(relative)?;
        let mut directory = self.0.try_clone()?;
        for component in components {
            directory = directory.open_child(&component, false)?;
        }
        Ok(Self(directory))
    }

    pub(crate) fn read_link_component(&self, name: &OsStr) -> io::Result<OsString> {
        validate_name(name)?;
        self.0.read_link(name)
    }

    pub(crate) fn open_file_beneath(
        &self,
        relative: &Path,
        security: FileSecurity,
    ) -> io::Result<OpenedFile> {
        let mut components = normal_components(relative)?;
        let name = components
            .pop()
            .ok_or_else(|| invalid_input("secret file name cannot be empty"))?;
        let mut directory = self.0.try_clone()?;
        for component in components {
            directory = directory.open_child(&component, false)?;
        }
        let file = directory.open_file(&name, security)?;
        let metadata = file.metadata()?;
        Ok(OpenedFile { file, metadata })
    }
}

pub(crate) fn open_file(path: &Path, security: FileSecurity) -> io::Result<Option<OpenedFile>> {
    let (directory, name) = match open_parent(path, false) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    match directory.0.open_file(&name, security) {
        Ok(file) => {
            let metadata = file.metadata()?;
            Ok(Some(OpenedFile { file, metadata }))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

pub(crate) fn write_private_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let (directory, name) = open_parent(path, true)?;
    let mut last_collision = None;
    for _ in 0..16 {
        let temporary = temporary_name();
        let mut file = match directory.0.create_private_file(&temporary) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                last_collision = Some(error);
                continue;
            }
            Err(error) => return Err(error),
        };
        let result = (|| {
            file.write_all(bytes)?;
            file.sync_all()?;
            directory.0.replace(&temporary, &name, &file)?;
            directory.0.sync()
        })();
        if result.is_err() {
            let _ = directory.0.remove_entry(&temporary);
        }
        return result;
    }
    Err(last_collision.unwrap_or_else(|| {
        io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate private temporary file",
        )
    }))
}

pub(crate) fn create_private(path: &Path, bytes: &[u8]) -> io::Result<File> {
    let (directory, name) = open_parent(path, true)?;
    let mut file = directory.0.create_private_file(&name)?;
    if let Err(error) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        let _ = directory.0.remove_entry(&name);
        return Err(error);
    }
    Ok(file)
}

pub(crate) fn delete_private(path: &Path) -> io::Result<()> {
    let (directory, name) = match open_parent(path, false) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    let file = match directory.0.open_file(&name, FileSecurity::Private) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    directory.0.remove_verified(&name, &file)
}

pub(crate) fn delete_private_verified(path: &Path, file: &File) -> io::Result<()> {
    let (directory, name) = match open_parent(path, false) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    directory.0.remove_verified(&name, file)
}

pub(crate) fn remove_untrusted_entry(path: &Path) -> io::Result<()> {
    let (directory, name) = match open_parent(path, false) {
        Ok(value) => value,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    directory.0.remove_entry(&name)
}

fn open_parent(path: &Path, create: bool) -> io::Result<(SecureDirectory, OsString)> {
    let name = path
        .file_name()
        .filter(|name| !name.is_empty())
        .ok_or_else(|| invalid_input("secret file name cannot be empty"))?
        .to_os_string();
    validate_name(&name)?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    SecureDirectory::open(parent, create).map(|directory| (directory, name))
}

fn normal_components(path: &Path) -> io::Result<Vec<OsString>> {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => {
                validate_name(value)?;
                components.push(value.to_os_string());
            }
            _ => {
                return Err(invalid_input(
                    "secret path must contain only normal relative components",
                ));
            }
        }
    }
    Ok(components)
}

fn validate_name(name: &OsStr) -> io::Result<()> {
    if name.is_empty() || name == "." || name == ".." {
        return Err(invalid_input("invalid secret path component"));
    }
    Ok(())
}

fn temporary_name() -> OsString {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!(
        ".prodex-secret.{}.{}.{}.tmp",
        std::process::id(),
        nanos,
        sequence
    )
    .into()
}

fn size_error(max_bytes: u64) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        format!("secret exceeds safe size limit ({max_bytes} bytes)"),
    )
}

fn invalid_input(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
