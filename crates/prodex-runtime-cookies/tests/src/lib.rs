use super::*;

fn capture(jar: &RuntimeProxyCookieJar, profile: &str, host: &str, path: &str, headers: &[&str]) {
    jar.capture_set_cookie_headers("", profile, host, path, true, headers.iter().copied());
}

fn merged(
    jar: &RuntimeProxyCookieJar,
    profile: &str,
    url: &str,
    headers: &[(String, String)],
) -> Option<String> {
    jar.merged_cookie_header_for_reqwest_in_namespace("", profile, url, headers)
}

#[test]
fn cookie_jar_replays_profile_and_host_scoped_cookie() {
    let jar = RuntimeProxyCookieJar::new();
    capture(
        &jar,
        "alpha",
        "chatgpt.com",
        "/backend-api/conversation",
        &["cf_clearance=token; Path=/backend-api; Secure; HttpOnly"],
    );

    assert_eq!(
        merged(
            &jar,
            "alpha",
            "https://chatgpt.com/backend-api/responses",
            &[],
        )
        .as_deref(),
        Some("cf_clearance=token")
    );
    assert_eq!(
        merged(
            &jar,
            "beta",
            "https://chatgpt.com/backend-api/responses",
            &[],
        ),
        None
    );
    assert_eq!(
        merged(
            &jar,
            "alpha",
            "https://api.openai.com/backend-api/responses",
            &[],
        ),
        None
    );
}

#[test]
fn cookie_jar_merges_caller_cookie_without_duplicate_name() {
    let jar = RuntimeProxyCookieJar::new();
    capture(
        &jar,
        "alpha",
        "chatgpt.com",
        "/backend-api/responses",
        &["cf_clearance=relayed; Path=/", "__cf_bm=bm; Path=/"],
    );

    assert_eq!(
        merged(
            &jar,
            "alpha",
            "https://chatgpt.com/backend-api/responses",
            &[(
                "Cookie".to_string(),
                "cf_clearance=caller; session=local".to_string(),
            )],
        )
        .as_deref(),
        Some("cf_clearance=caller; session=local; __cf_bm=bm")
    );
}

#[test]
fn cookie_jar_deletes_expired_cookie() {
    let jar = RuntimeProxyCookieJar::new();
    capture(
        &jar,
        "alpha",
        "chatgpt.com",
        "/",
        &["cf_clearance=token; Path=/"],
    );
    capture(
        &jar,
        "alpha",
        "chatgpt.com",
        "/",
        &["cf_clearance=gone; Max-Age=0; Path=/"],
    );

    assert_eq!(
        merged(
            &jar,
            "alpha",
            "https://chatgpt.com/backend-api/responses",
            &[]
        ),
        None
    );
}

#[test]
fn cookie_jar_keeps_same_name_on_distinct_paths_and_deletes_only_one_path() {
    let jar = RuntimeProxyCookieJar::new();
    capture(
        &jar,
        "alpha",
        "chatgpt.com",
        "/api/login",
        &["sid=root; Path=/", "sid=api; Path=/api"],
    );

    assert_eq!(
        merged(&jar, "alpha", "https://chatgpt.com/api/responses", &[]).as_deref(),
        Some("sid=api; sid=root")
    );
    assert_eq!(
        merged(&jar, "alpha", "https://chatgpt.com/other", &[]).as_deref(),
        Some("sid=root")
    );

    capture(
        &jar,
        "alpha",
        "chatgpt.com",
        "/api/login",
        &["sid=gone; Path=/api; Max-Age=0"],
    );
    assert_eq!(
        merged(&jar, "alpha", "https://chatgpt.com/api/responses", &[]).as_deref(),
        Some("sid=root")
    );
}
