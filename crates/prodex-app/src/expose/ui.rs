pub(super) struct ExposeHttpResponse {
    pub(super) status: u16,
    pub(super) headers: Vec<(String, String)>,
    pub(super) body: Vec<u8>,
}

pub(super) fn expose_text_response(status: u16, body: &str) -> ExposeHttpResponse {
    expose_response(status, body, "text/plain; charset=utf-8", None)
}

pub(super) fn expose_json_response(
    status: u16,
    body: &str,
    cookie: Option<&str>,
) -> ExposeHttpResponse {
    expose_response(status, body, "application/json; charset=utf-8", cookie)
}

pub(super) fn expose_html_response() -> ExposeHttpResponse {
    expose_response(200, expose_html(), "text/html; charset=utf-8", None)
}

pub(super) fn expose_js_response() -> ExposeHttpResponse {
    expose_response(
        200,
        EXPOSE_APP_JS,
        "application/javascript; charset=utf-8",
        None,
    )
}

fn expose_response(
    status: u16,
    body: impl Into<String>,
    content_type: &'static str,
    cookie: Option<&str>,
) -> ExposeHttpResponse {
    let mut headers = expose_security_headers(content_type);
    if let Some(cookie) = cookie {
        headers.push(("Set-Cookie".to_string(), cookie.to_string()));
    }
    ExposeHttpResponse {
        status,
        headers,
        body: body.into().into_bytes(),
    }
}

fn expose_security_headers(content_type: &'static str) -> Vec<(String, String)> {
    vec![
        ("Content-Type".to_string(), content_type.to_string()),
        ("Cache-Control".to_string(), "no-store".to_string()),
        (
            "X-Content-Type-Options".to_string(),
            "nosniff".to_string(),
        ),
        ("X-Frame-Options".to_string(), "DENY".to_string()),
        ("Referrer-Policy".to_string(), "no-referrer".to_string()),
        (
            "Content-Security-Policy".to_string(),
            "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'".to_string(),
        ),
    ]
}

fn expose_html() -> &'static str {
    r##"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Prodex Expose</title>
<style>
html,body,#terminal{height:100%;margin:0;background:#050505;color:#f5f5f5;}
body{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;}
#status{position:fixed;top:8px;right:10px;z-index:2;color:#aaa;font-size:12px;background:#111;padding:4px 6px;border:1px solid #333;}
#terminal{box-sizing:border-box;white-space:pre-wrap;overflow:auto;padding:12px;outline:none;font-size:13px;line-height:1.35;}
</style>
</head>
<body>
<div id="status">authorizing</div>
<pre id="terminal" tabindex="0" spellcheck="false"></pre>
<script src="/expose/app.js"></script>
</body>
</html>"##
}

const EXPOSE_APP_JS: &str = r#"(() => {
  const BASE = "/expose";
  const MAX_BATCH = 16 * 1024;
  const MAX_PENDING = 32 * 1024;
  const MAX_TERMINAL_CHARS = 1024 * 1024;
  const statusEl = document.getElementById("status");
  const terminal = document.getElementById("terminal");
  const decoder = new TextDecoder();
  const encoder = new TextEncoder();
  let csrf = "";
  let events;
  let pending = [];
  let pendingBytes = 0;
  let inputSending = false;
  let inputTimer;

  function writeData(bytes) {
    terminal.textContent += decoder.decode(bytes, { stream: true });
    if (terminal.textContent.length > MAX_TERMINAL_CHARS) {
      terminal.textContent = terminal.textContent.slice(-MAX_TERMINAL_CHARS);
    }
    terminal.scrollTop = terminal.scrollHeight;
  }

  function queueInput(text) {
    const bytes = encoder.encode(text);
    if (pendingBytes + bytes.length > MAX_PENDING) {
      statusEl.textContent = "input queue full";
      return;
    }
    pending.push(bytes);
    pendingBytes += bytes.length;
    clearTimeout(inputTimer);
    inputTimer = setTimeout(flushInput, 18);
  }

  function takeBatch() {
    const size = Math.min(pendingBytes, MAX_BATCH);
    const batch = new Uint8Array(size);
    let offset = 0;
    while (offset < size) {
      const chunk = pending[0];
      const take = Math.min(chunk.length, size - offset);
      batch.set(chunk.subarray(0, take), offset);
      offset += take;
      if (take === chunk.length) pending.shift();
      else pending[0] = chunk.subarray(take);
    }
    pendingBytes -= size;
    return batch;
  }

  async function flushInput() {
    if (inputSending || !csrf || pendingBytes === 0) return;
    inputSending = true;
    const batch = takeBatch();
    try {
      const response = await fetch(`${BASE}/input`, {
        method: "POST",
        headers: {
          "Content-Type": "application/octet-stream",
          "X-Prodex-CSRF": csrf,
        },
        body: batch,
        cache: "no-store",
      });
      if (!response.ok) statusEl.textContent = response.status === 429 ? "input rate limited" : "session expired";
    } catch (_) {
      statusEl.textContent = "input error";
    } finally {
      inputSending = false;
      if (pendingBytes) inputTimer = setTimeout(flushInput, 18);
    }
  }

  terminal.addEventListener("paste", event => {
    queueInput(event.clipboardData.getData("text"));
    event.preventDefault();
  });

  terminal.addEventListener("keydown", event => {
    const keys = {
      Enter: "\r", Backspace: "\x7f", Tab: "\t", Escape: "\x1b",
      ArrowUp: "\x1b[A", ArrowDown: "\x1b[B", ArrowRight: "\x1b[C", ArrowLeft: "\x1b[D",
      Home: "\x1b[H", End: "\x1b[F", Delete: "\x1b[3~",
    };
    if (event.ctrlKey && event.key.length === 1) {
      queueInput(String.fromCharCode(event.key.toUpperCase().charCodeAt(0) - 64));
    } else if (keys[event.key]) {
      queueInput(keys[event.key]);
    } else if (event.key.length === 1 && !event.metaKey && !event.altKey) {
      queueInput(event.key);
    } else {
      return;
    }
    event.preventDefault();
  });

  function connect() {
    events = new EventSource(`${BASE}/stream`);
    events.onopen = () => { statusEl.textContent = "connected"; terminal.focus(); };
    events.onmessage = event => {
      const raw = atob(event.data);
      writeData(Uint8Array.from(raw, ch => ch.charCodeAt(0)));
    };
    events.onerror = () => { statusEl.textContent = "reconnecting"; };
  }

  async function rotateSession() {
    if (!csrf || inputSending || pendingBytes) return;
    try {
      const response = await fetch(`${BASE}/session/rotate`, {
        method: "POST",
        headers: { "X-Prodex-CSRF": csrf },
        cache: "no-store",
      });
      if (!response.ok) throw new Error("rotate failed");
      csrf = (await response.json()).csrf;
    } catch (_) {
      statusEl.textContent = "session expired";
      if (events) events.close();
    }
  }

  async function authorize() {
    const bootstrap = new URLSearchParams(location.hash.slice(1)).get("bootstrap");
    history.replaceState(null, "", location.pathname);
    if (!bootstrap) throw new Error("bootstrap missing or already consumed");
    const response = await fetch(`${BASE}/session`, {
      method: "POST",
      headers: { Authorization: `Bearer ${bootstrap}` },
      cache: "no-store",
    });
    if (!response.ok) throw new Error("bootstrap expired or already consumed");
    csrf = (await response.json()).csrf;
    connect();
    setInterval(rotateSession, 5 * 60 * 1000);
  }

  addEventListener("pagehide", () => {
    if (!csrf) return;
    fetch(`${BASE}/session/revoke`, {
      method: "POST",
      headers: { "X-Prodex-CSRF": csrf },
      cache: "no-store",
      keepalive: true,
    }).catch(() => {});
  });

  authorize().catch(error => { statusEl.textContent = error.message; });
})();"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expose_ui_uses_fragment_bootstrap_and_batched_input_without_browser_storage() {
        assert!(EXPOSE_APP_JS.contains("location.hash"));
        assert!(EXPOSE_APP_JS.contains("setTimeout(flushInput, 18)"));
        assert!(EXPOSE_APP_JS.contains("MAX_TERMINAL_CHARS"));
        assert!(EXPOSE_APP_JS.contains("/session/rotate"));
        assert!(!EXPOSE_APP_JS.contains("localStorage"));
        assert!(!EXPOSE_APP_JS.contains("access_token"));
        assert!(!EXPOSE_APP_JS.contains("?access_token"));
        assert!(!EXPOSE_APP_JS.contains("?token"));
    }
}
