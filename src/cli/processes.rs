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

struct Process { name: String, command: String, child: Child, output: Arc<Mutex<VecDeque<u8>>>, started: Instant, exit: Option<String> }
struct Manager { processes: Vec<Process> }

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty() || name.len() > 40 || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') { return Err("process name must contain only letters, digits, hyphens or underscores (maximum 40)".into()); } Ok(())
}
fn capture<R: Read + Send + 'static>(mut stream: R, output: Arc<Mutex<VecDeque<u8>>>) {
    std::thread::spawn(move || {
        let mut buffer = [0; 4096];
        loop {
            let n = match stream.read(&mut buffer) { Ok(0) | Err(_) => break, Ok(n) => n };
            let Ok(mut data) = output.lock() else { break };
            for &byte in &buffer[..n] { if data.len() == MAX_OUTPUT { data.pop_front(); } data.push_back(byte); }
        }
    });
}
fn safe_terminal(text: &str) -> String {
    text.chars().map(|ch| if ch == '\n' || ch == '\t' || !ch.is_control() { ch } else { '�' }).collect()
}
impl Manager {
    fn new() -> Self { Self { processes: Vec::new() } }
    fn start(&mut self, root: &Path, name: &str, command: &str) -> Result<()> {
        validate_name(name)?;
        if command.trim().is_empty() || command.len() > MAX_COMMAND || command.chars().any(|c| c.is_control() && c != '\n' && c != '\t') { return Err("command must be 1 to 4096 bytes without terminal control characters".into()); }
        if self.processes.iter().any(|p| p.name == name) { return Err("process name already exists in this session".into()); }
        if self.processes.len() >= MAX_PROCESSES { return Err("process limit reached".into()); }
        #[cfg(not(unix))]
        { let _ = (root, command); return Err("process supervision is currently supported on Unix only".into()); }
        #[cfg(unix)] {
            use std::os::unix::process::CommandExt;
            let mut child = Command::new("bash").arg("--noprofile").arg("--norc").arg("-c").arg(command).current_dir(root)
                .env_remove("BASH_ENV").env_remove("ENV").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).process_group(0).spawn()?;
            let output = Arc::new(Mutex::new(VecDeque::with_capacity(MAX_OUTPUT)));
            if let Some(stdout) = child.stdout.take() { capture(stdout, output.clone()); }
            if let Some(stderr) = child.stderr.take() { capture(stderr, output.clone()); }
            self.processes.push(Process { name: name.into(), command: command.into(), child, output, started: Instant::now(), exit: None });
            Ok(())
        }
    }
    fn update(&mut self) -> Result<()> {
        for process in &mut self.processes {
            if process.exit.is_none() { if let Some(status) = process.child.try_wait()? { process.exit = Some(status.to_string()); } }
        }
        Ok(())
    }
    fn list(&mut self) -> Result<()> {
        self.update()?;
        if self.processes.is_empty() { println!("No processes in this session."); }
        for process in &self.processes {
            println!("{} · {} · {}s · {}", process.name, process.exit.as_deref().unwrap_or("running"), process.started.elapsed().as_secs(), safe_terminal(&process.command));
        }
        Ok(())
    }
    fn logs(&mut self, name: &str) -> Result<()> {
        let process = self.processes.iter_mut().find(|p| p.name == name).ok_or("unknown process")?;
        let data: Vec<u8> = process.output.lock().map_err(|_| "output buffer unavailable")?.iter().copied().collect();
        print!("{}", safe_terminal(&String::from_utf8_lossy(&data))); io::stdout().flush()?; Ok(())
    }
    fn remove(&mut self, name: &str) -> Result<()> {
        self.update()?;
        let index = self.processes.iter().position(|p| p.name == name).ok_or("unknown process")?;
        if self.processes[index].exit.is_none() { return Err("stop the process before removing it".into()); }
        self.processes.remove(index); Ok(())
    }
    fn stop(&mut self, name: &str) -> Result<()> {
        let process = self.processes.iter_mut().find(|p| p.name == name).ok_or("unknown process")?;
        if process.exit.is_some() { return Err("process has already exited".into()); }
        if let Some(status) = process.child.try_wait()? { process.exit = Some(status.to_string()); return Err("process has already exited".into()); }
        #[cfg(unix)] {
            let pid = i32::try_from(process.child.id())?;
            // The child was started as the leader of a new process group.
            let result = unsafe { libc::kill(-pid, libc::SIGTERM) };
            if result != 0 { return Err(io::Error::last_os_error().into()); }
            let deadline = Instant::now() + Duration::from_secs(2);
            while Instant::now() < deadline {
                if let Some(status) = process.child.try_wait()? { process.exit = Some(status.to_string()); return Ok(()); }
                std::thread::sleep(Duration::from_millis(50));
            }
            let result = unsafe { libc::kill(-pid, libc::SIGKILL) };
            if result != 0 { return Err(io::Error::last_os_error().into()); }
            process.exit = Some(process.child.wait()?.to_string()); Ok(())
        }
        #[cfg(not(unix))] { Err("process supervision is currently supported on Unix only".into()) }
    }
}
impl Drop for Manager {
    fn drop(&mut self) {
        let names: Vec<String> = self.processes.iter().filter(|p| p.exit.is_none()).map(|p| p.name.clone()).collect();
        for name in names { if let Err(error) = self.stop(&name) { eprintln!("Could not stop {name}: {error}"); } }
    }
}

pub fn interactive(root: &Path) -> Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() { return Err("process supervision requires an interactive terminal".into()); }
    println!("ISA process supervisor · {}\nCommands run without a sandbox and can access your files and network.\nUse: start <name> <bash command>, list, logs <name>, stop <name>, remove <name>, quit.\nProcesses are stopped when this supervisor exits.", safe_terminal(&root.display().to_string()));
    let mut manager = Manager::new(); let mut approved = HashSet::new();
    loop {
        print!("process> "); io::stdout().flush()?;
        let mut line = String::new(); if io::stdin().read_line(&mut line)? == 0 { break; }
        let line = line.trim(); if line == "quit" || line == "exit" { break; }
        let (action, rest) = line.split_once(' ').unwrap_or((line, "")); let rest = rest.trim();
        let result = match action {
            "list" if rest.is_empty() => manager.list(),
            "logs" if !rest.is_empty() => manager.logs(rest),
            "stop" if !rest.is_empty() => manager.stop(rest),
            "remove" if !rest.is_empty() => manager.remove(rest),
            "start" => {
                let (name, command) = match rest.split_once(' ') { Some(pair) => pair, None => { eprintln!("Use: start <name> <bash command>"); continue; } };
                if let Err(error) = validate_name(name) { eprintln!("{error}"); continue; }
                if command.trim().is_empty() || command.len() > MAX_COMMAND || command.chars().any(|c| c.is_control() && c != '\n' && c != '\t') { eprintln!("Invalid command."); continue; }
                if !approved.contains(command) {
                    println!("\nBash command in {}:\n{}\n1. Allow once\n2. Allow always for this exact command in this session\n3. Deny", safe_terminal(&root.display().to_string()), safe_terminal(command));
                    print!("Decision [1/2/3]: "); io::stdout().flush()?;
                    let mut decision = String::new(); if io::stdin().read_line(&mut decision)? == 0 { break; }
                    match decision.trim() { "1" => (), "2" => { approved.insert(command.to_owned()); }, _ => { println!("Denied. No command was run; choose a different approach."); continue; } }
                }
                manager.start(root, name, command).map(|_| println!("Started {name}."))
            }
            _ => { println!("Use: start <name> <bash command>, list, logs <name>, stop <name>, remove <name>, quit."); Ok(()) }
        };
        if let Err(error) = result { eprintln!("Error: {error}"); }
    }
    Ok(())
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn rejects_invalid_names_and_sanitizes_terminal() { assert!(validate_name("../bad").is_err()); assert!(validate_name("valid-name_1").is_ok()); assert!(!safe_terminal("\u{1b}[31m").contains('\u{1b}')); }
    #[cfg(unix)]
    #[test] fn starts_and_stops_real_process() {
        let mut manager = Manager::new();
        manager.start(Path::new("."), "test", "while :; do sleep 1; done").unwrap();
        assert!(manager.processes[0].child.try_wait().unwrap().is_none());
        assert!(manager.start(Path::new("."), "test", "true").is_err());
        manager.stop("test").unwrap(); assert!(manager.processes[0].exit.is_some()); manager.remove("test").unwrap(); assert!(manager.processes.is_empty());
    }
}
