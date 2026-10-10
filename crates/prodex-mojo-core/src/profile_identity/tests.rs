use super::*;

#[test]
fn profile_identity_kernel_smoke() {
    assert_eq!(
        normalize_email(" User@Example.COM ").unwrap(),
        "user@example.com"
    );
    assert_eq!(normalize_account_id(" acct ").unwrap(), "acct");
    assert_eq!(
        profile_name_from_email(" User+Work@example.com ").unwrap(),
        "user-work_example.com"
    );
    assert_eq!(
        profile_name_candidate("", "fallback", 0).unwrap(),
        "fallback"
    );
    assert_eq!(
        profile_name_candidate("  base  ", "fallback", 1).unwrap(),
        "  base  -2"
    );
    assert_eq!(
        profile_name_candidate("base", "fallback", 10).unwrap(),
        "base-11"
    );
    assert_eq!(
        sanitize_profile_slug("  API_KEY_User@EXAMPLE.com  ").unwrap(),
        "api_key_user_example.com"
    );
    assert_eq!(
        sanitize_profile_slug("雪@EXAMPLE.com").unwrap(),
        "example.com"
    );
    assert_eq!(sanitize_profile_slug("...").unwrap(), "api_key");
    assert_eq!(sanitize_profile_slug(" A/B ").unwrap(), "a-b");
    assert_eq!(
        canonical_profile_identity_key(Some(" acct "), Some(" User@Example.COM ")).unwrap(),
        Some("account:acct|email:user@example.com".to_string())
    );
    assert_eq!(
        validate_profile_name("bad/name").unwrap(),
        ProfileNameValidation::PathSeparator
    );
    assert_eq!(
        add_profile_source_plan(false, false, true).unwrap(),
        AddProfileSourcePlan::CopyCurrent
    );
    assert!(should_activate_profile(false, false).unwrap());
    assert!(trimmed_equal(" GitHub.COM ", "GitHub.COM").unwrap());
    assert!(!trimmed_equal("GitHub.COM", "github.com").unwrap());
    assert!(trimmed_casefold_equal(" User@Example.COM ", "user@example.com").unwrap());
    assert!(!is_trimmed_nonempty(" \u{2003}\u{3000}").unwrap());
    assert!(is_trimmed_nonempty("\u{2003}user@example.com\u{3000}").unwrap());
    assert!(optional_trimmed_casefold_equal(Some(" ACCT "), Some("acct")).unwrap());
    assert!(!optional_trimmed_casefold_equal(Some(""), None).unwrap());
    assert!(optional_trimmed_casefold_wildcard(None, Some("oauth")).unwrap());
    assert!(optional_trimmed_casefold_wildcard(Some(" OAuth "), Some("oauth")).unwrap());
    assert!(optional_nonempty_trimmed_casefold_equal(Some("  "), None).unwrap());
    assert!(!optional_nonempty_trimmed_casefold_equal(Some("arn:a"), None).unwrap());
    assert_eq!(
        first_present_identity_source(&[false, true, true]).unwrap(),
        Some(1)
    );
    assert_eq!(
        first_present_identity_source(&[false, false]).unwrap(),
        None
    );
    assert_eq!(
        removed_active_profile_choice(true, true, true).unwrap(),
        RemovedActiveProfileChoice::FirstRemaining
    );
    assert_eq!(
        removed_active_profile_choice(true, false, true).unwrap(),
        RemovedActiveProfileChoice::Current
    );
}
