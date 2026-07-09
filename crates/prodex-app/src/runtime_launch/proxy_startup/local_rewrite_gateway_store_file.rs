use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::Path;

use super::local_rewrite_gateway_store_types::RuntimeGatewayVirtualKeyStoreFile;

const RUNTIME_GATEWAY_STORE_FILE_MAX_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug)]
pub(super) enum RuntimeGatewayStoreFileLoadError {
    Invalid(String),
    Io(String),
}

impl std::fmt::Display for RuntimeGatewayStoreFileLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(message) | Self::Io(message) => f.write_str(message),
        }
    }
}

pub(super) fn runtime_gateway_virtual_key_store_file_load(
    path: &Path,
) -> Result<RuntimeGatewayVirtualKeyStoreFile, RuntimeGatewayStoreFileLoadError> {
    match runtime_gateway_read_regular_file(path) {
        Ok(Some(bytes)) => serde_json::from_slice::<RuntimeGatewayVirtualKeyStoreFile>(&bytes)
            .map_err(|err| RuntimeGatewayStoreFileLoadError::Invalid(err.to_string())),
        Ok(None) => Ok(RuntimeGatewayVirtualKeyStoreFile::default()),
        Err(err) => Err(RuntimeGatewayStoreFileLoadError::Io(err.to_string())),
    }
}

pub(super) fn runtime_gateway_read_regular_file(path: &Path) -> io::Result<Option<Vec<u8>>> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    if metadata.file_type().is_symlink() {
        return Err(io::Error::other(format!(
            "refusing to read gateway state through symlink {}",
            path.display()
        )));
    }
    if !metadata.file_type().is_file() {
        return Err(io::Error::other(format!(
            "gateway state path {} is not a file",
            path.display()
        )));
    }
    if metadata.len() > RUNTIME_GATEWAY_STORE_FILE_MAX_BYTES {
        return Err(io::Error::other(format!(
            "gateway state path {} exceeds safe size limit ({} bytes)",
            path.display(),
            RUNTIME_GATEWAY_STORE_FILE_MAX_BYTES
        )));
    }

    let file = File::open(path)?;
    if !runtime_gateway_same_file_metadata(&metadata, &file.metadata()?) {
        return Err(io::Error::other(format!(
            "gateway state path changed while opening {}",
            path.display()
        )));
    }
    let mut bytes = Vec::new();
    file.take(RUNTIME_GATEWAY_STORE_FILE_MAX_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > RUNTIME_GATEWAY_STORE_FILE_MAX_BYTES {
        return Err(io::Error::other(format!(
            "gateway state path {} exceeds safe size limit ({} bytes)",
            path.display(),
            RUNTIME_GATEWAY_STORE_FILE_MAX_BYTES
        )));
    }
    Ok(Some(bytes))
}

#[cfg(unix)]
fn runtime_gateway_same_file_metadata(left: &std::fs::Metadata, right: &std::fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    left.dev() == right.dev() && left.ino() == right.ino()
}

#[cfg(not(unix))]
fn runtime_gateway_same_file_metadata(
    _left: &std::fs::Metadata,
    _right: &std::fs::Metadata,
) -> bool {
    true
}

pub(super) fn runtime_gateway_virtual_key_store_file_save(
    path: &Path,
    store: &RuntimeGatewayVirtualKeyStoreFile,
) -> std::io::Result<()> {
    let payload = serde_json::to_vec_pretty(store).map_err(std::io::Error::other)?;
    runtime_gateway_write_file_atomic(path, "json.tmp", |file| file.write_all(&payload))
}

pub(super) fn runtime_gateway_write_file_atomic(
    path: &Path,
    tmp_extension: &str,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp_path = path.with_extension(tmp_extension);
    let mut file = open_gateway_temp_file(&tmp_path)?;
    write(&mut file)?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(tmp_path, path)?;
    Ok(())
}

fn open_gateway_temp_file(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(file) => Ok(file),
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
            let _ = std::fs::remove_file(path);
            options.open(path)
        }
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("prodex-gateway-store-file-{name}-{stamp}"))
    }

    #[test]
    fn load_missing_store_as_default_and_reject_invalid_json() {
        let root = temp_dir("load");
        std::fs::create_dir_all(&root).unwrap();

        let missing = runtime_gateway_virtual_key_store_file_load(&root.join("missing.json"))
            .expect("missing store should default");
        assert_eq!(missing.keys.len(), 0);

        let invalid_path = root.join("invalid.json");
        std::fs::write(&invalid_path, "{").unwrap();
        assert!(matches!(
            runtime_gateway_virtual_key_store_file_load(&invalid_path),
            Err(RuntimeGatewayStoreFileLoadError::Invalid(_))
        ));

        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn load_rejects_oversized_store_file() {
        let root = temp_dir("store-large");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("gateway-virtual-keys.json");
        let file = File::create(&path).unwrap();
        file.set_len(RUNTIME_GATEWAY_STORE_FILE_MAX_BYTES + 1)
            .unwrap();

        let err = runtime_gateway_virtual_key_store_file_load(&path)
            .expect_err("oversized gateway store should be rejected");

        assert!(err.to_string().contains("safe size limit"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn save_replaces_stale_temp_symlink_without_touching_target() {
        let root = temp_dir("tmp-symlink");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("gateway-virtual-keys.json");
        let tmp_path = path.with_extension("json.tmp");
        let target = root.join("target.json");
        std::fs::write(&target, "do not touch").unwrap();
        std::os::unix::fs::symlink(&target, &tmp_path).unwrap();

        runtime_gateway_virtual_key_store_file_save(
            &path,
            &RuntimeGatewayVirtualKeyStoreFile::default(),
        )
        .unwrap();

        assert_eq!(std::fs::read_to_string(&target).unwrap(), "do not touch");
        assert!(!std::fs::symlink_metadata(&tmp_path).is_ok());
        assert!(path.is_file());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn load_rejects_store_symlink_without_reading_target() {
        let root = temp_dir("store-symlink");
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("gateway-virtual-keys.json");
        let target = root.join("target.json");
        std::fs::write(&target, r#"{"version":1,"keys":[]}"#).unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();

        let err = runtime_gateway_virtual_key_store_file_load(&path)
            .expect_err("gateway store symlink should be rejected");

        assert!(err.to_string().contains("symlink"));
        std::fs::remove_dir_all(root).unwrap();
    }
}
