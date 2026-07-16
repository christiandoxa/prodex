use super::*;

pub(super) fn handle_expose(args: ExposeArgs) -> Result<()> {
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
    let mut tunnel_status = if tunnel_requested {
        "starting cloudflared".to_string()
    } else {
        "disabled by default; pass --tunnel to publish this remote shell".to_string()
    };
    let mut tunnel_url = None;
    let mut tunnel = if tunnel_requested {
        match start_cloudflared_tunnel(&format!("http://{listen_addr}")) {
            Ok(tunnel) => {
                if let Some(url) = &tunnel.url {
                    if let Some(host) = expose_public_host(url) {
                        shared.allow_host(host);
                    }
                    tunnel_status = "ready".to_string();
                    tunnel_url = Some(expose_access_url(url, &bootstrap));
                } else {
                    tunnel_status = "cloudflared started; public URL not reported".to_string();
                }
                Some(tunnel)
            }
            Err(err) => {
                tunnel_status = expose_tunnel_unavailable_status(&err);
                None
            }
        }
    } else {
        None
    };
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
    pub(super) pty: ExposePty,
    pub(super) shutdown: Arc<AtomicBool>,
    pub(super) active_clients: AtomicUsize,
    pub(super) active_requests: AtomicUsize,
    pub(super) peak_requests: AtomicUsize,
    pub(super) next_client_id: AtomicU64,
    pub(super) max_clients: usize,
}

impl ExposeShared {
    fn allow_host(&self, host: String) {
        if let Ok(mut hosts) = self.allowed_hosts.lock() {
            hosts.insert(host.to_ascii_lowercase());
        }
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
            while shared.pty.running.load(Ordering::SeqCst)
                && !shared.shutdown.load(Ordering::SeqCst)
            {
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
    pub(super) writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pub(super) scrollback: Arc<Mutex<VecDeque<u8>>>,
    pub(super) clients: Arc<Mutex<Vec<ExposeOutputClient>>>,
    pub(super) running: Arc<AtomicBool>,
    pub(super) killer: Mutex<Box<dyn ChildKiller + Send + Sync>>,
    pub(super) reader_thread: Mutex<Option<JoinHandle<()>>>,
    pub(super) wait_thread: Mutex<Option<JoinHandle<()>>>,
}

impl ExposePty {
    pub(super) fn spawn(args: &ExposeArgs) -> Result<Self> {
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
        if let Ok(cwd) = env::current_dir() {
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
            writer: Arc::new(Mutex::new(writer)),
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
        if let Ok(mut killer) = self.killer.lock() {
            let _ = killer.kill();
        }
        expose_join_thread(&self.wait_thread);
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
    child: std::process::Child,
    url: Option<String>,
    reader_threads: Vec<JoinHandle<()>>,
}

impl CloudflaredTunnel {
    pub(super) fn shutdown(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        for thread in self.reader_threads.drain(..) {
            let _ = thread.join();
        }
    }
}

impl Drop for CloudflaredTunnel {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub(super) fn start_cloudflared_tunnel(local_url: &str) -> Result<CloudflaredTunnel> {
    let mut child = Command::new("cloudflared")
        .args(["tunnel", "--protocol", "http2", "--url", local_url])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn cloudflared")?;
    let (tx, rx) = channel();
    let mut reader_threads = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        reader_threads.push(expose_scan_cloudflared_output(stdout, tx.clone()));
    }
    if let Some(stderr) = child.stderr.take() {
        reader_threads.push(expose_scan_cloudflared_output(stderr, tx));
    }
    let url = rx.recv_timeout(Duration::from_secs(12)).ok();
    Ok(CloudflaredTunnel {
        child,
        url,
        reader_threads,
    })
}

pub(super) fn expose_scan_cloudflared_output<R>(reader: R, tx: Sender<String>) -> JoinHandle<()>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut reader = io::BufReader::new(reader);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    if let Some(url) = expose_find_trycloudflare_url(&line) {
                        let _ = tx.send(url);
                    }
                }
                Err(_) => break,
            }
        }
    })
}

pub(super) fn expose_find_trycloudflare_url(line: &str) -> Option<String> {
    line.split_whitespace()
        .find(|part| part.starts_with("https://") && part.contains(".trycloudflare.com"))
        .map(|part| part.trim_matches(|ch| ch == ',' || ch == ';').to_string())
}

pub(super) fn expose_public_host(url: &str) -> Option<String> {
    let host = url.strip_prefix("https://")?.split('/').next()?;
    expose_valid_host(host).then(|| host.to_ascii_lowercase())
}
