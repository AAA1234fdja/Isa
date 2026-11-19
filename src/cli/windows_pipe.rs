//! Experimental nonblocking polling for anonymous Windows pipes.
//! Both original endpoints are non-inheritable.
use super::command_control;
use std::{
    fs::File,
    io::{self, Read},
    os::windows::io::{
        AsHandle, AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle, RawHandle,
    },
    ptr,
};
use windows_sys::Win32::{
    Foundation::{ERROR_BROKEN_PIPE, ERROR_NO_DATA, ERROR_PIPE_NOT_CONNECTED},
    System::Pipes::{CreatePipe, PeekNamedPipe},
};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum PipeState {
    Open,
    Closed,
}

pub(super) struct PipeReader {
    file: File,
}

/// Both returned endpoints are non-inheritable. Never pass all inheritable
/// process handles to a child; future integration needs an explicit handle list.
pub(super) fn pipe() -> io::Result<(PipeReader, OwnedHandle)> {
    let mut read = ptr::null_mut();
    let mut write = ptr::null_mut();
    // SAFETY: both out-pointers are valid; null security attributes make both
    // handles non-inheritable. Successful creation returns two owned handles.
    if unsafe { CreatePipe(&mut read, &mut write, ptr::null(), 0) } == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: handles were returned by successful CreatePipe and are distinct.
    let read = unsafe { OwnedHandle::from_raw_handle(read as RawHandle) };
    let write = unsafe { OwnedHandle::from_raw_handle(write as RawHandle) };
    Ok((
        PipeReader {
            file: File::from(read),
        },
        write,
    ))
}

impl PipeReader {
    pub(super) fn as_handle(&self) -> BorrowedHandle<'_> {
        self.file.as_handle()
    }
    /// Drain up to 64 KiB per call, preserving at most `limit` trailing bytes.
    /// There must be only one reader of this pipe; no duplicate read handles may
    /// consume between the peek and read. This is not a process-lifetime check.
    pub(super) fn drain(
        &mut self,
        tail: &mut Vec<u8>,
        limit: usize,
        truncated: &mut bool,
    ) -> io::Result<PipeState> {
        let mut buffer = [0u8; 4096];
        for _ in 0..16 {
            let mut available = 0u32;
            // SAFETY: file owns a live pipe handle and available is a valid out-pointer.
            let ok = unsafe {
                PeekNamedPipe(
                    self.file.as_raw_handle() as _,
                    ptr::null_mut(),
                    0,
                    ptr::null_mut(),
                    &mut available,
                    ptr::null_mut(),
                )
            };
            if ok == 0 {
                let error = io::Error::last_os_error();
                if matches!(error.raw_os_error(),Some(code) if code==ERROR_BROKEN_PIPE as i32 || code==ERROR_NO_DATA as i32 || code==ERROR_PIPE_NOT_CONNECTED as i32)
                {
                    return Ok(PipeState::Closed);
                }
                return Err(error);
            }
            if available == 0 {
                return Ok(PipeState::Open);
            }
            let count = (available as usize).min(buffer.len());
            let count = self.file.read(&mut buffer[..count])?;
            if count == 0 {
                return Ok(PipeState::Closed);
            }
            *truncated |= command_control::append_tail(tail, &buffer[..count], limit);
        }
        Ok(PipeState::Open)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use windows_sys::Win32::Foundation::{GetHandleInformation, HANDLE_FLAG_INHERIT};
    #[test]
    fn windows_pipe_endpoints_are_not_inheritable() {
        let (reader, writer) = pipe().unwrap();
        for handle in [reader.file.as_raw_handle(), writer.as_raw_handle()] {
            let mut flags = 0;
            // SAFETY: both handles remain owned and valid for the call.
            assert_ne!(unsafe { GetHandleInformation(handle as _, &mut flags) }, 0);
            assert_eq!(flags & HANDLE_FLAG_INHERIT, 0);
        }
    }
    #[test]
    fn windows_pipe_polling_drains_bounded_tail_and_detects_close() {
        let (mut reader, writer) = pipe().unwrap();
        let mut writer = File::from(writer);
        let mut output = Vec::new();
        let mut truncated = false;
        assert_eq!(
            reader.drain(&mut output, 4, &mut truncated).unwrap(),
            PipeState::Open
        );
        writer.write_all(b"abcdef").unwrap();
        assert_eq!(
            reader.drain(&mut output, 4, &mut truncated).unwrap(),
            PipeState::Open
        );
        assert_eq!(output, b"cdef");
        assert!(truncated);
        drop(writer);
        assert_eq!(
            reader.drain(&mut output, 4, &mut truncated).unwrap(),
            PipeState::Closed
        );
    }
}
