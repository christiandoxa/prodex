//! Bounded SOCKS5 negotiation. DNS ownership is selected before entering this encoder.
use std::io::{self, Read, Write};
use std::net::IpAddr;
use zeroize::Zeroizing;

pub(crate) enum SocksTarget<'a> {
    Address(IpAddr),
    Domain(&'a str),
}

pub(crate) fn negotiate(
    stream: &mut (impl Read + Write),
    proxy: &reqwest::Url,
    target: SocksTarget<'_>,
    port: u16,
) -> io::Result<()> {
    let username =
        Zeroizing::new(percent_encoding::percent_decode_str(proxy.username()).collect::<Vec<_>>());
    let password = Zeroizing::new(
        percent_encoding::percent_decode_str(proxy.password().unwrap_or_default())
            .collect::<Vec<_>>(),
    );
    if username.len() > 255 || password.len() > 255 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "SOCKS5 credentials exceed protocol limits",
        ));
    }
    let mut request = vec![5, 1, 0];
    match target {
        SocksTarget::Address(IpAddr::V4(ip)) => {
            request.push(1);
            request.extend(ip.octets());
        }
        SocksTarget::Address(IpAddr::V6(ip)) => {
            request.push(4);
            request.extend(ip.octets());
        }
        SocksTarget::Domain(host) => {
            let size = u8::try_from(host.len())
                .ok()
                .filter(|size| *size > 0)
                .ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "invalid SOCKS5 target name length",
                    )
                })?;
            request.extend([3, size]);
            request.extend(host.as_bytes());
        }
    }
    request.extend(port.to_be_bytes());
    stream.write_all(if username.is_empty() {
        &[5, 1, 0]
    } else {
        &[5, 2, 0, 2]
    })?;
    stream.flush()?;
    let mut selection = [0; 2];
    stream.read_exact(&mut selection)?;
    match selection {
        [5, 0] => {}
        [5, 2] if !username.is_empty() => {
            let mut auth = Zeroizing::new(vec![1, username.len() as u8]);
            auth.extend_from_slice(&username);
            auth.push(password.len() as u8);
            auth.extend_from_slice(&password);
            stream.write_all(&auth)?;
            stream.flush()?;
            let mut reply = [0; 2];
            stream.read_exact(&mut reply)?;
            if reply != [1, 0] {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "SOCKS5 proxy authentication failed",
                ));
            }
        }
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "SOCKS5 proxy rejected authentication methods",
            ));
        }
    }
    stream.write_all(&request)?;
    stream.flush()?;
    let mut reply = [0; 4];
    stream.read_exact(&mut reply)?;
    if reply[0] != 5 || reply[2] != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid SOCKS5 proxy response",
        ));
    }
    if reply[1] != 0 {
        return Err(io::Error::new(
            io::ErrorKind::ConnectionRefused,
            format!("SOCKS5 proxy CONNECT failed with code {}", reply[1]),
        ));
    }
    let address_length = match reply[3] {
        1 => 4,
        4 => 16,
        3 => {
            let mut length = [0];
            stream.read_exact(&mut length)?;
            usize::from(length[0])
        }
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid SOCKS5 bound address type",
            ));
        }
    };
    let mut bound = [0; 257];
    stream.read_exact(&mut bound[..address_length + 2])?;
    Ok(())
}

#[cfg(test)]
#[path = "socks_tests.rs"]
mod tests;
