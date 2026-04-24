use super::*;

const RUNTIME_PROXY_COOKIE_MAX_HOSTS: usize = 128;
const RUNTIME_PROXY_COOKIE_MAX_PER_HOST: usize = 32;
const RUNTIME_PROXY_COOKIE_MAX_NAME_BYTES: usize = 128;
const RUNTIME_PROXY_COOKIE_MAX_VALUE_BYTES: usize = 4096;
const RUNTIME_PROXY_COOKIE_MAX_PATH_BYTES: usize = 512;

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct RuntimeProxyCookieKey {
    namespace: String,
    profile_name: String,
    host: String,
}

#[derive(Debug, Clone)]
struct RuntimeProxyCookieEntry {
    name: String,
    value: String,
    path: String,
    secure: bool,
    expires_at: Option<SystemTime>,
    updated_at: SystemTime,
}

#[derive(Debug, Default)]
pub(crate) struct RuntimeProxyCookieJar {
    entries: Mutex<BTreeMap<RuntimeProxyCookieKey, BTreeMap<String, RuntimeProxyCookieEntry>>>,
}

impl RuntimeProxyCookieJar {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    fn capture_reqwest_response_in_namespace(
        &self,
        namespace: &str,
        profile_name: &str,
        response: &reqwest::Response,
    ) {
        let Some(host) = runtime_proxy_cookie_host_from_reqwest_url(response.url()) else {
            return;
        };
        self.capture_set_cookie_headers(
            namespace,
            profile_name,
            &host,
            response.url().path(),
            runtime_proxy_cookie_url_is_secure(response.url().scheme()),
            response
                .headers()
                .get_all(reqwest::header::SET_COOKIE)
                .iter()
                .filter_map(|value| value.to_str().ok()),
        );
    }

    fn capture_tungstenite_response_in_namespace(
        &self,
        namespace: &str,
        profile_name: &str,
        upstream_url: &str,
        headers: &tungstenite::http::HeaderMap,
    ) {
        let Ok(url) = reqwest::Url::parse(upstream_url) else {
            return;
        };
        let Some(host) = runtime_proxy_cookie_host_from_reqwest_url(&url) else {
            return;
        };
        self.capture_set_cookie_headers(
            namespace,
            profile_name,
            &host,
            url.path(),
            runtime_proxy_cookie_url_is_secure(url.scheme()),
            headers
                .get_all(WsHeaderName::from_static("set-cookie"))
                .iter()
                .filter_map(|value| value.to_str().ok()),
        );
    }

    fn capture_set_cookie_headers<'a>(
        &self,
        namespace: &str,
        profile_name: &str,
        host: &str,
        request_path: &str,
        secure_origin: bool,
        set_cookie_headers: impl IntoIterator<Item = &'a str>,
    ) {
        let now = SystemTime::now();
        let key = RuntimeProxyCookieKey {
            namespace: namespace.to_string(),
            profile_name: profile_name.to_string(),
            host: host.to_string(),
        };
        let default_path = runtime_proxy_cookie_default_path(request_path);
        let mut jar = self
            .entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        runtime_proxy_cookie_prune_expired_locked(&mut jar, now);

        for header in set_cookie_headers {
            let Some(change) =
                RuntimeProxyCookieChange::parse(header, &default_path, secure_origin, now)
            else {
                continue;
            };
            let cookies = jar.entry(key.clone()).or_default();
            match change {
                RuntimeProxyCookieChange::Set(entry) => {
                    cookies.insert(entry.name.clone(), entry);
                    runtime_proxy_cookie_prune_host_locked(cookies);
                }
                RuntimeProxyCookieChange::Delete(name) => {
                    cookies.remove(&name);
                }
            }
        }

        jar.retain(|_, cookies| !cookies.is_empty());
        runtime_proxy_cookie_prune_global_locked(&mut jar);
    }

    fn merged_cookie_header_for_reqwest_in_namespace(
        &self,
        namespace: &str,
        profile_name: &str,
        upstream_url: &str,
        request_headers: &[(String, String)],
    ) -> Option<String> {
        let url = reqwest::Url::parse(upstream_url).ok()?;
        let host = runtime_proxy_cookie_host_from_reqwest_url(&url)?;
        self.merged_cookie_header(
            namespace,
            profile_name,
            &host,
            url.path(),
            runtime_proxy_cookie_url_is_secure(url.scheme()),
            request_headers,
        )
    }

    fn merged_cookie_header_for_websocket_in_namespace(
        &self,
        namespace: &str,
        profile_name: &str,
        upstream_url: &str,
        request_headers: &[(String, String)],
    ) -> Option<String> {
        self.merged_cookie_header_for_reqwest_in_namespace(
            namespace,
            profile_name,
            upstream_url,
            request_headers,
        )
    }

    fn merged_cookie_header(
        &self,
        namespace: &str,
        profile_name: &str,
        host: &str,
        path: &str,
        secure_request: bool,
        request_headers: &[(String, String)],
    ) -> Option<String> {
        let mut caller_segments = Vec::new();
        let mut caller_names = BTreeSet::new();
        for (name, value) in request_headers {
            if !name.eq_ignore_ascii_case("cookie") {
                continue;
            }
            for segment in value.split(';') {
                let segment = segment.trim();
                if segment.is_empty() {
                    continue;
                }
                if let Some(cookie_name) = runtime_proxy_cookie_name_from_pair(segment) {
                    caller_names.insert(cookie_name.to_string());
                }
                caller_segments.push(segment.to_string());
            }
        }

        let key = RuntimeProxyCookieKey {
            namespace: namespace.to_string(),
            profile_name: profile_name.to_string(),
            host: host.to_string(),
        };
        let now = SystemTime::now();
        let mut relayed_segments = Vec::new();
        {
            let mut jar = self
                .entries
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            runtime_proxy_cookie_prune_expired_locked(&mut jar, now);
            if let Some(cookies) = jar.get(&key) {
                for entry in cookies.values() {
                    if caller_names.contains(entry.name.as_str()) {
                        continue;
                    }
                    if entry.secure && !secure_request {
                        continue;
                    }
                    if !runtime_proxy_cookie_path_matches(path, &entry.path) {
                        continue;
                    }
                    relayed_segments.push(format!("{}={}", entry.name, entry.value));
                }
            }
        }

        caller_segments.extend(relayed_segments);
        (!caller_segments.is_empty()).then(|| caller_segments.join("; "))
    }
}

enum RuntimeProxyCookieChange {
    Set(RuntimeProxyCookieEntry),
    Delete(String),
}

impl RuntimeProxyCookieChange {
    fn parse(
        header: &str,
        default_path: &str,
        secure_origin: bool,
        now: SystemTime,
    ) -> Option<Self> {
        let mut parts = header.split(';');
        let first = parts.next()?.trim();
        let (name, value) = first.split_once('=')?;
        let name = name.trim();
        let value = value.trim();
        if !runtime_proxy_cookie_name_is_safe(name)
            || !runtime_proxy_cookie_value_is_safe(value)
            || value.len() > RUNTIME_PROXY_COOKIE_MAX_VALUE_BYTES
        {
            return None;
        }

        let mut path = default_path.to_string();
        let mut secure = false;
        let mut expires_at = None;
        let mut delete = false;
        for attr in parts {
            let attr = attr.trim();
            if attr.eq_ignore_ascii_case("secure") {
                secure = true;
                continue;
            }
            let Some((attr_name, attr_value)) = attr.split_once('=') else {
                continue;
            };
            let attr_name = attr_name.trim();
            let attr_value = attr_value.trim();
            if attr_name.eq_ignore_ascii_case("path") {
                if attr_value.starts_with('/')
                    && !attr_value.contains(['\r', '\n'])
                    && attr_value.len() <= RUNTIME_PROXY_COOKIE_MAX_PATH_BYTES
                {
                    path = attr_value.to_string();
                }
            } else if attr_name.eq_ignore_ascii_case("max-age") {
                if let Ok(seconds) = attr_value.parse::<i64>() {
                    if seconds <= 0 {
                        delete = true;
                    } else {
                        expires_at = Some(
                            now.checked_add(Duration::from_secs(seconds as u64))
                                .unwrap_or(now),
                        );
                    }
                }
            } else if attr_name.eq_ignore_ascii_case("expires")
                && let Some(expires) = runtime_proxy_cookie_parse_expires(attr_value)
            {
                if expires <= now {
                    delete = true;
                } else {
                    expires_at = Some(expires);
                }
            }
        }

        if delete {
            return Some(Self::Delete(name.to_string()));
        }

        Some(Self::Set(RuntimeProxyCookieEntry {
            name: name.to_string(),
            value: value.to_string(),
            path,
            secure: secure && secure_origin,
            expires_at,
            updated_at: now,
        }))
    }
}

fn runtime_proxy_cookie_host_from_reqwest_url(url: &reqwest::Url) -> Option<String> {
    url.host_str()
        .map(str::trim)
        .filter(|host| !host.is_empty())
        .map(|host| host.trim_matches('.').to_ascii_lowercase())
}

fn runtime_proxy_cookie_url_is_secure(scheme: &str) -> bool {
    matches!(scheme, "https" | "wss")
}

fn runtime_proxy_cookie_default_path(path: &str) -> String {
    if !path.starts_with('/') {
        return "/".to_string();
    }
    let Some(index) = path.rfind('/') else {
        return "/".to_string();
    };
    if index == 0 {
        return "/".to_string();
    }
    path[..index].to_string()
}

fn runtime_proxy_cookie_name_from_pair(pair: &str) -> Option<&str> {
    let (name, _) = pair.split_once('=')?;
    let name = name.trim();
    runtime_proxy_cookie_name_is_safe(name).then_some(name)
}

fn runtime_proxy_cookie_name_is_safe(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= RUNTIME_PROXY_COOKIE_MAX_NAME_BYTES
        && name
            .bytes()
            .all(|byte| matches!(byte, b'!' | b'#'..=b'\'' | b'*'..=b'+' | b'-'..=b'.' | b'0'..=b'9' | b'A'..=b'Z' | b'^'..=b'z' | b'|' | b'~'))
}

fn runtime_proxy_cookie_value_is_safe(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| matches!(byte, 0x21 | 0x23..=0x2b | 0x2d..=0x3a | 0x3c..=0x5b | 0x5d..=0x7e))
}

fn runtime_proxy_cookie_path_matches(request_path: &str, cookie_path: &str) -> bool {
    if cookie_path == "/" || request_path == cookie_path {
        return true;
    }
    request_path
        .strip_prefix(cookie_path)
        .is_some_and(|suffix| cookie_path.ends_with('/') || suffix.starts_with('/'))
}

fn runtime_proxy_cookie_parse_expires(value: &str) -> Option<SystemTime> {
    chrono::DateTime::parse_from_rfc2822(value)
        .ok()
        .and_then(|timestamp| {
            let seconds = timestamp.timestamp();
            (seconds >= 0).then(|| UNIX_EPOCH + Duration::from_secs(seconds as u64))
        })
}

fn runtime_proxy_cookie_prune_expired_locked(
    jar: &mut BTreeMap<RuntimeProxyCookieKey, BTreeMap<String, RuntimeProxyCookieEntry>>,
    now: SystemTime,
) {
    for cookies in jar.values_mut() {
        cookies.retain(|_, entry| entry.expires_at.is_none_or(|expires| expires > now));
    }
    jar.retain(|_, cookies| !cookies.is_empty());
}

fn runtime_proxy_cookie_prune_host_locked(cookies: &mut BTreeMap<String, RuntimeProxyCookieEntry>) {
    while cookies.len() > RUNTIME_PROXY_COOKIE_MAX_PER_HOST {
        let Some(oldest_name) = cookies
            .values()
            .min_by_key(|entry| entry.updated_at)
            .map(|entry| entry.name.clone())
        else {
            break;
        };
        cookies.remove(&oldest_name);
    }
}

fn runtime_proxy_cookie_prune_global_locked(
    jar: &mut BTreeMap<RuntimeProxyCookieKey, BTreeMap<String, RuntimeProxyCookieEntry>>,
) {
    while jar.len() > RUNTIME_PROXY_COOKIE_MAX_HOSTS {
        let Some(oldest_key) = jar
            .iter()
            .filter_map(|(key, cookies)| {
                cookies
                    .values()
                    .map(|entry| entry.updated_at)
                    .min()
                    .map(|updated_at| (key.clone(), updated_at))
            })
            .min_by_key(|(_, updated_at)| *updated_at)
            .map(|(key, _)| key)
        else {
            break;
        };
        jar.remove(&oldest_key);
    }
}

fn runtime_proxy_cookie_jar() -> &'static RuntimeProxyCookieJar {
    static JAR: OnceLock<RuntimeProxyCookieJar> = OnceLock::new();
    JAR.get_or_init(RuntimeProxyCookieJar::new)
}

fn runtime_proxy_cookie_namespace(shared: &RuntimeRotationProxyShared) -> String {
    shared.log_path.display().to_string()
}

pub(super) fn runtime_proxy_cookie_header_for_reqwest(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    upstream_url: &str,
    request_headers: &[(String, String)],
) -> Option<String> {
    let namespace = runtime_proxy_cookie_namespace(shared);
    runtime_proxy_cookie_jar().merged_cookie_header_for_reqwest_in_namespace(
        &namespace,
        profile_name,
        upstream_url,
        request_headers,
    )
}

pub(super) fn runtime_proxy_capture_reqwest_cookies(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    response: &reqwest::Response,
) {
    let namespace = runtime_proxy_cookie_namespace(shared);
    runtime_proxy_cookie_jar().capture_reqwest_response_in_namespace(
        &namespace,
        profile_name,
        response,
    );
}

pub(super) fn runtime_proxy_cookie_header_for_websocket(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    upstream_url: &str,
    request_headers: &[(String, String)],
) -> Option<String> {
    let namespace = runtime_proxy_cookie_namespace(shared);
    runtime_proxy_cookie_jar().merged_cookie_header_for_websocket_in_namespace(
        &namespace,
        profile_name,
        upstream_url,
        request_headers,
    )
}

pub(super) fn runtime_proxy_capture_websocket_cookies(
    shared: &RuntimeRotationProxyShared,
    profile_name: &str,
    upstream_url: &str,
    headers: &tungstenite::http::HeaderMap,
) {
    let namespace = runtime_proxy_cookie_namespace(shared);
    runtime_proxy_cookie_jar().capture_tungstenite_response_in_namespace(
        &namespace,
        profile_name,
        upstream_url,
        headers,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capture(
        jar: &RuntimeProxyCookieJar,
        profile: &str,
        host: &str,
        path: &str,
        headers: &[&str],
    ) {
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
}
