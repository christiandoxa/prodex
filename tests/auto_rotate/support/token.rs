use base64::Engine as _;

pub(crate) fn chatgpt_id_token(email: &str) -> String {
    let header =
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(r#"{"alg":"none","typ":"JWT"}"#);
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(format!(r#"{{"email":"{email}"}}"#));
    let signature = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode("sig");
    format!("{header}.{payload}.{signature}")
}
