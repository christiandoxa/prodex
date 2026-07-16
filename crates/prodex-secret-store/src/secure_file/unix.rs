use super::FileSecurity;
use std::ffi::{CString, OsStr, OsString};
use std::fs::{File, Metadata};
use std::io;
use std::mem::MaybeUninit;
use std::os::fd::{AsRawFd as _, FromRawFd as _, RawFd};
use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
use std::os::unix::fs::MetadataExt as _;
use std::path::{Component, Path};

pub(super) struct Directory {
    file: File,
}

impl Directory {
    pub(super) fn open_path(path: &Path, create: bool) -> io::Result<Self> {
        let mut directory = if path.is_absolute() {
            Self::open_start(c"/")?
        } else {
            Self::open_start(c".")?
        };

        for component in path.components() {
            match component {
                Component::RootDir | Component::CurDir => {}
                Component::Normal(name) => {
                    directory = directory.open_child(name, create)?;
                }
                Component::ParentDir | Component::Prefix(_) => {
                    return Err(invalid_input("parent traversal is not allowed"));
                }
            }
        }
        validate_directory(&directory.file.metadata()?)?;
        Ok(directory)
    }

    fn open_start(path: &std::ffi::CStr) -> io::Result<Self> {
        // SAFETY: `path` is a valid, NUL-terminated C string and the returned fd
        // is checked before ownership is transferred to `File`.
        let fd = unsafe {
            libc::open(
                path.as_ptr(),
                libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
        file_from_fd(fd).map(|file| Self { file })
    }

    pub(super) fn try_clone(&self) -> io::Result<Self> {
        self.file.try_clone().map(|file| Self { file })
    }

    pub(super) fn open_child(&self, name: &OsStr, create: bool) -> io::Result<Self> {
        let name = c_name(name)?;
        let mut fd = openat_directory(self.file.as_raw_fd(), &name);
        if fd == -1 && create && io::Error::last_os_error().kind() == io::ErrorKind::NotFound {
            // SAFETY: the parent fd is live and `name` is a single NUL-terminated
            // component. `mkdirat` cannot follow a final symlink.
            let result = unsafe { libc::mkdirat(self.file.as_raw_fd(), name.as_ptr(), 0o700) };
            if result == -1 {
                let error = io::Error::last_os_error();
                if error.kind() != io::ErrorKind::AlreadyExists {
                    return Err(error);
                }
            }
            fd = openat_directory(self.file.as_raw_fd(), &name);
        }
        let file = file_from_fd(fd)?;
        validate_directory(&file.metadata()?)?;
        Ok(Self { file })
    }

    pub(super) fn open_file(&self, name: &OsStr, security: FileSecurity) -> io::Result<File> {
        let name = c_name(name)?;
        // SAFETY: the parent fd is live and `name` is a single NUL-terminated
        // component. `O_NOFOLLOW` makes the final symlink check part of open.
        let fd = unsafe {
            libc::openat(
                self.file.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
        let file = file_from_fd(fd)?;
        validate_file(&file.metadata()?, security)?;
        Ok(file)
    }

    pub(super) fn create_private_file(&self, name: &OsStr) -> io::Result<File> {
        let name = c_name(name)?;
        // SAFETY: the parent fd is live and `name` is a single NUL-terminated
        // component. `O_EXCL|O_NOFOLLOW` creates one new private regular file.
        let fd = unsafe {
            libc::openat(
                self.file.as_raw_fd(),
                name.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                0o600,
            )
        };
        let file = file_from_fd(fd)?;
        validate_file(&file.metadata()?, FileSecurity::Private)?;
        Ok(file)
    }

    pub(super) fn replace(&self, from: &OsStr, to: &OsStr, file: &File) -> io::Result<()> {
        let from = c_name(from)?;
        let to = c_name(to)?;
        // SAFETY: both names are single components relative to the same live
        // directory handle, so `renameat` is an atomic in-directory replacement.
        let result = unsafe {
            libc::renameat(
                self.file.as_raw_fd(),
                from.as_ptr(),
                self.file.as_raw_fd(),
                to.as_ptr(),
            )
        };
        if result == -1 {
            return Err(io::Error::last_os_error());
        }
        require_name_identity(self.file.as_raw_fd(), &to, &file.metadata()?)
    }

    pub(super) fn remove_verified(&self, name: &OsStr, file: &File) -> io::Result<()> {
        let name = c_name(name)?;
        require_name_identity(self.file.as_raw_fd(), &name, &file.metadata()?)?;
        unlinkat_file(self.file.as_raw_fd(), &name)
    }

    pub(super) fn verify(&self, name: &OsStr, file: &File) -> io::Result<()> {
        let name = c_name(name)?;
        require_name_identity(self.file.as_raw_fd(), &name, &file.metadata()?)
    }

    pub(super) fn remove_entry(&self, name: &OsStr) -> io::Result<()> {
        let name = c_name(name)?;
        unlinkat_file(self.file.as_raw_fd(), &name)
    }

    pub(super) fn read_link(&self, name: &OsStr) -> io::Result<OsString> {
        let name = c_name(name)?;
        let mut bytes = vec![0u8; 1024];
        // SAFETY: both pointers reference initialized writable storage for the
        // declared length and the parent fd remains live for the call.
        let read = unsafe {
            libc::readlinkat(
                self.file.as_raw_fd(),
                name.as_ptr(),
                bytes.as_mut_ptr().cast(),
                bytes.len(),
            )
        };
        if read == -1 {
            return Err(io::Error::last_os_error());
        }
        let read = usize::try_from(read).map_err(|_| invalid_input("invalid link length"))?;
        if read == bytes.len() {
            return Err(invalid_input("projected generation link is too long"));
        }
        bytes.truncate(read);
        Ok(OsString::from_vec(bytes))
    }

    pub(super) fn sync(&self) -> io::Result<()> {
        self.file.sync_all()
    }
}

fn openat_directory(parent: RawFd, name: &std::ffi::CStr) -> RawFd {
    // SAFETY: `parent` is a live directory fd and `name` is a single valid,
    // NUL-terminated component. The caller validates the returned fd.
    unsafe {
        libc::openat(
            parent,
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    }
}

fn file_from_fd(fd: RawFd) -> io::Result<File> {
    if fd == -1 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ELOOP) {
            return Err(permission_denied("secret path contains a symlink"));
        }
        return Err(error);
    }
    // SAFETY: a nonnegative fd returned by `open`/`openat` is uniquely owned
    // here and ownership is transferred exactly once to `File`.
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn validate_directory(metadata: &Metadata) -> io::Result<()> {
    if !metadata.is_dir() {
        return Err(permission_denied("secret parent is not a directory"));
    }
    let euid = effective_uid();
    let mode = metadata.mode();
    let trusted_owner = metadata.uid() == euid || metadata.uid() == 0;
    #[cfg(target_vendor = "apple")]
    let sticky_mask = u32::from(libc::S_ISVTX);
    #[cfg(not(target_vendor = "apple"))]
    let sticky_mask = libc::S_ISVTX;
    let sticky_root = metadata.uid() == 0 && mode & sticky_mask != 0;
    if !trusted_owner || (mode & 0o022 != 0 && !sticky_root) {
        return Err(permission_denied("secret parent directory is not trusted"));
    }
    Ok(())
}

fn validate_file(metadata: &Metadata, security: FileSecurity) -> io::Result<()> {
    if !metadata.is_file() {
        return Err(permission_denied("secret is not a regular file"));
    }
    let euid = effective_uid();
    match security {
        FileSecurity::Private => {
            if metadata.uid() != euid || metadata.mode() & 0o077 != 0 {
                return Err(permission_denied("secret file is not private to its owner"));
            }
        }
        FileSecurity::Projected => {
            if (metadata.uid() != euid && metadata.uid() != 0)
                || metadata.mode() & 0o037 != 0
                || metadata.mode() & 0o040 != 0 && !process_in_group(metadata.gid())?
            {
                return Err(permission_denied("projected secret file is not private"));
            }
        }
    }
    Ok(())
}

fn require_name_identity(
    parent: RawFd,
    name: &std::ffi::CStr,
    expected: &Metadata,
) -> io::Result<()> {
    let mut stat = MaybeUninit::<libc::stat>::uninit();
    // SAFETY: `stat` points to sufficient writable storage, `parent` is live,
    // and `name` is NUL-terminated. `AT_SYMLINK_NOFOLLOW` checks the named entry.
    let result = unsafe {
        libc::fstatat(
            parent,
            name.as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if result == -1 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful `fstatat` initialized the complete `stat` value.
    let stat = unsafe { stat.assume_init() };
    if stat.st_dev != expected.dev() as libc::dev_t || stat.st_ino != expected.ino() as libc::ino_t
    {
        return Err(permission_denied("secret file changed during operation"));
    }
    Ok(())
}

fn unlinkat_file(parent: RawFd, name: &std::ffi::CStr) -> io::Result<()> {
    // SAFETY: `parent` is a live directory fd and `name` is a single valid,
    // NUL-terminated component. Flags zero remove only a non-directory entry.
    let result = unsafe { libc::unlinkat(parent, name.as_ptr(), 0) };
    if result == -1 {
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::NotFound {
            return Ok(());
        }
        return Err(error);
    }
    Ok(())
}

fn c_name(name: &OsStr) -> io::Result<CString> {
    CString::new(name.as_bytes()).map_err(|_| invalid_input("secret path contains NUL"))
}

fn effective_uid() -> libc::uid_t {
    // SAFETY: `geteuid` has no preconditions and does not access Rust memory.
    unsafe { libc::geteuid() }
}

fn process_in_group(group: libc::gid_t) -> io::Result<bool> {
    // SAFETY: `getegid` has no preconditions and does not access Rust memory.
    if group == unsafe { libc::getegid() } {
        return Ok(true);
    }
    // SAFETY: a zero count with a null pointer requests only the required size.
    let count = unsafe { libc::getgroups(0, std::ptr::null_mut()) };
    if count == -1 {
        return Err(io::Error::last_os_error());
    }
    let mut groups = vec![0; usize::try_from(count).unwrap_or(0)];
    // SAFETY: `groups` has capacity for exactly `count` gid values.
    let actual = unsafe { libc::getgroups(count, groups.as_mut_ptr()) };
    if actual == -1 {
        return Err(io::Error::last_os_error());
    }
    groups.truncate(usize::try_from(actual).unwrap_or(0));
    Ok(groups.contains(&group))
}

fn invalid_input(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn permission_denied(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}
