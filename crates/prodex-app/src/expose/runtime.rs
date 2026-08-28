use super::super_expose::{ensure_cloudflared_available, handle_super_expose};
use super::*;
mod cloudflared_startup;
mod hostname;
use std::fs::OpenOptions;
use std::net::SocketAddr;
#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

pub(super) fn handle_expose(args: ExposeArgs) -> Result<()> {
    if args.invocation == prodex_cli::ExposeInvocation::SuperAlias
        && !args.tunnel
        && !args.no_tunnel
    {
        return handle_super_expose(args);
    }
    handle_legacy_expose(args)
}

fn handle_legacy_expose(args: ExposeArgs) -> Result<()> {
    if args.no_tunnel {
        eprintln!("warning: --no-tunnel is deprecated; expose is loopback-only by default");
    }
    let max_clients = usize::from(args.max_clients).clamp(1, EXPOSE_MAX_CLIENTS_LIMIT);
    let bootstrap = expose_random_token()?;
    let pty = ExposePty::spawn(&args)?;
    let listener =
        TcpListener::bind("127.0.0.1:0").context("failed to bind expose server on 127.0.0.1:0")?;
    let listen_addr = listener
        .local_addr()
        .context("failed to inspect expose listen address")?;
    let shutdown = Arc::new(AtomicBool::new(false));
    let shared = Arc::new(ExposeShared {
        sessions: Mutex::new(ExposeSessionStore::new(
            &bootstrap,
            max_clients,
            Instant::now(),
        )),
        allowed_hosts: Mutex::new(BTreeSet::from([listen_addr
            .to_string()
            .to_ascii_lowercase()])),
        mcp_only_hosts: Mutex::new(BTreeSet::new()),
        mcp: None,
        pty,
        shutdown: Arc::clone(&shutdown),
        active_clients: AtomicUsize::new(0),
        active_requests: AtomicUsize::new(0),
        peak_requests: AtomicUsize::new(0),
        next_client_id: AtomicU64::new(1),
        max_clients,
    });
    let mut http = ExposeHttpServer::start(listener, Arc::clone(&shared))
        .context("failed to start expose HTTP server")?;
    let local_url = expose_access_url(&format!("http://{listen_addr}"), &bootstrap);

    let tunnel_requested = args.tunnel && !args.no_tunnel;
    let (tunnel_status, tunnel_url, mut tunnel) =
        expose_start_tunnel(&shared, listen_addr, &bootstrap, tunnel_requested);
    print_expose_status(
        &local_url,
        max_clients,
        &tunnel_status,
        tunnel_url.as_deref(),
        tunnel_requested,
    )?;
    drop(bootstrap);

    while shared.pty.running.load(Ordering::SeqCst) && !shared.shutdown.load(Ordering::SeqCst) {
        thread::sleep(Duration::from_millis(250));
    }
    shared.shutdown.store(true, Ordering::SeqCst);
    if let Some(tunnel) = tunnel.as_mut() {
        tunnel.shutdown();
    }
    shared.pty.shutdown();
    http.shutdown();
    Ok(())
}

fn expose_start_tunnel(
    shared: &ExposeShared,
    listen_addr: SocketAddr,
    bootstrap: &str,
    requested: bool,
) -> (String, Option<String>, Option<CloudflaredTunnel>) {
    if !requested {
        return (
            "disabled by default; pass --tunnel to publish this remote shell".to_string(),
            None,
            None,
        );
    }
    if let Err(err) = ensure_cloudflared_available() {
        return (expose_tunnel_unavailable_status(&err), None, None);
    }
    match start_cloudflared_tunnel(&format!("http://{listen_addr}")) {
        Ok(tunnel) => {
            let Some(url) = tunnel.url.as_deref() else {
                return (
                    "cloudflared started; public URL not reported".to_string(),
                    None,
                    Some(tunnel),
                );
            };
            if let Some(host) = expose_public_host(url) {
                shared.allow_host(host);
            }
            (
                "ready".to_string(),
                Some(expose_access_url(url, bootstrap)),
                Some(tunnel),
            )
        }
        Err(err) => (expose_tunnel_unavailable_status(&err), None, None),
    }
}

pub(super) fn print_expose_status(
    local_url: &str,
    max_clients: usize,
    tunnel_status: &str,
    tunnel_url: Option<&str>,
    tunnel_requested: bool,
) -> Result<()> {
    print_panel(
        "Expose",
        &expose_status_fields(
            local_url,
            max_clients,
            tunnel_status,
            tunnel_url,
            tunnel_requested,
        ),
    )?;
    Ok(())
}

pub(super) fn expose_status_fields(
    local_url: &str,
    max_clients: usize,
    tunnel_status: &str,
    tunnel_url: Option<&str>,
    tunnel_requested: bool,
) -> Vec<(String, String)> {
    let mut fields = vec![
        ("One-time local URL".to_string(), local_url.to_string()),
        (
            "Security".to_string(),
            format!(
                "loopback listener; fragment bootstrap expires in {}s; HttpOnly session; fixed workers={}; max clients={max_clients}",
                EXPOSE_BOOTSTRAP_TTL.as_secs(),
                expose_worker_count(max_clients),
            ),
        ),
        ("Tunnel".to_string(), tunnel_status.to_string()),
    ];
    if tunnel_requested {
        fields.push((
            "WARNING".to_string(),
            "REMOTE SHELL ENABLED: anyone with the one-time URL can control this shell".to_string(),
        ));
    }
    if let Some(url) = tunnel_url {
        fields.push(("One-time tunnel URL".to_string(), url.to_string()));
    }
    fields
}

pub(super) fn expose_access_url(origin: &str, bootstrap: &str) -> String {
    format!(
        "{}{EXPOSE_BASE_PATH}#bootstrap={bootstrap}",
        origin.trim_end_matches('/')
    )
}

pub(super) fn expose_tunnel_unavailable_status(err: &anyhow::Error) -> String {
    format!(
        "unavailable: {}; local access remains available",
        redaction_redact_secret_like_text(&format!("{err:#}"))
    )
}

pub(super) struct ExposeShared {
    pub(super) sessions: Mutex<ExposeSessionStore>,
    pub(super) allowed_hosts: Mutex<BTreeSet<String>>,
    pub(super) mcp_only_hosts: Mutex<BTreeSet<String>>,
    pub(super) mcp: Option<Arc<ExposeMcpEndpoint>>,
    pub(super) pty: ExposePty,
    pub(super) shutdown: Arc<AtomicBool>,
    pub(super) active_clients: AtomicUsize,
    pub(super) active_requests: AtomicUsize,
    pub(super) peak_requests: AtomicUsize,
    pub(super) next_client_id: AtomicU64,
    pub(super) max_clients: usize,
}

impl ExposeShared {
    pub(super) fn allow_host(&self, host: String) {
        if let Ok(mut hosts) = self.allowed_hosts.lock() {
            hosts.insert(host.to_ascii_lowercase());
        }
    }

    pub(super) fn allow_mcp_only_host(&self, host: String) {
        if let Ok(mut hosts) = self.mcp_only_hosts.lock() {
            hosts.insert(host.to_ascii_lowercase());
        }
    }

    pub(super) fn is_mcp_only_host(&self, host: &str) -> bool {
        self.mcp_only_hosts
            .lock()
            .map(|hosts| hosts.contains(&host.to_ascii_lowercase()))
            .unwrap_or(false)
    }
}

pub(super) struct ExposeHttpServer {
    pub(super) shutdown: Arc<AtomicBool>,
    pub(super) accept_thread: Option<JoinHandle<()>>,
    pub(super) worker_threads: Vec<JoinHandle<()>>,
}

impl ExposeHttpServer {
    pub(super) fn start(listener: TcpListener, shared: Arc<ExposeShared>) -> io::Result<Self> {
        listener.set_nonblocking(true)?;
        let (request_tx, request_rx) =
            mpsc::sync_channel::<TcpStream>(EXPOSE_REQUEST_QUEUE_CAPACITY);
        let request_rx = Arc::new(Mutex::new(request_rx));
        let mut worker_threads = Vec::with_capacity(expose_worker_count(shared.max_clients));
        for _ in 0..expose_worker_count(shared.max_clients) {
            let request_rx = Arc::clone(&request_rx);
            let shared = Arc::clone(&shared);
            worker_threads.push(thread::spawn(move || {
                expose_worker_loop(&request_rx, &shared)
            }));
        }
        let shutdown = Arc::clone(&shared.shutdown);
        let accept_thread = thread::spawn(move || {
            while !shared.shutdown.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _peer)) => match request_tx.try_send(stream) {
                        Ok(()) => {}
                        Err(TrySendError::Full(mut stream)) => {
                            let _ = stream.set_write_timeout(Some(Duration::from_millis(250)));
                            let _ = expose_write_http_response(
                                &mut stream,
                                expose_text_response(503, "expose server overloaded"),
                            );
                        }
                        Err(TrySendError::Disconnected(mut stream)) => {
                            let _ = stream.set_write_timeout(Some(Duration::from_millis(250)));
                            let _ = expose_write_http_response(
                                &mut stream,
                                expose_text_response(503, "server stopping"),
                            );
                            break;
                        }
                    },
                    Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(10));
                    }
                    Err(_) => {
                        shared.shutdown.store(true, Ordering::SeqCst);
                        break;
                    }
                }
            }
        });
        Ok(Self {
            shutdown,
            accept_thread: Some(accept_thread),
            worker_threads,
        })
    }

    pub(super) fn shutdown(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        if let Some(thread) = self.accept_thread.take() {
            let _ = thread.join();
        }
        for thread in self.worker_threads.drain(..) {
            let _ = thread.join();
        }
    }
}

impl Drop for ExposeHttpServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub(super) fn expose_worker_count(max_clients: usize) -> usize {
    max_clients.clamp(1, EXPOSE_MAX_CLIENTS_LIMIT) + EXPOSE_SHORT_REQUEST_WORKERS
}

pub(super) fn expose_worker_loop(
    request_rx: &Arc<Mutex<Receiver<TcpStream>>>,
    shared: &Arc<ExposeShared>,
) {
    while !shared.shutdown.load(Ordering::SeqCst) {
        let received = match request_rx.lock() {
            Ok(rx) => rx.recv_timeout(Duration::from_millis(250)),
            Err(_) => break,
        };
        match received {
            Ok(mut stream) => {
                let _guard = ExposeRequestGuard::new(shared);
                let timeout = expose_request_io_timeout();
                let _ = stream.set_read_timeout(Some(timeout));
                let _ = stream.set_write_timeout(Some(timeout));
                match expose_read_http_request(&mut stream) {
                    Ok(request) => {
                        handle_expose_request(ExposeHttpRequest { request, stream }, shared)
                    }
                    Err(error) => {
                        let _ = expose_write_http_response(
                            &mut stream,
                            expose_text_response(error.status, error.message),
                        );
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}

pub(super) fn expose_request_io_timeout() -> Duration {
    if cfg!(test) {
        Duration::from_millis(250)
    } else {
        Duration::from_secs(5)
    }
}

pub(super) struct ExposeRequestGuard<'a>(&'a ExposeShared);

impl<'a> ExposeRequestGuard<'a> {
    fn new(shared: &'a ExposeShared) -> Self {
        let active = shared.active_requests.fetch_add(1, Ordering::SeqCst) + 1;
        shared.peak_requests.fetch_max(active, Ordering::SeqCst);
        Self(shared)
    }
}

impl Drop for ExposeRequestGuard<'_> {
    fn drop(&mut self) {
        self.0.active_requests.fetch_sub(1, Ordering::SeqCst);
    }
}

pub(super) struct ExposePty {
    pub(super) master: Mutex<Option<Box<dyn MasterPty + Send>>>,
    pub(super) writer: Arc<Mutex<Option<Box<dyn Write + Send>>>>,
    pub(super) scrollback: Arc<Mutex<VecDeque<u8>>>,
    pub(super) clients: Arc<Mutex<Vec<ExposeOutputClient>>>,
    pub(super) running: Arc<AtomicBool>,
    pub(super) killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    pub(super) reader_thread: Mutex<Option<JoinHandle<()>>>,
    pub(super) wait_thread: Mutex<Option<JoinHandle<()>>>,
}

impl ExposePty {
    pub(super) fn spawn(args: &ExposeArgs) -> Result<Self> {
        let cwd = env::current_dir().ok();
        Self::spawn_in_cwd(args, cwd.as_deref())
    }

    pub(super) fn spawn_in_cwd(args: &ExposeArgs, cwd: Option<&std::path::Path>) -> Result<Self> {
        let pair = native_pty_system()
            .openpty(PtySize {
                rows: args.rows.max(8),
                cols: args.cols.max(20),
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("failed to open PTY")?;
        let mut command = expose_command_builder(args.command.as_deref());
        command.env("TERM", "xterm-256color");
        if let Some(cwd) = cwd {
            command.cwd(cwd.as_os_str());
        }
        let mut child = pair
            .slave
            .spawn_command(command)
            .context("failed to spawn exposed shell")?;
        let killer = child.clone_killer();
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .context("failed to clone PTY reader")?;
        let writer = pair
            .master
            .take_writer()
            .context("failed to take PTY writer")?;
        let scrollback = Arc::new(Mutex::new(VecDeque::new()));
        let clients = Arc::new(Mutex::new(Vec::new()));
        let running = Arc::new(AtomicBool::new(true));

        let reader_thread = {
            let scrollback = Arc::clone(&scrollback);
            let clients = Arc::clone(&clients);
            let running = Arc::clone(&running);
            thread::spawn(move || {
                let mut buf = [0_u8; 8192];
                while running.load(Ordering::SeqCst) {
                    match reader.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => expose_broadcast_output(&scrollback, &clients, &buf[..n]),
                        Err(_) => break,
                    }
                }
                running.store(false, Ordering::SeqCst);
            })
        };
        let wait_thread = {
            let running = Arc::clone(&running);
            thread::spawn(move || {
                let _ = child.wait();
                running.store(false, Ordering::SeqCst);
            })
        };

        Ok(Self {
            master: Mutex::new(Some(pair.master)),
            writer: Arc::new(Mutex::new(Some(writer))),
            scrollback,
            clients,
            running,
            killer: Mutex::new(killer),
            reader_thread: Mutex::new(Some(reader_thread)),
            wait_thread: Mutex::new(Some(wait_thread)),
        })
    }

    pub(super) fn shutdown(&self) {
        self.running.store(false, Ordering::SeqCst);
        if let Ok(mut writer) = self.writer.lock() {
            drop(writer.take());
        }
        if let Ok(mut killer) = self.killer.lock() {
            let _ = killer.kill();
        }
        expose_join_thread(&self.wait_thread);
        if let Ok(mut master) = self.master.lock() {
            drop(master.take());
        }
        expose_join_thread(&self.reader_thread);
    }
}

impl Drop for ExposePty {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub(super) fn expose_join_thread(thread: &Mutex<Option<JoinHandle<()>>>) {
    if let Ok(mut thread) = thread.lock()
        && let Some(thread) = thread.take()
    {
        let _ = thread.join();
    }
}

pub(super) struct CloudflaredTunnel {
    pub(super) child: std::process::Child,
    pub(super) url: Option<String>,
    pub(super) effective_transport: Option<CloudflaredTransport>,
    reader_threads: Vec<JoinHandle<()>>,
    config: CloudflaredConfigIsolation,
    pub(super) startup_timed_out: bool,
    pub(super) startup_failure: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CloudflaredTransport {
    Auto,
    Quic,
    Http2,
}

impl CloudflaredTransport {
    fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Quic => "quic",
            Self::Http2 => "http2",
        }
    }
}

const CLOUDFLARED_TRANSPORT_NEGOTIATION_TIMEOUT: Duration = if cfg!(test) {
    if cfg!(windows) {
        Duration::from_secs(30)
    } else {
        Duration::from_secs(5)
    }
} else {
    Duration::from_secs(20)
};
const CLOUDFLARED_EVENT_PREFIX: &str = "\0prodex-cloudflared-transport=";
static NEXT_CLOUDFLARED_CONFIG_ID: AtomicU64 = AtomicU64::new(1);

struct CloudflaredConfigIsolation {
    directory: std::path::PathBuf,
    path: std::path::PathBuf,
    cleaned: bool,
}

impl CloudflaredConfigIsolation {
    fn create() -> Result<Self> {
        let unique_id = NEXT_CLOUDFLARED_CONFIG_ID.fetch_add(1, Ordering::Relaxed);
        let directory = (0..16)
            .map(|attempt| {
                std::env::temp_dir().join(format!(
                    "prodex-cloudflared-{}-{unique_id}-{attempt}",
                    std::process::id(),
                ))
            })
            .find(|directory| std::fs::create_dir(directory).is_ok())
            .context("failed to create private cloudflared config directory")?;
        #[cfg(unix)]
        std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o700))?;
        let path = directory.join("config.yaml");
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        options.mode(0o600);
        if let Err(error) = options.open(&path) {
            let _ = std::fs::remove_dir(&directory);
            return Err(error).context("failed to create private cloudflared config");
        }
        let metadata = match std::fs::symlink_metadata(&path) {
            Ok(metadata) => metadata,
            Err(error) => {
                let _ = std::fs::remove_file(&path);
                let _ = std::fs::remove_dir(&directory);
                return Err(error).context("failed to inspect private cloudflared config");
            }
        };
        if !metadata.file_type().is_file() {
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_dir(&directory);
            anyhow::bail!("private cloudflared config is not a regular file");
        }
        Ok(Self {
            directory,
            path,
            cleaned: false,
        })
    }

    fn cleanup(&mut self) {
        if self.cleaned {
            return;
        }
        self.cleaned = true;
        let _ = std::fs::remove_file(&self.path);
        let _ = std::fs::remove_dir(&self.directory);
    }
}

impl Drop for CloudflaredConfigIsolation {
    fn drop(&mut self) {
        self.cleanup();
    }
}

pub(super) fn cloudflared_command() -> Command {
    #[cfg(test)]
    if let Some(script) = std::env::var_os("PRODEX_TEST_CLOUDFLARED_SCRIPT") {
        let mut command = Command::new(if cfg!(windows) { "python" } else { "python3" });
        command.arg(script);
        return command;
    }
    Command::new("cloudflared")
}

impl CloudflaredTunnel {
    #[cfg(test)]
    pub(super) fn config_path(&self) -> &std::path::Path {
        &self.config.path
    }

    pub(super) fn exited(&mut self) -> Option<std::process::ExitStatus> {
        self.child.try_wait().ok().flatten()
    }

    pub(super) fn shutdown(&mut self) {
        let _ = crate::terminate_child_process_tree(&mut self.child, true);
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            match self.child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) => thread::sleep(Duration::from_millis(20)),
                Err(_) => break,
            }
        }
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        for thread in self.reader_threads.drain(..) {
            let _ = crate::join_thread_with_timeout(
                thread,
                Duration::from_secs(1),
                "cloudflared output reader",
            );
        }
        self.config.cleanup();
    }
}

impl Drop for CloudflaredTunnel {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub(super) fn start_cloudflared_tunnel(local_url: &str) -> Result<CloudflaredTunnel> {
    let mut auto = start_cloudflared_attempt(local_url, CloudflaredTransport::Auto)?;
    if auto.effective_transport.is_some() {
        return Ok(auto);
    }

    let auto_can_fallback = auto.url.is_some() && auto.startup_timed_out;
    if auto_can_fallback {
        auto.shutdown();
        crate::print_launch_status("Cloudflare QUIC unavailable; using HTTP/2 fallback.");
        let mut http2 = start_cloudflared_attempt(local_url, CloudflaredTransport::Http2)?;
        if http2.effective_transport.is_some() {
            return Ok(http2);
        }
        http2.startup_failure = Some(
            "Cloudflare edge transport unavailable: QUIC/UDP 7844 and HTTP/2/TCP 7844 failed"
                .to_string(),
        );
        http2.shutdown();
        return Ok(http2);
    }

    if auto.startup_failure.is_none() {
        auto.startup_failure =
            Some("Cloudflare Quick Tunnel did not register a transport connection".to_string());
    }
    auto.shutdown();
    Ok(auto)
}

fn start_cloudflared_attempt(
    local_url: &str,
    transport: CloudflaredTransport,
) -> Result<CloudflaredTunnel> {
    let config = CloudflaredConfigIsolation::create()?;
    let (mut child, rx, reader_threads) =
        cloudflared_startup::spawn(local_url, transport, &config)?;
    let startup = cloudflared_startup::wait(&mut child, &rx, transport);
    Ok(CloudflaredTunnel {
        child,
        url: startup.url,
        effective_transport: startup.effective_transport,
        reader_threads,
        config,
        startup_timed_out: startup.startup_timed_out,
        startup_failure: startup.startup_failure,
    })
}

pub(super) fn expose_scan_cloudflared_output<R>(
    reader: R,
    tx: mpsc::SyncSender<String>,
) -> JoinHandle<()>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut reader = io::BufReader::new(reader);
        let mut bytes = [0_u8; 1024];
        let mut line = Vec::with_capacity(EXPOSE_CLOUDFLARED_LINE_MAX_BYTES);
        loop {
            match reader.read(&mut bytes) {
                Ok(0) => break,
                Ok(size) => {
                    for byte in &bytes[..size] {
                        expose_scan_cloudflared_byte(&mut line, *byte, &tx);
                    }
                }
                Err(_) => break,
            }
        }
        expose_scan_cloudflared_line(&line, &tx);
    })
}

fn expose_scan_cloudflared_byte(line: &mut Vec<u8>, byte: u8, tx: &mpsc::SyncSender<String>) {
    if line.len() < EXPOSE_CLOUDFLARED_LINE_MAX_BYTES {
        line.push(byte);
    }
    if byte == b'\n' {
        expose_scan_cloudflared_line(line, tx);
        line.clear();
    }
}

fn expose_scan_cloudflared_line(line: &[u8], tx: &mpsc::SyncSender<String>) {
    let Ok(line) = std::str::from_utf8(line) else {
        return;
    };
    if line.contains("Registered tunnel connection") {
        let protocol = line.split_whitespace().find_map(|part| {
            part.strip_prefix("protocol=")
                .filter(|value| matches!(*value, "quic" | "http2"))
        });
        if let Some(protocol) = protocol {
            let _ = tx.try_send(format!("{CLOUDFLARED_EVENT_PREFIX}{protocol}"));
        }
    }
    if let Some(url) = expose_find_trycloudflare_url(line) {
        let _ = tx.try_send(url);
    }
}

pub(super) fn expose_find_trycloudflare_url(line: &str) -> Option<String> {
    line.split_whitespace().find_map(|part| {
        let candidate = part.trim_matches(|ch| matches!(ch, ',' | ';' | '(' | ')' | '[' | ']'));
        expose_public_host(candidate).map(|host| format!("https://{host}"))
    })
}

pub(super) fn expose_public_host(url: &str) -> Option<String> {
    if !url.is_ascii() {
        return None;
    }
    let authority = url
        .strip_prefix("https://")?
        .split(['/', '?', '#'])
        .next()?;
    if authority.contains([':', '@']) {
        return None;
    }
    let parsed = url::Url::parse(url).ok()?;
    if parsed.scheme() != "https"
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.port().is_some()
        || parsed.query().is_some()
        || parsed.fragment().is_some()
        || !matches!(parsed.path(), "" | "/")
    {
        return None;
    }
    let host = parsed.host_str()?.to_ascii_lowercase();
    (host.ends_with(".trycloudflare.com")
        && host != "trycloudflare.com"
        && expose_valid_dns_hostname(&host))
    .then_some(host)
}

pub(super) fn expose_valid_dns_hostname(host: &str) -> bool {
    hostname::expose_valid_dns_hostname(host)
}
