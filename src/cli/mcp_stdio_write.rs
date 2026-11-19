//! Bounded outgoing frame bookkeeping; no OS I/O, cancellation, or retries.
use std::io;

pub(super) struct Outgoing {
    bytes: Vec<u8>,
    sent: usize,
}
impl Outgoing {
    pub(super) fn new(mut bytes: Vec<u8>, limit: usize) -> io::Result<Self> {
        if bytes.len() > limit || bytes.contains(&b'\n') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid MCP request frame",
            ));
        }
        bytes.push(b'\n');
        Ok(Self { bytes, sent: 0 })
    }
    pub(super) fn pending(&self) -> &[u8] {
        &self.bytes[self.sent..]
    }
    pub(super) fn dispatched(&self) -> bool {
        self.sent > 0
    }
    pub(super) fn advance(&mut self, n: usize) -> io::Result<()> {
        if n == 0 || n > self.pending().len() {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "invalid MCP write progress",
            ));
        }
        self.sent += n;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mcp_stdio_write_tracks_partial_dispatch_and_limits() {
        let mut f = Outgoing::new(b"{}".to_vec(), 2).unwrap();
        assert!(!f.dispatched());
        assert_eq!(f.pending(), b"{}\n");
        f.advance(1).unwrap();
        assert!(f.dispatched());
        assert_eq!(f.pending(), b"}\n");
        assert!(f.advance(0).is_err());
        assert!(f.advance(3).is_err());
        f.advance(2).unwrap();
        assert!(f.pending().is_empty());
        assert!(Outgoing::new(b"123".to_vec(), 2).is_err());
        assert!(Outgoing::new(b"a\nb".to_vec(), 8).is_err());
    }
}
