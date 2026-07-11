use std::io::Cursor;
use tiny_http::{Header as TinyHeader, Response as TinyResponse};

pub(super) fn expose_text_response(status: u16, body: &str) -> TinyResponse<Cursor<Vec<u8>>> {
    let mut response = TinyResponse::from_string(body.to_string()).with_status_code(status);
    for (name, value) in expose_security_headers("text/plain; charset=utf-8") {
        if let Ok(header) = TinyHeader::from_bytes(name, value) {
            response.add_header(header);
        }
    }
    response
}

pub(super) fn expose_html_response() -> TinyResponse<Cursor<Vec<u8>>> {
    let body = expose_html();
    let mut response = TinyResponse::from_string(body);
    for (name, value) in expose_security_headers("text/html; charset=utf-8") {
        if let Ok(header) = TinyHeader::from_bytes(name, value) {
            response.add_header(header);
        }
    }
    response
}

pub(super) fn expose_js_response() -> TinyResponse<Cursor<Vec<u8>>> {
    let mut response = TinyResponse::from_string(EXPOSE_APP_JS);
    for (name, value) in expose_security_headers("application/javascript; charset=utf-8") {
        if let Ok(header) = TinyHeader::from_bytes(name, value) {
            response.add_header(header);
        }
    }
    response
}

fn expose_security_headers(content_type: &'static str) -> Vec<(&'static str, &'static str)> {
    vec![
        ("Content-Type", content_type),
        ("Cache-Control", "no-store"),
        ("X-Content-Type-Options", "nosniff"),
        ("X-Frame-Options", "DENY"),
        ("Referrer-Policy", "no-referrer"),
        (
            "Content-Security-Policy",
            "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; connect-src 'self'",
        ),
    ]
}

fn expose_html() -> String {
    r##"<!doctype html>
<html>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Prodex Expose</title>
<style>
html,body,#terminal{{height:100%;margin:0;background:#050505;color:#f5f5f5;}}
body{{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;}}
#status{{position:fixed;top:8px;right:10px;z-index:2;color:#aaa;font-size:12px;background:#111;padding:4px 6px;border:1px solid #333;}}
#terminal{{box-sizing:border-box;white-space:pre-wrap;overflow:auto;padding:12px;outline:none;font-size:13px;line-height:1.35;}}
</style>
</head>
<body>
<div id="status">connecting</div>
<pre id="terminal" tabindex="0" spellcheck="false"></pre>
<script src="/app.js"></script>
</body>
</html>"##
        .to_string()
}

const EXPOSE_APP_JS: &str = r#"(() => {
  const statusEl = document.getElementById("status");
  const terminal = document.getElementById("terminal");
  const token = location.pathname.split("/").filter(Boolean).pop();
  const streamUrl = `/stream/${token}?access_token=${encodeURIComponent(token)}`;
  const inputUrl = `/input/${token}?access_token=${encodeURIComponent(token)}`;
  const decoder = new TextDecoder();
  function writeData(data) {
    terminal.textContent += data;
    terminal.scrollTop = terminal.scrollHeight;
  }
  function sendInput(data) {
    fetch(inputUrl, { method: "POST", body: data, cache: "no-store", keepalive: false })
      .catch(() => { statusEl.textContent = "input error"; });
  }
  terminal.addEventListener("keydown", event => {
    if (event.ctrlKey && event.key.length === 1) {
      sendInput(String.fromCharCode(event.key.toUpperCase().charCodeAt(0) - 64));
      event.preventDefault();
      return;
    }
    if (event.key === "Enter") {
      sendInput("\r");
    } else if (event.key === "Backspace") {
      sendInput("\x7f");
    } else if (event.key === "Tab") {
      sendInput("\t");
    } else if (event.key.length === 1 && !event.metaKey && !event.altKey) {
      sendInput(event.key);
    } else {
      return;
    }
    event.preventDefault();
  });
  function connect() {
    const events = new EventSource(streamUrl);
    events.onopen = () => { statusEl.textContent = "connected"; terminal.focus(); };
    events.onmessage = event => {
      const raw = atob(event.data);
      const bytes = Uint8Array.from(raw, ch => ch.charCodeAt(0));
      writeData(decoder.decode(bytes));
    };
    events.onerror = () => {
      statusEl.textContent = "disconnected";
      events.close();
      setTimeout(connect, 1500);
    };
  }
  connect();
})();"#;
