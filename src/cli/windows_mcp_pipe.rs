//! Staged pipe pair for cancellable Windows MCP stdin. Not connected to MCP.
//! A private DACL and random name do not protect against another process running
//! as the same user or an administrator. Overlapped I/O and cancellation still
//! need an independently validated lifetime-safe implementation.
use std::{
    io,
    mem::zeroed,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle},
    ptr,
};
use windows_sys::Win32::{
    Foundation::{ERROR_IO_PENDING, ERROR_PIPE_CONNECTED, GENERIC_READ, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{
        CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED,
        OPEN_EXISTING, PIPE_ACCESS_OUTBOUND,
    },
    System::{
        Pipes::{
            ConnectNamedPipe, CreateNamedPipeW, GetNamedPipeClientProcessId, PIPE_READMODE_BYTE,
            PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_WAIT,
        },
        Threading::{CreateEventW, GetCurrentProcessId},
        IO::OVERLAPPED,
    },
};

/// Returns an overlapped server write handle and synchronous child read handle.
/// Both handles are non-inheritable. The child end must only be inherited via
/// the explicit handle list. This is not yet a complete MCP transport.
pub(super) fn stdin_pipe() -> io::Result<(OwnedHandle, OwnedHandle)> {
    let mut nonce = [0u8; 16];
    getrandom::getrandom(&mut nonce)
        .map_err(|e| io::Error::other(format!("pipe name randomness unavailable: {e}")))?;
    let suffix: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
    let name: Vec<u16> = format!(r"\\.\pipe\isa-mcp-{}-{suffix}", std::process::id())
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let descriptor = super::windows_private_acl::PrivateDescriptor::for_current_user(false)?;
    let attributes = descriptor.attributes();
    // SAFETY: name is NUL-terminated and the security descriptor remains live.
    let server = unsafe {
        CreateNamedPipeW(
            name.as_ptr(),
            PIPE_ACCESS_OUTBOUND | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            1,
            4096,
            4096,
            0,
            &attributes,
        )
    };
    if server == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful CreateNamedPipeW returned an owned handle.
    let server = unsafe { OwnedHandle::from_raw_handle(server as RawHandle) };
    // SAFETY: name and security attributes remain valid, and OPEN_EXISTING does
    // not create a filesystem object. The returned client handle is synchronous.
    let client = unsafe {
        CreateFileW(
            name.as_ptr(),
            GENERIC_READ,
            0,
            ptr::null(),
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            ptr::null_mut(),
        )
    };
    if client == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: successful CreateFileW returned a new owned handle.
    let client = unsafe { OwnedHandle::from_raw_handle(client as RawHandle) };
    // The client has connected before ConnectNamedPipe; Windows should report
    // ERROR_PIPE_CONNECTED. Do not leave an overlapped connection pending.
    let event = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
    if event.is_null() {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: CreateEventW returned an owned event handle.
    let event = unsafe { OwnedHandle::from_raw_handle(event as RawHandle) };
    // SAFETY: zero is a valid initial OVERLAPPED; event is live for the call.
    let mut overlapped: OVERLAPPED = unsafe { zeroed() };
    overlapped.hEvent = event.as_raw_handle() as _;
    let connected = unsafe { ConnectNamedPipe(server.as_raw_handle() as _, &mut overlapped) };
    if connected == 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() != Some(ERROR_PIPE_CONNECTED as i32) {
            // A pending operation must finish before its OVERLAPPED is dropped.
            if error.raw_os_error() == Some(ERROR_IO_PENDING as i32) {
                unsafe {
                    windows_sys::Win32::System::IO::CancelIoEx(
                        server.as_raw_handle() as _,
                        &overlapped,
                    );
                }
                let mut transferred = 0u32;
                // Preserving the OVERLAPPED lifetime may block if the OS does
                // not complete cancellation. This staged path is not enabled.
                unsafe {
                    windows_sys::Win32::System::IO::GetOverlappedResult(
                        server.as_raw_handle() as _,
                        &overlapped,
                        &mut transferred,
                        1,
                    );
                }
            }
            return Err(error);
        }
    }
    let mut client_pid = 0u32;
    // SAFETY: server owns a connected pipe, and client_pid is writable.
    if unsafe { GetNamedPipeClientProcessId(server.as_raw_handle() as _, &mut client_pid) } == 0 {
        return Err(io::Error::last_os_error());
    }
    if client_pid != unsafe { GetCurrentProcessId() } {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "unexpected pipe client",
        ));
    }
    Ok((server, client))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs::File, io::Read};
    use windows_sys::Win32::{
        Foundation::{GetHandleInformation, ERROR_IO_PENDING, HANDLE_FLAG_INHERIT},
        Storage::FileSystem::WriteFile,
        System::IO::GetOverlappedResult,
    };
    #[test]
    fn windows_mcp_pipe_is_noninheritable_and_transports_bytes() {
        let (writer, reader) = stdin_pipe().unwrap();
        for handle in [&writer, &reader] {
            let mut flags = 0;
            assert_ne!(
                unsafe { GetHandleInformation(handle.as_raw_handle() as _, &mut flags) },
                0
            );
            assert_eq!(flags & HANDLE_FLAG_INHERIT, 0);
        }
        let event = unsafe { CreateEventW(ptr::null(), 1, 0, ptr::null()) };
        assert!(!event.is_null());
        let event = unsafe { OwnedHandle::from_raw_handle(event as RawHandle) };
        let mut op: OVERLAPPED = unsafe { zeroed() };
        op.hEvent = event.as_raw_handle() as _;
        let bytes = b"hello\n";
        let mut written = 0;
        let ok = unsafe {
            WriteFile(
                writer.as_raw_handle() as _,
                bytes.as_ptr(),
                bytes.len() as u32,
                &mut written,
                &mut op,
            )
        };
        if ok == 0 {
            assert_eq!(
                io::Error::last_os_error().raw_os_error(),
                Some(ERROR_IO_PENDING as i32)
            );
            assert_ne!(
                unsafe { GetOverlappedResult(writer.as_raw_handle() as _, &op, &mut written, 1) },
                0
            );
        }
        assert_eq!(written, bytes.len() as u32);
        let mut reader = File::from(reader);
        let mut got = [0u8; 6];
        reader.read_exact(&mut got).unwrap();
        assert_eq!(&got, bytes);
    }
}
