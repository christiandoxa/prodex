use std::env;
use std::ffi::{OsStr, OsString};

pub(super) fn claude_bin() -> OsString {
    env::var_os("PRODEX_CLAUDE_BIN").unwrap_or_else(|| OsString::from("claude"))
}

pub(super) fn gemini_bin() -> OsString {
    env::var_os("PRODEX_GEMINI_BIN").unwrap_or_else(|| OsString::from("gemini"))
}

pub(super) fn copilot_bin() -> OsString {
    env::var_os("PRODEX_COPILOT_BIN").unwrap_or_else(|| OsString::from("copilot"))
}

pub(super) fn agy_bin() -> OsString {
    env::var_os("PRODEX_AGY_BIN").unwrap_or_else(|| OsString::from("agy"))
}

fn command_exists_on_path(command: &str, paths: Option<&OsStr>) -> bool {
    prodex_core::resolve_binary_path_in_path(&OsString::from(command), paths).is_some()
}

pub(super) fn kiro_bin() -> OsString {
    kiro_bin_from(env::var_os("PRODEX_KIRO_BIN"), env::var_os("PATH"))
}

fn kiro_bin_from(override_path: Option<OsString>, paths: Option<OsString>) -> OsString {
    if let Some(path) = override_path {
        return path;
    }
    if command_exists_on_path("kiro-cli-chat", paths.as_deref()) {
        return OsString::from("kiro-cli-chat");
    }
    if command_exists_on_path("kiro-cli", paths.as_deref()) {
        return OsString::from("kiro-cli");
    }
    OsString::from("kiro-cli-chat")
}

#[cfg(test)]
mod tests {
    use super::kiro_bin_from;
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn kiro_bin_prefers_explicit_override() {
        assert_eq!(
            kiro_bin_from(Some(OsString::from("/tmp/custom-kiro")), None),
            OsString::from("/tmp/custom-kiro")
        );
    }

    #[test]
    fn kiro_bin_falls_back_to_kiro_cli_when_chat_binary_is_missing() {
        let root = env::temp_dir().join(format!(
            "prodex-kiro-bin-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("temp dir should exist");
        fs::write(
            root.join("kiro-cli"),
            b"#!/bin/sh
",
        )
        .expect("fake binary should write");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            fs::set_permissions(root.join("kiro-cli"), fs::Permissions::from_mode(0o755))
                .expect("fake binary should be executable");
        }

        assert_eq!(
            kiro_bin_from(None, Some(root.clone().into_os_string())),
            OsString::from("kiro-cli")
        );
        let _ = fs::remove_dir_all(root);
    }
}
