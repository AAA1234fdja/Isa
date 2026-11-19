//! Framing for the staged Windows undo store, not cryptographic integrity.
//! The trailer detects an incomplete sequential write; it cannot authenticate a
//! record, detect same-length corruption, or guarantee crash durability.
use std::io;
const HEADER: &[u8; 8] = b"ISAUND01";
const TRAILER: &[u8; 8] = b"ENDUNDO1";
const OVERHEAD: usize = 24;
pub(super) const MAX_PAYLOAD: usize = 8 * 1024 * 1024;
pub(super) const MAX_FRAME: usize = MAX_PAYLOAD + OVERHEAD;

pub(super) fn encode(payload: &[u8]) -> io::Result<Vec<u8>> {
    if payload.len() > MAX_PAYLOAD { return Err(invalid("undo record too large")); }
    let mut frame = Vec::with_capacity(payload.len() + OVERHEAD);
    frame.extend_from_slice(HEADER);
    frame.extend_from_slice(&(payload.len() as u64).to_le_bytes());
    frame.extend_from_slice(payload);
    frame.extend_from_slice(TRAILER);
    Ok(frame)
}
pub(super) fn decode(frame: &[u8]) -> io::Result<&[u8]> {
    if frame.len() < OVERHEAD || frame.len() > MAX_FRAME || &frame[..8] != HEADER { return Err(invalid("invalid undo record frame")); }
    let length = u64::from_le_bytes(frame[8..16].try_into().expect("length is checked"));
    if length > MAX_PAYLOAD as u64 || length as usize != frame.len() - OVERHEAD || &frame[frame.len()-8..] != TRAILER {
        return Err(invalid("incomplete or invalid undo record"));
    }
    Ok(&frame[16..frame.len()-8])
}
fn invalid(message: &'static str) -> io::Error { io::Error::new(io::ErrorKind::InvalidData, message) }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_undo_frame_rejects_truncation_and_invalid_lengths() {
        for payload in [&b""[..], &b"example\0contents"[..]] {
            let frame=encode(payload).unwrap();assert_eq!(decode(&frame).unwrap(),payload);
            for end in 0..frame.len() {assert!(decode(&frame[..end]).is_err(),"accepted prefix {end}");}
            let mut extended=frame.clone();extended.push(0);assert!(decode(&extended).is_err());
            let mut header=frame.clone();header[0]^=1;assert!(decode(&header).is_err());
            let mut length=frame.clone();length[8..16].copy_from_slice(&u64::MAX.to_le_bytes());assert!(decode(&length).is_err());
            let mut trailer=frame.clone();*trailer.last_mut().unwrap()^=1;assert!(decode(&trailer).is_err());
        }
        assert!(encode(&vec![0;MAX_PAYLOAD+1]).is_err());
    }
}
