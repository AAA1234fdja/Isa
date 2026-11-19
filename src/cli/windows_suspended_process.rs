//! Experimental suspended-process primitive used by opt-in Windows execution.
//! This is not a sandbox: children use the server user's Windows privileges.
//!
//! The caller provides an absolute program path and independent arguments.
//! Windows CRT-style argument quoting is not shell escaping or approval.
//! Standard handles use a staged explicit list. Only SystemRoot is supplied
//! in the child environment. Handle inheritance still has concurrency risks.
//! Production use requires further design and tests.
use super::{
    command_control::{stop_reason, StopReason},
    windows_command_line,
    windows_job::WindowsJob,
    windows_pipe::{self, PipeReader, PipeState},
    windows_process_attributes::{inheritable_duplicate, HandleList},
};
use std::{
    ffi::{OsStr, OsString},
    io,
    mem::{size_of, zeroed},
    os::windows::{
        ffi::OsStrExt,
        io::{AsHandle, AsRawHandle, FromRawHandle, OwnedHandle, RawHandle},
    },
    path::Path,
    ptr,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
use windows_sys::Win32::Foundation::{WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows_sys::Win32::System::Threading::{
    CreateProcessW, GetExitCodeProcess, ResumeThread, TerminateProcess, WaitForSingleObject,
    CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, EXTENDED_STARTUPINFO_PRESENT,
    PROCESS_INFORMATION, STARTF_USESTDHANDLES, STARTUPINFOEXW,
};

fn wide(value: &OsStr) -> io::Result<Vec<u16>> {
    let mut result: Vec<u16> = value.encode_wide().collect();
    if result.contains(&0) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "embedded NUL in Windows argument",
        ));
    }
    result.push(0);
    Ok(result)
}

/// Raw bytes are untrusted and may be truncated or incomplete. Cancellation
/// and timeout are requests, not proof that every external effect was undone.
pub(super) struct CapturedProcess {
    pub(super) exit_code: u32,
    pub(super) timed_out: bool,
    pub(super) cancelled: bool,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
    pub(super) truncated: bool,
    pub(super) pipes_incomplete: bool,
}

pub(super) struct SuspendedProcess {
    job: WindowsJob,
    process: OwnedHandle,
    thread: OwnedHandle,
    resumed: bool,
    stdout: PipeReader,
    stderr: PipeReader,
}
impl SuspendedProcess {
    pub(super) fn create(program: &Path, args: &[OsString], root: &Path) -> io::Result<Self> {
        if !program.is_absolute() || !root.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "absolute program and workspace paths are required",
            ));
        }
        let program_units: Vec<u16> = program.as_os_str().encode_wide().collect();
        let arg_units: Vec<Vec<u16>> = args.iter().map(|arg| arg.encode_wide().collect()).collect();
        let mut command_line = windows_command_line::build(&program_units, &arg_units)?;
        let program = wide(program.as_os_str())?;
        let root = wide(root.as_os_str())?;
        let system_root = std::env::var_os("SystemRoot")
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "SystemRoot is unavailable"))?;
        let mut environment: Vec<u16> = "SystemRoot=".encode_utf16().collect();
        environment.extend(wide(&system_root)?);
        environment.push(0); // double-NUL terminator
        let job = WindowsJob::new()?;
        let (input_reader, input_writer) = windows_pipe::pipe()?;
        let (stdout, output_writer) = windows_pipe::pipe()?;
        let (stderr, error_writer) = windows_pipe::pipe()?;
        let child_in = inheritable_duplicate(input_reader.as_handle())?;
        let child_out = inheritable_duplicate(output_writer.as_handle())?;
        let child_err = inheritable_duplicate(error_writer.as_handle())?;
        let attributes = HandleList::new([&child_in, &child_out, &child_err])?;
        // SAFETY: zero initializes the Win32 structs; cb covers STARTUPINFOEXW.
        let mut startup: STARTUPINFOEXW = unsafe { zeroed() };
        startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = child_in.as_raw_handle() as _;
        startup.StartupInfo.hStdOutput = child_out.as_raw_handle() as _;
        startup.StartupInfo.hStdError = child_err.as_raw_handle() as _;
        startup.lpAttributeList = attributes.as_ptr();
        let mut information: PROCESS_INFORMATION = unsafe { zeroed() };
        // SAFETY: the explicit handle list and strings remain valid for the
        // call; only the listed inheritable duplicates are passed to this child.
        let ok = unsafe {
            CreateProcessW(
                program.as_ptr(),
                command_line.as_mut_ptr(),
                ptr::null(),
                ptr::null(),
                1,
                CREATE_SUSPENDED
                    | CREATE_NO_WINDOW
                    | CREATE_UNICODE_ENVIRONMENT
                    | EXTENDED_STARTUPINFO_PRESENT,
                environment.as_ptr().cast(),
                root.as_ptr(),
                &startup.StartupInfo,
                &mut information,
            )
        };
        // Capture the error before cleanup can overwrite the thread's last error.
        let error = if ok == 0 {
            Some(io::Error::last_os_error())
        } else {
            None
        };
        drop(attributes);
        drop(child_in);
        drop(child_out);
        drop(child_err);
        drop(input_reader);
        drop(input_writer);
        drop(output_writer);
        drop(error_writer);
        if let Some(error) = error {
            return Err(error);
        }
        // SAFETY: successful CreateProcessW returns owned process and thread handles.
        let process = unsafe { OwnedHandle::from_raw_handle(information.hProcess as RawHandle) };
        let thread = unsafe { OwnedHandle::from_raw_handle(information.hThread as RawHandle) };
        let result = Self {
            job,
            process,
            thread,
            resumed: false,
            stdout,
            stderr,
        };
        // SAFETY: result owns the valid process handle and its primary thread is
        // still suspended. On failure Drop attempts to terminate the process.
        unsafe {
            result.job.assign(result.process.as_raw_handle())?;
        }
        Ok(result)
    }
    pub(super) fn resume(&mut self) -> io::Result<()> {
        if self.resumed {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "process already resumed",
            ));
        }
        // SAFETY: this is the primary thread handle returned by CreateProcessW.
        let previous = unsafe { ResumeThread(self.thread.as_raw_handle() as _) };
        if previous == u32::MAX {
            return Err(io::Error::last_os_error());
        }
        if previous != 1 {
            return Err(io::Error::other("unexpected suspend count"));
        }
        self.resumed = true;
        Ok(())
    }
    pub(super) fn terminate(&self) -> io::Result<()> {
        self.job.terminate()
    }
    pub(super) fn drain_output(
        &mut self,
        out: &mut Vec<u8>,
        err: &mut Vec<u8>,
        limit: usize,
        truncated: &mut bool,
    ) -> io::Result<(PipeState, PipeState)> {
        Ok((
            self.stdout.drain(out, limit, truncated)?,
            self.stderr.drain(err, limit, truncated)?,
        ))
    }
    /// Runs a previously created suspended process. Callers must enforce
    /// approval and handle the remaining handle-inheritance race limitations.
    pub(super) fn run(
        mut self,
        timeout: Duration,
        cancel: &AtomicBool,
    ) -> io::Result<CapturedProcess> {
        if cancel.load(Ordering::SeqCst) {
            return Err(io::Error::other("command was stopped before starting"));
        }
        if timeout.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "timeout must be positive",
            ));
        }
        let deadline = Instant::now()
            .checked_add(timeout)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "timeout too large"))?;
        self.resume()?;
        let mut out = Vec::new();
        let mut err = Vec::new();
        let mut truncated = false;
        let mut timed_out = false;
        let mut cancelled = false;
        loop {
            self.drain_output(&mut out, &mut err, 8192, &mut truncated)?;
            // SAFETY: process is a live owned process handle. A bounded wait
            // keeps cancellation and output draining responsive.
            let wait = unsafe { WaitForSingleObject(self.process.as_raw_handle() as _, 20) };
            if wait == WAIT_OBJECT_0 {
                break;
            }
            if wait != WAIT_TIMEOUT {
                return Err(io::Error::last_os_error());
            }
            if let Some(reason) =
                stop_reason(cancel.load(Ordering::SeqCst), Instant::now() >= deadline)
            {
                cancelled = reason == StopReason::Cancelled;
                timed_out = reason == StopReason::TimedOut;
                self.terminate()?;
                // SAFETY: the process handle remains valid. Never wait forever
                // after termination; failure is reported as an unknown outcome.
                let stopped =
                    unsafe { WaitForSingleObject(self.process.as_raw_handle() as _, 5000) };
                if stopped != WAIT_OBJECT_0 {
                    return Err(io::Error::other("terminated process did not exit in time"));
                }
                break;
            }
        }
        let mut exit_code = 0u32;
        // SAFETY: the process is signalled and exit_code is writable.
        if unsafe { GetExitCodeProcess(self.process.as_raw_handle() as _, &mut exit_code) } == 0 {
            return Err(io::Error::last_os_error());
        }
        let drain_deadline = Instant::now() + Duration::from_millis(200);
        let pipes_incomplete = loop {
            let (stdout, stderr) = self.drain_output(&mut out, &mut err, 8192, &mut truncated)?;
            if stdout == PipeState::Closed && stderr == PipeState::Closed {
                break false;
            }
            if Instant::now() >= drain_deadline {
                break true;
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        Ok(CapturedProcess {
            exit_code,
            timed_out,
            cancelled,
            stdout: out,
            stderr: err,
            truncated,
            pipes_incomplete,
        })
    }
}
impl Drop for SuspendedProcess {
    fn drop(&mut self) {
        // Also attempt to kill the primary process if job assignment failed.
        // This is best effort; handle closing and Job Object cleanup follow.
        unsafe {
            TerminateProcess(self.process.as_raw_handle() as _, 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        ffi::OsString,
        fs,
        os::windows::ffi::OsStringExt,
        time::{Duration, Instant},
    };
    #[test]
    fn windows_suspended_process_rejects_relative_and_nul_inputs() {
        assert!(wide(&OsString::from_wide(&[65, 0, 66])).is_err());
        assert!(SuspendedProcess::create(Path::new("relative.exe"), &[], Path::new(".")).is_err());
    }
    fn shell(script: &str) -> SuspendedProcess {
        let powershell = Path::new(&std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let args: Vec<OsString> = ["-NoProfile", "-NonInteractive", "-Command", script]
            .iter()
            .map(|arg| OsString::from(*arg))
            .collect();
        SuspendedProcess::create(&powershell, &args, &std::env::temp_dir()).unwrap()
    }
    #[test]
    fn windows_suspended_process_collects_exit_and_bounded_output() {
        let process =
            shell("[Console]::Out.WriteLine('hello'); [Console]::Error.WriteLine('error'); exit 7");
        let result = process
            .run(Duration::from_secs(10), &AtomicBool::new(false))
            .unwrap();
        assert_eq!(result.exit_code, 7);
        assert!(!result.timed_out);
        assert!(!result.cancelled);
        assert!(result.stdout.windows(5).any(|w| w == b"hello"));
        assert!(result.stderr.windows(5).any(|w| w == b"error"));
        assert!(!result.truncated);
        assert!(!result.pipes_incomplete);
    }
    #[test]
    fn windows_suspended_process_honors_timeout_and_pre_cancel() {
        assert!(shell("Start-Sleep -Seconds 30")
            .run(Duration::from_secs(1), &AtomicBool::new(true))
            .is_err());
        let result = shell("Start-Sleep -Seconds 30")
            .run(Duration::from_millis(100), &AtomicBool::new(false))
            .unwrap();
        assert!(result.timed_out);
        assert!(!result.cancelled);
    }
    #[test]
    fn windows_suspended_process_observes_running_cancel() {
        let cancel = std::sync::Arc::new(AtomicBool::new(false));
        let signal = cancel.clone();
        let waiter = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            signal.store(true, Ordering::SeqCst);
        });
        let result = shell("Start-Sleep -Seconds 30")
            .run(Duration::from_secs(10), &cancel)
            .unwrap();
        waiter.join().unwrap();
        assert!(result.cancelled);
        assert!(!result.timed_out);
    }
    #[test]
    fn windows_suspended_process_runs_only_after_assignment_and_resume() {
        let root = std::env::temp_dir().join(format!(
            "isa-win-suspended-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let marker = root.join("marker.txt");
        let powershell = Path::new(&std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let escaped = marker.to_string_lossy().replace('\'', "''");
        // Use .NET directly: the child intentionally inherits only SystemRoot,
        // so PowerShell module auto-loading and external commands are not reliable.
        let script=format!("[Console]::Out.WriteLine('OUT'); [Console]::Error.WriteLine('ERR'); [System.IO.File]::WriteAllText('{}', 'ready'); [System.Threading.Thread]::Sleep(30000)",escaped);
        let args: Vec<OsString> = ["-NoProfile", "-NonInteractive", "-Command", &script]
            .iter()
            .map(|arg| OsString::from(*arg))
            .collect();
        let mut process = SuspendedProcess::create(&powershell, &args, &root).unwrap();
        std::thread::sleep(Duration::from_millis(150));
        assert!(!marker.exists(), "suspended process ran before resume");
        let mut out = Vec::new();
        let mut err = Vec::new();
        let mut truncated = false;
        assert_eq!(
            process
                .drain_output(&mut out, &mut err, 1024, &mut truncated)
                .unwrap(),
            (PipeState::Open, PipeState::Open)
        );
        process.resume().unwrap();
        assert!(process.resume().is_err());
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            process
                .drain_output(&mut out, &mut err, 8192, &mut truncated)
                .unwrap();
            if marker.exists()
                && out.windows(3).any(|w| w == b"OUT")
                && err.windows(3).any(|w| w == b"ERR")
            {
                break;
            }
            // Report child diagnostics if it exits or times out, rather than
            // hiding a PowerShell or filesystem error behind a missing marker.
            let status = unsafe { WaitForSingleObject(process.process.as_raw_handle() as _, 0) };
            assert_eq!(
                status,
                WAIT_TIMEOUT,
                "child exited before marker/output; stdout: {}; stderr: {}",
                String::from_utf8_lossy(&out),
                String::from_utf8_lossy(&err)
            );
            assert!(
                Instant::now() < deadline,
                "resumed process did not produce marker/output; stdout: {}; stderr: {}",
                String::from_utf8_lossy(&out),
                String::from_utf8_lossy(&err)
            );
            std::thread::sleep(Duration::from_millis(25));
        }
        assert!(!truncated);
        process.terminate().unwrap();
        let exited = unsafe {
            windows_sys::Win32::System::Threading::WaitForSingleObject(
                process.process.as_raw_handle() as _,
                10_000,
            )
        };
        assert_eq!(exited, windows_sys::Win32::Foundation::WAIT_OBJECT_0);
        drop(process);
        fs::remove_dir_all(root).unwrap();
    }
}
