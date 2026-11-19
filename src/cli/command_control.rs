//! Platform-neutral pieces shared by bounded command supervisors.
//! These helpers do not spawn, contain, or terminate a process.

#[derive(Debug, PartialEq, Eq)]
pub(super) enum StopReason { Cancelled, TimedOut }

pub(super) fn stop_reason(cancelled: bool, deadline_reached: bool) -> Option<StopReason> {
    if cancelled { Some(StopReason::Cancelled) }
    else if deadline_reached { Some(StopReason::TimedOut) }
    else { None }
}

/// Retain at most `limit` trailing bytes; return whether any bytes were lost.
/// Memory use is bounded even when a single read exceeds the limit.
pub(super) fn append_tail(buffer: &mut Vec<u8>, data: &[u8], limit: usize) -> bool {
    if data.len() >= limit {
        let lost = !buffer.is_empty() || data.len() > limit;
        buffer.clear();
        buffer.extend_from_slice(&data[data.len()-limit..]);
        return lost;
    }
    let excess = buffer.len().saturating_add(data.len()).saturating_sub(limit);
    if excess > 0 { buffer.drain(..excess); }
    buffer.extend_from_slice(data);
    excess > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn command_control_keeps_only_tail_and_reports_loss() {
        let mut data=Vec::new();
        assert!(!append_tail(&mut data,b"ab",4));assert_eq!(data,b"ab");
        assert!(!append_tail(&mut data,b"cd",4));assert_eq!(data,b"abcd");
        assert!(append_tail(&mut data,b"ef",4));assert_eq!(data,b"cdef");
        assert!(append_tail(&mut data,b"0123456789",4));assert_eq!(data,b"6789");
        assert!(!append_tail(&mut Vec::new(),b"1234",4));
        assert!(append_tail(&mut data,b"x",0));assert!(data.is_empty());
        assert!(!append_tail(&mut data,b"",0));
    }
    #[test]
    fn command_control_matches_reference_across_chunk_sizes() {
        for limit in 0..24 {
            let mut retained=Vec::new();let mut reference=Vec::new();let mut lost=false;
            for size in 0..40 {
                let chunk:Vec<u8>=(0..size).map(|i|((i+size)%251) as u8).collect();
                lost |= append_tail(&mut retained,&chunk,limit);
                reference.extend_from_slice(&chunk);
                assert_eq!(retained,reference[reference.len().saturating_sub(limit)..]);
                assert_eq!(lost,reference.len()>limit);
            }
        }
    }
    #[test]
    fn command_control_classifies_timeout_and_cancel_without_process_claims() {
        assert_eq!(stop_reason(false,false),None);
        assert_eq!(stop_reason(false,true),Some(StopReason::TimedOut));
        assert_eq!(stop_reason(true,false),Some(StopReason::Cancelled));
        assert_eq!(stop_reason(true,true),Some(StopReason::Cancelled));
    }
}
