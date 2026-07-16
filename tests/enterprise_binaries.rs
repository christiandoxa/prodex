use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::thread;

#[macro_use]
#[path = "support/enterprise_binaries_support.rs"]
mod enterprise_binaries_support;
use enterprise_binaries_support::{bin, closed_otlp_endpoint};

fn start_otlp_capture_server() -> (String, thread::JoinHandle<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind OTLP test listener");
    let addr = listener.local_addr().expect("resolve OTLP listener addr");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept OTLP export");
        let mut header_bytes = Vec::new();
        let mut byte = [0_u8; 1];
        while !header_bytes.ends_with(b"\r\n\r\n") {
            stream
                .read_exact(&mut byte)
                .expect("read OTLP request header byte");
            header_bytes.push(byte[0]);
        }
        let headers = String::from_utf8(header_bytes).expect("OTLP headers should be utf8");
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length").then(|| {
                    value
                        .trim()
                        .parse::<usize>()
                        .expect("OTLP content-length should parse")
                })
            })
            .expect("OTLP content-length header");
        let mut body = vec![0_u8; content_length];
        stream
            .read_exact(&mut body)
            .expect("read OTLP request body");
        stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
            .expect("write OTLP response");
        format!(
            "{}{}",
            headers,
            String::from_utf8(body).expect("OTLP body should be utf8")
        )
    });
    (format!("http://{addr}/v1/logs"), handle)
}

fn hanging_otlp_endpoint(delay_ms: u64) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind OTLP hanging listener");
    let addr = listener
        .local_addr()
        .expect("resolve OTLP hanging listener addr");
    let handle = thread::spawn(move || {
        let (_stream, _) = listener.accept().expect("accept OTLP hanging export");
        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
    });
    (format!("http://{addr}/v1/logs"), handle)
}

#[path = "enterprise_binaries/http_plan_admin_failures.rs"]
mod http_plan_admin_failures;
#[path = "enterprise_binaries/http_plan_billing.rs"]
mod http_plan_billing;
#[path = "enterprise_binaries/http_plan_core.rs"]
mod http_plan_core;
#[path = "enterprise_binaries/http_plan_idempotency.rs"]
mod http_plan_idempotency;
#[path = "enterprise_binaries/http_plan_identity.rs"]
mod http_plan_identity;
#[path = "enterprise_binaries/http_plan_preconditions.rs"]
mod http_plan_preconditions;
#[path = "enterprise_binaries/http_plan_route_failures.rs"]
mod http_plan_route_failures;
#[path = "enterprise_binaries/http_plan_scim.rs"]
mod http_plan_scim;
#[path = "enterprise_binaries/http_plan_tenant_audit.rs"]
mod http_plan_tenant_audit;
#[path = "enterprise_binaries/http_plan_virtual_key.rs"]
mod http_plan_virtual_key;
#[path = "enterprise_binaries/publication_compact.rs"]
mod publication_compact;
#[path = "enterprise_binaries/publication_delivery.rs"]
mod publication_delivery;
#[path = "enterprise_binaries/publication_plan.rs"]
mod publication_plan;
#[path = "enterprise_binaries/publication_transport.rs"]
mod publication_transport;
#[path = "enterprise_binaries/smoke_gateway.rs"]
mod smoke_gateway;
