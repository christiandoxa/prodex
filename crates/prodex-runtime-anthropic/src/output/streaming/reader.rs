use super::*;

impl Read for RuntimeAnthropicSseReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let read = buf.len().min(self.pending.len());
            if read > 0 {
                for (index, byte) in self.pending.drain(..read).enumerate() {
                    buf[index] = byte;
                }
                return Ok(read);
            }
            if self.inner_finished {
                return Ok(0);
            }

            let mut upstream_buffer = [0_u8; 8192];
            let read = self.inner.read(&mut upstream_buffer)?;
            if read == 0 {
                self.finish_success();
                continue;
            }
            self.observe_upstream_bytes(&upstream_buffer[..read])?;
        }
    }
}
