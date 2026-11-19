use super::Result;
use std::collections::{HashSet, VecDeque};
use std::io::{self, IsTerminal, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MAX_PROCESSES: usize = 8;
const MAX_OUTPUT: usize = 64 * 1024;
const MAX_COMMAND: usize = 4096;

struct Process {
    name: String,
    command: String,
    #[cfg(unix)]
    child: Child,
    #[cfg(windows)]
    worker: WindowsWorker,
    output: Arc<Mutex<VecDeque<u8>>>,
    started: Instant,
    exit: Option<String>,
}
#[cfg(windows)]
struct WindowsWorker {
    cancel: Arc<std::sync::atomic::AtomicBool>,
    state: Arc<Mutex<Option<WindowsCompletion>>>,
}
#[cfg(windows)]
#[derive(Clone)]
struct WindowsCompletion {
    status: String,
    cancelled: bool,
}
struct Manager {
    processes: Vec<Process>,
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > 40
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(
            "process name must contain only letters, digits, hyphens or underscores (maximum 40)"
                .into(),
        );
    }
    Ok(())
}
fn capture<R: Read + Send + 'static>(mut stream: R, output: Arc<Mutex<VecDeque<u8>>>) {
    std::thread::spawn(move || {
        let mut buffer = [0; 4096];
        loop {
            let n = match stream.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => n,
            };
            let Ok(mut data) = output.lock() else { break };
            for &byte in &buffer[..n] {
                if data.len() == MAX_OUTPUT {
                    data.pop_front();
                }
                data.push_back(byte);
            }
        }
    });
}
fn unsafe_display_char(ch: char) -> bool {
    ch.is_control() && ch != '\n' && ch != '\t'
        || matches!(ch, '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}
fn safe_terminal(text: &str) -> String {
    text.chars()
        .map(|ch| if unsafe_display_char(ch) { '�' } else { ch })
        .collect()
}
fn valid_command(command: &str) -> bool {
    !command.trim().is_empty()
        && command.len() <= MAX_COMMAND
        && !command.chars().any(unsafe_display_char)
}
#[cfg(windows)]
fn windows_enabled(value: Option<&std::ffi::OsStr>) -> bool {
    value == Some(std::ffi::OsStr::new("1"))
}
impl Manager {
    fn new() -> Self {
        Self {
            processes: Vec::new(),
        }
    }
    fn start(&mut self, root: &Path, name: &str, command: &str) -> Result<()> {
        validate_name(name)?;
        if !valid_command(command) {
            return Err("command must be 1 to 4096 bytes without unsafe display controls".into());
        }
        if self.processes.iter().any(|p| p.name == name) {
            return Err("process name already exists in this session".into());
        }
        if self.processes.len() >= MAX_PROCESSES {
            return Err("process limit reached".into());
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = (root, command);
            return Err("process supervision is unavailable on this platform".into());
        }
        #[cfg(windows)]
        {
            if !windows_enabled(std::env::var_os("ISA_WINDOWS_PROCESSES_EXPERIMENTAL").as_deref()) {
                return Err("Windows process supervision is experimental and disabled".into());
            }
            self.start_windows(root, name, command)
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            let mut child = Command::new("bash")
                .arg("--noprofile")
                .arg("--norc")
                .arg("-c")
                .arg(command)
                .current_dir(root)
                .env_remove("BASH_ENV")
                .env_remove("ENV")
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .process_group(0)
                .spawn()?;
            let output = Arc::new(Mutex::new(VecDeque::with_capacity(MAX_OUTPUT)));
            if let Some(stdout) = child.stdout.take() {
                capture(stdout, output.clone());
            }
            if let Some(stderr) = child.stderr.take() {
                capture(stderr, output.clone());
            }
            self.processes.push(Process {
                name: name.into(),
                command: command.into(),
                child,
                output,
                started: Instant::now(),
                exit: None,
            });
            Ok(())
        }
    }
    #[cfg(windows)]
    fn start_windows(&mut self, root: &Path, name: &str, command: &str) -> Result<()> {
        use std::{ffi::OsString, sync::atomic::AtomicBool};
        // Called by start after name, command and capacity checks. Test callers
        // repeat them here to avoid a less-restricted internal entry point.
        validate_name(name)?;
        if !valid_command(command)
            || self.processes.len() >= MAX_PROCESSES
            || self.processes.iter().any(|p| p.name == name)
        {
            return Err("invalid or duplicate process".into());
        }
        let root = std::fs::canonicalize(root)?;
        let program = super::windows_powershell::powershell_path()?;
        let args: Vec<OsString> = super::powershell_command::arguments(command)?
            .into_iter()
            .map(OsString::from)
            .collect();
        let process =
            super::windows_suspended_process::SuspendedProcess::create(&program, &args, &root)?;
        let output = Arc::new(Mutex::new(VecDeque::new()));
        let output_thread = output.clone();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_thread = cancel.clone();
        let state = Arc::new(Mutex::new(None));
        let state_thread = state.clone();
        // Output is collected by the runner and published after exit only.
        // A thread creation error drops the still-suspended process and job.
        std::thread::Builder::new().name("isa-windows-process".into()).spawn(move || {
            let result=process.run(Duration::from_secs(24*60*60),&cancel_thread);
            let status=match result {
                Ok(result)=>{
                    if let Ok(mut buffer)=output_thread.lock() {
                        buffer.extend(b"--- stdout (tail) ---\n");buffer.extend(result.stdout);
                        buffer.extend(b"\n--- stderr (tail) ---\n");buffer.extend(result.stderr);
                    }
                    WindowsCompletion {status:format!("exit {}; timed out: {}; cancelled: {}; truncated: {}; pipes incomplete: {}",result.exit_code,result.timed_out,result.cancelled,result.truncated,result.pipes_incomplete),cancelled:result.cancelled}
                },
                Err(_)=>WindowsCompletion {status:"process outcome unavailable; inspect the workspace".to_owned(),cancelled:false},
            };
            if let Ok(mut s)=state_thread.lock() {*s=Some(status);}
        })?;
        self.processes.push(Process {
            name: name.into(),
            command: command.into(),
            worker: WindowsWorker { cancel, state },
            output,
            started: Instant::now(),
            exit: None,
        });
        Ok(())
    }
    fn update(&mut self) -> Result<()> {
        for process in &mut self.processes {
            #[cfg(unix)]
            if process.exit.is_none() {
                if let Some(status) = process.child.try_wait()? {
                    process.exit = Some(status.to_string());
                }
            }
            #[cfg(windows)]
            if process.exit.is_none() {
                process.exit = process
                    .worker
                    .state
                    .lock()
                    .map_err(|_| "process state unavailable")?
                    .as_ref()
                    .map(|result| result.status.clone());
            }
        }
        Ok(())
    }
    fn list(&mut self) -> Result<()> {
        self.update()?;
        if self.processes.is_empty() {
            println!("No processes in this session.");
        }
        for process in &self.processes {
            println!(
                "{} · {} · {}s · {}",
                process.name,
                process.exit.as_deref().unwrap_or("running"),
                process.started.elapsed().as_secs(),
                safe_terminal(&process.command)
            );
        }
        Ok(())
    }
    // Deliberately excludes commands, logs, PIDs and workspace paths. Names are
    // restricted to ASCII identifier characters, so they require no escaping.
    fn snapshot(&mut self) -> Result<String> {
        self.update()?;
        let entries: Vec<String> = self
            .processes
            .iter()
            .map(|p| {
                format!(
                    "{{\"name\":\"{}\",\"state\":\"{}\",\"elapsedSeconds\":{}}}",
                    p.name,
                    if p.exit.is_some() {
                        "exited"
                    } else {
                        "running"
                    },
                    p.started.elapsed().as_secs()
                )
            })
            .collect();
        Ok(format!(
            "{{\"version\":1,\"processes\":[{}]}}",
            entries.join(",")
        ))
    }
    fn logs(&mut self, name: &str) -> Result<()> {
        let process = self
            .processes
            .iter_mut()
            .find(|p| p.name == name)
            .ok_or("unknown process")?;
        let data: Vec<u8> = process
            .output
            .lock()
            .map_err(|_| "output buffer unavailable")?
            .iter()
            .copied()
            .collect();
        print!("{}", safe_terminal(&String::from_utf8_lossy(&data)));
        io::stdout().flush()?;
        Ok(())
    }
    fn remove(&mut self, name: &str) -> Result<()> {
        self.update()?;
        let index = self
            .processes
            .iter()
            .position(|p| p.name == name)
            .ok_or("unknown process")?;
        if self.processes[index].exit.is_none() {
            return Err("stop the process before removing it".into());
        }
        self.processes.remove(index);
        Ok(())
    }
    fn stop(&mut self, name: &str) -> Result<()> {
        let process = self
            .processes
            .iter_mut()
            .find(|p| p.name == name)
            .ok_or("unknown process")?;
        if process.exit.is_some() {
            return Err("process has already exited".into());
        }
        #[cfg(unix)]
        if let Some(status) = process.child.try_wait()? {
            process.exit = Some(status.to_string());
            return Err("process has already exited".into());
        }
        #[cfg(unix)]
        {
            let pid = i32::try_from(process.child.id())?;
            // The child was started as the leader of a new process group.
            let result = unsafe { libc::kill(-pid, libc::SIGTERM) };
            if result != 0 {
                return Err(io::Error::last_os_error().into());
            }
            let deadline = Instant::now() + Duration::from_secs(2);
            while Instant::now() < deadline {
                if let Some(status) = process.child.try_wait()? {
                    process.exit = Some(status.to_string());
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            let result = unsafe { libc::kill(-pid, libc::SIGKILL) };
            if result != 0 {
                return Err(io::Error::last_os_error().into());
            }
            process.exit = Some(process.child.wait()?.to_string());
            Ok(())
        }
        #[cfg(windows)]
        {
            use std::sync::atomic::Ordering;
            if let Some(result) = process
                .worker
                .state
                .lock()
                .map_err(|_| "process state unavailable")?
                .clone()
            {
                process.exit = Some(result.status);
                return Err("process has already exited or its outcome is unavailable".into());
            }
            process.worker.cancel.store(true, Ordering::SeqCst);
            let deadline = Instant::now() + Duration::from_secs(7);
            while Instant::now() < deadline {
                if let Some(result) = process
                    .worker
                    .state
                    .lock()
                    .map_err(|_| "process state unavailable")?
                    .clone()
                {
                    process.exit = Some(result.status);
                    return if result.cancelled {
                        Ok(())
                    } else {
                        Err("stop requested, but cancellation was not confirmed".into())
                    };
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err("stop requested, but process termination could not be confirmed".into())
        }
        #[cfg(not(any(unix, windows)))]
        {
            Err("process supervision is unavailable on this platform".into())
        }
    }
}
impl Drop for Manager {
    fn drop(&mut self) {
        let names: Vec<String> = self
            .processes
            .iter()
            .filter(|p| p.exit.is_none())
            .map(|p| p.name.clone())
            .collect();
        for name in names {
            if let Err(error) = self.stop(&name) {
                eprintln!("Could not stop {name}: {error}");
            }
        }
    }
}

pub fn interactive(root: &Path) -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err("process supervision requires an interactive terminal".into());
    }
    #[cfg(not(windows))]
    println!("ISA process supervisor · {}\nCommands run without a sandbox and inherit your environment; they can access your files, credentials and network.\nUse: start <name> <bash command>, list, snapshot, logs <name>, stop <name>, remove <name>, quit.\nOn exit, the supervisor attempts to stop tracked process groups; escaped descendants may continue and completed leaders may leave descendants. Abrupt termination may leave processes running.", safe_terminal(&root.display().to_string()));
    #[cfg(windows)]
    println!("ISA process supervisor · {}\nWindows mode is experimental and requires ISA_WINDOWS_PROCESSES_EXPERIMENTAL=1. Commands are PowerShell scripts, run unsandboxed with your permissions and network, with only SystemRoot in their environment. Output is available after exit only, at most 8 KiB per stream. Processes time out after 24 hours.\nUse: start <name> <PowerShell script>, list, snapshot, logs <name>, stop <name>, remove <name>, quit. Stop and Job Object cleanup are best effort and do not undo side effects; descendants may survive. Encoded scripts are not secret.", safe_terminal(&root.display().to_string()));
    let mut manager = Manager::new();
    let mut approved = HashSet::new();
    loop {
        print!("process> ");
        io::stdout().flush()?;
        let mut line = String::new();
        if io::stdin().read_line(&mut line)? == 0 {
            break;
        }
        let line = line.trim();
        if line == "quit" || line == "exit" {
            break;
        }
        let (action, rest) = line.split_once(' ').unwrap_or((line, ""));
        let rest = rest.trim();
        let result = match action {
            "list" if rest.is_empty() => manager.list(),
            "snapshot" if rest.is_empty() => manager.snapshot().map(|data| println!("{data}")),
            "logs" if !rest.is_empty() => manager.logs(rest),
            "stop" if !rest.is_empty() => manager.stop(rest),
            "remove" if !rest.is_empty() => manager.remove(rest),
            "start" => {
                let (name, command) = match rest.split_once(' ') {
                    Some(pair) => pair,
                    None => {
                        eprintln!("Use: start <name> <bash command>");
                        continue;
                    }
                };
                if let Err(error) = validate_name(name) {
                    eprintln!("{error}");
                    continue;
                }
                if !valid_command(command) {
                    eprintln!("Invalid command.");
                    continue;
                }
                if !approved.contains(command) {
                    let shell = if cfg!(windows) {
                        "PowerShell script"
                    } else {
                        "Bash command"
                    };
                    println!("\n{shell} in {}:\n{}\n1. Allow once\n2. Allow always for this exact command in this session\n3. Deny", safe_terminal(&root.display().to_string()), safe_terminal(command));
                    print!("Decision [1/2/3]: ");
                    io::stdout().flush()?;
                    let mut decision = String::new();
                    if io::stdin().read_line(&mut decision)? == 0 {
                        break;
                    }
                    match decision.trim() {
                        "1" => (),
                        "2" => {
                            approved.insert(command.to_owned());
                        }
                        _ => {
                            println!("Denied. No command was run; choose a different approach.");
                            continue;
                        }
                    }
                }
                manager
                    .start(root, name, command)
                    .map(|_| println!("Started {name}."))
            }
            _ => {
                println!("Use: start <name> <bash command>, list, snapshot, logs <name>, stop <name>, remove <name>, quit.");
                Ok(())
            }
        };
        if let Err(error) = result {
            eprintln!("Error: {error}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_invalid_names_and_sanitizes_terminal() {
        assert!(validate_name("../bad").is_err());
        assert!(validate_name("valid-name_1").is_ok());
        assert!(!safe_terminal("\u{1b}[31m\u{202e}hidden").contains('\u{1b}'));
        assert!(!safe_terminal("\u{202e}hidden").contains('\u{202e}'));
        assert!(!valid_command("echo \u{202e}hidden"));
        assert!(valid_command("echo ok\n"));
    }
    #[test]
    fn captured_output_is_bounded_and_keeps_the_tail() {
        let output = Arc::new(Mutex::new(VecDeque::new()));
        let mut bytes = vec![b'a'; MAX_OUTPUT + 4096];
        bytes.extend_from_slice(b"END");
        capture(std::io::Cursor::new(bytes), output.clone());
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            {
                let captured = output.lock().unwrap();
                if captured.len() == MAX_OUTPUT
                    && captured
                        .iter()
                        .rev()
                        .take(3)
                        .copied()
                        .eq(b"DNE".iter().copied())
                {
                    break;
                }
            }
            assert!(Instant::now() < deadline, "capture did not finish");
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    #[cfg(unix)]
    #[test]
    fn snapshot_omits_command_and_is_point_in_time() {
        let mut manager = Manager::new();
        assert_eq!(
            manager.snapshot().unwrap(),
            r#"{"version":1,"processes":[]}"#
        );
        manager
            .start(Path::new("."), "one", "sleep 20 # private-secret")
            .unwrap();
        let snapshot = manager.snapshot().unwrap();
        assert!(snapshot.contains(r#""name":"one","state":"running""#));
        assert!(!snapshot.contains("private-secret"));
        manager.stop("one").unwrap();
        assert!(manager.snapshot().unwrap().contains(r#""state":"exited""#));
    }
    #[cfg(unix)]
    #[test]
    fn starts_and_stops_real_process() {
        let mut manager = Manager::new();
        manager
            .start(Path::new("."), "test", "while :; do sleep 1; done")
            .unwrap();
        assert!(manager.processes[0].child.try_wait().unwrap().is_none());
        assert!(manager.start(Path::new("."), "test", "true").is_err());
        manager.stop("test").unwrap();
        assert!(manager.processes[0].exit.is_some());
        manager.remove("test").unwrap();
        assert!(manager.processes.is_empty());
    }
}

#[cfg(all(test, windows))]
mod windows_fail_closed_tests {
    use super::*;
    #[test]
    fn windows_fail_closed_process_manager_requires_explicit_opt_in() {
        assert!(!windows_enabled(None));
        assert!(!windows_enabled(Some(std::ffi::OsStr::new("true"))));
        assert!(windows_enabled(Some(std::ffi::OsStr::new("1"))));
        let mut manager = Manager::new();
        assert!(manager
            .start(Path::new("."), "../bad", "echo test")
            .is_err());
        assert!(manager.processes.is_empty());
    }
}

#[cfg(all(test, windows))]
mod windows_processes_experimental_tests {
    use super::*;
    #[test]
    fn windows_processes_experimental_captures_after_exit_and_respects_limits() {
        let root = std::env::temp_dir();
        let mut manager = Manager::new();
        manager
            .start_windows(
                &root,
                "sample",
                "[Console]::Out.Write(('x' * 12000) + 'END'); exit 7",
            )
            .unwrap();
        assert!(manager.start_windows(&root, "sample", "exit 0").is_err());
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            manager.update().unwrap();
            if manager.processes[0].exit.is_some() {
                break;
            }
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(20));
        }
        let process = &manager.processes[0];
        assert!(process.exit.as_ref().unwrap().contains("exit 7"));
        assert!(process.exit.as_ref().unwrap().contains("truncated: true"));
        let data: Vec<u8> = process.output.lock().unwrap().iter().copied().collect();
        assert!(data.len() <= MAX_OUTPUT);
        assert!(data.windows(3).any(|w| w == b"END"));
        manager.remove("sample").unwrap();
    }
    #[test]
    fn windows_processes_experimental_stop_and_snapshot() {
        let root = std::env::temp_dir().join(format!(
            "isa-proc-stop-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let mut manager = Manager::new();
        manager.start_windows(&root,"running","Set-Content -LiteralPath 'ready.txt' -Value ready; Start-Sleep -Seconds 30 # private-secret").unwrap();
        let deadline = Instant::now() + Duration::from_secs(15);
        while !root.join("ready.txt").exists() {
            assert!(Instant::now() < deadline);
            std::thread::sleep(Duration::from_millis(20));
        }
        let snapshot = manager.snapshot().unwrap();
        assert!(!snapshot.contains("private-secret"));
        assert!(snapshot.contains("running"));
        manager.stop("running").unwrap();
        assert!(manager.snapshot().unwrap().contains("exited"));
        manager.remove("running").unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
