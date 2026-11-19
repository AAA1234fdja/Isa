//! Staged overlapped pipe writes for MCP. Not connected to any MCP client.
//! Timeout/cancellation cannot prove that the server did not receive bytes or
//! perform side effects. Every error consumes the connection; never retry it.
use std::{
    io,
    mem::zeroed,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle},
    ptr,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{ERROR_IO_PENDING, WAIT_OBJECT_0, WAIT_TIMEOUT},
    Storage::FileSystem::WriteFile,
    System::{
        Threading::{CreateEventW, WaitForSingleObject},
        IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED},
    },
};
const MAX_FRAME: usize = 128 * 1024 + 1;
const CHUNK: usize = 4096;
const MAX_OPERATIONS: usize = 8;
static OPERATIONS: AtomicUsize = AtomicUsize::new(0);

#[derive(Debug, PartialEq, Eq)]
pub(super) enum FailureKind {
    Invalid,
    Busy,
    Cancelled,
    TimedOut,
    Io,
}
#[derive(Debug)]
pub(super) struct WriteFailure {
    pub(super) kind: FailureKind,
    // Conservative: true once any write was submitted, even if later cancelled.
    pub(super) possibly_dispatched: bool,
    pub(super) detail: String,
}
fn failure(
    kind: FailureKind,
    possibly_dispatched: bool,
    detail: impl Into<String>,
) -> WriteFailure {
    WriteFailure {
        kind,
        possibly_dispatched,
        detail: detail.into(),
    }
}

struct Slot;
impl Slot {
    fn acquire() -> io::Result<Self> {
        OPERATIONS
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
                if n < MAX_OPERATIONS {
                    Some(n + 1)
                } else {
                    None
                }
            })
            .map(|_| Self)
            .map_err(|_| {
                io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "MCP write operation capacity exhausted",
                )
            })
    }
}
impl Drop for Slot {
    fn drop(&mut self) {
        OPERATIONS.fetch_sub(1, Ordering::AcqRel);
    }
}

// Heap addresses of both the OVERLAPPED and its data remain stable until the
// kernel signals completion. Owns the only parent-side writer for this call.
struct Operation {
    writer: Option<OwnedHandle>,
    event: OwnedHandle,
    overlapped: OVERLAPPED,
    data: Vec<u8>,
    submitted: bool,
    _slot: Slot,
}
struct Pending(Option<Box<Operation>>);
impl Pending {
    fn new(writer: OwnedHandle, data: Vec<u8>) -> io::Result<Self> {
        let slot = Slot::acquire()?;
        // SAFETY: unnamed, non-inheritable event; success returns an owned handle.
        let raw = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
        if raw.is_null() {
            return Err(io::Error::last_os_error());
        }
        let event = unsafe { OwnedHandle::from_raw_handle(raw as RawHandle) };
        // SAFETY: zero is valid initialization for OVERLAPPED.
        let mut overlapped: OVERLAPPED = unsafe { zeroed() };
        overlapped.hEvent = event.as_raw_handle() as _;
        Ok(Self(Some(Box::new(Operation {
            writer: Some(writer),
            event,
            overlapped,
            data,
            submitted: false,
            _slot: slot,
        }))))
    }
    fn start(&mut self) -> io::Result<()> {
        let op = self.0.as_mut().expect("pending operation exists");
        let handle = op.writer.as_ref().unwrap().as_raw_handle() as _;
        // SAFETY: op and its buffer are heap-allocated and retained until the
        // completion event is signalled; this is an overlapped-capable handle.
        let ok = unsafe {
            WriteFile(
                handle,
                op.data.as_ptr(),
                op.data.len() as u32,
                ptr::null_mut(),
                &mut op.overlapped,
            )
        };
        if ok != 0 {
            op.submitted = true;
            return Ok(());
        }
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(ERROR_IO_PENDING as i32) {
            op.submitted = true;
            Ok(())
        } else {
            Err(error)
        }
    }
    fn wait(&self, millis: u32) -> io::Result<bool> {
        let op = self.0.as_ref().unwrap();
        // SAFETY: event remains owned by this operation.
        match unsafe { WaitForSingleObject(op.event.as_raw_handle() as _, millis) } {
            WAIT_OBJECT_0 => Ok(true),
            WAIT_TIMEOUT => Ok(false),
            _ => Err(io::Error::last_os_error()),
        }
    }
    fn finish(mut self) -> io::Result<(OwnedHandle, usize)> {
        let op = self.0.as_mut().unwrap();
        let mut written = 0u32;
        // Called only after the operation's dedicated event is signalled.
        let ok = unsafe {
            GetOverlappedResult(
                op.writer.as_ref().unwrap().as_raw_handle() as _,
                &op.overlapped,
                &mut written,
                0,
            )
        };
        let error = if ok == 0 {
            Some(io::Error::last_os_error())
        } else {
            None
        };
        op.submitted = false;
        if let Some(error) = error {
            return Err(error);
        }
        if written == 0 || written as usize > op.data.len() {
            return Err(io::Error::new(
                io::ErrorKind::WriteZero,
                "invalid overlapped write result",
            ));
        }
        Ok((op.writer.take().unwrap(), written as usize))
    }
    fn cancel(&self) {
        if let Some(op) = self.0.as_ref() {
            if op.submitted {
                // SAFETY: operation and both handles remain live through this call.
                unsafe {
                    CancelIoEx(
                        op.writer.as_ref().unwrap().as_raw_handle() as _,
                        &op.overlapped,
                    );
                }
            }
        }
    }
}
impl Drop for Pending {
    fn drop(&mut self) {
        let Some(op) = self.0.take() else {
            return;
        };
        if op.submitted {
            // The kernel may still reference the OVERLAPPED and its buffer.
            unsafe {
                CancelIoEx(
                    op.writer.as_ref().unwrap().as_raw_handle() as _,
                    &op.overlapped,
                );
            }
            let done =
                unsafe { WaitForSingleObject(op.event.as_raw_handle() as _, 100) } == WAIT_OBJECT_0;
            if !done {
                // Deliberate bounded quarantine: retain memory, handles and slot
                // rather than cause use-after-free. At most MAX_OPERATIONS such
                // objects can exist; exhaustion makes future writes fail closed.
                std::mem::forget(op);
                return;
            }
        }
    }
}

/// Write one already-framed request. Consumes the writer and returns it only on
/// complete success. On failure the caller must stop the server/session; bytes
/// could already have been read. A timeout is not proof of non-execution.
pub(super) fn write_frame(
    mut writer: OwnedHandle,
    frame: &[u8],
    timeout: Duration,
    cancel: &AtomicBool,
) -> Result<OwnedHandle, WriteFailure> {
    if frame.is_empty() || frame.len() > MAX_FRAME || timeout.is_zero() {
        return Err(failure(
            FailureKind::Invalid,
            false,
            "invalid MCP frame or timeout",
        ));
    }
    let deadline = Instant::now()
        .checked_add(timeout)
        .ok_or_else(|| failure(FailureKind::Invalid, false, "timeout too large"))?;
    let mut offset = 0usize;
    let mut dispatched = false;
    while offset < frame.len() {
        if cancel.load(Ordering::SeqCst) {
            return Err(failure(
                FailureKind::Cancelled,
                dispatched,
                "MCP write cancelled",
            ));
        }
        if Instant::now() >= deadline {
            return Err(failure(
                FailureKind::TimedOut,
                dispatched,
                "MCP write timed out",
            ));
        }
        let end = (offset + CHUNK).min(frame.len());
        let mut pending = Pending::new(writer, frame[offset..end].to_vec()).map_err(|e| {
            failure(
                if e.kind() == io::ErrorKind::WouldBlock {
                    FailureKind::Busy
                } else {
                    FailureKind::Io
                },
                dispatched,
                e.to_string(),
            )
        })?;
        pending
            .start()
            .map_err(|e| failure(FailureKind::Io, dispatched, e.to_string()))?;
        dispatched = true;
        loop {
            if cancel.load(Ordering::SeqCst) {
                pending.cancel();
                return Err(failure(
                    FailureKind::Cancelled,
                    dispatched,
                    "MCP write cancelled; server may have received data",
                ));
            }
            let now = Instant::now();
            if now >= deadline {
                pending.cancel();
                return Err(failure(
                    FailureKind::TimedOut,
                    dispatched,
                    "MCP write timed out; server may have received data",
                ));
            }
            let wait = deadline
                .saturating_duration_since(now)
                .as_millis()
                .clamp(1, 20) as u32;
            if pending
                .wait(wait)
                .map_err(|e| failure(FailureKind::Io, dispatched, e.to_string()))?
            {
                break;
            }
        }
        let (next, n) = pending
            .finish()
            .map_err(|e| failure(FailureKind::Io, dispatched, e.to_string()))?;
        writer = next;
        offset += n;
    }
    Ok(writer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs::File, io::Read, sync::Arc};
    #[test]
    fn windows_mcp_write_sends_bytes_and_keeps_connection() {
        let (writer, reader) = super::super::windows_mcp_pipe::stdin_pipe().unwrap();
        let writer = write_frame(
            writer,
            b"first\n",
            Duration::from_secs(5),
            &AtomicBool::new(false),
        )
        .unwrap();
        let writer = write_frame(
            writer,
            b"second\n",
            Duration::from_secs(5),
            &AtomicBool::new(false),
        )
        .unwrap();
        let mut reader = File::from(reader);
        let mut bytes = [0u8; 13];
        reader.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"first\nsecond\n");
        drop(writer);
    }
    #[test]
    fn windows_mcp_write_times_out_when_peer_never_reads() {
        let (writer, _reader) = super::super::windows_mcp_pipe::stdin_pipe().unwrap();
        let started = Instant::now();
        let failure = write_frame(
            writer,
            &vec![b'x'; MAX_FRAME],
            Duration::from_millis(200),
            &AtomicBool::new(false),
        )
        .unwrap_err();
        assert_eq!(failure.kind, FailureKind::TimedOut);
        assert!(failure.possibly_dispatched);
        assert!(started.elapsed() < Duration::from_secs(3));
    }
    #[test]
    fn windows_mcp_write_respects_precancel_and_running_cancel() {
        let (writer, _reader) = super::super::windows_mcp_pipe::stdin_pipe().unwrap();
        let failure = write_frame(
            writer,
            b"data",
            Duration::from_secs(5),
            &AtomicBool::new(true),
        )
        .unwrap_err();
        assert_eq!(failure.kind, FailureKind::Cancelled);
        assert!(!failure.possibly_dispatched);
        let (writer, _reader) = super::super::windows_mcp_pipe::stdin_pipe().unwrap();
        let cancel = Arc::new(AtomicBool::new(false));
        let signal = cancel.clone();
        let watcher = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            signal.store(true, Ordering::SeqCst);
        });
        let failure = write_frame(
            writer,
            &vec![b'x'; MAX_FRAME],
            Duration::from_secs(10),
            &cancel,
        )
        .unwrap_err();
        watcher.join().unwrap();
        assert_eq!(failure.kind, FailureKind::Cancelled);
    }
    #[test]
    fn windows_mcp_write_rejects_invalid_sizes_without_dispatch() {
        let (writer, _reader) = super::super::windows_mcp_pipe::stdin_pipe().unwrap();
        let error = write_frame(
            writer,
            &vec![0; MAX_FRAME + 1],
            Duration::from_secs(1),
            &AtomicBool::new(false),
        )
        .unwrap_err();
        assert_eq!(error.kind, FailureKind::Invalid);
        assert!(!error.possibly_dispatched);
    }
}
