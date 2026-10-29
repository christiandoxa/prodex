use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;
use tungstenite::http::{HeaderMap, header::SET_COOKIE};

use super::RuntimeProxyCookieJar;

type PinMap = BTreeMap<(String, String), (String, Arc<RuntimeProxyRoutingPin>, Instant)>;
static PINS: OnceLock<Mutex<PinMap>> = OnceLock::new();
const MAX_PINS: usize = 256;
pub(super) const CHATGPT_HOST: &str = "chatgpt.com";

/// Records pins for committed response IDs within one proxy runtime.
pub fn remember_runtime_guardian_pins(
    namespace: &str,
    profile: &str,
    response_ids: &[String],
    pin: Option<&Arc<RuntimeProxyRoutingPin>>,
) {
    if response_ids.is_empty() {
        return;
    }
    let mut pins = PINS
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    for response_id in response_ids.iter().take(MAX_PINS) {
        if response_id.is_empty() || response_id.len() > 512 {
            continue;
        }
        let key = (namespace.to_owned(), response_id.clone());
        if let Some(pin) = pin {
            pins.insert(key, (profile.to_owned(), Arc::clone(pin), Instant::now()));
        } else {
            pins.remove(&key);
        }
    }
    while pins.len() > MAX_PINS {
        if let Some(oldest) = pins
            .iter()
            .min_by_key(|(_, (_, _, at))| *at)
            .map(|(key, _)| key.clone())
        {
            pins.remove(&oldest);
        }
    }
}

/// Finds a pin only within its original runtime and owning profile.
pub fn runtime_guardian_pin(
    namespace: &str,
    profile: &str,
    response_id: &str,
) -> Option<Arc<RuntimeProxyRoutingPin>> {
    let pins = PINS
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let (owner, pin, _) = pins.get(&(namespace.to_owned(), response_id.to_owned()))?;
    (owner == profile).then(|| Arc::clone(pin))
}

/// Routing cookies captured from one upstream WebSocket handshake.
pub struct RuntimeProxyRoutingPin {
    endpoint: reqwest::Url,
    jar: RuntimeProxyCookieJar,
}

impl fmt::Debug for RuntimeProxyRoutingPin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeProxyRoutingPin")
            .finish_non_exhaustive()
    }
}

impl RuntimeProxyRoutingPin {
    /// Captures unambiguous routing cookies, scoped to their secure ChatGPT endpoint.
    pub fn from_handshake(upstream_url: &str, headers: &HeaderMap) -> Option<Self> {
        let endpoint = secure_chatgpt_endpoint(upstream_url)?;
        let host = endpoint.host_str()?;
        let mut selected = HeaderMap::new();
        for name in ["__oailb", "__cflb"] {
            let mut matching = headers.get_all(SET_COOKIE).iter().filter(|header| {
                header
                    .to_str()
                    .ok()
                    .and_then(|value| value.split_once('='))
                    .is_some_and(|(key, _)| key.trim() == name)
            });
            if let Some(header) = matching.next() {
                if header.as_bytes().len() > 8192
                    || matching.next().is_some()
                    || !domain_matches(host, header.to_str().ok()?)
                {
                    return None;
                }
                selected.append(SET_COOKIE, header.clone());
            }
        }
        let jar = RuntimeProxyCookieJar::new();
        jar.capture_tungstenite_response_in_namespace("", "", endpoint.as_str(), &selected);
        jar.merged_cookie_header_for_websocket_in_namespace("", "", endpoint.as_str(), &[])?;
        Some(Self { endpoint, jar })
    }

    /// Returns the still-valid pin only for the original secure origin and path.
    pub fn cookie_header(&self, upstream_url: &str) -> Option<String> {
        let endpoint = secure_chatgpt_endpoint(upstream_url)?;
        if endpoint.origin() != self.endpoint.origin() || endpoint.path() != self.endpoint.path() {
            return None;
        }
        self.jar
            .merged_cookie_header_for_websocket_in_namespace("", "", endpoint.as_str(), &[])
    }

    /// Replaces only routing cookies while preserving unrelated request cookies.
    pub fn merge_cookie_header(
        &self,
        upstream_url: &str,
        existing: Option<&str>,
    ) -> Option<String> {
        let pin = self.cookie_header(upstream_url)?;
        let mut parts = existing
            .into_iter()
            .flat_map(|value| value.split(';'))
            .map(str::trim)
            .filter(|part| {
                !part.is_empty()
                    && !part
                        .split_once('=')
                        .is_some_and(|(name, _)| matches!(name.trim(), "__oailb" | "__cflb"))
            })
            .collect::<Vec<_>>();
        parts.push(&pin);
        Some(parts.join("; "))
    }
}

fn secure_chatgpt_endpoint(value: &str) -> Option<reqwest::Url> {
    let mut url = reqwest::Url::parse(value).ok()?;
    match url.scheme() {
        "wss" => url.set_scheme("https").ok()?,
        "https" => {}
        _ => return None,
    }
    let host = url.host_str()?;
    if !matches!(
        host,
        CHATGPT_HOST | "chat.openai.com" | "chatgpt-staging.com"
    ) && !host.ends_with(".chatgpt.com")
        && !host.ends_with(".chatgpt-staging.com")
    {
        return None;
    }
    Some(url)
}

fn domain_matches(host: &str, cookie: &str) -> bool {
    for attribute in cookie.split(';').skip(1) {
        let Some((key, domain)) = attribute.trim().split_once('=') else {
            continue;
        };
        if key.trim().eq_ignore_ascii_case("domain") {
            let domain = domain.trim().trim_start_matches('.').to_ascii_lowercase();
            if !domain.contains('.')
                || (host != domain
                    && !host
                        .strip_suffix(&domain)
                        .is_some_and(|prefix| prefix.ends_with('.')))
            {
                return false;
            }
        }
    }
    true
}
