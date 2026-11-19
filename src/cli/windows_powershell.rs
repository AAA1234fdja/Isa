//! Experimental Windows PowerShell entry point used by opt-in manual web shell.
//! Scripts run unsandboxed with the server user's privileges; not production ready.
use super::{
    powershell_command,
    windows_suspended_process::{CapturedProcess, SuspendedProcess},
};
use std::{
    ffi::OsString,
    io,
    os::windows::ffi::OsStringExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use windows_sys::Win32::System::SystemInformation::GetSystemDirectoryW;

pub(super) fn powershell_path() -> io::Result<PathBuf> {
    let mut buffer = vec![0u16; 32768];
    // SAFETY: buffer is writable for the full size supplied to the Win32 API.
    let len = unsafe { GetSystemDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) } as usize;
    if len == 0 {
        return Err(io::Error::last_os_error());
    }
    if len >= buffer.len() {
        return Err(io::Error::other("Windows system directory is too long"));
    }
    let path = PathBuf::from(OsString::from_wide(&buffer[..len]))
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe");
    if !path.is_absolute() || !path.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "Windows PowerShell is unavailable",
        ));
    }
    Ok(path)
}

pub(super) fn run(
    root: &Path,
    script: &str,
    timeout: Duration,
    cancel: &AtomicBool,
) -> io::Result<CapturedProcess> {
    let args: Vec<OsString> = powershell_command::arguments(script)?
        .into_iter()
        .map(OsString::from)
        .collect();
    if cancel.load(Ordering::SeqCst) {
        return Err(io::Error::other("command was stopped before starting"));
    }
    if timeout.is_zero() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "timeout must be positive",
        ));
    }
    let path = powershell_path()?;
    SuspendedProcess::create(&path, &args, root)?.run(timeout, cancel)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_powershell_encoded_command_runs_and_captures_output() {
        let result = run(
            &std::env::temp_dir(),
            "[Console]::Out.WriteLine('hello'); [Console]::Error.WriteLine('error'); exit 7",
            Duration::from_secs(10),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(result.exit_code, 7);
        assert!(!result.timed_out);
        assert!(!result.cancelled);
        assert!(!result.pipes_incomplete);
        assert!(result.stdout.windows(5).any(|w| w == b"hello"));
        assert!(result.stderr.windows(5).any(|w| w == b"error"));
    }
    #[test]
    fn windows_powershell_large_output_is_bounded() {
        let result=run(&std::env::temp_dir(),"[Console]::Out.Write(('x' * 100000) + 'END'); [Console]::Error.Write(('y' * 20000) + 'ERR'); exit 0",Duration::from_secs(20),&AtomicBool::new(false)).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(!result.timed_out);
        assert!(!result.cancelled);
        assert!(result.truncated);
        assert!(!result.pipes_incomplete);
        assert!(result.stdout.len() <= 8192 && result.stderr.len() <= 8192);
        assert!(result.stdout.ends_with(b"END"));
        assert!(result.stderr.ends_with(b"ERR"));
    }
    #[test]
    fn windows_powershell_timeout_is_reported() {
        let result = run(
            &std::env::temp_dir(),
            "Start-Sleep -Seconds 30",
            Duration::from_millis(250),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert!(result.timed_out);
        assert!(!result.cancelled);
    }
    #[test]
    fn windows_powershell_cancel_stops_fixture_descendant_before_marker() {
        use std::{fs, sync::Arc, time::Instant};
        let root = std::env::temp_dir().join(format!(
            "isa-win-descendant-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let child_started = root.join("child-started.txt");
        let escaped = root.join("escaped.txt");
        let control_started = root.join("control-started.txt");
        let control_finished = root.join("control-finished.txt");
        let quoted = |path: &Path| path.to_string_lossy().replace('\'', "''");
        let executable = powershell_path().unwrap();
        let parent_script = |started: &Path, finished: &Path, delay_ms: u32| {
            // The marker is written by the descendant, not by its parent:
            // observing it proves that the child actually began executing.
            let child_script = format!(
                "try {{ [System.IO.File]::WriteAllText('{}', 'started'); [System.Threading.Thread]::Sleep({}); [System.IO.File]::WriteAllText('{}', 'finished') }} catch {{ [Console]::Error.WriteLine($_.ToString()); exit 1 }}",
                quoted(started), delay_ms, quoted(finished)
            );
            let encoded = powershell_command::arguments(&child_script)
                .unwrap()
                .pop()
                .unwrap();
            // Use .NET rather than cmdlets that depend on module auto-loading;
            // the supervised process deliberately receives only SystemRoot.
            format!(
                "try {{ $info = [System.Diagnostics.ProcessStartInfo]::new(); $info.FileName = '{}'; $info.Arguments = '-NoProfile -NonInteractive -EncodedCommand {}'; $info.UseShellExecute = $false; $p = [System.Diagnostics.Process]::Start($info); $p.WaitForExit(); exit $p.ExitCode }} catch {{ [Console]::Error.WriteLine($_.ToString()); exit 1 }}",
                quoted(&executable), encoded
            )
        };
        // Positive control: without cancellation the same child fixture must
        // create both markers, so a broken fixture cannot make the test pass.
        let control = run(
            &root,
            &parent_script(&control_started, &control_finished, 300),
            Duration::from_secs(20),
            &AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(
            control.exit_code,
            0,
            "control failed: {}",
            String::from_utf8_lossy(&control.stderr)
        );
        assert!(!control.timed_out && !control.cancelled);
        assert!(
            control_started.exists() && control_finished.exists(),
            "control descendant did not write both markers"
        );

        let cancel = Arc::new(AtomicBool::new(false));
        let signal = cancel.clone();
        let ready = child_started.clone();
        let watcher = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(20);
            while !ready.exists() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(20));
            }
            let observed = ready.exists();
            signal.store(true, Ordering::SeqCst);
            observed
        });
        let result = run(
            &root,
            &parent_script(&child_started, &escaped, 10000),
            Duration::from_secs(30),
            &cancel,
        );
        let observed = watcher.join().unwrap();
        let result = result.expect("fixture execution failed");
        assert!(
            observed,
            "fixture descendant did not signal readiness; exit: {}, stdout: {}, stderr: {}",
            result.exit_code,
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.cancelled, "cancellation was not reported");
        assert!(!result.timed_out);
        // Wait past the child's full delay, not just past the parent's exit.
        std::thread::sleep(Duration::from_secs(11));
        assert!(
            !escaped.exists(),
            "fixture descendant continued after cancellation"
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn windows_powershell_respects_precancel() {
        assert!(run(
            &std::env::temp_dir(),
            "Write-Output 'no'",
            Duration::from_secs(1),
            &AtomicBool::new(true)
        )
        .is_err());
    }
}
