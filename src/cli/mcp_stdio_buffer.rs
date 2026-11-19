//! Bounded newline framing for MCP stdio. This is not a process transport and
//! does not parse JSON or establish trust in server messages.
use std::io;

pub(super) struct LineBuffer {
    bytes: Vec<u8>,
    received: usize,
    max_frame: usize,
    max_total: usize,
}
impl LineBuffer {
    pub(super) fn new(max_frame: usize, max_total: usize) -> Self {
        Self {
            bytes: Vec::new(),
            received: 0,
            max_frame,
            max_total,
        }
    }
    /// The caller must pop complete lines before reading further. Bound each
    /// read to 4096 bytes so allocation is at most max_frame plus one chunk.
    pub(super) fn push(&mut self, chunk: &[u8]) -> io::Result<()> {
        if chunk.len() > 4096
            || self.bytes.len() > self.max_frame
            || self
                .received
                .checked_add(chunk.len())
                .is_none_or(|n| n > self.max_total)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "MCP response budget exceeded",
            ));
        }
        self.received += chunk.len();
        self.bytes.extend_from_slice(chunk);
        Ok(())
    }
    pub(super) fn next_line(&mut self) -> io::Result<Option<Vec<u8>>> {
        if let Some(end) = self.bytes.iter().position(|b| *b == b'\n') {
            if end > self.max_frame {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "MCP response frame exceeds limit",
                ));
            }
            return Ok(Some(self.bytes.drain(..=end).collect()));
        }
        if self.bytes.len() > self.max_frame {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "MCP response frame exceeds limit",
            ));
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mcp_stdio_buffer_handles_split_and_multiple_lines() {
        let mut b = LineBuffer::new(16, 64);
        b.push(b"first").unwrap();
        assert!(b.next_line().unwrap().is_none());
        b.push(b" line\nsecond\nlast").unwrap();
        assert_eq!(b.next_line().unwrap().unwrap(), b"first line\n");
        assert_eq!(b.next_line().unwrap().unwrap(), b"second\n");
        assert!(b.next_line().unwrap().is_none());
        b.push(b"\n").unwrap();
        assert_eq!(b.next_line().unwrap().unwrap(), b"last\n");
    }
    #[test]
    fn mcp_stdio_buffer_preserves_arbitrary_chunk_boundaries() {
        let stream = b"{}\n{\"message\":\"hello\"}\n\n{\"id\":42}\n";
        let expected: Vec<Vec<u8>> = stream
            .split_inclusive(|b| *b == b'\n')
            .map(|line| line.to_vec())
            .collect();
        for size in 1..=stream.len() {
            let mut buffer = LineBuffer::new(64, 1024);
            let mut lines = Vec::new();
            for chunk in stream.chunks(size) {
                buffer.push(chunk).unwrap();
                while let Some(line) = buffer.next_line().unwrap() {
                    lines.push(line);
                }
            }
            assert_eq!(lines, expected, "chunk size {size}");
            assert!(buffer.next_line().unwrap().is_none());
        }
    }
    #[test]
    fn mcp_stdio_buffer_fails_closed_at_frame_and_total_limits() {
        let mut b = LineBuffer::new(4, 16);
        b.push(b"1234\n").unwrap();
        assert_eq!(b.next_line().unwrap().unwrap(), b"1234\n");
        b.push(b"12345\n").unwrap();
        assert!(b.next_line().is_err());
        let mut b = LineBuffer::new(4, 6);
        b.push(b"a\n").unwrap();
        assert!(b.next_line().unwrap().is_some());
        b.push(b"b\n").unwrap();
        assert!(b.next_line().unwrap().is_some());
        assert!(b.push(b"abc").is_err());
        let mut b = LineBuffer::new(4, 9999);
        b.push(b"12345").unwrap();
        assert!(b.next_line().is_err());
        assert!(b.push(b"a").is_err());
        assert!(LineBuffer::new(8000, 8000).push(&vec![0; 4097]).is_err());
    }
}
