//! Experimental Windows Job Object primitive, not a sandbox or a command runner.
//!
//! The experimental runner assigns the process while suspended before resuming.
//! This is not a sandbox: Job membership does not contain all possible indirect
//! effects or elevated processes, and execution remains unvalidated on Windows.
use std::{io, mem::{size_of, zeroed}, os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle, RawHandle}, ptr};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    SetInformationJobObject, TerminateJobObject,
};

pub(super) struct WindowsJob { handle: OwnedHandle }

impl WindowsJob {
    pub(super) fn new() -> io::Result<Self> {
        // SAFETY: null attributes make the unnamed job handle non-inheritable.
        let raw = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if raw.is_null() { return Err(io::Error::last_os_error()); }
        // SAFETY: CreateJobObjectW returned a new, owned, valid handle.
        let handle = unsafe { OwnedHandle::from_raw_handle(raw as RawHandle) };
        // SAFETY: all-zero fields are valid for the Win32 limits structure.
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: `limits` is initialized and has the expected type and size;
        // the job handle remains valid throughout the call.
        let ok = unsafe { SetInformationJobObject(handle.as_raw_handle() as _, JobObjectExtendedLimitInformation, (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(), size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32) };
        if ok == 0 { return Err(io::Error::last_os_error()); }
        Ok(Self { handle })
    }

    /// Assign a process to this job. This does not suspend or resume it.
    ///
    /// # Safety
    /// `process` must be null or remain a valid process handle for the duration
    /// of the call. Production callers must also ensure the process cannot
    /// execute or create children until assignment succeeds, and terminate it
    /// on failure.
    pub(super) unsafe fn assign(&self, process: RawHandle) -> io::Result<()> {
        if process.is_null() { return Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid process handle")); }
        // SAFETY: handle lifetime is the caller's obligation, as stated above.
        let ok = unsafe { AssignProcessToJobObject(self.handle.as_raw_handle() as _, process as _) };
        if ok == 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
    }

    pub(super) fn terminate(&self) -> io::Result<()> {
        // SAFETY: the owned job handle remains valid until self is dropped.
        let ok = unsafe { TerminateJobObject(self.handle.as_raw_handle() as _, 1) };
        if ok == 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{os::windows::io::AsRawHandle, process::{Child, Command, Stdio}, time::{Duration, Instant}};
    struct ChildGuard(Child);
    impl Drop for ChildGuard { fn drop(&mut self) { let _=self.0.kill();let _=self.0.wait(); } }
    fn waiting_process() -> ChildGuard {
        // This fixture does not start descendants. Starting a real command this
        // way would be racy and must not be used by the production supervisor.
        ChildGuard(Command::new("powershell.exe").args(["-NoProfile","-NonInteractive","-Command","Start-Sleep -Seconds 30"]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().unwrap())
    }
    fn assert_exited(child: &mut Child) {
        let deadline=Instant::now()+Duration::from_secs(10);
        loop { if child.try_wait().unwrap().is_some() { return; } assert!(Instant::now()<deadline,"job process did not terminate");std::thread::sleep(Duration::from_millis(20)); }
    }
    #[test]
    fn windows_job_terminates_assigned_test_process() {
        let job=WindowsJob::new().unwrap();let mut child=waiting_process();
        // SAFETY: Child owns the live handle. The race is acceptable only for
        // this controlled fixture, which does not create child processes.
        unsafe { job.assign(child.0.as_raw_handle()) }.unwrap();
        job.terminate().unwrap();assert_exited(&mut child.0);
    }
    #[test]
    fn windows_job_closing_last_handle_terminates_assigned_test_process() {
        let job=WindowsJob::new().unwrap();let mut child=waiting_process();
        // SAFETY: Child owns the live handle; this fixture has no descendants.
        unsafe { job.assign(child.0.as_raw_handle()) }.unwrap();
        drop(job);assert_exited(&mut child.0);
    }
    #[test]
    fn windows_job_rejects_null_process_handle() {
        let job=WindowsJob::new().unwrap();
        // SAFETY: the null input is explicitly rejected before any Win32 call.
        assert_eq!(unsafe { job.assign(ptr::null_mut()) }.unwrap_err().kind(),io::ErrorKind::InvalidInput);
    }
}
