use anyhow::{Context, Result, bail};
use sha2::{Digest as _, Sha256};
use std::env;
use std::ffi::{OsStr, OsString};
use std::fmt::Write as _;
use std::fs;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Command;

const CODEX_COMMAND: &str = "codex";
const MINIMUM_CODEX_VERSION: (u64, u64, u64) = (0, 153, 2);
const LEGACY_BUNDLED_CODEX_SHA256: &[&str] = &[
    "31b366ac32f41988056f56ce299d84489b90a18623fa8419dcfd2d39065dd252",
    "61303e421811460cc8f5a86d765c5b2bbc1578ef8ff606103ef22da0fd74de05",
    "7990011a697152a285b8511705572400756febac2fbd5d9d8219a876019ed0b7",
    "e73621dc0acad757636a5ac81d40c727f0a3f055af3ddaab8861aeaf2ba2fa61",
    "f45c814867246310abf3dfdfc1e8f0e95123b096ef3bcd089562694fe05f2497",
    "9f5a4e4fbbd784a18e4a46f977aaeb957510e4f6e1b97ef9a31091151b42df71",
];
const CODEX_INSTALL_GUIDANCE: &str = concat!(
    "Install the official Codex CLI with `npm install -g @openai/codex` ",
    "(https://developers.openai.com/codex/cli), verify `codex --version`, ",
    "or set PRODEX_CODEX_BIN to its executable path."
);

pub(crate) fn codex_bin() -> OsString {
    #[cfg(not(test))]
    {
        static RESOLVED: std::sync::OnceLock<OsString> = std::sync::OnceLock::new();
        RESOLVED.get_or_init(resolve_codex_binary).clone()
    }
    #[cfg(test)]
    resolve_codex_binary()
}

fn resolve_codex_binary() -> OsString {
    if let Some(configured) = env::var_os("PRODEX_CODEX_BIN") {
        return configured;
    }
    resolve_external_codex_from_path(
        env::var_os("PATH").as_deref(),
        env::current_exe().ok().as_deref(),
    )
    .map_or_else(
        || OsString::from(CODEX_COMMAND),
        |path| path.into_os_string(),
    )
}

pub(crate) fn validate_selected_codex_binary(binary: &OsStr) -> Result<()> {
    if binary != codex_bin() {
        return Ok(());
    }
    #[cfg(test)]
    {
        if env::var_os("PRODEX_TEST_VALIDATE_CODEX_BINARY").is_some() {
            return validate_codex_binary(binary);
        }
        Ok(())
    }
    #[cfg(not(test))]
    {
        static VALIDATED: std::sync::OnceLock<std::result::Result<(), String>> =
            std::sync::OnceLock::new();
        VALIDATED
            .get_or_init(|| validate_codex_binary(binary).map_err(|error| format!("{error:#}")))
            .clone()
            .map_err(anyhow::Error::msg)
    }
}

fn validate_codex_binary(binary: &OsStr) -> Result<()> {
    let current_exe = env::current_exe().ok();
    let Some(path) = resolve_requested_codex(binary, env::var_os("PATH").as_deref()) else {
        bail!("Codex CLI is unavailable. {CODEX_INSTALL_GUIDANCE}");
    };
    if is_recursive_prodex_wrapper(&path, current_exe.as_deref()) {
        bail!(
            "Codex executable resolves to a Prodex wrapper: {}. {CODEX_INSTALL_GUIDANCE}",
            path.display()
        );
    }
    if is_legacy_bundled_codex(&path) {
        bail!(
            "Codex executable {} is the legacy Prodex-bundled runtime from 0.426.1. \
             It was left untouched, but Prodex 0.427.0 will not run it. {CODEX_INSTALL_GUIDANCE}",
            path.display()
        );
    }

    let version_output = probe_codex(&path, &["--version"], "Codex version probe")?;
    let version_text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&version_output.stdout),
        String::from_utf8_lossy(&version_output.stderr)
    );
    let version = parse_codex_version(&version_text).with_context(|| {
        format!(
            "Codex CLI at {} did not report a recognizable version. {CODEX_INSTALL_GUIDANCE}",
            path.display()
        )
    })?;
    if version < MINIMUM_CODEX_VERSION {
        bail!(
            "Codex CLI at {} reports {}.{}.{}; Prodex requires 0.153.2 or newer. \
             {CODEX_INSTALL_GUIDANCE}",
            path.display(),
            version.0,
            version.1,
            version.2
        );
    }
    probe_codex(
        &path,
        &["app-server", "--help"],
        "Codex app-server capability probe",
    )?;
    Ok(())
}

fn probe_codex(path: &Path, args: &[&str], label: &str) -> Result<std::process::Output> {
    let mut command = Command::new(path);
    command
        .args(args)
        .env_remove("CODEX_HOME")
        .env_remove("TEST_CODEX_LOG")
        .env_remove("TEST_CODEX_LOG_APPEND")
        .env_remove("TEST_CODEX_ARGS_LOG")
        .env_remove("TEST_CODEX_ARGS_LOG_APPEND")
        .env_remove("TEST_CODEX_STDIN_LOG");
    let output = crate::command_probe_output(&mut command, label)
        .with_context(|| format!("failed to probe Codex CLI at {}", path.display()))?;
    if !output.status.success() {
        bail!(
            "{label} failed for {} with status {}. {CODEX_INSTALL_GUIDANCE}",
            path.display(),
            crate::child_exit_code(&output.status)
        );
    }
    Ok(output)
}

fn parse_codex_version(output: &str) -> Option<(u64, u64, u64)> {
    if !output.to_ascii_lowercase().contains("codex") {
        return None;
    }
    let token = crate::quota_support::parse_codex_cli_version_output(output)?;
    let mut parts = token.split('.');
    let version = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    parts.next().is_none().then_some(version)
}

fn resolve_requested_codex(binary: &OsStr, path_var: Option<&OsStr>) -> Option<PathBuf> {
    let requested = PathBuf::from(binary);
    if requested.components().count() > 1 {
        return executable_path(&requested);
    }
    env::split_paths(path_var?)
        .flat_map(|directory| command_candidates(&directory, binary))
        .find_map(|candidate| executable_path(&candidate))
}

fn resolve_external_codex_from_path(
    path_var: Option<&OsStr>,
    current_exe: Option<&Path>,
) -> Option<PathBuf> {
    env::split_paths(path_var?)
        .flat_map(|directory| command_candidates(&directory, OsStr::new(CODEX_COMMAND)))
        .filter_map(|candidate| executable_path(&candidate))
        .find(|candidate| {
            !is_recursive_prodex_wrapper(candidate, current_exe)
                && !is_legacy_bundled_codex(candidate)
        })
}

fn command_candidates(directory: &Path, command: &OsStr) -> Vec<PathBuf> {
    let base = directory.join(command);
    #[cfg(windows)]
    {
        let mut candidates = vec![base.clone()];
        if base.extension().is_none() {
            for suffix in ["exe", "cmd", "bat", "com"] {
                candidates.push(base.with_extension(suffix));
            }
        }
        candidates
    }
    #[cfg(not(windows))]
    vec![base]
}

fn executable_path(candidate: &Path) -> Option<PathBuf> {
    let metadata = fs::metadata(candidate).ok()?;
    if !metadata.is_file() {
        return None;
    }
    #[cfg(unix)]
    if metadata.permissions().mode() & 0o111 == 0 {
        return None;
    }
    fs::canonicalize(candidate)
        .ok()
        .or_else(|| Some(candidate.to_path_buf()))
}

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt as _;

fn is_recursive_prodex_wrapper(candidate: &Path, current_exe: Option<&Path>) -> bool {
    if let (Ok(candidate), Some(Ok(current))) = (
        fs::canonicalize(candidate),
        current_exe.map(fs::canonicalize),
    ) && candidate == current
    {
        return true;
    }
    if candidate
        .file_name()
        .and_then(OsStr::to_str)
        .is_some_and(|name| {
            name.eq_ignore_ascii_case("prodex") || name.eq_ignore_ascii_case("prodex.exe")
        })
    {
        return true;
    }
    let Ok(file) = fs::File::open(candidate) else {
        return false;
    };
    let mut bytes = Vec::new();
    if file.take(8192).read_to_end(&mut bytes).is_err() || bytes.contains(&0) {
        return false;
    }
    let Ok(text) = String::from_utf8(bytes) else {
        return false;
    };
    let lower = text.to_ascii_lowercase();
    lower.contains("codex-shim")
        || lower.contains("@christiandoxa/prodex")
        || lower.lines().any(|line| {
            let line = line.trim();
            (line.starts_with("exec ") || line.contains("spawn")) && line.contains("prodex")
        })
}

fn is_legacy_bundled_codex(path: &Path) -> bool {
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => hasher.update(&buffer[..read]),
            Err(_) => return false,
        }
    }
    let mut digest = String::with_capacity(64);
    for byte in hasher.finalize() {
        let _ = write!(&mut digest, "{byte:02x}");
    }
    is_legacy_bundled_codex_digest(&digest)
}

fn is_legacy_bundled_codex_digest(digest: &str) -> bool {
    LEGACY_BUNDLED_CODEX_SHA256.contains(&digest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        let root = env::temp_dir().join(format!(
            "prodex-codex-discovery-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn write_executable(path: &Path, contents: &str) {
        fs::write(path, contents).unwrap();
        #[cfg(unix)]
        fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    fn path_resolution_skips_prodex_shims() {
        let root = temp_root();
        let shim_dir = root.join("shim");
        let official_dir = root.join("official");
        fs::create_dir_all(&shim_dir).unwrap();
        fs::create_dir_all(&official_dir).unwrap();
        write_executable(&shim_dir.join("codex"), "#!/bin/sh\nexec prodex \"$@\"\n");
        write_executable(
            &official_dir.join("codex"),
            "#!/bin/sh\nprintf 'codex-cli 0.153.4\\n'\n",
        );
        let path = env::join_paths([shim_dir, official_dir.clone()]).unwrap();

        assert_eq!(
            resolve_external_codex_from_path(Some(&path), None),
            Some(fs::canonicalize(official_dir.join("codex")).unwrap())
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn version_parser_requires_codex_and_minimum_is_ordered() {
        assert_eq!(parse_codex_version("codex-cli 0.153.4"), Some((0, 153, 4)));
        assert_eq!(parse_codex_version("other-cli 0.153.4"), None);
        assert!((0, 153, 1) < MINIMUM_CODEX_VERSION);
        assert!((0, 153, 2) >= MINIMUM_CODEX_VERSION);
        assert!(is_legacy_bundled_codex_digest(
            "9f5a4e4fbbd784a18e4a46f977aaeb957510e4f6e1b97ef9a31091151b42df71"
        ));
        assert!(!is_legacy_bundled_codex_digest(&"0".repeat(64)));
    }

    #[cfg(unix)]
    #[test]
    fn validation_requires_supported_version_and_app_server() {
        let root = temp_root();
        let codex = root.join("codex");
        write_executable(
            &codex,
            "#!/bin/sh\ncase \"$*\" in\n  --version) echo 'codex-cli 0.153.4';;\n  'app-server --help') echo 'Codex app-server';;\n  *) exit 2;;\nesac\n",
        );
        validate_codex_binary(codex.as_os_str()).unwrap();

        write_executable(&codex, "#!/bin/sh\necho 'codex-cli 0.153.1'\n");
        let error = validate_codex_binary(codex.as_os_str()).unwrap_err();
        assert!(error.to_string().contains("requires 0.153.2 or newer"));
        fs::remove_dir_all(root).unwrap();
    }
}
