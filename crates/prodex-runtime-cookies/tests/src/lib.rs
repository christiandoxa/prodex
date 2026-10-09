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
fn cookie_jar_rejects_secure_cookie_from_insecure_origin() {
    let jar = RuntimeProxyCookieJar::new();
    jar.capture_set_cookie_headers(
        "",
        "alpha",
        "chatgpt.com",
        "/backend-api/responses",
        false,
        ["cf_clearance=token; Path=/; Secure"],
    );

    assert_eq!(
        merged(
            &jar,
            "alpha",
            "https://chatgpt.com/backend-api/responses",
            &[],
        ),
        None
    );
}

#[test]
fn cookie_policy_is_case_insensitive_through_mojo() {
    let jar = RuntimeProxyCookieJar::new();
    jar.capture_set_cookie_headers(
        "",
        "alpha",
        "chatgpt.com",
        "/backend-api/responses",
        true,
        [
            "case_cookie=live; pAtH=/backend-api; sEcUrE; eXpIrEs=Tue, 01 Jan 2030 00:00:00 GMT",
            "deleted=gone; mAx-aGe=0; pAtH=/backend-api",
        ],
    );

    assert_eq!(
        merged(
            &jar,
            "alpha",
            "https://chatgpt.com/backend-api/responses",
            &[("cOoKiE".to_string(), "caller=one".to_string())],
        )
        .as_deref(),
        Some("caller=one; case_cookie=live")
    );
}

#[test]
fn cookie_jar_gives_max_age_precedence_over_expires_in_any_order() {
    let jar = RuntimeProxyCookieJar::new();
    capture(
        &jar,
        "alpha",
        "chatgpt.com",
        "/",
        &[
            "max_age_last=gone; Expires=Tue, 01 Jan 2030 00:00:00 GMT; Max-Age=0; Path=/",
            "max_age_first=live; Max-Age=3600; Expires=Thu, 01 Jan 1970 00:00:00 GMT; Path=/",
        ],
    );

    assert_eq!(
        merged(&jar, "alpha", "https://chatgpt.com/", &[]).as_deref(),
        Some("max_age_first=live")
    );
}

#[test]
fn cookie_jar_ignores_invalid_leading_plus_max_age() {
    let jar = RuntimeProxyCookieJar::new();
    capture(
        &jar,
        "alpha",
        "chatgpt.com",
        "/",
        &["plus=live; Max-Age=+0; Expires=Tue, 01 Jan 2030 00:00:00 GMT; Path=/"],
    );

    assert_eq!(
        merged(&jar, "alpha", "https://chatgpt.com/", &[]).as_deref(),
        Some("plus=live")
    );
}

#[test]
fn cookie_jar_treats_pre_epoch_expires_as_expired() {
    let jar = RuntimeProxyCookieJar::new();
    capture(
        &jar,
        "alpha",
        "chatgpt.com",
        "/",
        &["expired=token; Expires=Wed, 31 Dec 1969 23:59:59 GMT; Path=/"],
    );

    assert_eq!(merged(&jar, "alpha", "https://chatgpt.com/", &[]), None);
}

#[test]
fn cookie_jar_uses_last_valid_expires_attribute() {
    let jar = RuntimeProxyCookieJar::new();
    capture(
        &jar,
        "alpha",
        "chatgpt.com",
        "/",
        &[
            "restored=live; Expires=Wed, 31 Dec 1969 23:59:59 GMT; Expires=Tue, 01 Jan 2030 00:00:00 GMT; Path=/",
            "deleted=gone; Expires=Tue, 01 Jan 2030 00:00:00 GMT; Expires=Wed, 31 Dec 1969 23:59:59 GMT; Path=/",
        ],
    );

    assert_eq!(
        merged(&jar, "alpha", "https://chatgpt.com/", &[]).as_deref(),
        Some("restored=live")
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
fn cookie_jar_preserves_multiple_caller_cookie_fields_without_comma_joining() {
    let jar = RuntimeProxyCookieJar::new();
    capture(
        &jar,
        "alpha",
        "chatgpt.com",
        "/backend-api/responses",
        &["relayed=three; Path=/"],
    );

    let header = merged(
        &jar,
        "alpha",
        "https://chatgpt.com/backend-api/responses",
        &[
            ("Cookie".to_string(), "first=one".to_string()),
            ("cookie".to_string(), "second=two".to_string()),
        ],
    )
    .expect("caller and relayed cookies should be merged");

    assert_eq!(header, "first=one; second=two; relayed=three");
    assert!(!header.contains(','));
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

#[test]
fn cookie_jar_debug_redacts_cookie_values() {
    let jar = RuntimeProxyCookieJar::new();
    capture(
        &jar,
        "alpha",
        "chatgpt.com",
        "/",
        &["session=synthetic-cookie-value; Path=/"],
    );

    let debug = format!("{jar:?}");
    assert!(debug.contains("RuntimeProxyCookieJar"), "{debug}");
    assert!(debug.contains("RuntimeProxyCookieEntry"), "{debug}");
    assert!(debug.contains("<redacted>"), "{debug}");
    assert!(!debug.contains("synthetic-cookie-value"), "{debug}");
}

#[test]
fn cookie_jar_prunes_oldest_host_cookie_through_mojo_policy() {
    let jar = RuntimeProxyCookieJar::new();
    let headers = (0..33)
        .map(|index| format!("cookie{index:02}=value; Path=/"))
        .collect::<Vec<_>>();
    jar.capture_set_cookie_headers(
        "",
        "alpha",
        "chatgpt.com",
        "/",
        true,
        headers.iter().map(String::as_str),
    );

    let merged = merged(&jar, "alpha", "https://chatgpt.com/", &[])
        .expect("cookie jar should retain the host after pruning");
    assert!(!merged.contains("cookie00=value"), "{merged}");
    assert!(merged.contains("cookie32=value"), "{merged}");
}

#[test]
fn cookie_jar_prunes_globally_oldest_host_and_keeps_first_tie_break() {
    for tied in [false, true] {
        let mut jar = RuntimeProxyCookieJar::default();
        let entries = jar.entries.get_mut().expect("isolated jar lock");
        for index in 0..=RUNTIME_PROXY_COOKIE_MAX_HOSTS {
            let host = format!("host{index:03}.chatgpt.com");
            let cookie = RuntimeProxyCookieEntry {
                name: "probe".to_string(),
                value: "synthetic".to_string(),
                path: "/".to_string(),
                secure: true,
                expires_at: None,
                updated_at: UNIX_EPOCH + Duration::from_secs(if tied { 1 } else { index as u64 }),
            };
            entries.insert(
                RuntimeProxyCookieKey {
                    namespace: "".to_string(),
                    profile_name: "alpha".to_string(),
                    host,
                },
                BTreeMap::from([(
                    RuntimeProxyCookieIdentity {
                        name: "probe".to_string(),
                        path: "/".to_string(),
                    },
                    cookie,
                )]),
            );
        }
        runtime_proxy_cookie_prune_global_locked(entries);
        assert_eq!(entries.len(), RUNTIME_PROXY_COOKIE_MAX_HOSTS);
        assert!(!entries.keys().any(|key| key.host == "host000.chatgpt.com"));
        assert!(entries.keys().any(
            |key| key.host == format!("host{:03}.chatgpt.com", RUNTIME_PROXY_COOKIE_MAX_HOSTS)
        ));
    }
}
