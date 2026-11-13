use super::*;
use std::io::{self, Cursor, Read, Write};
use tungstenite::protocol::Role;

#[derive(Default)]
struct RecordingIo {
    bytes: Vec<u8>,
    flushes: usize,
    fail_after_flushes: Option<usize>,
}

impl Read for RecordingIo {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::ErrorKind::WouldBlock.into())
    }
}

impl Write for RecordingIo {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self
            .fail_after_flushes
            .is_some_and(|limit| self.flushes >= limit)
        {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        Ok(())
    }
}

fn frame_headers(wire: &[u8]) -> Vec<(u8, bool, usize)> {
    let mut cursor = Cursor::new(wire);
    let mut frames = Vec::new();
    while cursor.position() < wire.len() as u64 {
        let mut header = [0; 2];
        cursor.read_exact(&mut header).unwrap();
        assert_eq!(header[0] & 0x70, 0, "no reserved frame bits");
        assert_ne!(header[1] & 0x80, 0, "client frames remain masked");
        let len = match header[1] & 0x7f {
            126 => {
                let mut bytes = [0; 2];
                cursor.read_exact(&mut bytes).unwrap();
                u16::from_be_bytes(bytes) as usize
            }
            127 => {
                let mut bytes = [0; 8];
                cursor.read_exact(&mut bytes).unwrap();
                usize::try_from(u64::from_be_bytes(bytes)).unwrap()
            }
            value => usize::from(value),
        };
        let mut mask = [0; 4];
        cursor.read_exact(&mut mask).unwrap();
        cursor.set_position(cursor.position() + len as u64);
        assert!(cursor.position() <= wire.len() as u64);
        frames.push((header[0] & 0x0f, header[0] & 0x80 != 0, len));
    }
    frames
}

#[test]
fn upstream_text_frame_boundaries_keep_one_masked_message_and_flush_each_frame() {
    for len in [
        0,
        1,
        UPSTREAM_TEXT_FRAME_BYTES,
        UPSTREAM_TEXT_FRAME_BYTES + 1,
        UPSTREAM_TEXT_FRAME_BYTES * 2,
        UPSTREAM_TEXT_FRAME_BYTES * 2 + 17,
    ] {
        let text = "x".repeat(len);
        let mut sender =
            tungstenite::WebSocket::from_raw_socket(RecordingIo::default(), Role::Client, None);
        send_text_with_bounded_frames(&mut sender, text.clone().into()).unwrap();
        let captured = sender.into_inner();
        let expected_count = len.div_ceil(UPSTREAM_TEXT_FRAME_BYTES).max(1);
        let frames = frame_headers(&captured.bytes);
        assert_eq!(frames.len(), expected_count);
        assert_eq!(captured.flushes, expected_count);
        for (index, (opcode, final_frame, bytes)) in frames.into_iter().enumerate() {
            assert_eq!(opcode, if index == 0 { 1 } else { 0 });
            assert_eq!(final_frame, index == expected_count - 1);
            assert!(bytes <= UPSTREAM_TEXT_FRAME_BYTES);
        }
        let mut receiver = tungstenite::WebSocket::from_raw_socket(
            Cursor::new(captured.bytes),
            Role::Server,
            None,
        );
        assert_eq!(receiver.read().unwrap(), WsMessage::Text(text.into()));
    }
}

#[test]
fn upstream_text_fragmentation_preserves_utf8_across_the_frame_boundary() {
    let text = format!("{}雪🙂tail", "x".repeat(UPSTREAM_TEXT_FRAME_BYTES - 1));
    let mut sender =
        tungstenite::WebSocket::from_raw_socket(RecordingIo::default(), Role::Client, None);
    send_text_with_bounded_frames(&mut sender, text.clone().into()).unwrap();
    let wire = sender.into_inner().bytes;
    assert_eq!(
        frame_headers(&wire),
        [(1, false, UPSTREAM_TEXT_FRAME_BYTES), (0, true, 10)]
    );
    let mut receiver =
        tungstenite::WebSocket::from_raw_socket(Cursor::new(wire), Role::Server, None);
    assert_eq!(receiver.read().unwrap(), WsMessage::Text(text.into()));
}

#[test]
fn upstream_text_fragment_send_failure_stops_before_the_next_fragment() {
    let io = RecordingIo {
        fail_after_flushes: Some(1),
        ..Default::default()
    };
    let mut sender = tungstenite::WebSocket::from_raw_socket(io, Role::Client, None);
    let err = send_text_with_bounded_frames(
        &mut sender,
        "x".repeat(UPSTREAM_TEXT_FRAME_BYTES * 3).into(),
    )
    .unwrap_err();
    assert!(
        matches!(err, tungstenite::Error::Io(error) if error.kind() == io::ErrorKind::BrokenPipe)
    );
    let io = sender.into_inner();
    assert_eq!(
        frame_headers(&io.bytes),
        [(1, false, UPSTREAM_TEXT_FRAME_BYTES)]
    );
    assert_eq!(io.flushes, 1);
}
