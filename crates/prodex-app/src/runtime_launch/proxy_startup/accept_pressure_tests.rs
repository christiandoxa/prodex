//! A real local HTTP listener under EMFILE, isolated from the test runner.
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

const CHILD_ENV: &str = "PRODEX_TEST_LOCAL_ACCEPT_PRESSURE_0438";

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn local_proxy_accept_survives_descriptor_pressure_0438() {
    run_pressure_scenario("stream-split");
}

#[test]
fn local_proxy_accept_loop_survives_descriptor_pressure_0438() {
    run_pressure_scenario("accept");
}

fn run_pressure_scenario(phase: &str) {
    if std::env::var_os(CHILD_ENV).is_some() {
        pressure_child(phase);
        return;
    }
    let test_name = std::thread::current().name().unwrap().to_owned();
    let child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &test_name, "--nocapture", "--test-threads=1"])
        .env(CHILD_ENV, phase)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut child = OwnedChild(child);
    let stdout = child.0.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    let reader = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if sender.send(line.unwrap_or_default()).is_err() {
                break;
            }
        }
    });
    let address: SocketAddr = loop {
        let line = receiver
            .recv_timeout(Duration::from_secs(15))
            .expect("child readiness");
        if let Some((_, addr)) = line.split_once("ACCEPT_PRESSURE_ADDR=") {
            break addr.trim().parse().unwrap();
        }
    };
    let mut first = TcpStream::connect_timeout(&address, Duration::from_secs(3)).unwrap();
    first
        .set_read_timeout(Some(Duration::from_secs(8)))
        .unwrap();
    first
        .write_all(b"GET /first HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .unwrap();
    // The accept thread is waiting on an actual connection while the child
    // cannot allocate any descriptors. Its listener must survive that error.
    std::thread::sleep(Duration::from_millis(400));
    child
        .0
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"release\n")
        .unwrap();
    let mut reply = String::new();
    first
        .read_to_string(&mut reply)
        .expect("the queued connection survives EMFILE");
    assert!(reply.starts_with("HTTP/1.1 200"), "{reply}");
    assert!(reply.ends_with("recovered-0"), "{reply}");

    let mut second = TcpStream::connect_timeout(&address, Duration::from_secs(3))
        .expect("same listener address remains available after recovery");
    second
        .set_read_timeout(Some(Duration::from_secs(8)))
        .unwrap();
    second
        .write_all(b"GET /second HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut reply = String::new();
    second.read_to_string(&mut reply).unwrap();
    assert!(reply.starts_with("HTTP/1.1 200"), "{reply}");
    assert!(reply.ends_with("recovered-1"), "{reply}");
    let status = child.0.wait().unwrap();
    reader.join().unwrap();
    assert!(status.success());
}

fn pressure_child(phase: &str) {
    let mut listener = Some(TcpListener::bind("127.0.0.1:0").unwrap());
    let address = listener.as_ref().unwrap().local_addr().unwrap();
    let mut server = (phase == "stream-split")
        .then(|| tiny_http::Server::from_listener(listener.take().unwrap(), None).unwrap());
    let mut old = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // Only this separately spawned test process changes its soft FD limit.
    assert_eq!(unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut old) }, 0);
    let limit = libc::rlimit {
        rlim_cur: old.rlim_cur.min(96),
        rlim_max: old.rlim_max,
    };
    assert!(limit.rlim_cur >= 32);
    assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &limit) }, 0);
    let mut files = Vec::new();
    loop {
        match File::open("/dev/null") {
            Ok(file) => files.push(file),
            Err(error) => {
                assert_eq!(error.raw_os_error(), Some(libc::EMFILE));
                break;
            }
        }
    }
    if server.is_none() {
        server = Some(tiny_http::Server::from_listener(listener.take().unwrap(), None).unwrap());
    }
    let server = server.unwrap();
    println!("ACCEPT_PRESSURE_ADDR={address}");
    std::io::stdout().flush().unwrap();
    let mut command = String::new();
    std::io::stdin().read_line(&mut command).unwrap();
    assert_eq!(command.trim(), "release");
    drop(files);
    assert_eq!(unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &old) }, 0);
    for index in 0..2 {
        let request = server
            .recv_timeout(Duration::from_secs(6))
            .expect("listener must not surface a permanent accept failure")
            .expect("request after descriptor release");
        request
            .respond(tiny_http::Response::from_string(format!(
                "recovered-{index}"
            )))
            .unwrap();
    }
}
