use super::same_context_file_version;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, Metadata};
use std::io;
use std::path::{Component, Path};

#[cfg(not(unix))]
use std::fs::OpenOptions;

#[cfg(any(windows, not(any(unix, windows))))]
use std::path::PathBuf;

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt as _;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;

#[cfg(unix)]
use std::os::fd::{AsRawFd as _, FromRawFd as _, RawFd};

#[cfg(unix)]
use std::ffi::CString;

#[cfg(unix)]
pub(super) struct ContextParent {
    file: File,
}

#[cfg(unix)]
impl ContextParent {
    pub(super) fn open_for(path: &Path) -> io::Result<(Self, OsString)> {
        let name = context_file_name(path)?;
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let mut directory = Self {
            file: open_start(parent.is_absolute())?,
        };
        for component in parent.components() {
            directory = match component {
                Component::RootDir | Component::CurDir => directory,
                Component::Normal(name) => Self {
                    file: open_directory_at(directory.file.as_raw_fd(), name)?,
                },
                Component::ParentDir => Self {
                    file: open_directory_at(directory.file.as_raw_fd(), OsStr::new(".."))?,
                },
                Component::Prefix(_) => {
                    return Err(invalid_input("context path has an unsupported prefix"));
                }
            };
        }
        Ok((directory, name))
    }

    pub(super) fn open_existing(&self, name: &OsStr) -> io::Result<File> {
        let name = c_name(name)?;
        let fd = unsafe {
            libc::openat(
                self.file.as_raw_fd(),
                name.as_ptr(),
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
            )
        };
        let file = file_from_fd(fd)?;
        if !file.metadata()?.is_file() {
            return Err(invalid_input("context path is not a regular file"));
        }
        Ok(file)
    }

    pub(super) fn create_new(
        &self,
        name: &OsStr,
        permissions: &fs::Permissions,
    ) -> io::Result<File> {
        let name = c_name(name)?;
        let fd = unsafe {
            libc::openat(
                self.file.as_raw_fd(),
                name.as_ptr(),
                libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_CLOEXEC | libc::O_NOFOLLOW,
                permissions.mode(),
            )
        };
        file_from_fd(fd)
    }

    pub(super) fn entry_exists(&self, name: &OsStr) -> io::Result<bool> {
        match self.open_existing(name) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(_) => Ok(true),
        }
    }

    pub(super) fn remove_if_owned(&self, name: &OsStr, expected: &Metadata) -> io::Result<bool> {
        let current = match self.open_existing(name) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error),
        };
        let metadata = current.metadata()?;
        if !same_context_file_version(expected, &metadata) {
            return Ok(false);
        }
        self.remove_entry(name)?;
        self.sync()?;
        Ok(true)
    }

    pub(super) fn remove_entry(&self, name: &OsStr) -> io::Result<()> {
        let name = c_name(name)?;
        let result = unsafe { libc::unlinkat(self.file.as_raw_fd(), name.as_ptr(), 0) };
        if result == -1 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::NotFound {
                return Ok(());
            }
            return Err(error);
        }
        Ok(())
    }

    pub(super) fn replace(
        &self,
        source: &OsStr,
        temp: &OsStr,
        expected: &Metadata,
        temp_file: &File,
    ) -> io::Result<()> {
        let current = self.open_existing(source)?.metadata()?;
        if !same_context_file_version(expected, &current) {
            return Err(changed_error());
        }

        #[cfg(target_os = "linux")]
        {
            self.replace_linux(source, temp, expected)?;
        }
        #[cfg(not(target_os = "linux"))]
        {
            let source = c_name(source)?;
            let temp = c_name(temp)?;
            let result = unsafe {
                libc::renameat(
                    self.file.as_raw_fd(),
                    temp.as_ptr(),
                    self.file.as_raw_fd(),
                    source.as_ptr(),
                )
            };
            if result == -1 {
                return Err(io::Error::last_os_error());
            }
        }

        let replaced = self.open_existing(source)?.metadata()?;
        if !same_context_file_version(&temp_file.metadata()?, &replaced) {
            return Err(changed_error());
        }
        Ok(())
    }

    pub(super) fn sync(&self) -> io::Result<()> {
        self.file.sync_all()
    }

    #[cfg(target_os = "linux")]
    fn replace_linux(&self, source: &OsStr, temp: &OsStr, expected: &Metadata) -> io::Result<()> {
        let guard = loop {
            let counter = super::CONTEXT_COMPRESS_TEMP_COUNTER
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let candidate = OsString::from(format!(
                ".{}.{}.{}.guard",
                source.to_string_lossy(),
                std::process::id(),
                counter
            ));
            match rename_noreplace(
                self.file.as_raw_fd(),
                source,
                self.file.as_raw_fd(),
                &candidate,
            ) {
                Ok(()) => break candidate,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        };

        let guard_file = self.open_existing(&guard)?;
        let guard_metadata = guard_file.metadata()?;
        let result = if same_context_file_version(expected, &guard_metadata) {
            match rename_noreplace(self.file.as_raw_fd(), temp, self.file.as_raw_fd(), source) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Err(changed_error()),
                Err(error) => Err(error),
            }
        } else {
            Err(changed_error())
        };

        if result.is_ok() {
            let _ = self.remove_if_owned(&guard, &guard_metadata)?;
        } else if !self.entry_exists(source)? {
            let _ = rename_noreplace(self.file.as_raw_fd(), &guard, self.file.as_raw_fd(), source);
        } else {
            let _ = self.remove_if_owned(&guard, &guard_metadata);
        }
        result
    }
}

#[cfg(unix)]
fn open_start(absolute: bool) -> io::Result<File> {
    let path = if absolute { "/" } else { "." };
    let path = CString::new(path).map_err(|_| invalid_input("invalid context root"))?;
    let fd = unsafe {
        libc::open(
            path.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    file_from_fd(fd)
}

#[cfg(unix)]
fn open_directory_at(parent: RawFd, name: &OsStr) -> io::Result<File> {
    let name = c_name(name)?;
    let fd = unsafe {
        libc::openat(
            parent,
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW,
        )
    };
    file_from_fd(fd)
}

#[cfg(unix)]
fn file_from_fd(fd: RawFd) -> io::Result<File> {
    if fd == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

#[cfg(target_os = "linux")]
fn rename_noreplace(
    from_directory: RawFd,
    from: &OsStr,
    to_directory: RawFd,
    to: &OsStr,
) -> io::Result<()> {
    let from = c_name(from)?;
    let to = c_name(to)?;
    let result = unsafe {
        libc::renameat2(
            from_directory,
            from.as_ptr(),
            to_directory,
            to.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    if result == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(unix)]
fn c_name(name: &OsStr) -> io::Result<CString> {
    CString::new(name.as_bytes()).map_err(|_| invalid_input("context path contains NUL"))
}

#[cfg(unix)]
fn context_file_name(path: &Path) -> io::Result<OsString> {
    path.file_name()
        .filter(|name| !name.is_empty())
        .map(OsStr::to_os_string)
        .ok_or_else(|| invalid_input("context file name is empty"))
}

#[cfg(unix)]
fn changed_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "context file changed during compression",
    )
}

#[cfg(windows)]
mod windows_parent {
    use super::*;
    use std::mem::size_of;
    use std::os::windows::ffi::OsStrExt as _;
    use std::os::windows::fs::{MetadataExt as _, OpenOptionsExt as _};
    use std::os::windows::io::AsRawHandle as _;
    use std::ptr;
    use windows_sys::Win32::Storage::FileSystem::{
        DELETE, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_DISPOSITION_INFO,
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_GENERIC_READ,
        FILE_GENERIC_WRITE, FILE_NAME_NORMALIZED, FILE_RENAME_INFO, FILE_SHARE_DELETE,
        FILE_SHARE_READ, FILE_SHARE_WRITE, FileDispositionInfo, FileRenameInfo,
        GetFinalPathNameByHandleW, SetFileInformationByHandle, VOLUME_NAME_DOS,
    };

    pub(super) struct ContextParent {
        file: File,
        path: PathBuf,
    }

    impl ContextParent {
        pub(super) fn open_for(path: &Path) -> io::Result<(Self, OsString)> {
            let name = context_file_name(path)?;
            let parent = path.parent().unwrap_or_else(|| Path::new("."));
            let absolute = if parent.is_absolute() {
                parent.to_path_buf()
            } else {
                std::env::current_dir()?.join(parent)
            };
            let mut current = PathBuf::new();
            let mut directory = None;
            for component in absolute.components() {
                match component {
                    Component::Prefix(prefix) => current.push(prefix.as_os_str()),
                    Component::RootDir => current.push(component.as_os_str()),
                    Component::CurDir => {}
                    Component::Normal(component) => {
                        current.push(component);
                        let file = open_directory(&current)?;
                        validate_directory(&file)?;
                        directory = Some(file);
                    }
                    Component::ParentDir => {
                        return Err(invalid_input("parent traversal is not allowed"));
                    }
                }
            }
            let file = match directory {
                Some(file) => file,
                None => {
                    let file = open_directory(&absolute)?;
                    validate_directory(&file)?;
                    file
                }
            };
            Ok((
                Self {
                    file,
                    path: absolute,
                },
                name,
            ))
        }

        pub(super) fn open_existing(&self, name: &OsStr) -> io::Result<File> {
            let file = open_regular(&self.path.join(name), false)?;
            validate_regular(&file)?;
            require_beneath(&self.file, &file)?;
            Ok(file)
        }

        pub(super) fn create_new(
            &self,
            name: &OsStr,
            _permissions: &fs::Permissions,
        ) -> io::Result<File> {
            let file = open_regular(&self.path.join(name), true)?;
            if let Err(error) =
                validate_regular(&file).and_then(|()| require_beneath(&self.file, &file))
            {
                let _ = delete_opened_file(&file);
                return Err(error);
            }
            Ok(file)
        }

        pub(super) fn entry_exists(&self, name: &OsStr) -> io::Result<bool> {
            match fs::symlink_metadata(self.path.join(name)) {
                Ok(_) => Ok(true),
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
                Err(_) => Ok(true),
            }
        }

        pub(super) fn remove_if_owned(
            &self,
            name: &OsStr,
            expected: &Metadata,
        ) -> io::Result<bool> {
            let file = match open_regular_for_delete(&self.path.join(name)) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
                Err(error) => return Err(error),
            };
            validate_regular(&file)?;
            require_beneath(&self.file, &file)?;
            if !same_context_file_version(expected, &file.metadata()?) {
                return Ok(false);
            }
            delete_opened_file(&file)?;
            self.sync()?;
            Ok(true)
        }

        pub(super) fn remove_entry(&self, name: &OsStr) -> io::Result<()> {
            let file = match open_regular_for_delete(&self.path.join(name)) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(error) => return Err(error),
            };
            validate_regular(&file)?;
            require_beneath(&self.file, &file)?;
            delete_opened_file(&file)
        }

        pub(super) fn replace(
            &self,
            source: &OsStr,
            _temp: &OsStr,
            expected: &Metadata,
            temp_file: &File,
        ) -> io::Result<()> {
            let current = self.open_existing(source)?.metadata()?;
            if !same_context_file_version(expected, &current) {
                return Err(changed_error());
            }
            require_beneath(&self.file, temp_file)?;
            rename_opened_file(&self.file, source, temp_file)?;
            let replaced = self.open_existing(source)?.metadata()?;
            if !same_context_file_version(&temp_file.metadata()?, &replaced) {
                return Err(changed_error());
            }
            Ok(())
        }

        pub(super) fn sync(&self) -> io::Result<()> {
            Ok(())
        }
    }

    fn open_directory(path: &Path) -> io::Result<File> {
        OpenOptions::new()
            .access_mode(FILE_GENERIC_READ)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)
    }

    fn open_regular(path: &Path, create: bool) -> io::Result<File> {
        let mut options = OpenOptions::new();
        options
            .access_mode(if create {
                FILE_GENERIC_READ | FILE_GENERIC_WRITE | DELETE
            } else {
                FILE_GENERIC_READ
            })
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
        if create {
            options.write(true).create_new(true);
        }
        options.open(path)
    }

    fn open_regular_for_delete(path: &Path) -> io::Result<File> {
        OpenOptions::new()
            .access_mode(FILE_GENERIC_READ | DELETE)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)
    }

    fn validate_directory(file: &File) -> io::Result<()> {
        let metadata = file.metadata()?;
        if metadata.file_attributes() & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY)
            != FILE_ATTRIBUTE_DIRECTORY
        {
            return Err(invalid_input(
                "context parent is not a non-reparse directory",
            ));
        }
        Ok(())
    }

    fn validate_regular(file: &File) -> io::Result<()> {
        let metadata = file.metadata()?;
        if metadata.file_attributes() & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY)
            != 0
        {
            return Err(invalid_input(
                "context path is not a non-reparse regular file",
            ));
        }
        Ok(())
    }

    fn require_beneath(parent: &File, child: &File) -> io::Result<()> {
        let mut parent = final_path(parent)?;
        let mut child = final_path(child)?;
        parent.make_ascii_lowercase();
        child.make_ascii_lowercase();
        let boundary = if parent.ends_with('\\') { "" } else { "\\" };
        if child != parent && !child.starts_with(&format!("{parent}{boundary}")) {
            return Err(invalid_input("context path escaped its trusted parent"));
        }
        Ok(())
    }

    fn final_path(file: &File) -> io::Result<String> {
        let mut buffer = vec![0u16; 32_768];
        let written = unsafe {
            GetFinalPathNameByHandleW(
                file.as_raw_handle().cast(),
                buffer.as_mut_ptr(),
                u32::try_from(buffer.len()).unwrap_or(u32::MAX),
                FILE_NAME_NORMALIZED | VOLUME_NAME_DOS,
            )
        };
        if written == 0 {
            return Err(io::Error::last_os_error());
        }
        let written = usize::try_from(written).map_err(|_| invalid_input("invalid final path"))?;
        if written >= buffer.len() {
            return Err(invalid_input("context path is too long"));
        }
        buffer.truncate(written);
        String::from_utf16(&buffer).map_err(|_| invalid_input("context path is not UTF-16"))
    }

    fn rename_opened_file(parent: &File, name: &OsStr, file: &File) -> io::Result<()> {
        let name: Vec<u16> = name.encode_wide().collect();
        if name.is_empty() || name.contains(&0) {
            return Err(invalid_input("context path contains an invalid name"));
        }
        let size = size_of::<FILE_RENAME_INFO>() + (name.len() - 1) * size_of::<u16>();
        let mut storage = vec![0u64; size.div_ceil(size_of::<u64>())];
        let info = storage.as_mut_ptr().cast::<FILE_RENAME_INFO>();
        unsafe {
            (*info).Anonymous.ReplaceIfExists = true;
            (*info).RootDirectory = parent.as_raw_handle().cast();
            (*info).FileNameLength = u32::try_from(name.len() * size_of::<u16>())
                .map_err(|_| invalid_input("context path is too long"))?;
            ptr::copy_nonoverlapping(name.as_ptr(), (*info).FileName.as_mut_ptr(), name.len());
            if SetFileInformationByHandle(
                file.as_raw_handle().cast(),
                FileRenameInfo,
                info.cast(),
                u32::try_from(size).map_err(|_| invalid_input("context path is too long"))?,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }

    fn delete_opened_file(file: &File) -> io::Result<()> {
        let information = FILE_DISPOSITION_INFO { DeleteFile: true };
        unsafe {
            if SetFileInformationByHandle(
                file.as_raw_handle().cast(),
                FileDispositionInfo,
                (&information as *const FILE_DISPOSITION_INFO).cast(),
                u32::try_from(size_of::<FILE_DISPOSITION_INFO>()).unwrap_or(u32::MAX),
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }

    fn context_file_name(path: &Path) -> io::Result<OsString> {
        path.file_name()
            .filter(|name| !name.is_empty())
            .map(OsStr::to_os_string)
            .ok_or_else(|| invalid_input("context file name is empty"))
    }

    fn changed_error() -> io::Error {
        io::Error::new(
            io::ErrorKind::WouldBlock,
            "context file changed during compression",
        )
    }
}

#[cfg(windows)]
pub(super) use windows_parent::ContextParent;

#[cfg(not(any(unix, windows)))]
pub(super) struct ContextParent {
    path: PathBuf,
}

#[cfg(not(any(unix, windows)))]
impl ContextParent {
    pub(super) fn open_for(path: &Path) -> io::Result<(Self, OsString)> {
        Ok((
            Self {
                path: path
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .to_path_buf(),
            },
            context_file_name(path)?,
        ))
    }

    pub(super) fn open_existing(&self, name: &OsStr) -> io::Result<File> {
        let path = self.path.join(name);
        if fs::symlink_metadata(&path)?.file_type().is_symlink() {
            return Err(invalid_input("context file must not be a symlink"));
        }
        let file = OpenOptions::new().read(true).open(path)?;
        if !file.metadata()?.is_file() {
            return Err(invalid_input("context path is not a regular file"));
        }
        Ok(file)
    }

    pub(super) fn create_new(
        &self,
        name: &OsStr,
        _permissions: &fs::Permissions,
    ) -> io::Result<File> {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.path.join(name))
    }

    pub(super) fn entry_exists(&self, name: &OsStr) -> io::Result<bool> {
        match fs::symlink_metadata(self.path.join(name)) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }

    pub(super) fn remove_if_owned(&self, name: &OsStr, expected: &Metadata) -> io::Result<bool> {
        let current = match fs::symlink_metadata(self.path.join(name)) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error),
        };
        if current.file_type().is_symlink() || !same_context_file_version(expected, &current) {
            return Ok(false);
        }
        self.remove_entry(name)?;
        Ok(true)
    }

    pub(super) fn remove_entry(&self, name: &OsStr) -> io::Result<()> {
        match fs::remove_file(self.path.join(name)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    pub(super) fn replace(
        &self,
        source: &OsStr,
        temp: &OsStr,
        expected: &Metadata,
        temp_file: &File,
    ) -> io::Result<()> {
        let current = self.open_existing(source)?.metadata()?;
        if !same_context_file_version(expected, &current) {
            return Err(changed_error());
        }
        fs::rename(self.path.join(temp), self.path.join(source))?;
        let replaced = self.open_existing(source)?.metadata()?;
        if !same_context_file_version(&temp_file.metadata()?, &replaced) {
            return Err(changed_error());
        }
        Ok(())
    }

    pub(super) fn sync(&self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(not(any(unix, windows)))]
fn context_file_name(path: &Path) -> io::Result<OsString> {
    path.file_name()
        .filter(|name| !name.is_empty())
        .map(OsStr::to_os_string)
        .ok_or_else(|| invalid_input("context file name is empty"))
}

#[cfg(not(any(unix, windows)))]
fn changed_error() -> io::Error {
    io::Error::new(
        io::ErrorKind::WouldBlock,
        "context file changed during compression",
    )
}

fn invalid_input(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
