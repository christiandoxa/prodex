use std::{
    io::{self, Read, Write},
    num::NonZeroUsize,
};

use bytes::Bytes;
use tokio::sync::mpsc;

pub struct GatewayInProcessUpgrade {
    inbound: mpsc::Receiver<Bytes>,
    outbound: mpsc::Sender<Bytes>,
    current: Bytes,
    offset: usize,
}

pub struct GatewayInProcessUpgradeHandoff {
    inbound: mpsc::Sender<Bytes>,
    outbound: mpsc::Receiver<Bytes>,
}

pub fn bounded_in_process_upgrade(
    capacity: NonZeroUsize,
) -> (GatewayInProcessUpgrade, GatewayInProcessUpgradeHandoff) {
    let (inbound_sender, inbound_receiver) = mpsc::channel(capacity.get());
    let (outbound_sender, outbound_receiver) = mpsc::channel(capacity.get());
    (
        GatewayInProcessUpgrade {
            inbound: inbound_receiver,
            outbound: outbound_sender,
            current: Bytes::new(),
            offset: 0,
        },
        GatewayInProcessUpgradeHandoff {
            inbound: inbound_sender,
            outbound: outbound_receiver,
        },
    )
}

impl GatewayInProcessUpgradeHandoff {
    pub(crate) fn into_channels(self) -> (mpsc::Sender<Bytes>, mpsc::Receiver<Bytes>) {
        (self.inbound, self.outbound)
    }
}

impl Read for GatewayInProcessUpgrade {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        while self.offset == self.current.len() {
            let Some(bytes) = self.inbound.blocking_recv() else {
                return Ok(0);
            };
            self.current = bytes;
            self.offset = 0;
        }
        let read = buffer.len().min(self.current.len() - self.offset);
        buffer[..read].copy_from_slice(&self.current[self.offset..self.offset + read]);
        self.offset += read;
        Ok(read)
    }
}

impl Write for GatewayInProcessUpgrade {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        self.outbound
            .blocking_send(Bytes::copy_from_slice(buffer))
            .map_err(|_| io::Error::new(io::ErrorKind::BrokenPipe, "gateway upgrade closed"))?;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
