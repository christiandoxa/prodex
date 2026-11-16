use super::{SocksTarget, negotiate};
use std::io::{self, Cursor, Read, Write};

#[derive(Default)]
struct ScriptedProxy {
    received: Vec<u8>,
    replies: Cursor<Vec<u8>>,
}
impl Read for ScriptedProxy {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.replies.read(bytes)
    }
}
impl Write for ScriptedProxy {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.received.extend(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn proxy(replies: &[u8]) -> ScriptedProxy {
    ScriptedProxy {
        replies: Cursor::new(replies.to_vec()),
        ..ScriptedProxy::default()
    }
}

#[test]
fn socks5h_sends_the_domain_without_local_resolution() {
    let mut stream = proxy(&[5, 0, 5, 0, 0, 1, 127, 0, 0, 1, 0, 0]);
    negotiate(
        &mut stream,
        &"socks5h://proxy.example:1080".parse().unwrap(),
        SocksTarget::Domain("upstream.example"),
        443,
    )
    .unwrap();
    let mut expected = vec![5, 1, 0, 5, 1, 0, 3, 16];
    expected.extend(b"upstream.example");
    expected.extend([1, 187]);
    assert_eq!(stream.received, expected);
    assert_eq!(
        stream.replies.position() as usize,
        stream.replies.get_ref().len()
    );
}

#[test]
fn socks5_auth_decodes_url_userinfo_and_preserves_literal_plus() {
    let mut stream = proxy(&[
        5, 2, 1, 0, 5, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0,
    ]);
    let url = "socks5://test%40user:p%3Ass+word@proxy.example:1080"
        .parse()
        .unwrap();
    negotiate(
        &mut stream,
        &url,
        SocksTarget::Address("192.0.2.1".parse().unwrap()),
        80,
    )
    .unwrap();
    let mut expected = vec![5, 2, 0, 2, 1, 9];
    expected.extend(b"test@user");
    expected.push(9);
    expected.extend(b"p:ss+word");
    expected.extend([5, 1, 0, 1, 192, 0, 2, 1, 0, 80]);
    assert_eq!(stream.received, expected);
}

#[test]
fn socks5_ipv6_and_domain_bound_responses_are_consumed_exactly() {
    let mut stream = proxy(&[5, 0, 5, 0, 0, 3, 1, b'x', 0, 1]);
    negotiate(
        &mut stream,
        &"socks5://proxy.example".parse().unwrap(),
        SocksTarget::Address("2001:db8::1".parse().unwrap()),
        8443,
    )
    .unwrap();
    assert_eq!(&stream.received[..7], &[5, 1, 0, 5, 1, 0, 4]);
    assert_eq!(
        &stream.received[7..23],
        &"2001:db8::1"
            .parse::<std::net::Ipv6Addr>()
            .unwrap()
            .octets()
    );
    assert_eq!(&stream.received[23..], &8443_u16.to_be_bytes());
    assert_eq!(stream.replies.position(), 10);
}

#[test]
fn socks5_proxy_denial_never_sends_a_target_request_or_leaks_credentials() {
    for replies in [&[5, 255][..], &[4, 0], &[5, 2, 1, 1]] {
        let mut stream = proxy(replies);
        let url = "socks5://private-user:private-password@proxy.example"
            .parse()
            .unwrap();
        let error = negotiate(
            &mut stream,
            &url,
            SocksTarget::Domain("upstream.example"),
            443,
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(!error.to_string().contains("private-"));
        assert!(
            !stream
                .received
                .windows(16)
                .any(|bytes| bytes == b"upstream.example")
        );
    }
}

#[test]
fn socks5_rejects_oversized_inputs_before_emitting_credentials() {
    let mut stream = ScriptedProxy::default();
    let url = format!("socks5://{}:password@proxy.example", "u".repeat(256))
        .parse()
        .unwrap();
    assert!(
        negotiate(
            &mut stream,
            &url,
            SocksTarget::Domain("upstream.example"),
            443
        )
        .is_err()
    );
    assert!(stream.received.is_empty());
    let url = "socks5://proxy.example".parse().unwrap();
    assert!(
        negotiate(
            &mut stream,
            &url,
            SocksTarget::Domain(&"x".repeat(256)),
            443
        )
        .is_err()
    );
    assert!(stream.received.is_empty());
}

#[test]
fn socks5_connect_refusal_and_malformed_replies_remain_transport_errors() {
    for replies in [
        &[5, 0, 5, 2, 0, 1][..],
        &[5, 0, 4, 0, 0, 1],
        &[5, 0, 5, 0, 1, 1],
        &[5, 0, 5, 0, 0, 9],
        &[5, 0, 5, 0],
    ] {
        let mut stream = proxy(replies);
        assert!(
            negotiate(
                &mut stream,
                &"socks5://proxy.example".parse().unwrap(),
                SocksTarget::Domain("upstream.example"),
                443
            )
            .is_err()
        );
    }
}
