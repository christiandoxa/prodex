//! Runtime proxy cookie relay helpers.
//!
//! The live proxy owns transport and profile selection. This crate owns the
//! profile-scoped cookie jar used to replay upstream `Set-Cookie` values on
//! later requests without mixing profiles or runtime instances.

use prodex_mojo_core::rich::ascii_casefold_equal_exact;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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

#[derive(Clone)]
struct RuntimeProxyCookieEntry {
    name: String,
    value: String,
    path: String,
    secure: bool,
    expires_at: Option<SystemTime>,
    updated_at: SystemTime,
}

impl fmt::Debug for RuntimeProxyCookieEntry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeProxyCookieEntry")
            .field("name", &self.name)
            .field("value", &"<redacted>")
            .field("path", &self.path)
            .field("secure", &self.secure)
            .field("expires_at", &self.expires_at)
            .field("updated_at", &self.updated_at)
            .finish()
    }
}

#[derive(Debug, Clone, Eq, Ord, PartialEq, PartialOrd)]
struct RuntimeProxyCookieIdentity {
    name: String,
    path: String,
}

#[derive(Default)]
pub struct RuntimeProxyCookieJar {
    entries: Mutex<
        BTreeMap<
            RuntimeProxyCookieKey,
            BTreeMap<RuntimeProxyCookieIdentity, RuntimeProxyCookieEntry>,
        >,
    >,
}

impl fmt::Debug for RuntimeProxyCookieJar {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let entries = self
            .entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        formatter
            .debug_struct("RuntimeProxyCookieJar")
            .field("entries", &*entries)
            .finish()
    }
}

impl RuntimeProxyCookieJar {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn capture_reqwest_response_in_namespace(
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

    pub fn capture_tungstenite_response_in_namespace(
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
                .get_all(tungstenite::http::header::SET_COOKIE)
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
        let changes = set_cookie_headers
            .into_iter()
            .filter_map(|header| {
                RuntimeProxyCookieChange::parse(header, &default_path, secure_origin, now)
            })
            .collect::<Vec<_>>();
        let mut jar = self
            .entries
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        runtime_proxy_cookie_prune_expired_locked(&mut jar, now);

        for change in changes {
            let cookies = jar.entry(key.clone()).or_default();
            match change {
                RuntimeProxyCookieChange::Set(entry) => {
                    cookies.insert(
                        RuntimeProxyCookieIdentity {
                            name: entry.name.clone(),
                            path: entry.path.clone(),
                        },
                        entry,
                    );
                    runtime_proxy_cookie_prune_host_locked(cookies);
                }
                RuntimeProxyCookieChange::Delete(identity) => {
                    cookies.remove(&identity);
                }
            }
        }

        jar.retain(|_, cookies| !cookies.is_empty());
        runtime_proxy_cookie_prune_global_locked(&mut jar);
    }

    pub fn merged_cookie_header_for_reqwest_in_namespace(
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

    pub fn merged_cookie_header_for_websocket_in_namespace(
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
        let (mut caller_segments, caller_names) =
            runtime_proxy_cookie_caller_segments(request_headers);

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
                let mut matching = cookies
                    .values()
                    .filter(|entry| {
                        !caller_names.contains(entry.name.as_str())
                            && (!entry.secure || secure_request)
                            && runtime_proxy_cookie_path_matches(path, &entry.path)
                    })
                    .collect::<Vec<_>>();
                matching.sort_by(|left, right| {
                    right
                        .path
                        .len()
                        .cmp(&left.path.len())
                        .then_with(|| left.name.cmp(&right.name))
                });
                for entry in matching {
                    relayed_segments.push(format!("{}={}", entry.name, entry.value));
                }
            }
        }

        caller_segments.extend(relayed_segments);
        (!caller_segments.is_empty()).then(|| caller_segments.join("; "))
    }
}

fn runtime_proxy_cookie_caller_segments(
    request_headers: &[(String, String)],
) -> (Vec<String>, BTreeSet<String>) {
    let mut segments = Vec::new();
    let mut names = BTreeSet::new();
    for (_, value) in request_headers.iter().filter(|(name, _)| {
        ascii_casefold_equal_exact(name, "cookie").expect("Mojo Cookie header comparison failed")
    }) {
        for segment in value
            .split(';')
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            if let Some(name) = runtime_proxy_cookie_name_from_pair(segment) {
                names.insert(name.to_string());
            }
            segments.push(segment.to_string());
        }
    }
    (segments, names)
}

enum RuntimeProxyCookieChange {
    Set(RuntimeProxyCookieEntry),
    Delete(RuntimeProxyCookieIdentity),
}

impl RuntimeProxyCookieChange {
    fn parse(
        header: &str,
        default_path: &str,
        secure_origin: bool,
        now: SystemTime,
    ) -> Option<Self> {
        use prodex_mojo_core::runtime_cookie_policy::CookieAttributePlan;

        let mut parts = header.split(';');
        let first = parts.next()?.trim();
        let pair = prodex_mojo_core::runtime_cookie_policy::set_cookie_pair(
            first,
            RUNTIME_PROXY_COOKIE_MAX_NAME_BYTES,
            RUNTIME_PROXY_COOKIE_MAX_VALUE_BYTES,
        )
        .expect("Mojo Set-Cookie pair policy returned invalid output")?;
        let name = &first[pair.name];
        let value = &first[pair.value];

        let mut path = default_path.to_string();
        let mut secure = false;
        let mut expires_at = None;
        let mut delete = false;
        let mut max_age_seen = false;
        for attr in parts {
            let attr = attr.trim();
            match prodex_mojo_core::runtime_cookie_policy::attribute_plan(
                attr,
                RUNTIME_PROXY_COOKIE_MAX_PATH_BYTES,
                max_age_seen,
            )
            .expect("Mojo cookie attribute policy returned invalid output")
            {
                CookieAttributePlan::Ignore => {}
                CookieAttributePlan::Secure => secure = true,
                CookieAttributePlan::Path(range) => path = attr[range].to_string(),
                CookieAttributePlan::MaxAge(seconds) => {
                    max_age_seen = true;
                    delete = seconds <= 0;
                    expires_at = if seconds <= 0 {
                        None
                    } else {
                        Some(
                            now.checked_add(Duration::from_secs(seconds as u64))
                                .unwrap_or(now),
                        )
                    };
                }
                CookieAttributePlan::Expires(range) => {
                    if let Some(expires) = runtime_proxy_cookie_parse_expires(&attr[range]) {
                        delete = expires <= now;
                        expires_at = (!delete).then_some(expires);
                    }
                }
            }
        }

        if secure && !secure_origin {
            return None;
        }

        if delete {
            return Some(Self::Delete(RuntimeProxyCookieIdentity {
                name: name.to_string(),
                path,
            }));
        }

        Some(Self::Set(RuntimeProxyCookieEntry {
            name: name.to_string(),
            value: value.to_string(),
            path,
            secure,
            expires_at,
            updated_at: now,
        }))
    }
}

fn runtime_proxy_cookie_host_from_reqwest_url(url: &reqwest::Url) -> Option<String> {
    url.host_str().and_then(|host| {
        prodex_mojo_core::runtime_cookie_policy::normalize_host(host)
            .expect("Mojo cookie host-normalization policy returned invalid output")
    })
}

fn runtime_proxy_cookie_url_is_secure(scheme: &str) -> bool {
    prodex_mojo_core::runtime_cookie_policy::scheme_is_secure(scheme)
        .expect("Mojo cookie scheme policy returned invalid output")
}

fn runtime_proxy_cookie_default_path(path: &str) -> String {
    use prodex_mojo_core::runtime_cookie_policy::CookieDefaultPathPlan;

    match prodex_mojo_core::runtime_cookie_policy::default_path_plan(path)
        .expect("Mojo cookie default-path policy returned invalid output")
    {
        CookieDefaultPathPlan::Root => "/".to_string(),
        CookieDefaultPathPlan::Prefix(end) => path[..end].to_string(),
    }
}

fn runtime_proxy_cookie_name_from_pair(pair: &str) -> Option<&str> {
    let range = prodex_mojo_core::runtime_cookie_policy::caller_cookie_name(
        pair,
        RUNTIME_PROXY_COOKIE_MAX_NAME_BYTES,
    )
    .expect("Mojo caller-cookie pair policy returned invalid output")?;
    Some(&pair[range])
}

fn runtime_proxy_cookie_path_matches(request_path: &str, cookie_path: &str) -> bool {
    prodex_mojo_core::runtime_cookie_policy::path_matches(request_path, cookie_path)
        .expect("Mojo cookie path-match policy returned invalid output")
}

fn runtime_proxy_cookie_parse_expires(value: &str) -> Option<SystemTime> {
    chrono::DateTime::parse_from_rfc2822(value)
        .ok()
        .and_then(|timestamp| {
            let seconds = timestamp.timestamp();
            if seconds >= 0 {
                UNIX_EPOCH.checked_add(Duration::from_secs(seconds as u64))
            } else {
                UNIX_EPOCH.checked_sub(Duration::from_secs(seconds.unsigned_abs()))
            }
        })
}

fn runtime_proxy_cookie_prune_expired_locked(
    jar: &mut BTreeMap<
        RuntimeProxyCookieKey,
        BTreeMap<RuntimeProxyCookieIdentity, RuntimeProxyCookieEntry>,
    >,
    now: SystemTime,
) {
    for cookies in jar.values_mut() {
        cookies.retain(|_, entry| entry.expires_at.is_none_or(|expires| expires > now));
    }
    jar.retain(|_, cookies| !cookies.is_empty());
}

fn runtime_proxy_cookie_prune_host_locked(
    cookies: &mut BTreeMap<RuntimeProxyCookieIdentity, RuntimeProxyCookieEntry>,
) {
    while cookies.len() > RUNTIME_PROXY_COOKIE_MAX_PER_HOST {
        let candidates = cookies
            .iter()
            .map(|(identity, entry)| (identity.clone(), entry.updated_at))
            .collect::<Vec<_>>();
        let Some(oldest_index) = prodex_mojo_core::runtime_cookie_policy::oldest_timestamp_index(
            &candidates
                .iter()
                .map(|(_, updated_at)| *updated_at)
                .collect::<Vec<_>>(),
        )
        .expect("Mojo cookie eviction policy returned invalid output") else {
            break;
        };
        cookies.remove(&candidates[oldest_index].0);
    }
}

fn runtime_proxy_cookie_prune_global_locked(
    jar: &mut BTreeMap<
        RuntimeProxyCookieKey,
        BTreeMap<RuntimeProxyCookieIdentity, RuntimeProxyCookieEntry>,
    >,
) {
    while jar.len() > RUNTIME_PROXY_COOKIE_MAX_HOSTS {
        let candidates = jar
            .iter()
            .flat_map(|(key, cookies)| {
                cookies
                    .values()
                    .map(move |entry| (key.clone(), entry.updated_at))
            })
            .collect::<Vec<_>>();
        let Some(oldest_index) = prodex_mojo_core::runtime_cookie_policy::oldest_timestamp_index(
            &candidates
                .iter()
                .map(|(_, updated_at)| *updated_at)
                .collect::<Vec<_>>(),
        )
        .expect("Mojo cookie eviction policy returned invalid output") else {
            break;
        };
        jar.remove(&candidates[oldest_index].0);
    }
}

fn runtime_proxy_cookie_jar() -> &'static RuntimeProxyCookieJar {
    static JAR: OnceLock<RuntimeProxyCookieJar> = OnceLock::new();
    JAR.get_or_init(RuntimeProxyCookieJar::new)
}

pub fn runtime_proxy_cookie_header_for_reqwest_namespace(
    namespace: &str,
    profile_name: &str,
    upstream_url: &str,
    request_headers: &[(String, String)],
) -> Option<String> {
    runtime_proxy_cookie_jar().merged_cookie_header_for_reqwest_in_namespace(
        namespace,
        profile_name,
        upstream_url,
        request_headers,
    )
}

pub fn runtime_proxy_capture_reqwest_cookies_namespace(
    namespace: &str,
    profile_name: &str,
    response: &reqwest::Response,
) {
    runtime_proxy_cookie_jar().capture_reqwest_response_in_namespace(
        namespace,
        profile_name,
        response,
    );
}

pub fn runtime_proxy_cookie_header_for_websocket_namespace(
    namespace: &str,
    profile_name: &str,
    upstream_url: &str,
    request_headers: &[(String, String)],
) -> Option<String> {
    runtime_proxy_cookie_jar().merged_cookie_header_for_websocket_in_namespace(
        namespace,
        profile_name,
        upstream_url,
        request_headers,
    )
}

pub fn runtime_proxy_capture_websocket_cookies_namespace(
    namespace: &str,
    profile_name: &str,
    upstream_url: &str,
    headers: &tungstenite::http::HeaderMap,
) {
    runtime_proxy_cookie_jar().capture_tungstenite_response_in_namespace(
        namespace,
        profile_name,
        upstream_url,
        headers,
    );
}

#[cfg(test)]
#[path = "../tests/src/lib.rs"]
mod tests;
