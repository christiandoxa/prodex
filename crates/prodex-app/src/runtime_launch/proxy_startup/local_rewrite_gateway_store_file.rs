use std::path::Path;

use super::local_rewrite_gateway_store_types::RuntimeGatewayVirtualKeyStoreFile;

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
    match std::fs::read(path) {
        Ok(bytes) => serde_json::from_slice::<RuntimeGatewayVirtualKeyStoreFile>(&bytes)
            .map_err(|err| RuntimeGatewayStoreFileLoadError::Invalid(err.to_string())),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            Ok(RuntimeGatewayVirtualKeyStoreFile::default())
        }
        Err(err) => Err(RuntimeGatewayStoreFileLoadError::Io(err.to_string())),
    }
}

pub(super) fn runtime_gateway_virtual_key_store_file_save(
    path: &Path,
    store: &RuntimeGatewayVirtualKeyStoreFile,
) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let payload = serde_json::to_vec_pretty(store).map_err(std::io::Error::other)?;
    let tmp_path = path.with_extension("json.tmp");
    std::fs::write(&tmp_path, payload)?;
    std::fs::rename(tmp_path, path)?;
    Ok(())
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
}
