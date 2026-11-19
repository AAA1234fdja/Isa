//! Staged bounded framing for a future host/guest broker; not a transport.
//!
//! Frames consist of a four-byte big-endian length and a nonempty payload.
//! Framing does not authenticate a peer, parse messages, enforce deadlines or
//! provide confidentiality. Callers must close a failed or truncated stream.

const HEADER: usize = 4;
const MAX_CHUNK: usize = 4096;
const MAX_FRAME: usize = 64 * 1024;
const MAX_TOTAL: usize = 4 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum FrameError {
    ChunkTooLarge,
    TotalExceeded,
    InvalidLength,
    Truncated,
    Failed,
    Closed,
}

/// Encodes frames into bounded buffers. A caller must separately implement
/// complete/partial writes, deadlines, cancellation and peer authentication.
pub(crate) struct Encoder {
    sent: usize,
    failed: bool,
}

impl Encoder {
    pub(crate) fn new() -> Self { Self { sent: 0, failed: false } }

    /// The total budget includes headers; an error permanently poisons the
    /// encoder so the caller cannot silently continue a damaged session.
    pub(crate) fn encode(&mut self, payload: &[u8]) -> Result<Vec<u8>, FrameError> {
        if self.failed { return Err(FrameError::Failed); }
        let result = self.encode_inner(payload);
        if result.is_err() { self.failed = true; }
        result
    }

    fn encode_inner(&mut self, payload: &[u8]) -> Result<Vec<u8>, FrameError> {
        if payload.is_empty() || payload.len() > MAX_FRAME { return Err(FrameError::InvalidLength); }
        let frame_size = HEADER + payload.len(); // bounded by MAX_FRAME above
        let total = self.sent.checked_add(frame_size).filter(|n| *n <= MAX_TOTAL).ok_or(FrameError::TotalExceeded)?;
        let mut frame = Vec::with_capacity(frame_size);
        frame.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        frame.extend_from_slice(payload);
        self.sent = total;
        Ok(frame)
    }
}

pub(crate) struct Decoder {
    header: [u8; HEADER],
    header_len: usize,
    expected: usize,
    payload: Vec<u8>,
    received: usize,
    failed: bool,
    closed: bool,
}

impl Decoder {
    pub(crate) fn new() -> Self {
        Self { header: [0; HEADER], header_len: 0, expected: 0, payload: Vec::new(), received: 0, failed: false, closed: false }
    }

    /// Call with at most one bounded read at a time. Returned frames include
    /// only payload bytes. On any error no frames from this call are returned
    /// and the decoder becomes unusable; earlier frames are not rolled back.
    pub(crate) fn push(&mut self, mut input: &[u8]) -> Result<Vec<Vec<u8>>, FrameError> {
        if self.failed { return Err(FrameError::Failed); }
        if self.closed { return Err(FrameError::Closed); }
        let result = self.decode(&mut input);
        if result.is_err() { self.failed = true; }
        result
    }

    fn decode(&mut self, input: &mut &[u8]) -> Result<Vec<Vec<u8>>, FrameError> {
        if input.len() > MAX_CHUNK { return Err(FrameError::ChunkTooLarge); }
        self.received = self.received.checked_add(input.len()).filter(|n| *n <= MAX_TOTAL).ok_or(FrameError::TotalExceeded)?;
        let mut frames = Vec::new();
        while !input.is_empty() {
            if self.header_len < HEADER {
                let count = (HEADER - self.header_len).min(input.len());
                self.header[self.header_len..self.header_len + count].copy_from_slice(&input[..count]);
                self.header_len += count;
                *input = &input[count..];
                if self.header_len < HEADER { break; }
                self.expected = u32::from_be_bytes(self.header) as usize;
                if self.expected == 0 || self.expected > MAX_FRAME { return Err(FrameError::InvalidLength); }
                self.payload = Vec::with_capacity(self.expected);
            }
            let count = (self.expected - self.payload.len()).min(input.len());
            self.payload.extend_from_slice(&input[..count]);
            *input = &input[count..];
            if self.payload.len() == self.expected {
                frames.push(std::mem::take(&mut self.payload));
                self.header_len = 0;
                self.expected = 0;
            }
        }
        Ok(frames)
    }

    /// A partial header or body is a truncated protocol message, never success.
    pub(crate) fn finish(&mut self) -> Result<(), FrameError> {
        if self.failed { return Err(FrameError::Failed); }
        if self.closed { return Err(FrameError::Closed); }
        if self.header_len != 0 {
            self.failed = true;
            return Err(FrameError::Truncated);
        }
        self.closed = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn frame(payload: &[u8]) -> Vec<u8> {
        let mut out = (payload.len() as u32).to_be_bytes().to_vec(); out.extend_from_slice(payload); out
    }
    #[test]
    fn encoder_round_trips_at_boundaries() {
        let mut encoder = Encoder::new(); let mut decoder = Decoder::new();
        for payload in [vec![1], vec![2; MAX_FRAME]] {
            let bytes = encoder.encode(&payload).unwrap(); let mut frames = Vec::new();
            for chunk in bytes.chunks(MAX_CHUNK) { frames.extend(decoder.push(chunk).unwrap()); }
            assert_eq!(frames, vec![payload]);
        }
        assert_eq!(decoder.finish(), Ok(()));
    }
    #[test]
    fn encoder_rejects_invalid_frames_and_exhaustion() {
        for payload in [Vec::new(), vec![0; MAX_FRAME + 1]] {
            let mut e = Encoder::new(); assert_eq!(e.encode(&payload), Err(FrameError::InvalidLength));
            assert_eq!(e.encode(b"ok"), Err(FrameError::Failed));
        }
        let mut e = Encoder::new(); let payload = vec![0; MAX_FRAME];
        for _ in 0..MAX_TOTAL / (MAX_FRAME + HEADER) { e.encode(&payload).unwrap(); }
        assert_eq!(e.encode(&payload), Err(FrameError::TotalExceeded));
        assert_eq!(e.encode(b"ok"), Err(FrameError::Failed));
    }
    #[test]
    fn all_chunk_boundaries_and_multiple_frames() {
        let mut bytes = frame(b"first"); bytes.extend(frame(b"second"));
        for n in 1..=bytes.len() {
            let mut d = Decoder::new(); let mut frames = Vec::new();
            for chunk in bytes.chunks(n) { frames.extend(d.push(chunk).unwrap()); }
            assert_eq!(frames, vec![b"first".to_vec(), b"second".to_vec()]); assert_eq!(d.finish(), Ok(()));
            assert_eq!(d.push(&frame(b"late")), Err(FrameError::Closed));
        }
    }
    #[test]
    fn rejects_bad_length_and_poisoned_stream() {
        for n in [0u32, MAX_FRAME as u32 + 1, u32::MAX] {
            let mut d = Decoder::new(); assert_eq!(d.push(&n.to_be_bytes()), Err(FrameError::InvalidLength));
            assert_eq!(d.push(&frame(b"ok")), Err(FrameError::Failed)); assert_eq!(d.finish(), Err(FrameError::Failed));
        }
    }
    #[test]
    fn detects_truncated_header_and_body() {
        for bytes in [&[0, 0][..], &[0, 0, 0, 2, 1][..]] {
            let mut d = Decoder::new(); d.push(bytes).unwrap(); assert_eq!(d.finish(), Err(FrameError::Truncated));
            assert_eq!(d.push(&[1]), Err(FrameError::Failed));
        }
    }
    #[test]
    fn enforces_chunk_and_lifetime_budget_including_headers() {
        let mut d = Decoder::new(); assert_eq!(d.push(&vec![0; MAX_CHUNK + 1]), Err(FrameError::ChunkTooLarge));
        let mut d = Decoder::new(); let f = frame(b"a");
        for _ in 0..MAX_TOTAL / f.len() { assert_eq!(d.push(&f).unwrap(), vec![b"a".to_vec()]); }
        assert_eq!(d.push(&f), Err(FrameError::TotalExceeded)); assert_eq!(d.finish(), Err(FrameError::Failed));
    }
    #[test]
    fn permits_maximum_frame_and_rejects_extra_large_prefix_before_allocating() {
        let data = vec![7; MAX_FRAME]; let encoded = frame(&data); let mut d = Decoder::new();
        let mut out = Vec::new(); for chunk in encoded.chunks(MAX_CHUNK) { out.extend(d.push(chunk).unwrap()); }
        assert_eq!(out, vec![data]); assert_eq!(d.finish(), Ok(()));
    }
}
