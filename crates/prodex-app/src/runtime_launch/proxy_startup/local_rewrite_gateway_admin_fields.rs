pub(super) fn runtime_gateway_validate_virtual_key_name(name: &str) -> Result<(), &'static str> {
    let valid = !name.is_empty()
        && name.len() <= 128
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
    if valid {
        Ok(())
    } else {
        Err("gateway virtual key name must use 1-128 ASCII letters, numbers, '.', '-', or '_'")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn virtual_key_name_accepts_only_stable_ascii_names() {
        assert!(runtime_gateway_validate_virtual_key_name("team.alpha-1").is_ok());
        assert!(runtime_gateway_validate_virtual_key_name("").is_err());
        assert!(runtime_gateway_validate_virtual_key_name("bad space").is_err());
        assert!(runtime_gateway_validate_virtual_key_name(" team.alpha-1 ").is_err());
    }
}
