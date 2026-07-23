use super::FileSecurity;
use std::ffi::{OsStr, OsString};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::mem::{MaybeUninit, size_of};
use std::os::windows::ffi::OsStrExt as _;
use std::os::windows::fs::{MetadataExt as _, OpenOptionsExt as _};
use std::os::windows::io::AsRawHandle as _;
use std::path::{Component, Path, PathBuf};
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_SUCCESS, GENERIC_ALL, GENERIC_READ, GENERIC_WRITE, HANDLE, LocalFree,
};
use windows_sys::Win32::Security::Authorization::{
    GetSecurityInfo, SE_FILE_OBJECT, SetSecurityInfo,
};
use windows_sys::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACL, ACL_REVISION, AddAccessAllowedAceEx, CheckTokenMembership,
    DACL_SECURITY_INFORMATION, EqualSid, GetAce, GetLengthSid, GetSecurityDescriptorControl,
    GetTokenInformation, INHERIT_ONLY_ACE, InitializeAcl, IsWellKnownSid,
    OWNER_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, PSID, SE_DACL_PRESENT,
    SE_DACL_PROTECTED, TOKEN_QUERY, TOKEN_USER, TokenUser, WinAnonymousSid,
    WinAuthenticatedUserSid, WinBuiltinAdministratorsSid, WinBuiltinGuestsSid, WinBuiltinUsersSid,
    WinLocalSystemSid, WinWorldSid,
};
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, DELETE, FILE_ADD_FILE, FILE_ADD_SUBDIRECTORY, FILE_ALL_ACCESS,
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_DELETE_CHILD,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_GENERIC_READ,
    FILE_GENERIC_WRITE, FILE_NAME_NORMALIZED, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    GetFileInformationByHandle, GetFinalPathNameByHandleW, MOVEFILE_REPLACE_EXISTING,
    MOVEFILE_WRITE_THROUGH, MoveFileExW, READ_CONTROL, VOLUME_NAME_DOS, WRITE_DAC, WRITE_OWNER,
};
use windows_sys::Win32::System::SystemServices::{ACCESS_ALLOWED_ACE_TYPE, ACCESS_DENIED_ACE_TYPE};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

pub(super) struct Directory {
    path: PathBuf,
    file: File,
    identity: FileIdentity,
}

impl Directory {
    pub(super) fn open_path(path: &Path, create: bool) -> io::Result<Self> {
        let absolute = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()?.join(path)
        };
        let mut current = PathBuf::new();
        let mut opened = None;
        for component in absolute.components() {
            match component {
                Component::Prefix(prefix) => current.push(prefix.as_os_str()),
                Component::RootDir => current.push(component.as_os_str()),
                Component::CurDir => {}
                Component::Normal(name) => {
                    current.push(name);
                    let created = if create && !current.exists() {
                        match fs::create_dir(&current) {
                            Ok(()) => true,
                            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
                            Err(error) => return Err(error),
                        }
                    } else {
                        false
                    };
                    let file = open_directory(&current, created)?;
                    if created {
                        set_private_acl(&file, true)?;
                    }
                    validate_directory(&file)?;
                    opened = Some(Self::from_file(current.clone(), file)?);
                }
                Component::ParentDir => {
                    return Err(invalid_input("parent traversal is not allowed"));
                }
            }
        }
        let directory = match opened {
            Some(directory) => directory,
            None => {
                let file = open_directory(&absolute, false)?;
                validate_directory(&file)?;
                Self::from_file(absolute, file)?
            }
        };
        validate_acl(&directory.file, AclUse::Directory)?;
        Ok(directory)
    }

    fn from_file(path: PathBuf, file: File) -> io::Result<Self> {
        let identity = file_identity(&file)?;
        Ok(Self {
            path,
            file,
            identity,
        })
    }

    pub(super) fn try_clone(&self) -> io::Result<Self> {
        let file = self.file.try_clone()?;
        Ok(Self {
            path: self.path.clone(),
            file,
            identity: self.identity,
        })
    }

    pub(super) fn open_child(&self, name: &OsStr, create: bool) -> io::Result<Self> {
        self.require_path_identity()?;
        let path = self.path.join(name);
        let created = if create && !path.exists() {
            match fs::create_dir(&path) {
                Ok(()) => true,
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => false,
                Err(error) => return Err(error),
            }
        } else {
            false
        };
        let file = open_directory(&path, created)?;
        if created {
            set_private_acl(&file, true)?;
        }
        validate_directory(&file)?;
        require_beneath(&self.file, &file)?;
        validate_acl(&file, AclUse::Directory)?;
        Self::from_file(path, file)
    }

    pub(super) fn ensure_private_child(&self, name: &OsStr) -> io::Result<Self> {
        self.require_path_identity()?;
        let path = self.path.join(name);
        if let Err(error) = fs::create_dir(&path) {
            if error.kind() != io::ErrorKind::AlreadyExists {
                return Err(error);
            }
        }
        let file = open_directory(&path, true)?;
        validate_directory(&file)?;
        require_beneath(&self.file, &file)?;
        set_private_acl(&file, true)?;
        validate_acl(&file, AclUse::Directory)?;
        Self::from_file(path, file)
    }

    pub(super) fn open_file(&self, name: &OsStr, security: FileSecurity) -> io::Result<File> {
        self.require_path_identity()?;
        let path = self.path.join(name);
        let file = open_regular(&path, false)?;
        validate_regular(&file)?;
        require_beneath(&self.file, &file)?;
        validate_acl(
            &file,
            match security {
                FileSecurity::Private => AclUse::PrivateFile,
                FileSecurity::UnsealedPrivate => AclUse::UnsealedPrivateFile,
                FileSecurity::Projected => AclUse::ProjectedFile,
            },
        )?;
        Ok(file)
    }

    pub(super) fn create_private_file(&self, name: &OsStr) -> io::Result<File> {
        self.require_path_identity()?;
        let path = self.path.join(name);
        let created = open_regular(&path, true)?;
        let result = (|| {
            validate_regular(&created)?;
            set_private_acl(&created, false)?;
            validate_acl(&created, AclUse::PrivateFile)?;

            // Publish a share-delete handle only after the ACL is complete. The
            // creation handle prevents another process from deleting the path
            // while it observes the inherited, not-yet-protected ACL.
            let file = open_private_regular(&path)?;
            validate_regular(&file)?;
            if file_identity(&created)? != file_identity(&file)? {
                return Err(permission_denied("secret file changed during creation"));
            }
            Ok(file)
        })();
        drop(created);
        if result.is_err() {
            let _ = fs::remove_file(&path);
        }
        result
    }

    pub(super) fn replace(&self, from: &OsStr, to: &OsStr, file: &File) -> io::Result<()> {
        self.require_path_identity()?;
        let from = wide_path(&self.path.join(from))?;
        let to_path = self.path.join(to);
        let to = wide_path(&to_path)?;
        // SAFETY: both buffers are NUL-terminated Windows paths and remain live
        // for the call. The source and target share one verified parent.
        let result = unsafe {
            MoveFileExW(
                from.as_ptr(),
                to.as_ptr(),
                MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
            )
        };
        if result == 0 {
            return Err(io::Error::last_os_error());
        }
        let replaced = open_regular(&to_path, false)?;
        if file_identity(file)? != file_identity(&replaced)? {
            return Err(permission_denied("secret file changed during replacement"));
        }
        Ok(())
    }

    pub(super) fn remove_verified(&self, name: &OsStr, file: &File) -> io::Result<()> {
        self.require_path_identity()?;
        let path = self.path.join(name);
        let current = match open_regular(&path, false) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        };
        if file_identity(file)? != file_identity(&current)? {
            return Err(permission_denied("secret file changed during removal"));
        }
        fs::remove_file(path)
    }

    pub(super) fn verify(&self, name: &OsStr, file: &File) -> io::Result<()> {
        self.require_path_identity()?;
        let current = open_regular(&self.path.join(name), false)?;
        if file_identity(file)? != file_identity(&current)? {
            return Err(permission_denied("secret file changed during verification"));
        }
        Ok(())
    }

    pub(super) fn remove_entry(&self, name: &OsStr) -> io::Result<()> {
        self.require_path_identity()?;
        match fs::remove_file(self.path.join(name)) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error),
        }
    }

    pub(super) fn read_link(&self, name: &OsStr) -> io::Result<OsString> {
        self.require_path_identity()?;
        let path = self.path.join(name);
        let metadata = fs::symlink_metadata(&path)?;
        if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT == 0 {
            return Err(permission_denied(
                "projected generation marker is not a reparse point",
            ));
        }
        fs::read_link(path).map(PathBuf::into_os_string)
    }

    pub(super) fn sync(&self) -> io::Result<()> {
        // MoveFileExW uses WRITE_THROUGH. Windows directory handles do not
        // consistently support FlushFileBuffers, so no second flush is needed.
        Ok(())
    }

    fn require_path_identity(&self) -> io::Result<()> {
        let current = open_directory(&self.path, false)?;
        if file_identity(&current)? != self.identity {
            return Err(permission_denied("secret parent changed during operation"));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileIdentity {
    volume: u32,
    index_high: u32,
    index_low: u32,
}

#[derive(Clone, Copy)]
enum AclUse {
    Directory,
    PrivateFile,
    UnsealedPrivateFile,
    ProjectedFile,
}

fn open_directory(path: &Path, needs_write_dac: bool) -> io::Result<File> {
    let mut options = OpenOptions::new();
    let mut access = FILE_GENERIC_READ;
    if needs_write_dac {
        access |= WRITE_DAC | WRITE_OWNER;
    }
    options
        .access_mode(access)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
        .open(path)
}

fn open_regular(path: &Path, create: bool) -> io::Result<File> {
    let mut options = OpenOptions::new();
    let access = if create {
        FILE_GENERIC_READ | FILE_GENERIC_WRITE | WRITE_DAC | WRITE_OWNER | READ_CONTROL | DELETE
    } else {
        FILE_GENERIC_READ
    };
    options
        .access_mode(access)
        .share_mode(if create {
            FILE_SHARE_READ | FILE_SHARE_WRITE
        } else {
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE
        })
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    if create {
        options.write(true).create_new(true);
    }
    options.open(path)
}

fn open_private_regular(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(true)
        .access_mode(FILE_GENERIC_READ | FILE_GENERIC_WRITE | WRITE_DAC | WRITE_OWNER)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
}

fn validate_directory(file: &File) -> io::Result<()> {
    let attributes = handle_information(file)?.dwFileAttributes;
    if attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 || attributes & FILE_ATTRIBUTE_DIRECTORY == 0
    {
        return Err(permission_denied(
            "secret parent is not a non-reparse directory",
        ));
    }
    Ok(())
}

fn validate_regular(file: &File) -> io::Result<()> {
    let attributes = handle_information(file)?.dwFileAttributes;
    if attributes & (FILE_ATTRIBUTE_REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY) != 0 {
        return Err(permission_denied(
            "secret is not a non-reparse regular file",
        ));
    }
    Ok(())
}

fn file_identity(file: &File) -> io::Result<FileIdentity> {
    let information = handle_information(file)?;
    Ok(FileIdentity {
        volume: information.dwVolumeSerialNumber,
        index_high: information.nFileIndexHigh,
        index_low: information.nFileIndexLow,
    })
}

fn handle_information(file: &File) -> io::Result<BY_HANDLE_FILE_INFORMATION> {
    let mut information = MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::zeroed();
    // SAFETY: `file` owns a live kernel handle and `information` points to
    // writable storage of the exact structure expected by the API.
    let result = unsafe {
        GetFileInformationByHandle(file.as_raw_handle().cast(), information.as_mut_ptr())
    };
    if result == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: the successful API call initialized the complete structure.
    Ok(unsafe { information.assume_init() })
}

fn require_beneath(parent: &File, child: &File) -> io::Result<()> {
    let mut parent = final_path(parent)?;
    let mut child = final_path(child)?;
    parent.make_ascii_lowercase();
    child.make_ascii_lowercase();
    let boundary = if parent.ends_with('\\') { "" } else { "\\" };
    if child != parent && !child.starts_with(&format!("{parent}{boundary}")) {
        return Err(permission_denied("secret path escaped its trusted parent"));
    }
    Ok(())
}

fn final_path(file: &File) -> io::Result<String> {
    let mut buffer = vec![0u16; 32_768];
    // SAFETY: the file handle is live and `buffer` is writable for its declared
    // length. Flags request a normalized DOS path without following a new name.
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
        return Err(invalid_input("final secret path is too long"));
    }
    buffer.truncate(written);
    String::from_utf16(&buffer).map_err(|_| invalid_input("final secret path is not UTF-16"))
}

fn set_private_acl(file: &File, directory: bool) -> io::Result<()> {
    let user = CurrentUserSid::load()?;
    // SAFETY: `user.sid()` points into `user`'s aligned live buffer.
    let sid_len = unsafe { GetLengthSid(user.sid()) };
    let acl_len = size_of::<ACL>() + size_of::<ACCESS_ALLOWED_ACE>() - size_of::<u32>()
        + usize::try_from(sid_len).unwrap_or(0);
    let mut storage = vec![0usize; acl_len.div_ceil(size_of::<usize>())];
    let acl = storage.as_mut_ptr().cast::<ACL>();
    // SAFETY: `storage` is aligned and large enough for `acl_len`; the user SID
    // remains valid through ACL construction and SetSecurityInfo.
    unsafe {
        if InitializeAcl(
            acl,
            u32::try_from(acl_len).unwrap_or(u32::MAX),
            ACL_REVISION,
        ) == 0
        {
            return Err(io::Error::last_os_error());
        }
        let inheritance = if directory { 0x01 | 0x02 } else { 0 };
        if AddAccessAllowedAceEx(acl, ACL_REVISION, inheritance, FILE_ALL_ACCESS, user.sid()) == 0 {
            return Err(io::Error::last_os_error());
        }
        let status = SetSecurityInfo(
            file.as_raw_handle().cast(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION
                | DACL_SECURITY_INFORMATION
                | PROTECTED_DACL_SECURITY_INFORMATION,
            user.sid(),
            std::ptr::null_mut(),
            acl,
            std::ptr::null_mut(),
        );
        if status != ERROR_SUCCESS {
            return Err(io::Error::from_raw_os_error(status as i32));
        }
    }
    Ok(())
}

fn validate_acl(file: &File, usage: AclUse) -> io::Result<()> {
    let user = CurrentUserSid::load()?;
    let mut owner = std::ptr::null_mut();
    let mut dacl = std::ptr::null_mut();
    let mut descriptor = std::ptr::null_mut();
    // SAFETY: output pointers are valid and the file handle carries READ_CONTROL.
    let status = unsafe {
        GetSecurityInfo(
            file.as_raw_handle().cast(),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            &mut owner,
            std::ptr::null_mut(),
            &mut dacl,
            std::ptr::null_mut(),
            &mut descriptor,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    let descriptor = LocalSecurityDescriptor(descriptor);
    if owner.is_null() || dacl.is_null() {
        return Err(permission_denied(
            "secret object has no private owner or DACL",
        ));
    }
    if matches!(usage, AclUse::PrivateFile) {
        // SAFETY: both SID pointers are live for this descriptor/user buffer.
        if unsafe { EqualSid(owner, user.sid()) } == 0 {
            return Err(permission_denied(
                "private secret is not owned by this user",
            ));
        }
    } else if !principal_is_trusted(owner, user.sid(), usage)? {
        return Err(permission_denied("secret object owner is not trusted"));
    }

    let mut control = 0u16;
    let mut revision = 0u32;
    // SAFETY: the descriptor returned by GetSecurityInfo remains live in guard.
    if unsafe { GetSecurityDescriptorControl(descriptor.0, &mut control, &mut revision) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if control & SE_DACL_PRESENT == 0
        || matches!(usage, AclUse::PrivateFile) && control & SE_DACL_PROTECTED == 0
    {
        return Err(permission_denied("secret object DACL is not private"));
    }

    // SAFETY: `dacl` points into the live descriptor and AceCount bounds GetAce.
    let ace_count = unsafe { (*dacl).AceCount };
    for index in 0..u32::from(ace_count) {
        let mut ace = std::ptr::null_mut();
        // SAFETY: index is below AceCount and `ace` is a valid output pointer.
        if unsafe { GetAce(dacl, index, &mut ace) } == 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: GetAce returned a live ACE_HEADER inside the DACL.
        let header = unsafe { &*ace.cast::<windows_sys::Win32::Security::ACE_HEADER>() };
        if u32::from(header.AceFlags) & INHERIT_ONLY_ACE != 0
            || u32::from(header.AceType) == ACCESS_DENIED_ACE_TYPE
        {
            continue;
        }
        if u32::from(header.AceType) != ACCESS_ALLOWED_ACE_TYPE {
            return Err(permission_denied(
                "secret object has an unsupported allow ACE",
            ));
        }
        // SAFETY: the ACE type was checked before interpreting its fixed prefix.
        let allowed = unsafe { &*ace.cast::<ACCESS_ALLOWED_ACE>() };
        if allowed.Mask & sensitive_access(usage) == 0 {
            continue;
        }
        let sid = std::ptr::addr_of!(allowed.SidStart).cast_mut().cast();
        if !principal_is_trusted(sid, user.sid(), usage)? {
            return Err(permission_denied(
                "secret object grants sensitive access to an untrusted principal",
            ));
        }
    }
    Ok(())
}

fn sensitive_access(usage: AclUse) -> u32 {
    match usage {
        AclUse::Directory => {
            FILE_ADD_FILE
                | FILE_ADD_SUBDIRECTORY
                | FILE_DELETE_CHILD
                | DELETE
                | WRITE_DAC
                | WRITE_OWNER
                | GENERIC_ALL
                | GENERIC_WRITE
        }
        AclUse::PrivateFile | AclUse::UnsealedPrivateFile | AclUse::ProjectedFile => {
            FILE_GENERIC_READ
                | FILE_GENERIC_WRITE
                | DELETE
                | WRITE_DAC
                | WRITE_OWNER
                | GENERIC_ALL
                | GENERIC_READ
                | GENERIC_WRITE
        }
    }
}

fn principal_is_trusted(candidate: PSID, user: PSID, usage: AclUse) -> io::Result<bool> {
    // SAFETY: both SID pointers come from validated Windows security structures.
    if unsafe { EqualSid(candidate, user) } != 0 {
        return Ok(true);
    }
    if is_well_known(candidate, WinLocalSystemSid)
        || is_well_known(candidate, WinBuiltinAdministratorsSid)
    {
        return Ok(true);
    }
    if [
        WinWorldSid,
        WinAuthenticatedUserSid,
        WinBuiltinUsersSid,
        WinAnonymousSid,
        WinBuiltinGuestsSid,
    ]
    .into_iter()
    .any(|kind| is_well_known(candidate, kind))
    {
        return Ok(false);
    }
    if !matches!(usage, AclUse::ProjectedFile) {
        return Ok(false);
    }
    let mut member = 0;
    // SAFETY: a null token requests the effective process token and `candidate`
    // is a live SID. `member` is valid writable output storage.
    if unsafe { CheckTokenMembership(std::ptr::null_mut(), candidate, &mut member) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(member != 0)
}

fn is_well_known(sid: PSID, kind: i32) -> bool {
    // SAFETY: `sid` points into a live security descriptor and `kind` is one of
    // the documented WELL_KNOWN_SID_TYPE constants supplied by the caller.
    unsafe { IsWellKnownSid(sid, kind) != 0 }
}

struct CurrentUserSid {
    storage: Vec<usize>,
}

impl CurrentUserSid {
    fn load() -> io::Result<Self> {
        let mut token = std::ptr::null_mut();
        // SAFETY: GetCurrentProcess returns a valid pseudo-handle and `token` is
        // writable output storage. A successful real token handle is guarded.
        if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let token = OwnedHandle(token);
        let mut required = 0u32;
        // SAFETY: the first call deliberately supplies no buffer to query size.
        unsafe {
            GetTokenInformation(token.0, TokenUser, std::ptr::null_mut(), 0, &mut required);
        }
        if required == 0 {
            return Err(io::Error::last_os_error());
        }
        let words = usize::try_from(required)
            .unwrap_or(usize::MAX)
            .div_ceil(size_of::<usize>());
        let mut storage = vec![0usize; words];
        // SAFETY: `storage` is aligned and contains at least `required` bytes.
        if unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                storage.as_mut_ptr().cast(),
                required,
                &mut required,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(Self { storage })
    }

    fn sid(&self) -> PSID {
        // SAFETY: `load` populated this aligned buffer with a TOKEN_USER whose
        // SID remains owned by the same buffer for `self`'s lifetime.
        unsafe { (*self.storage.as_ptr().cast::<TOKEN_USER>()).User.Sid }
    }
}

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: this guard owns one real process-token handle exactly once.
        unsafe {
            CloseHandle(self.0);
        }
    }
}

struct LocalSecurityDescriptor(windows_sys::Win32::Security::PSECURITY_DESCRIPTOR);

impl Drop for LocalSecurityDescriptor {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: GetSecurityInfo allocated this descriptor with LocalAlloc;
            // the guard releases it exactly once with LocalFree.
            unsafe {
                LocalFree(self.0);
            }
        }
    }
}

fn wide_path(path: &Path) -> io::Result<Vec<u16>> {
    let mut wide: Vec<u16> = path.as_os_str().encode_wide().collect();
    if wide.contains(&0) {
        return Err(invalid_input("secret path contains NUL"));
    }
    wide.push(0);
    Ok(wide)
}

fn invalid_input(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn permission_denied(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::Security::{
        CreateWellKnownSid, SECURITY_MAX_SID_SIZE, WinInteractiveSid,
    };

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
        // SAFETY: the aligned buffer contains `SECURITY_MAX_SID_SIZE` writable
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
        set_test_acl_with_group(&file, user.sid(), group);

        let error = validate_acl(&file, AclUse::PrivateFile).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);

        drop(file);
        fs::remove_dir_all(root).unwrap();
    }

    fn set_test_acl_with_group(file: &File, user: PSID, group: PSID) {
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
        // SAFETY: `storage` is aligned and large enough for both allow ACEs;
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
                AddAccessAllowedAceEx(acl, ACL_REVISION, 0, FILE_GENERIC_READ, group),
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
}
