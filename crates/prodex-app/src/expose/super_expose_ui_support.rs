use super::PublicMcpEndpoint;
use base64::Engine;
use std::io::{self, Write};

pub(super) fn visible_url(url: &str, offset: usize, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let characters = url.chars().collect::<Vec<_>>();
    if characters.len() <= width {
        return url.to_string();
    }
    let requested_start = offset.min(characters.len());
    let at_end = requested_start == characters.len();
    let prefix_len = usize::from(requested_start > 0);
    let tail_len = usize::from(!at_end);
    let body_len = width.saturating_sub(prefix_len + tail_len);
    let start = if at_end {
        characters.len().saturating_sub(body_len)
    } else {
        requested_start
    };
    let end = start.saturating_add(body_len).min(characters.len());
    let mut output = String::with_capacity(width);
    if prefix_len != 0 {
        output.push('<');
    }
    output.extend(characters[start..end].iter());
    if end < characters.len() {
        output.push('>');
    }
    output
}

pub(super) fn copy_public_url_to_clipboard(public_url: &PublicMcpEndpoint) -> io::Result<()> {
    copy_public_url_to_clipboard_with(public_url, write_clipboard_escape)
}

pub(super) fn copy_public_url_to_clipboard_with(
    public_url: &PublicMcpEndpoint,
    write_clipboard: impl FnOnce(&[u8]) -> io::Result<()>,
) -> io::Result<()> {
    write_clipboard(public_url.as_str().as_bytes())
}

fn write_clipboard_escape(bytes: &[u8]) -> io::Result<()> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
    let mut stderr = io::stderr().lock();
    stderr.write_all(b"\x1b]52;c;")?;
    stderr.write_all(encoded.as_bytes())?;
    stderr.write_all(b"\x07")?;
    stderr.flush()
}
