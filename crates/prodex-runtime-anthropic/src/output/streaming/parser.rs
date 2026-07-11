use super::*;

impl RuntimeAnthropicSseReader {
    fn process_upstream_event(&mut self) -> io::Result<()> {
        if self.upstream_data_lines.is_empty() {
            return Ok(());
        }
        let payload = self.upstream_data_lines.join("\n");
        self.upstream_data_lines.clear();
        let value = serde_json::from_str::<serde_json::Value>(&payload).map_err(|err| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("failed to parse runtime Responses SSE payload: {err}"),
            )
        })?;
        self.observe_upstream_event(&value);
        Ok(())
    }

    pub(super) fn observe_upstream_bytes(&mut self, chunk: &[u8]) -> io::Result<()> {
        for byte in chunk {
            self.upstream_line.push(*byte);
            if *byte != b'\n' {
                continue;
            }
            let line_text = String::from_utf8_lossy(&self.upstream_line);
            let trimmed = line_text.trim_end_matches(['\r', '\n']);
            if trimmed.is_empty() {
                self.process_upstream_event()?;
                self.upstream_line.clear();
                if self.inner_finished {
                    break;
                }
                continue;
            }
            if let Some(payload) = trimmed.strip_prefix("data:") {
                self.upstream_data_lines
                    .push(payload.trim_start().to_string());
            }
            self.upstream_line.clear();
        }
        Ok(())
    }
}
