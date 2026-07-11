use super::*;

pub(super) struct ExposeParsedRequest {
    pub(super) method: String,
    pub(super) target: String,
    pub(super) headers: BTreeMap<String, Vec<String>>,
    pub(super) body: Vec<u8>,
}

pub(super) struct ExposeHttpRequest {
    pub(super) request: ExposeParsedRequest,
    pub(super) stream: TcpStream,
}

impl ExposeHttpRequest {
    fn method(&self) -> &str {
        &self.request.method
    }

    fn target(&self) -> &str {
        &self.request.target
    }

    fn body(&self) -> &[u8] {
        &self.request.body
    }

    pub(super) fn header(&self, name: &str) -> Option<&str> {
        let values = self.request.headers.get(&name.to_ascii_lowercase())?;
        let [value] = values.as_slice() else {
            return None;
        };
        Some(value)
    }

    fn has_header(&self, name: &str) -> bool {
        self.request
            .headers
            .contains_key(&name.to_ascii_lowercase())
    }

    fn respond(mut self, response: ui::ExposeHttpResponse) -> io::Result<()> {
        expose_write_http_response(&mut self.stream, response)
    }
}

#[derive(Debug)]
pub(super) struct ExposeHttpParseError {
    pub(super) status: u16,
    pub(super) message: &'static str,
}

pub(super) fn expose_read_http_request(
    stream: &mut TcpStream,
) -> std::result::Result<ExposeParsedRequest, ExposeHttpParseError> {
    let mut received = Vec::with_capacity(4096);
    let header_end = loop {
        if let Some(index) = received.windows(4).position(|window| window == b"\r\n\r\n") {
            break index;
        }
        if received.len() >= EXPOSE_MAX_HEADER_BYTES {
            return Err(expose_parse_error(431, "request headers too large"));
        }
        let mut chunk = [0_u8; 2048];
        let read = stream.read(&mut chunk).map_err(|err| {
            if matches!(
                err.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            ) {
                expose_parse_error(408, "request timeout")
            } else {
                expose_parse_error(400, "invalid request")
            }
        })?;
        if read == 0 {
            return Err(expose_parse_error(400, "invalid request"));
        }
        received.extend_from_slice(&chunk[..read]);
    };
    if header_end > EXPOSE_MAX_HEADER_BYTES {
        return Err(expose_parse_error(431, "request headers too large"));
    }
    let head = std::str::from_utf8(&received[..header_end])
        .map_err(|_| expose_parse_error(400, "invalid request"))?;
    let mut lines = head.split("\r\n");
    let request_line = lines
        .next()
        .ok_or_else(|| expose_parse_error(400, "invalid request"))?;
    let mut request_parts = request_line.split(' ');
    let method = request_parts.next().unwrap_or_default();
    let target = request_parts.next().unwrap_or_default();
    let version = request_parts.next().unwrap_or_default();
    if request_parts.next().is_some()
        || !expose_valid_method(method)
        || !expose_valid_request_target(target)
        || version != "HTTP/1.1"
    {
        return Err(expose_parse_error(400, "invalid request"));
    }

    let mut headers: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (index, line) in lines.enumerate() {
        if index >= EXPOSE_MAX_HEADERS
            || line.starts_with([' ', '\t'])
            || line
                .bytes()
                .any(|byte| byte == 0 || byte == b'\n' || byte == b'\r')
        {
            return Err(expose_parse_error(400, "invalid request headers"));
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| expose_parse_error(400, "invalid request headers"))?;
        if !expose_valid_header_name(name) {
            return Err(expose_parse_error(400, "invalid request headers"));
        }
        let value = value.trim_matches([' ', '\t']);
        if value.bytes().any(|byte| byte.is_ascii_control()) {
            return Err(expose_parse_error(400, "invalid request headers"));
        }
        headers
            .entry(name.to_ascii_lowercase())
            .or_default()
            .push(value.to_string());
    }
    if headers.contains_key("transfer-encoding") {
        return Err(expose_parse_error(400, "transfer encoding is unsupported"));
    }
    if headers.contains_key("expect") {
        return Err(expose_parse_error(417, "expectation failed"));
    }
    let content_length = match headers.get("content-length").map(Vec::as_slice) {
        None => 0,
        Some([value]) if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) => {
            value
                .parse::<usize>()
                .map_err(|_| expose_parse_error(400, "invalid content length"))?
        }
        Some(_) => return Err(expose_parse_error(400, "invalid content length")),
    };
    if content_length > EXPOSE_MAX_INPUT_BYTES {
        return Err(expose_parse_error(413, "request body too large"));
    }
    let body_start = header_end + 4;
    let initial_body = &received[body_start..];
    if initial_body.len() > content_length {
        return Err(expose_parse_error(400, "unexpected request bytes"));
    }
    let mut body = Vec::with_capacity(content_length);
    body.extend_from_slice(initial_body);
    while body.len() < content_length {
        let remaining = content_length - body.len();
        let mut chunk = [0_u8; 2048];
        let chunk_len = remaining.min(chunk.len());
        let read = stream.read(&mut chunk[..chunk_len]).map_err(|err| {
            if matches!(
                err.kind(),
                io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
            ) {
                expose_parse_error(408, "request timeout")
            } else {
                expose_parse_error(400, "invalid request body")
            }
        })?;
        if read == 0 {
            return Err(expose_parse_error(400, "invalid request body"));
        }
        body.extend_from_slice(&chunk[..read]);
    }
    Ok(ExposeParsedRequest {
        method: method.to_string(),
        target: target.to_string(),
        headers,
        body,
    })
}

pub(super) fn expose_parse_error(status: u16, message: &'static str) -> ExposeHttpParseError {
    ExposeHttpParseError { status, message }
}

pub(super) fn expose_valid_method(method: &str) -> bool {
    !method.is_empty()
        && method.len() <= 16
        && method
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte == b'-')
}

pub(super) fn expose_valid_request_target(target: &str) -> bool {
    target.starts_with('/')
        && target.len() <= EXPOSE_MAX_REQUEST_TARGET_BYTES
        && target.is_ascii()
        && !target
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte == b' ')
}

pub(super) fn expose_valid_header_name(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'!' | b'#'
                        | b'$'
                        | b'%'
                        | b'&'
                        | b'\''
                        | b'*'
                        | b'+'
                        | b'-'
                        | b'.'
                        | b'^'
                        | b'_'
                        | b'`'
                        | b'|'
                        | b'~'
                )
        })
}

pub(super) fn expose_write_http_response(
    stream: &mut TcpStream,
    response: ui::ExposeHttpResponse,
) -> io::Result<()> {
    write!(
        stream,
        "HTTP/1.1 {} {}\r\nContent-Length: {}\r\nConnection: close\r\n",
        response.status,
        expose_http_reason(response.status),
        response.body.len(),
    )?;
    for (name, value) in response.headers {
        if name.contains(['\r', '\n']) || value.contains(['\r', '\n']) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid expose response header",
            ));
        }
        write!(stream, "{name}: {value}\r\n")?;
    }
    stream.write_all(b"\r\n")?;
    stream.write_all(&response.body)?;
    stream.flush()
}

pub(super) fn expose_http_reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        413 => "Content Too Large",
        415 => "Unsupported Media Type",
        417 => "Expectation Failed",
        429 => "Too Many Requests",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "Error",
    }
}

pub(super) fn handle_expose_request(request: ExposeHttpRequest, shared: &Arc<ExposeShared>) {
    if !expose_request_host_allowed(&request, shared) {
        let _ = request.respond(expose_text_response(403, "forbidden"));
        return;
    }
    let path = request.target().to_string();
    if path.contains('?') || path.contains('#') {
        let _ = request.respond(expose_text_response(404, "not found"));
        return;
    }
    match path.as_str() {
        EXPOSE_BASE_PATH if request.method() == "GET" => {
            let _ = request.respond(expose_html_response());
        }
        "/expose/app.js" if request.method() == "GET" => {
            let _ = request.respond(expose_js_response());
        }
        "/expose/session" => handle_expose_session_exchange(request, shared),
        "/expose/session/rotate" => handle_expose_session_rotate(request, shared),
        "/expose/session/revoke" => handle_expose_session_revoke(request, shared),
        "/expose/stream" => handle_expose_stream(request, shared),
        "/expose/input" => handle_expose_input(request, shared),
        _ => {
            let _ = request.respond(expose_text_response(404, "not found"));
        }
    }
}

pub(super) fn handle_expose_session_exchange(
    request: ExposeHttpRequest,
    shared: &Arc<ExposeShared>,
) {
    if request.method() != "POST" {
        let _ = request.respond(expose_text_response(405, "method not allowed"));
        return;
    }
    let Some(secure) = expose_secure_same_origin(&request) else {
        let _ = request.respond(expose_text_response(403, "forbidden"));
        return;
    };
    let Some(bootstrap) = expose_bearer_token(&request).map(str::to_string) else {
        let _ = request.respond(expose_text_response(401, "unauthorized"));
        return;
    };
    if let Some((status, message)) = expose_empty_body_error(&request) {
        let _ = request.respond(expose_text_response(status, message));
        return;
    }
    let issued = shared
        .sessions
        .lock()
        .map_err(|_| ExposeSessionError::Internal)
        .and_then(|mut sessions| sessions.exchange_bootstrap(&bootstrap, Instant::now()));
    expose_respond_session_result(request, issued, secure);
}

pub(super) fn handle_expose_session_rotate(request: ExposeHttpRequest, shared: &Arc<ExposeShared>) {
    if request.method() != "POST" {
        let _ = request.respond(expose_text_response(405, "method not allowed"));
        return;
    }
    let Some(secure) = expose_secure_same_origin(&request) else {
        let _ = request.respond(expose_text_response(403, "forbidden"));
        return;
    };
    let Some(session) = expose_session_cookie(&request).map(str::to_string) else {
        let _ = request.respond(expose_text_response(401, "unauthorized"));
        return;
    };
    let Some(csrf) = expose_single_header(&request, "X-Prodex-CSRF").map(str::to_string) else {
        let _ = request.respond(expose_text_response(403, "forbidden"));
        return;
    };
    if let Some((status, message)) = expose_empty_body_error(&request) {
        let _ = request.respond(expose_text_response(status, message));
        return;
    }
    let issued = shared
        .sessions
        .lock()
        .map_err(|_| ExposeSessionError::Internal)
        .and_then(|mut sessions| sessions.rotate(&session, &csrf, Instant::now()));
    expose_respond_session_result(request, issued, secure);
}

pub(super) fn handle_expose_session_revoke(request: ExposeHttpRequest, shared: &Arc<ExposeShared>) {
    if request.method() != "POST" {
        let _ = request.respond(expose_text_response(405, "method not allowed"));
        return;
    }
    let Some(secure) = expose_secure_same_origin(&request) else {
        let _ = request.respond(expose_text_response(403, "forbidden"));
        return;
    };
    let Some(session) = expose_session_cookie(&request).map(str::to_string) else {
        let _ = request.respond(expose_text_response(401, "unauthorized"));
        return;
    };
    let Some(csrf) = expose_single_header(&request, "X-Prodex-CSRF").map(str::to_string) else {
        let _ = request.respond(expose_text_response(403, "forbidden"));
        return;
    };
    if let Some((status, message)) = expose_empty_body_error(&request) {
        let _ = request.respond(expose_text_response(status, message));
        return;
    }
    let revoked = shared
        .sessions
        .lock()
        .map(|mut sessions| sessions.revoke(&session, &csrf, Instant::now()))
        .unwrap_or(false);
    if revoked {
        let cookie = expose_expired_session_cookie(secure);
        let _ = request.respond(expose_json_response(204, "", Some(&cookie)));
    } else {
        let _ = request.respond(expose_text_response(401, "unauthorized"));
    }
}

pub(super) fn expose_respond_session_result(
    request: ExposeHttpRequest,
    issued: std::result::Result<ExposeIssuedSession, ExposeSessionError>,
    secure: bool,
) {
    match issued {
        Ok(issued) => {
            let cookie = expose_session_cookie_header(&issued.session, secure);
            let body = format!(r#"{{"csrf":"{}"}}"#, issued.csrf);
            let _ = request.respond(expose_json_response(201, &body, Some(&cookie)));
        }
        Err(ExposeSessionError::Full) => {
            let _ = request.respond(expose_text_response(429, "too many sessions"));
        }
        Err(ExposeSessionError::Unauthorized | ExposeSessionError::RateLimited) => {
            let _ = request.respond(expose_text_response(401, "unauthorized"));
        }
        Err(ExposeSessionError::Internal) => {
            let _ = request.respond(expose_text_response(500, "session unavailable"));
        }
    }
}

pub(super) fn expose_empty_body_error(request: &ExposeHttpRequest) -> Option<(u16, &'static str)> {
    if request.body().is_empty() {
        None
    } else {
        Some((413, "request body not allowed"))
    }
}

pub(super) fn handle_expose_stream(request: ExposeHttpRequest, shared: &Arc<ExposeShared>) {
    if request.method() != "GET" {
        let _ = request.respond(expose_text_response(405, "method not allowed"));
        return;
    }
    if !expose_optional_origin_allowed(&request) {
        let _ = request.respond(expose_text_response(403, "forbidden"));
        return;
    }
    let Some(session) = expose_session_cookie(&request).map(str::to_string) else {
        let _ = request.respond(expose_text_response(401, "unauthorized"));
        return;
    };
    let authorized = shared
        .sessions
        .lock()
        .map(|mut sessions| sessions.authenticate(&session, None, Instant::now(), true))
        .unwrap_or(false);
    if !authorized {
        let _ = request.respond(expose_text_response(401, "unauthorized"));
        return;
    }
    if !expose_try_acquire_client(shared) {
        let _ = request.respond(expose_text_response(429, "too many clients"));
        return;
    }

    let client_id = shared.next_client_id.fetch_add(1, Ordering::SeqCst);
    let (tx, rx) = mpsc::sync_channel::<Vec<u8>>(EXPOSE_CLIENT_QUEUE_CAPACITY);
    match shared.pty.clients.lock() {
        Ok(mut clients) => clients.push(ExposeOutputClient {
            id: client_id,
            sender: tx,
        }),
        Err(_) => {
            shared.active_clients.fetch_sub(1, Ordering::SeqCst);
            let _ = request.respond(expose_text_response(500, "stream unavailable"));
            return;
        }
    }
    let _guard = ExposeClientGuard {
        shared: Arc::clone(shared),
        client_id,
    };

    let mut writer = request.stream;
    if write!(
        writer,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nX-Content-Type-Options: nosniff\r\nX-Frame-Options: DENY\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\n\r\n"
    )
    .and_then(|_| writer.flush())
    .is_err()
    {
        return;
    }
    if let Ok(scrollback) = shared.pty.scrollback.lock() {
        let bytes: Vec<u8> = scrollback.iter().copied().collect();
        if !bytes.is_empty() && expose_write_sse(&mut writer, &bytes).is_err() {
            return;
        }
    }
    let mut last_keepalive = Instant::now();
    while shared.pty.running.load(Ordering::SeqCst)
        && !shared.shutdown.load(Ordering::SeqCst)
        && expose_session_valid(shared, &session)
    {
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(bytes) => {
                if expose_write_sse(&mut writer, &bytes).is_err() {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if last_keepalive.elapsed() >= EXPOSE_STREAM_KEEPALIVE {
                    if writer
                        .write_all(b": keepalive\n\n")
                        .and_then(|_| writer.flush())
                        .is_err()
                    {
                        break;
                    }
                    last_keepalive = Instant::now();
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

pub(super) fn handle_expose_input(mut request: ExposeHttpRequest, shared: &Arc<ExposeShared>) {
    if request.method() != "POST" {
        let _ = request.respond(expose_text_response(405, "method not allowed"));
        return;
    }
    if expose_secure_same_origin(&request).is_none() {
        let _ = request.respond(expose_text_response(403, "forbidden"));
        return;
    }
    if expose_single_header(&request, "Content-Type").and_then(|value| value.split(';').next())
        != Some("application/octet-stream")
    {
        let _ = request.respond(expose_text_response(415, "unsupported media type"));
        return;
    }
    let Some(session) = expose_session_cookie(&request).map(str::to_string) else {
        let _ = request.respond(expose_text_response(401, "unauthorized"));
        return;
    };
    let Some(csrf) = expose_single_header(&request, "X-Prodex-CSRF").map(str::to_string) else {
        let _ = request.respond(expose_text_response(403, "forbidden"));
        return;
    };
    let preliminarily_authorized = shared
        .sessions
        .lock()
        .map(|mut sessions| sessions.authenticate(&session, Some(&csrf), Instant::now(), false))
        .unwrap_or(false);
    if !preliminarily_authorized {
        let _ = request.respond(expose_text_response(401, "unauthorized"));
        return;
    }
    if request.body().is_empty() {
        let _ = request.respond(expose_text_response(400, "invalid input"));
        return;
    }
    let body = std::mem::take(&mut request.request.body);
    let admitted = shared
        .sessions
        .lock()
        .map_err(|_| ExposeSessionError::Internal)
        .and_then(|mut sessions| sessions.admit_input(&session, &csrf, body.len(), Instant::now()));
    match admitted {
        Ok(()) => match expose_write_input(&shared.pty.writer, &body) {
            Ok(()) => {
                let _ = request.respond(expose_text_response(204, ""));
            }
            Err(_) => {
                let _ = request.respond(expose_text_response(500, "input unavailable"));
            }
        },
        Err(ExposeSessionError::RateLimited) => {
            let _ = request.respond(expose_text_response(429, "input rate limited"));
        }
        Err(ExposeSessionError::Unauthorized) => {
            let _ = request.respond(expose_text_response(401, "unauthorized"));
        }
        Err(ExposeSessionError::Full | ExposeSessionError::Internal) => {
            let _ = request.respond(expose_text_response(500, "input unavailable"));
        }
    }
}

pub(super) fn expose_session_valid(shared: &ExposeShared, session: &str) -> bool {
    shared
        .sessions
        .lock()
        .map(|mut sessions| sessions.authenticate(session, None, Instant::now(), false))
        .unwrap_or(false)
}

pub(super) fn expose_try_acquire_client(shared: &ExposeShared) -> bool {
    expose_try_acquire(&shared.active_clients, shared.max_clients)
}

pub(super) fn expose_try_acquire(active_clients: &AtomicUsize, max_clients: usize) -> bool {
    let mut active = active_clients.load(Ordering::SeqCst);
    loop {
        if active >= max_clients {
            return false;
        }
        match active_clients.compare_exchange_weak(
            active,
            active + 1,
            Ordering::SeqCst,
            Ordering::SeqCst,
        ) {
            Ok(_) => return true,
            Err(observed) => active = observed,
        }
    }
}

pub(super) fn expose_write_sse(writer: &mut impl Write, bytes: &[u8]) -> io::Result<()> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    write!(writer, "data: {encoded}\n\n")?;
    writer.flush()
}

pub(super) struct ExposeOutputClient {
    pub(super) id: u64,
    pub(super) sender: mpsc::SyncSender<Vec<u8>>,
}

pub(super) struct ExposeClientGuard {
    shared: Arc<ExposeShared>,
    client_id: u64,
}

impl Drop for ExposeClientGuard {
    fn drop(&mut self) {
        self.shared.active_clients.fetch_sub(1, Ordering::SeqCst);
        if let Ok(mut clients) = self.shared.pty.clients.lock() {
            clients.retain(|client| client.id != self.client_id);
        }
    }
}

pub(super) fn expose_command_builder(command: Option<&str>) -> CommandBuilder {
    match command {
        Some(command) if !command.trim().is_empty() => {
            let shell = env::var("SHELL").unwrap_or_else(|_| "sh".to_string());
            let mut builder = CommandBuilder::new(shell);
            builder.arg("-lc");
            builder.arg(command);
            builder
        }
        _ => CommandBuilder::new(env::var("SHELL").unwrap_or_else(|_| "sh".to_string())),
    }
}

pub(super) fn expose_broadcast_output(
    scrollback: &Arc<Mutex<VecDeque<u8>>>,
    clients: &Arc<Mutex<Vec<ExposeOutputClient>>>,
    bytes: &[u8],
) {
    if let Ok(mut buffer) = scrollback.lock() {
        buffer.extend(bytes);
        while buffer.len() > EXPOSE_SCROLLBACK_LIMIT {
            buffer.pop_front();
        }
    }
    if let Ok(mut clients) = clients.lock() {
        clients.retain(|client| client.sender.try_send(bytes.to_vec()).is_ok());
    }
}

pub(super) fn expose_write_input(
    writer: &Arc<Mutex<Box<dyn Write + Send>>>,
    bytes: &[u8],
) -> io::Result<()> {
    if bytes.is_empty() || bytes.len() > EXPOSE_MAX_INPUT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid expose input size",
        ));
    }
    let mut writer = writer
        .lock()
        .map_err(|_| io::Error::other("expose PTY writer unavailable"))?;
    writer.write_all(bytes)?;
    writer.flush()
}

pub(super) fn expose_request_host_allowed(
    request: &ExposeHttpRequest,
    shared: &ExposeShared,
) -> bool {
    let Some(host) = expose_single_header(request, "Host") else {
        return false;
    };
    if !expose_valid_host(host) {
        return false;
    }
    shared
        .allowed_hosts
        .lock()
        .map(|hosts| hosts.contains(&host.to_ascii_lowercase()))
        .unwrap_or(false)
}

pub(super) fn expose_valid_host(host: &str) -> bool {
    !host.is_empty()
        && host == host.trim()
        && !host
            .chars()
            .any(|ch| ch.is_whitespace() || matches!(ch, '/' | '\\' | '@' | '?' | '#'))
}

pub(super) fn expose_secure_same_origin(request: &ExposeHttpRequest) -> Option<bool> {
    expose_origin_policy(
        expose_single_header(request, "Host"),
        expose_single_header(request, "Origin"),
    )
}

pub(super) fn expose_optional_origin_allowed(request: &ExposeHttpRequest) -> bool {
    if !request.has_header("Origin") {
        return true;
    }
    expose_secure_same_origin(request).is_some()
}

pub(super) fn expose_origin_matches_host(host: &str, origin: &str) -> Option<bool> {
    if !expose_valid_host(host) || origin != origin.trim() {
        return None;
    }
    let host = host.to_ascii_lowercase();
    let origin = origin.to_ascii_lowercase();
    if origin == format!("https://{host}") {
        Some(true)
    } else if origin == format!("http://{host}") {
        Some(false)
    } else {
        None
    }
}

pub(super) fn expose_origin_policy(host: Option<&str>, origin: Option<&str>) -> Option<bool> {
    expose_origin_matches_host(host?, origin?)
}

pub(super) fn expose_single_header<'a>(
    request: &'a ExposeHttpRequest,
    name: &str,
) -> Option<&'a str> {
    request.header(name)
}

pub(super) fn expose_bearer_token(request: &ExposeHttpRequest) -> Option<&str> {
    let value = expose_single_header(request, "Authorization")?;
    let token = value.strip_prefix("Bearer ")?;
    expose_valid_token(token).then_some(token)
}

pub(super) fn expose_session_cookie(request: &ExposeHttpRequest) -> Option<&str> {
    let cookies = expose_single_header(request, "Cookie")?;
    let mut matches = cookies.split(';').filter_map(|cookie| {
        let (name, value) = cookie.trim().split_once('=')?;
        (name == EXPOSE_SESSION_COOKIE && expose_valid_token(value)).then_some(value)
    });
    let value = matches.next()?;
    matches.next().is_none().then_some(value)
}

pub(super) fn expose_valid_token(token: &str) -> bool {
    (32..=128).contains(&token.len())
        && token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

pub(super) fn expose_session_cookie_header(session: &str, secure: bool) -> String {
    format!(
        "{EXPOSE_SESSION_COOKIE}={session}; HttpOnly; SameSite=Strict; Path={EXPOSE_BASE_PATH}; Max-Age={}{}",
        EXPOSE_SESSION_TTL.as_secs(),
        if secure { "; Secure" } else { "" },
    )
}

pub(super) fn expose_expired_session_cookie(secure: bool) -> String {
    format!(
        "{EXPOSE_SESSION_COOKIE}=; HttpOnly; SameSite=Strict; Path={EXPOSE_BASE_PATH}; Max-Age=0{}",
        if secure { "; Secure" } else { "" },
    )
}
