use super::*;

pub(super) fn digest_key(namespace: &str, sensitive_key: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(namespace.as_bytes());
    hasher.update([0]);
    hasher.update(sensitive_key);
    hex_lower(&hasher.finalize())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn next_fence_token() -> String {
    let sequence = FENCE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let material = format!(
        "{}:{}:{sequence}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    );
    digest_key("refresh-lease-fence", material.as_bytes())
}

pub(super) fn lock_record_matches(
    path: &Path,
    fence_token: &str,
) -> Result<bool, RefreshLeaseError> {
    let Some(opened) = secure_file::open_file(path, FileSecurity::Private)
        .map_err(|error| RefreshLeaseError::io(path, error))?
    else {
        return Ok(false);
    };
    let bytes = opened
        .read_bounded(REFRESH_LEASE_LOCK_MAX_BYTES)
        .map_err(|error| RefreshLeaseError::io(path, error))?;
    let Ok(record) = std::str::from_utf8(&bytes) else {
        return Ok(false);
    };
    let mut lines = record.lines();
    Ok(lines.next() == Some(LOCK_RECORD_MAGIC)
        && record_version(lines.next()) == Some(REFRESH_LEASE_LOCK_RECORD_VERSION)
        && lines
            .next()
            .and_then(|line| line.strip_prefix("fence_token="))
            == Some(fence_token))
}

pub(super) fn create_lock(path: &Path) -> io::Result<(File, String)> {
    let fence_token = next_fence_token();
    let content = format!(
        "{LOCK_RECORD_MAGIC}\nversion={REFRESH_LEASE_LOCK_RECORD_VERSION}\nfence_token={fence_token}\npid={}\ncreated_unix_ms={}\n",
        std::process::id(),
        unix_millis(SystemTime::now())
    );
    let file = secure_file::create_private(path, content.as_bytes())?;
    file.lock()?;
    if let Err(error) = secure_file::verify_private_file(path, &file) {
        let _ = secure_file::delete_private_verified(path, &file);
        return Err(io::Error::new(io::ErrorKind::AlreadyExists, error));
    }
    Ok((file, fence_token))
}

pub(super) fn write_result(
    path: &Path,
    fence_token: &str,
    result_json: &str,
) -> Result<(), RefreshLeaseError> {
    if result_json.len() as u64 > REFRESH_LEASE_RESULT_MAX_BYTES {
        return Err(refresh_result_size_error(path));
    }

    let record = Zeroizing::new(format!(
        "{RESULT_RECORD_MAGIC}\nversion={REFRESH_LEASE_RESULT_RECORD_VERSION}\nfence_token={fence_token}\n\n{result_json}"
    ));
    secure_file::write_private_atomic(path, record.as_bytes())
        .map_err(|error| RefreshLeaseError::io(path, error))
}

fn decode_result_record(mut record: Zeroizing<String>) -> Option<Zeroizing<String>> {
    if !record.starts_with(RESULT_RECORD_MAGIC) {
        return (record.len() as u64 <= REFRESH_LEASE_RESULT_MAX_BYTES).then_some(record);
    }
    let payload_start = record.find("\n\n")? + 2;
    let header = &record[..payload_start - 2];
    let mut lines = header.lines();
    if lines.next() != Some(RESULT_RECORD_MAGIC)
        || record_version(lines.next()) != Some(REFRESH_LEASE_RESULT_RECORD_VERSION)
    {
        return None;
    }
    let fence_token = lines.next()?.strip_prefix("fence_token=")?;
    if lines.next().is_some()
        || !valid_fence_token(fence_token)
        || (record.len() - payload_start) as u64 > REFRESH_LEASE_RESULT_MAX_BYTES
    {
        return None;
    }
    record.drain(..payload_start);
    Some(record)
}

fn valid_fence_token(token: &str) -> bool {
    token.len() == 64
        && token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn record_version(line: Option<&str>) -> Option<u32> {
    line?.strip_prefix("version=")?.parse().ok()
}

pub(super) fn read_fresh_result(
    path: &Path,
    ttl: Duration,
) -> Result<Option<Zeroizing<String>>, RefreshLeaseError> {
    let opened = match secure_file::open_file(path, FileSecurity::Private) {
        Ok(Some(opened)) => opened,
        Ok(None) => return Ok(None),
        Err(error) if unsafe_entry_error(&error) => {
            remove_entry(path)?;
            return Ok(None);
        }
        Err(error) => return Err(RefreshLeaseError::io(path, error)),
    };
    if metadata_is_stale(opened.metadata(), ttl)
        || opened.metadata().len() > REFRESH_LEASE_RECORD_MAX_BYTES
    {
        remove_entry(path)?;
        return Ok(None);
    }
    let mut bytes = Zeroizing::new(
        opened
            .read_bounded(REFRESH_LEASE_RECORD_MAX_BYTES)
            .map_err(|error| RefreshLeaseError::io(path, error))?,
    );
    match String::from_utf8(std::mem::take(&mut *bytes)) {
        Ok(value) => match decode_result_record(Zeroizing::new(value)) {
            Some(value) => Ok(Some(value)),
            None => {
                remove_entry(path)?;
                Ok(None)
            }
        },
        Err(error) => {
            drop(Zeroizing::new(error.into_bytes()));
            remove_entry(path)?;
            Ok(None)
        }
    }
}
