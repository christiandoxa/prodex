use super::super_expose::handle_super_expose;
use super::*;
use anyhow::bail;
#[path = "runtime/cloudflared.rs"]
mod cloudflared;
mod cloudflared_existing;
mod cloudflared_startup;
#[cfg(test)]
#[path = "runtime/config_isolation_tests.rs"]
mod config_isolation_tests;
mod hostname;
#[path = "runtime/openai_tunnel.rs"]
mod openai_tunnel;
pub(super) use cloudflared::*;
pub(super) use cloudflared_existing::*;
pub(super) use openai_tunnel::*;
use std::net::SocketAddr;
#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle};

pub(super) fn handle_expose(args: ExposeArgs) -> Result<()> {
    if args.invocation == prodex_cli::ExposeInvocation::SuperAlias {
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
    if let Err(err) = cloudflared::ensure_cloudflared_available() {
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
    process_id: Option<u32>,
    process_tree_terminated: Arc<AtomicBool>,
    #[cfg(windows)]
    process_job: Mutex<Option<OwnedHandle>>,
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
        command.env_remove("CONTROL_PLANE_API_KEY");
        if let Some(cwd) = cwd {
            command.cwd(cwd.as_os_str());
        }
        let mut child = pair
            .slave
            .spawn_command(command)
            .context("failed to spawn exposed shell")?;
        let process_id = child.process_id();
        #[cfg(windows)]
        // ponytail: nested Windows jobs may reject assignment; taskkill remains
        // the bounded fallback until a breakaway policy is needed.
        let process_job = child
            .as_raw_handle()
            .and_then(|handle| assign_expose_process_job(handle).ok());
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
        let process_tree_terminated = Arc::new(AtomicBool::new(false));

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
            let process_tree_terminated = Arc::clone(&process_tree_terminated);
            thread::spawn(move || {
                let _ = child.wait();
                terminate_expose_pty_process_tree_once(process_id, &process_tree_terminated);
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
            process_id,
            process_tree_terminated,
            #[cfg(windows)]
            process_job: Mutex::new(process_job),
            reader_thread: Mutex::new(Some(reader_thread)),
            wait_thread: Mutex::new(Some(wait_thread)),
        })
    }

    pub(super) fn shutdown(&self) {
        self.running.store(false, Ordering::SeqCst);
        if let Ok(mut writer) = self.writer.lock() {
            drop(writer.take());
        }
        terminate_expose_pty_process_tree_once(self.process_id, &self.process_tree_terminated);
        if let Ok(mut killer) = self.killer.lock() {
            let _ = killer.kill();
        }
        #[cfg(windows)]
        if let Ok(mut process_job) = self.process_job.lock() {
            drop(process_job.take());
        }
        expose_join_thread(&self.wait_thread);
        if let Ok(mut master) = self.master.lock() {
            drop(master.take());
        }
        expose_join_thread(&self.reader_thread);
    }
}

#[cfg(windows)]
fn assign_expose_process_job(raw_handle: RawHandle) -> std::io::Result<OwnedHandle> {
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
        SetInformationJobObject,
    };

    // SAFETY: null name and security descriptor request a new unnamed job.
    let raw_job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if raw_job.is_null() {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: raw_job is a new owned kernel handle returned by CreateJobObjectW.
    let job = unsafe { OwnedHandle::from_raw_handle(raw_job) };
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    // SAFETY: job is live, limits points to the initialized structure, and the
    // API is called with the matching structure size and information class.
    let configured = unsafe {
        SetInformationJobObject(
            job.as_raw_handle(),
            JobObjectExtendedLimitInformation,
            std::ptr::from_ref(&limits).cast(),
            std::mem::size_of_val(&limits) as u32,
        )
    } != 0;
    if !configured {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: both handles are live and owned by this process.
    if unsafe { AssignProcessToJobObject(job.as_raw_handle(), raw_handle) } == 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(job)
}

impl Drop for ExposePty {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn terminate_expose_pty_process_tree_once(process_id: Option<u32>, terminated: &AtomicBool) {
    if !terminated.swap(true, Ordering::SeqCst) {
        terminate_expose_pty_process_tree(process_id);
    }
}

#[cfg(unix)]
fn terminate_expose_pty_process_tree(process_id: Option<u32>) {
    let Some(process_id) = process_id
        .and_then(|process_id| libc::pid_t::try_from(process_id).ok())
        .filter(|process_id| *process_id > 0)
    else {
        return;
    };
    // portable-pty creates the shell as a session/process-group leader. Signal
    // only that owned group; never use a broad process-name kill.
    let _ = unsafe { libc::kill(-process_id, libc::SIGKILL) };
}

#[cfg(windows)]
fn terminate_expose_pty_process_tree(process_id: Option<u32>) {
    let Some(process_id) = process_id else {
        return;
    };
    let _ = std::process::Command::new("taskkill")
        .args(["/PID", &process_id.to_string(), "/T", "/F"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
}

#[cfg(not(any(unix, windows)))]
fn terminate_expose_pty_process_tree(_process_id: Option<u32>) {}

pub(super) fn expose_join_thread(thread: &Mutex<Option<JoinHandle<()>>>) {
    if let Ok(mut thread) = thread.lock()
        && let Some(thread) = thread.take()
    {
        let _ = thread.join();
    }
}

pub(super) fn expose_display_name(
    requested: Option<&str>,
    workspace_root: &std::path::Path,
) -> anyhow::Result<String> {
    let name = requested
        .or_else(|| workspace_root.file_name().and_then(|name| name.to_str()))
        .unwrap_or("workspace");
    if name.is_empty()
        || name.len() > 64
        || name.as_bytes().contains(&0)
        || name.chars().any(|ch| ch.is_control())
    {
        bail!("expose name must be 1-64 characters without control characters")
    }
    Ok(name.to_string())
}
