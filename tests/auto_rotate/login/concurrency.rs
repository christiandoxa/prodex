use super::*;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Child;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

// Pause just the first identity response. Quota requests remain independently
// serviceable, so a blocked quota command proves local lifecycle-lock contention.
struct PausedUsage {
    url: String,
    paused: mpsc::Receiver<()>,
    release: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl PausedUsage {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/backend-api", listener.local_addr().unwrap());
        listener.set_nonblocking(true).unwrap();
        let release = Arc::new(AtomicBool::new(false));
        let stop = Arc::new(AtomicBool::new(false));
        let first = Arc::new(AtomicBool::new(false));
        let (tx, paused) = mpsc::channel();
        let release_worker = release.clone();
        let stop_worker = stop.clone();
        let worker = thread::spawn(move || {
            let mut handlers = Vec::new();
            while !stop_worker.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let release = release_worker.clone();
                        let first = first.clone();
                        let tx = tx.clone();
                        handlers.push(thread::spawn(move || reply(stream, release, first, tx)));
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
            for handler in handlers {
                handler.join().unwrap();
            }
        });
        Self {
            url,
            paused,
            release,
            stop,
            worker: Some(worker),
        }
    }
}
impl Drop for PausedUsage {
    fn drop(&mut self) {
        self.release.store(true, Ordering::SeqCst);
        self.stop.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}
fn reply(
    mut stream: TcpStream,
    release: Arc<AtomicBool>,
    first: Arc<AtomicBool>,
    tx: mpsc::Sender<()>,
) {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut request = Vec::new();
    let mut chunk = [0; 2048];
    while !request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => request.extend_from_slice(&chunk[..n]),
        }
    }
    let usage = String::from_utf8_lossy(&request)
        .lines()
        .next()
        .is_some_and(|line| line.contains("/wham/usage "));
    if usage && !first.swap(true, Ordering::SeqCst) {
        let _ = tx.send(());
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        while !release.load(Ordering::SeqCst) && std::time::Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
    }
    let body = if usage {
        json!({"email":"main@example.com","plan_type":"plus","rate_limit":{
            "primary_window":{"used_percent":25,"limit_window_seconds":18000,"reset_at":1900000000}
        }})
        .to_string()
    } else {
        json!({"error":"not_found"}).to_string()
    };
    let status = if usage { "200 OK" } else { "404 Not Found" };
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}

fn start_paused_login(fixture: &Fixture, server: &PausedUsage) -> OwnedChild {
    let mut state = read_state(&fixture.prodex_home);
    state["profiles"].as_object_mut().unwrap().remove("second");
    write_json(&fixture.prodex_home.join("state.json"), &state);
    let token = chatgpt_id_token("main@example.com");
    let child = OwnedChild(spawn_prodex_with_env(
        fixture,
        &["login"],
        &[
            ("CODEX_CHATGPT_BASE_URL", server.url.as_str()),
            ("TEST_LOGIN_ID_TOKEN", token.as_str()),
            ("TEST_LOGIN_ACCOUNT_ID", "main-account"),
        ],
    ));
    server
        .paused
        .recv_timeout(Duration::from_secs(10))
        .expect("legacy identity lookup must reach fixture");
    child
}
fn wait_success(child: &mut OwnedChild, label: &str) {
    let status = crate::test_wait::wait_for_poll(
        label,
        Duration::from_secs(10),
        Duration::from_millis(10),
        || {
            child
                .0
                .try_wait()
                .expect("child status must remain readable")
        },
    );
    assert!(status.success(), "{label}: {status}");
}

#[test]
fn quota_completes_while_legacy_login_identity_is_waiting() {
    let fixture = setup_fixture();
    let server = PausedUsage::start();
    let mut login = start_paused_login(&fixture, &server);
    let mut quota = OwnedChild(spawn_prodex_with_env(
        &fixture,
        &["quota", "--provider", "openai", "--once"],
        &[("CODEX_CHATGPT_BASE_URL", server.url.as_str())],
    ));
    wait_success(
        &mut quota,
        "independent quota must not wait for login identity",
    );
    assert!(
        login.0.try_wait().unwrap().is_none(),
        "login fixture must still be paused"
    );
    server.release.store(true, Ordering::SeqCst);
    wait_success(&mut login, "login after identity response");
    assert_eq!(read_state(&fixture.prodex_home)["active_profile"], "main");
}

#[test]
fn login_rechecks_disk_identity_before_overwriting_profile_credentials() {
    let fixture = setup_fixture();
    let server = PausedUsage::start();
    let mut login = start_paused_login(&fixture, &server);
    let replacement = json!({"tokens":{
        "id_token": chatgpt_id_token("replacement@example.com"),
        "access_token":"synthetic-replacement-token", "account_id":"replacement-account"
    }});
    write_json(&fixture.main_home.join("auth.json"), &replacement);
    server.release.store(true, Ordering::SeqCst);
    wait_success(&mut login, "login after concurrent identity replacement");
    let current: Value =
        serde_json::from_slice(&fs::read(fixture.main_home.join("auth.json")).unwrap()).unwrap();
    assert_eq!(
        current, replacement,
        "a stale identity lookup must not overwrite another account"
    );
    assert_eq!(
        read_state(&fixture.prodex_home)["active_profile"],
        "main_example.com"
    );
}
