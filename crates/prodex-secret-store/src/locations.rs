use crate::SecretLocation;
use std::path::{Path, PathBuf};

pub fn auth_json_path(codex_home: impl AsRef<Path>) -> PathBuf {
    codex_home.as_ref().join("auth.json")
}

pub fn auth_json_location(codex_home: impl AsRef<Path>) -> SecretLocation {
    SecretLocation::File(auth_json_path(codex_home))
}
