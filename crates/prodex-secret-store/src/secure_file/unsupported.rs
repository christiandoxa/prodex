use super::FileSecurity;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io;
use std::path::Path;

pub(super) struct Directory;

impl Directory {
    pub(super) fn open_path(_path: &Path, _create: bool) -> io::Result<Self> {
        Err(unsupported())
    }

    pub(super) fn try_clone(&self) -> io::Result<Self> {
        Err(unsupported())
    }

    pub(super) fn open_child(&self, _name: &OsStr, _create: bool) -> io::Result<Self> {
        Err(unsupported())
    }

    pub(super) fn open_file(&self, _name: &OsStr, _security: FileSecurity) -> io::Result<File> {
        Err(unsupported())
    }

    pub(super) fn create_private_file(&self, _name: &OsStr) -> io::Result<File> {
        Err(unsupported())
    }

    pub(super) fn replace(&self, _from: &OsStr, _to: &OsStr, _file: &File) -> io::Result<()> {
        Err(unsupported())
    }

    pub(super) fn remove_verified(&self, _name: &OsStr, _file: &File) -> io::Result<()> {
        Err(unsupported())
    }

    pub(super) fn verify(&self, _name: &OsStr, _file: &File) -> io::Result<()> {
        Err(unsupported())
    }

    pub(super) fn remove_entry(&self, _name: &OsStr) -> io::Result<()> {
        Err(unsupported())
    }

    pub(super) fn read_link(&self, _name: &OsStr) -> io::Result<OsString> {
        Err(unsupported())
    }

    pub(super) fn sync(&self) -> io::Result<()> {
        Err(unsupported())
    }
}

fn unsupported() -> io::Error {
    io::Error::new(
        io::ErrorKind::Unsupported,
        "secure secret files are supported only on Unix and Windows",
    )
}
