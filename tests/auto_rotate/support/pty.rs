use super::Fixture;
use std::fs::File;
use std::io::{Read, Write};
use std::os::fd::{FromRawFd, RawFd};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub(crate) struct PtyRunOutput {
    pub(crate) output: Output,
    pub(crate) tty_output: String,
}

pub(crate) fn run_prodex_with_pty_prompt_answer(
    fixture: &Fixture,
    args: &[&str],
    extra_env: &[(&str, &str)],
    prompt: &str,
    answer: &str,
) -> PtyRunOutput {
    let (master, slave) = open_pty();
    let slave_file = unsafe { File::from_raw_fd(slave) };
    let stdin_slave = slave_file
        .try_clone()
        .expect("failed to clone pty slave for stdin");
    let stderr_slave = slave_file
        .try_clone()
        .expect("failed to clone pty slave for stderr");

    let child = Command::new(env!("CARGO_BIN_EXE_prodex"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .env("PRODEX_HOME", &fixture.prodex_home)
        .env("PRODEX_SHARED_CODEX_HOME", &fixture.shared_codex_home)
        .env("PRODEX_CODEX_BIN", &fixture.codex_bin)
        .env("CODEX_CHATGPT_BASE_URL", &fixture.usage_base_url)
        .env("TEST_CODEX_LOG", &fixture.codex_log)
        .env("PRODEX_TEST_SKIP_BINARY_SHA256", "1")
        .env("PRODEX_RUNTIME_BROKER_READY_TIMEOUT_MS", "30000")
        .env("PRODEX_RUNTIME_BROKER_HEALTH_CONNECT_TIMEOUT_MS", "1500")
        .env("PRODEX_RUNTIME_BROKER_HEALTH_READ_TIMEOUT_MS", "3000")
        .env("PRODEX_RUNTIME_PROXY_HTTP_CONNECT_TIMEOUT_MS", "250")
        .env("PRODEX_RUNTIME_PROXY_STREAM_IDLE_TIMEOUT_MS", "250")
        .env("PRODEX_RUNTIME_PROXY_WEBSOCKET_CONNECT_TIMEOUT_MS", "250")
        .envs(extra_env.iter().copied())
        .args(args)
        .stdin(Stdio::from(stdin_slave))
        .stdout(Stdio::piped())
        .stderr(Stdio::from(stderr_slave))
        .spawn()
        .expect("failed to spawn prodex with pty");
    drop(slave_file);

    let mut master = master;
    let mut tty_output = String::new();
    read_until_prompt(&mut master, prompt, &mut tty_output);
    master
        .write_all(answer.as_bytes())
        .expect("failed to write prompt answer to pty");
    master
        .flush()
        .expect("failed to flush prompt answer to pty");

    let output = child
        .wait_with_output()
        .expect("failed to wait for prodex output");
    read_available(&mut master, &mut tty_output);
    PtyRunOutput { output, tty_output }
}

fn open_pty() -> (File, RawFd) {
    let mut master = -1;
    let mut slave = -1;
    let result = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    assert_eq!(result, 0, "openpty failed");
    set_nonblocking(master);
    let master = unsafe { File::from_raw_fd(master) };
    (master, slave)
}

fn set_nonblocking(fd: RawFd) {
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    assert!(flags >= 0, "fcntl F_GETFL failed");
    let result = unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
    assert_eq!(result, 0, "fcntl F_SETFL O_NONBLOCK failed");
}

fn read_until_prompt(master: &mut File, prompt: &str, output: &mut String) {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut buffer = [0; 256];
    while Instant::now() < deadline {
        match master.read(&mut buffer) {
            Ok(0) => break,
            Ok(count) => {
                output.push_str(&String::from_utf8_lossy(&buffer[..count]));
                if output.contains(prompt) {
                    return;
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
            }
            Err(err) => {
                panic!(
                    "failed to read pty output before prompt {prompt:?}: {err}; output was {output:?}"
                )
            }
        }
    }
    panic!("timed out waiting for prompt {prompt:?}; output was {output:?}");
}

fn read_available(master: &mut File, output: &mut String) {
    let mut buffer = [0; 1024];
    loop {
        match master.read(&mut buffer) {
            Ok(0) => return,
            Ok(count) => output.push_str(&String::from_utf8_lossy(&buffer[..count])),
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => return,
            Err(_) => return,
        }
    }
}
