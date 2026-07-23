use super::json;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub(crate) struct UsageServer {
    listen_addr: SocketAddr,
    shutdown: Arc<AtomicBool>,
    response_delay_ms: Arc<AtomicU64>,
    max_concurrent_requests: Arc<AtomicUsize>,
    thread: Option<JoinHandle<()>>,
}

impl UsageServer {
    pub(crate) fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind usage server");
        let listen_addr = listener
            .local_addr()
            .expect("failed to resolve usage server address");
        listener
            .set_nonblocking(true)
            .expect("failed to set usage server nonblocking");

        let shutdown = Arc::new(AtomicBool::new(false));
        let response_delay_ms = Arc::new(AtomicU64::new(0));
        let active_requests = Arc::new(AtomicUsize::new(0));
        let max_concurrent_requests = Arc::new(AtomicUsize::new(0));
        let shutdown_flag = Arc::clone(&shutdown);
        let response_delay_ms_flag = Arc::clone(&response_delay_ms);
        let active_requests_flag = Arc::clone(&active_requests);
        let max_concurrent_requests_flag = Arc::clone(&max_concurrent_requests);
        let thread = thread::spawn(move || {
            while !shutdown_flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let response_delay_ms_flag = Arc::clone(&response_delay_ms_flag);
                        let active_requests_flag = Arc::clone(&active_requests_flag);
                        let max_concurrent_requests_flag =
                            Arc::clone(&max_concurrent_requests_flag);
                        thread::spawn(move || {
                            handle_usage_request(
                                stream,
                                &response_delay_ms_flag,
                                &active_requests_flag,
                                &max_concurrent_requests_flag,
                            );
                        });
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(1));
                    }
                    Err(_) => break,
                }
            }
        });

        Self {
            listen_addr,
            shutdown,
            response_delay_ms,
            max_concurrent_requests,
            thread: Some(thread),
        }
    }

    pub(crate) fn base_url(&self) -> String {
        format!("http://{}/backend-api", self.listen_addr)
    }

    pub(crate) fn set_delay_ms(&self, delay_ms: u64) {
        self.response_delay_ms.store(delay_ms, Ordering::SeqCst);
        self.max_concurrent_requests.store(0, Ordering::SeqCst);
    }

    pub(crate) fn max_concurrent_requests(&self) -> usize {
        self.max_concurrent_requests.load(Ordering::SeqCst)
    }
}

impl Drop for UsageServer {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::SeqCst);
        let _ = TcpStream::connect(self.listen_addr);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn handle_usage_request(
    mut stream: TcpStream,
    response_delay_ms: &AtomicU64,
    active_requests: &AtomicUsize,
    max_concurrent_requests: &AtomicUsize,
) {
    let request = match read_http_request(&mut stream) {
        Some(request) => request,
        None => return,
    };

    let concurrent_requests = active_requests.fetch_add(1, Ordering::SeqCst) + 1;
    max_concurrent_requests.fetch_max(concurrent_requests, Ordering::SeqCst);

    let path = request
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .unwrap_or("/");
    let authorization = request_header(&request, "Authorization");
    let account_id = request_header(&request, "ChatGPT-Account-Id");

    let (status_line, body) =
        if !(path.ends_with("/backend-api/wham/usage") || path.ends_with("/api/codex/usage")) {
            (
                "HTTP/1.1 404 Not Found",
                json!({ "error": "not_found" }).to_string(),
            )
        } else {
            match (authorization.as_deref(), account_id.as_deref()) {
                (Some("Bearer test-token"), Some("main-account")) => {
                    ("HTTP/1.1 200 OK", main_usage_body())
                }
                (Some("Bearer test-token"), Some("second-account")) => {
                    ("HTTP/1.1 200 OK", second_usage_body())
                }
                (Some("Bearer test-token"), Some("third-account")) => {
                    ("HTTP/1.1 200 OK", third_usage_body())
                }
                (Some("Bearer test-token"), Some("spark-account")) => {
                    ("HTTP/1.1 200 OK", spark_usage_body())
                }
                (Some("Bearer test-token"), Some("elite-account")) => {
                    ("HTTP/1.1 200 OK", elite_usage_body())
                }
                _ => (
                    "HTTP/1.1 401 Unauthorized",
                    json!({ "error": "unauthorized" }).to_string(),
                ),
            }
        };

    let delay_ms = response_delay_ms.load(Ordering::SeqCst);
    if delay_ms > 0 {
        thread::sleep(Duration::from_millis(delay_ms));
    }

    let response = format!(
        "{status_line}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );

    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
    active_requests.fetch_sub(1, Ordering::SeqCst);
}

fn read_http_request(stream: &mut TcpStream) -> Option<String> {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let mut buffer = [0_u8; 1024];
    let mut request = Vec::new();

    loop {
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => {
                request.extend_from_slice(&buffer[..read]);
                if request.windows(4).any(|window| window == b"\r\n\r\n") {
                    break;
                }
            }
            Err(err)
                if matches!(
                    err.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                break;
            }
            Err(_) => return None,
        }
    }

    if request.is_empty() {
        return None;
    }

    Some(String::from_utf8_lossy(&request).into_owned())
}

fn request_header(request: &str, header_name: &str) -> Option<String> {
    request.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.trim().eq_ignore_ascii_case(header_name) {
            Some(value.trim().to_string())
        } else {
            None
        }
    })
}

fn future_epoch(offset_seconds: i64) -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_secs() as i64
        + offset_seconds
}

fn main_usage_body() -> String {
    json!({
        "email": "main@example.com",
        "plan_type": "plus",
        "rate_limit": {
            "primary_window": {
                "used_percent": 100,
                "reset_at": future_epoch(1_800),
                "limit_window_seconds": 18_000
            },
            "secondary_window": {
                "used_percent": 20,
                "reset_at": future_epoch(432_000),
                "limit_window_seconds": 604_800
            }
        }
    })
    .to_string()
}

fn second_usage_body() -> String {
    json!({
        "email": "second@example.com",
        "plan_type": "plus",
        "rate_limit": {
            "primary_window": {
                "used_percent": 20,
                "reset_at": future_epoch(7_200),
                "limit_window_seconds": 18_000
            },
            "secondary_window": {
                "used_percent": 30,
                "reset_at": future_epoch(518_400),
                "limit_window_seconds": 604_800
            }
        }
    })
    .to_string()
}

fn third_usage_body() -> String {
    json!({
        "email": "third@example.com",
        "plan_type": "plus",
        "rate_limit": {
            "primary_window": {
                "used_percent": 40,
                "reset_at": future_epoch(14_400),
                "limit_window_seconds": 18_000
            },
            "secondary_window": {
                "used_percent": 10,
                "reset_at": future_epoch(259_200),
                "limit_window_seconds": 604_800
            }
        }
    })
    .to_string()
}

fn spark_usage_body() -> String {
    json!({
        "email": "spark@example.com",
        "plan_type": "pro_lite",
        "rate_limit": {
            "primary_window": {
                "used_percent": 20,
                "reset_at": future_epoch(7_200),
                "limit_window_seconds": 18_000
            },
            "secondary_window": {
                "used_percent": 30,
                "reset_at": future_epoch(518_400),
                "limit_window_seconds": 604_800
            }
        },
        "additional_rate_limits": [
            {
                "limit_name": "GPT-5.3-Codex-Spark",
                "metered_feature": "codex_bengalfox",
                "rate_limit": {
                    "primary_window": {
                        "used_percent": 11,
                        "reset_at": future_epoch(18_000),
                        "limit_window_seconds": 18_000
                    },
                    "secondary_window": {
                        "used_percent": 3,
                        "reset_at": future_epoch(604_800),
                        "limit_window_seconds": 604_800
                    }
                }
            }
        ]
    })
    .to_string()
}

fn elite_usage_body() -> String {
    json!({
        "email": "elite@example.com",
        "plan_type": "team",
        "rate_limit": {
            "primary_window": {
                "used_percent": 1,
                "reset_at": future_epoch(3_600),
                "limit_window_seconds": 18_000
            },
            "secondary_window": {
                "used_percent": 1,
                "reset_at": future_epoch(172_800),
                "limit_window_seconds": 604_800
            }
        }
    })
    .to_string()
}
