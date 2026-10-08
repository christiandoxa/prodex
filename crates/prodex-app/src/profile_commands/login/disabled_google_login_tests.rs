use super::*;

use super::*;

#[test]
fn removed_google_oauth_flags_fail_with_migration_guidance() {
    for flag in ["--with-google", "--google"] {
        let error = match resolve_login_request(None, vec![OsString::from(flag)]) {
            Err(error) => error,
            Ok(_) => panic!("removed Gemini OAuth login must fail"),
        };
        let message = error.to_string();
        assert!(message.contains("unsupported and disabled"));
        assert!(message.contains("Gemini API key"));
        assert!(message.contains("Vertex AI"));
    }
}
