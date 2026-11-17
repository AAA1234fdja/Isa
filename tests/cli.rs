use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DIR: AtomicU64 = AtomicU64::new(0);

struct Workspace(PathBuf);

impl Workspace {
    fn new() -> Self {
        let id = NEXT_DIR.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("isa-test-{}-{id}", std::process::id()));
        fs::create_dir(&path).expect("create test workspace");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn write(&self, name: &str, contents: &str) {
        let path = self.0.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent directory");
        }
        fs::write(path, contents).expect("write test file");
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn cli(args: &[&str], workspace: &Workspace) -> Output {
    Command::new(env!("CARGO_BIN_EXE_isa-cli"))
        .current_dir(workspace.path())
        .args(args)
        .stdin(Stdio::null())
        .output()
        .expect("run ISA CLI")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn help_describes_available_commands() {
    let workspace = Workspace::new();
    let output = cli(&["help"], &workspace);
    assert!(output.status.success());
    assert!(stdout(&output).contains("isa-cli search"));
    assert!(stdout(&output).contains("isa-cli ask"));
}

#[test]
fn files_excludes_secret_and_generated_directories() {
    let workspace = Workspace::new();
    workspace.write("src/main.rs", "fn main() {}\n");
    workspace.write(".env", "TOP_SECRET=test\n");
    workspace.write(".env.local", "TOP_SECRET=test\n");
    workspace.write("node_modules/library/index.js", "ignored\n");
    workspace.write(".git/config", "ignored\n");
    let output = cli(&["files"], &workspace);
    assert!(output.status.success(), "{}", stderr(&output));
    let list = stdout(&output);
    assert!(list.contains("src/main.rs"));
    assert!(!list.contains(".env"));
    assert!(!list.contains("node_modules"));
    assert!(!list.contains(".git"));
}


#[test]
fn files_respects_gitignore_without_a_git_repository() {
    let workspace = Workspace::new();
    workspace.write(".gitignore", "ignored/\n*.secret\n");
    workspace.write("ignored/file.txt", "ignored");
    workspace.write("private.secret", "ignored");
    workspace.write("keep.txt", "included");
    let output = cli(&["files"], &workspace);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("keep.txt"));
    assert!(!stdout(&output).contains("ignored/file.txt"));
    assert!(!stdout(&output).contains("private.secret"));
}

#[test]
fn search_reports_matching_lines_without_searching_secrets() {
    let workspace = Workspace::new();
    workspace.write("src/lib.rs", "first\nneedle here\nlast\n");
    workspace.write(".env", "needle hidden\n");
    let output = cli(&["search", "needle"], &workspace);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("src/lib.rs:2:needle here"));
    assert!(!stdout(&output).contains("hidden"));
    assert!(stderr(&output).contains("1 match(es)"));
}

#[test]
fn rank_finds_relevant_files() {
    let workspace = Workspace::new();
    workspace.write("auth.rs", "fn authenticate() { /* token */ }\n");
    workspace.write("other.rs", "fn unrelated() {}\n");
    let output = cli(&["rank", "token"], &workspace);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("auth.rs"));
    assert!(!stdout(&output).contains("other.rs"));
}

#[test]
fn session_ids_reject_path_traversal() {
    let workspace = Workspace::new();
    let output = cli(&["history", "../outside"], &workspace);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("session ID"));
}

#[test]
fn existing_session_can_be_listed_and_read() {
    let workspace = Workspace::new();
    workspace.write(".isa/sessions/example.json", r#"{"version":1,"messages":[{"role":"user","content":"Hello"}]}"#);
    let sessions = cli(&["sessions"], &workspace);
    let history = cli(&["history", "example"], &workspace);
    assert!(sessions.status.success(), "{}", stderr(&sessions));
    assert!(history.status.success(), "{}", stderr(&history));
    assert_eq!(stdout(&sessions).trim(), "example");
    assert!(stdout(&history).contains("user: Hello"));
}

#[test]
fn unattended_replace_does_not_modify_files() {
    let workspace = Workspace::new();
    workspace.write("example.txt", "original value");
    let output = cli(&["replace", "example.txt", "original", "changed"], &workspace);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("interactive approval"));
    assert_eq!(fs::read_to_string(workspace.path().join("example.txt")).unwrap(), "original value");
}

#[test]
fn ambiguous_replacement_does_not_modify_files() {
    let workspace = Workspace::new();
    workspace.write("example.txt", "same same");
    let output = cli(&["replace", "example.txt", "same", "changed"], &workspace);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("exactly once"));
    assert_eq!(fs::read_to_string(workspace.path().join("example.txt")).unwrap(), "same same");
}

#[test]
fn replacement_cannot_escape_workspace() {
    let workspace = Workspace::new();
    let output = cli(&["replace", "../outside.txt", "old", "new"], &workspace);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("relative"));
}

#[cfg(unix)]
#[test]
fn listing_and_editing_do_not_follow_symlinks() {
    use std::os::unix::fs::symlink;
    let workspace = Workspace::new();
    let outside = Workspace::new();
    outside.write("secret.txt", "outside value");
    symlink(outside.path().join("secret.txt"), workspace.path().join("linked.txt")).unwrap();
    let files = cli(&["files"], &workspace);
    assert!(!stdout(&files).contains("linked.txt"));
    let edit = cli(&["replace", "linked.txt", "outside", "changed"], &workspace);
    assert!(!edit.status.success());
    assert_eq!(fs::read_to_string(outside.path().join("secret.txt")).unwrap(), "outside value");
}

#[test]
fn context_pack_outputs_json_and_marks_sources_untrusted() {
    let workspace = Workspace::new();
    workspace.write("auth.rs", "fn authenticate() { token(); }\n");
    let output = cli(&["pack", "token"], &workspace);
    assert!(output.status.success(), "{}", stderr(&output));
    let packed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(packed["sources_are_untrusted"], true);
    assert_eq!(packed["sources"][0]["path"], "auth.rs");
}

#[test]
fn session_search_is_case_insensitive() {
    let workspace = Workspace::new();
    workspace.write(".isa/sessions/example.json", r#"{"version":1,"messages":[{"role":"user","content":"Hello ISA"}]}"#);
    let output = cli(&["session-search", "hello"], &workspace);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("example [user] Hello ISA"));
}

#[test]
fn context_pack_ignores_gitignored_files() {
    let workspace = Workspace::new();
    workspace.write(".gitignore", "hidden.txt\n");
    workspace.write("hidden.txt", "needle in a secret file");
    workspace.write("public.txt", "needle in public file");
    let output = cli(&["pack", "needle"], &workspace);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("public.txt"));
    assert!(!stdout(&output).contains("hidden.txt"));
}

#[test]
fn git_diff_distinguishes_staged_and_unstaged_changes() {
    let workspace = Workspace::new();
    let run_git = |arguments: &[&str]| {
        let result = Command::new("git").current_dir(workspace.path()).env("GIT_CONFIG_NOSYSTEM", "1").env("GIT_CONFIG_GLOBAL", if cfg!(windows) { "NUL" } else { "/dev/null" }).args(arguments).output().expect("run git");
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    };
    run_git(&["init", "-q"]);
    workspace.write("tracked.txt", "original\n");
    run_git(&["add", "tracked.txt"]);
    run_git(&["-c", "user.name=ISA Test", "-c", "user.email=isa@example.invalid", "commit", "-qm", "initial"]);
    workspace.write("tracked.txt", "staged change\n");
    run_git(&["add", "tracked.txt"]);
    workspace.write("tracked.txt", "unstaged change\n");
    let staged = cli(&["diff", "--staged"], &workspace);
    let unstaged = cli(&["diff"], &workspace);
    assert!(staged.status.success(), "{}", stderr(&staged));
    assert!(unstaged.status.success(), "{}", stderr(&unstaged));
    assert!(stdout(&staged).contains("+staged change"));
    assert!(!stdout(&staged).contains("+unstaged change"));
    assert!(stdout(&unstaged).contains("+unstaged change"));
}

#[test]
fn contracts_and_goals_can_be_created_updated_and_linked() {
    let workspace = Workspace::new();
    for args in [
        vec!["contract", "create", "reviewer", "Review code"],
        vec!["contract", "set", "reviewer", "deliverables", "Report findings"],
        vec!["contract", "set", "reviewer", "status", "ready"],
        vec!["goal", "create", "audit", "Audit the project"],
        vec!["goal", "bind", "audit", "reviewer"],
        vec!["goal", "status", "audit", "completed"],
    ] {
        let result = cli(&args, &workspace);
        assert!(result.status.success(), "{}", stderr(&result));
    }
    let contract = cli(&["contract", "show", "reviewer"], &workspace);
    let goal = cli(&["goal", "show", "audit"], &workspace);
    let contract: serde_json::Value = serde_json::from_slice(&contract.stdout).unwrap();
    let goal: serde_json::Value = serde_json::from_slice(&goal.stdout).unwrap();
    assert_eq!(contract["status"], "ready");
    assert_eq!(contract["deliverables"], "Report findings");
    assert_eq!(goal["contract_id"], "reviewer");
    assert_eq!(goal["status"], "completed");
    assert!(stdout(&cli(&["contract", "list"], &workspace)).contains("reviewer"));
    assert!(stdout(&cli(&["goal", "list"], &workspace)).contains("audit"));
}

#[test]
fn registry_prevents_duplicate_ids_and_invalid_bindings() {
    let workspace = Workspace::new();
    assert!(cli(&["goal", "create", "one", "First goal"], &workspace).status.success());
    assert!(!cli(&["goal", "create", "one", "Other goal"], &workspace).status.success());
    assert!(!cli(&["goal", "bind", "one", "missing"], &workspace).status.success());
    assert!(!cli(&["goal", "status", "one", "running"], &workspace).status.success());
    assert!(!cli(&["contract", "create", "../escape", "Bad ID"], &workspace).status.success());
    let goal = cli(&["goal", "show", "one"], &workspace);
    let goal: serde_json::Value = serde_json::from_slice(&goal.stdout).unwrap();
    assert_eq!(goal["objective"], "First goal");
    assert_eq!(goal["status"], "open");
    assert!(goal["contract_id"].is_null());
}

#[cfg(unix)]
#[test]
fn registry_rejects_symlinked_store() {
    use std::os::unix::fs::symlink;
    let workspace = Workspace::new();
    let outside = Workspace::new();
    symlink(outside.path(), workspace.path().join(".isa")).unwrap();
    let result = cli(&["contract", "create", "test", "Example"], &workspace);
    assert!(!result.status.success());
    assert!(!outside.path().join("contracts").exists());
}

#[test]
fn work_status_counts_only_changes_after_start() {
    let workspace = Workspace::new();
    workspace.write("existing.txt", "already present\nunchanged\n");
    let start = cli(&["work", "start"], &workspace);
    assert!(start.status.success(), "{}", stderr(&start));
    assert!(!cli(&["work", "start"], &workspace).status.success());
    workspace.write("existing.txt", "replaced\nunchanged\n");
    workspace.write("new.txt", "first\nsecond\n");
    let status = cli(&["work", "status"], &workspace);
    assert!(status.status.success(), "{}", stderr(&status));
    let value: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(value["added"], 3);
    assert_eq!(value["removed"], 1);
    assert_eq!(value["changed_files"], 2);
    assert!(cli(&["work", "end"], &workspace).status.success());
    assert!(!cli(&["work", "status"], &workspace).status.success());
}

#[test]
fn work_status_skips_binary_and_ignored_files() {
    let workspace = Workspace::new();
    workspace.write(".gitignore", "ignored.txt\n");
    workspace.write("ignored.txt", "before\n");
    fs::write(workspace.path().join("binary.dat"), [0, 1, 2]).unwrap();
    assert!(cli(&["work", "start"], &workspace).status.success());
    workspace.write("ignored.txt", "after\n");
    fs::write(workspace.path().join("binary.dat"), [0, 4, 5]).unwrap();
    let status = cli(&["work", "status"], &workspace);
    assert!(status.status.success(), "{}", stderr(&status));
    let value: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(value["added"], 0);
    assert_eq!(value["removed"], 0);
    assert_eq!(value["skipped_files"], 1);
}

#[test]
fn utility_commands_do_not_modify_cli_conversation() {
    use std::io::Write;
    let workspace = Workspace::new();
    let original = r#"{"version":1,"messages":[{"role":"user","content":"Existing message"}]}"#;
    workspace.write(".isa/sessions/example.json", original);
    workspace.write(".isa/summaries/example-AC.md", "Existing summary");
    let mut child = Command::new(env!("CARGO_BIN_EXE_isa-cli"))
        .current_dir(workspace.path())
        .args(["chat", "example"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn().expect("start chat");
    child.stdin.take().unwrap().write_all(b"./help/.\n./resume-AC/.\n./resume-LP/.\n/quit\n").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("Existing summary"));
    assert!(stderr(&output).contains("No saved summary"));
    assert_eq!(fs::read_to_string(workspace.path().join(".isa/sessions/example.json")).unwrap(), original);
}

#[test]
fn resume_rejects_invalid_session_ids_and_variants() {
    let workspace = Workspace::new();
    assert!(!cli(&["resume", "../outside", "AC"], &workspace).status.success());
    assert!(!cli(&["resume", "example", "invalid"], &workspace).status.success());
}

#[test]
fn decoy_server_reports_missing_provider() {
    use std::io::{Read, Write};
    use std::net::TcpStream;
    use std::time::Duration;
    struct ChildGuard(std::process::Child);
    impl Drop for ChildGuard {
        fn drop(&mut self) { let _ = self.0.kill(); let _ = self.0.wait(); }
    }
    let workspace = Workspace::new();
    let process = Command::new(env!("CARGO_BIN_EXE_isa-decoy"))
        .current_dir(workspace.path()).stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().expect("start decoy");
    let mut guard = ChildGuard(process);
    let started = std::time::Instant::now();
    let mut stream = loop {
        if started.elapsed() > Duration::from_secs(5) { panic!("decoy did not start in time"); }
        if let Some(status) = guard.0.try_wait().unwrap() { panic!("decoy exited early: {status}"); }
        match TcpStream::connect("127.0.0.1:4180") {
            Ok(stream) => break stream,
            Err(_) => std::thread::sleep(Duration::from_millis(20)),
        }
    };
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    let body = r#"{"message":"Hello"}"#;
    write!(stream, "POST /v1/chat HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}", body.len(), body).unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    assert!(response.contains("503 Service Unavailable"), "{response}");
    let value: serde_json::Value = serde_json::from_str(response.split("\r\n\r\n").nth(1).unwrap()).unwrap();
    assert_eq!(value["provider"], "decoy");
    assert_eq!(value["error"]["code"], "no_provider");
}

#[test]
fn skill_commands_manage_reviewed_local_guidance() {
    let workspace = Workspace::new();
    workspace.write("skill.txt", "Review Rust tests before reporting completion.\n");
    let output = cli(&["skill", "create", "rust-testing", "skill.txt"], &workspace);
    assert!(output.status.success(), "{}", stderr(&output));
    let list = cli(&["skill", "list"], &workspace);
    assert!(stdout(&list).contains("rust-testing [disabled]"));
    assert!(cli(&["skill", "enable", "rust-testing"], &workspace).status.success());
    assert!(stdout(&cli(&["skill", "show", "rust-testing"], &workspace)).contains("[enabled]"));
    workspace.write("skill.txt", "Updated advice.\n");
    assert!(cli(&["skill", "edit", "rust-testing", "skill.txt"], &workspace).status.success());
    assert!(stdout(&cli(&["skill", "show", "rust-testing"], &workspace)).contains("[disabled]"));
    assert!(cli(&["skill", "delete", "rust-testing"], &workspace).status.success());
    assert!(stdout(&cli(&["skill", "list"], &workspace)).is_empty());
}

#[test]
fn skill_proposals_require_interactive_review() {
    let workspace = Workspace::new();
    let output = cli(&["skill", "propose", "test-skill", "Review the task"], &workspace);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("interactive terminal"));
    assert!(!workspace.path().join(".isa/skills/test-skill.json").exists());
}

#[test]
fn process_supervisor_requires_interactive_approval() {
    let workspace = Workspace::new();
    let output = cli(&["processes"], &workspace);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("interactive terminal"));
}

#[test]
fn check_edit_is_read_only_and_rejects_invalid_proposals() {
    let workspace = Workspace::new();
    let proposal = "File: src/a.txt\n<<<<<<< SEARCH\nold\n=======\nnew\n>>>>>>> REPLACE\n";
    workspace.write("src/a.txt", "first\nold text\n");
    workspace.write("proposal.txt", proposal);
    let result = cli(&["check-edit", "proposal.txt"], &workspace);
    assert!(result.status.success(), "{}", stderr(&result));
    let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["file"], "src/a.txt");
    assert_eq!(value["line"], 2);
    assert_eq!(value["readOnly"], true);
    assert_eq!(value["search"], "old");
    assert_eq!(value["replacement"], "new");
    assert_eq!(fs::read_to_string(workspace.path().join("src/a.txt")).unwrap(), "first\nold text\n");
    assert_eq!(fs::read_to_string(workspace.path().join("proposal.txt")).unwrap(), proposal);
    assert!(!workspace.path().join(".isa").exists());
    workspace.write("src/a.txt", "old old");
    let failed = cli(&["check-edit", "proposal.txt"], &workspace);
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    assert!(!workspace.path().join(".isa").exists());
    assert_eq!(fs::read_to_string(workspace.path().join("src/a.txt")).unwrap(), "old old");
    workspace.write("proposal.txt", "File: ../outside\n<<<<<<< SEARCH\nold\n=======\nnew\n>>>>>>> REPLACE");
    assert!(!cli(&["check-edit", "proposal.txt"], &workspace).status.success());
    workspace.write("proposal.txt", &"x".repeat(32769));
    assert!(!cli(&["check-edit", "proposal.txt"], &workspace).status.success());
    assert!(!cli(&["check-edit", "proposal.txt", ".", "extra"], &workspace).status.success());
}

#[cfg(unix)]
#[test]
fn check_edit_rejects_symlinked_proposal_and_target() {
    use std::os::unix::fs::symlink;
    let workspace = Workspace::new();
    workspace.write("actual.txt", "old");
    workspace.write("proposal.txt", "File: link.txt\n<<<<<<< SEARCH\nold\n=======\nnew\n>>>>>>> REPLACE");
    symlink("actual.txt", workspace.path().join("link.txt")).unwrap();
    symlink("proposal.txt", workspace.path().join("link-proposal.txt")).unwrap();
    assert!(!cli(&["check-edit", "proposal.txt"], &workspace).status.success());
    assert!(!cli(&["check-edit", "link-proposal.txt"], &workspace).status.success());
    assert_eq!(fs::read_to_string(workspace.path().join("actual.txt")).unwrap(), "old");
    assert!(!workspace.path().join(".isa").exists());
}

#[cfg(unix)]
#[test]
fn interactive_edit_reports_syntax_before_denied_approval() {
    use std::io::Write;
    use std::os::fd::FromRawFd;
    let workspace = Workspace::new();
    workspace.write("source.rs", "fn main() {}\n");
    workspace.write("proposal.txt", "File: source.rs\n<<<<<<< SEARCH\nmain()\n=======\nmain(\n>>>>>>> REPLACE");
    let mut master = -1; let mut slave = -1;
    let result = unsafe { libc::openpty(&mut master, &mut slave, std::ptr::null_mut(), std::ptr::null(), std::ptr::null()) };
    assert_eq!(result, 0, "could not allocate a pseudo-terminal");
    let mut terminal = unsafe { fs::File::from_raw_fd(master) };
    let input = unsafe { fs::File::from_raw_fd(slave) };
    let child = Command::new(env!("CARGO_BIN_EXE_isa-cli"))
        .current_dir(workspace.path()).args(["apply-edit", "proposal.txt"])
        .stdin(Stdio::from(input)).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    terminal.write_all(b"n\n").unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    let text = stdout(&output);
    assert!(text.contains("Syntax comparison (Tree-sitter, advisory only"), "{text}");
    assert!(text.contains("Before: no errors detected"), "{text}");
    assert!(text.contains("After: errors detected"), "{text}");
    assert!(stderr(&output).contains("Apply this replacement?"));
    assert_eq!(fs::read_to_string(workspace.path().join("source.rs")).unwrap(), "fn main() {}\n");
    assert!(!workspace.path().join(".isa").exists());
}

#[cfg(unix)]
#[test]
fn feedback_requires_separate_approvals_and_does_not_edit() {
    use std::io::{Read, Write};
    use std::os::fd::FromRawFd;
    fn run(workspace: &Workspace, command: &str, replies: &[u8]) -> Output {
        let mut master = -1; let mut slave = -1;
        assert_eq!(unsafe { libc::openpty(&mut master, &mut slave, std::ptr::null_mut(), std::ptr::null(), std::ptr::null()) }, 0);
        let mut terminal = unsafe { fs::File::from_raw_fd(master) };
        let input = unsafe { fs::File::from_raw_fd(slave) };
        let output_terminal = input.try_clone().unwrap();
        let child = Command::new(env!("CARGO_BIN_EXE_isa-cli")).current_dir(workspace.path())
            .args(["feedback", "source.rs", "fix it", "--", "sh", "-c", command])
            .stdin(Stdio::from(input)).stdout(Stdio::from(output_terminal)).stderr(Stdio::piped()).spawn().unwrap();
        terminal.write_all(replies).unwrap();
        let mut output = child.wait_with_output().unwrap();
        let _ = terminal.read_to_end(&mut output.stdout); // PTY may end with EIO on Linux.
        output
    }
    let workspace = Workspace::new(); workspace.write("source.rs", "fn main() {}\n");
    let denied = run(&workspace, "touch should-not-exist", b"n\n");
    assert!(!denied.status.success()); assert!(!workspace.path().join("should-not-exist").exists());
    let failed = run(&workspace, "printf test-failure >&2; exit 1", b"y\nn\n");
    assert!(!failed.status.success()); let text = stdout(&failed);
    assert!(text.contains("test-failure")); assert!(text.contains("Send file and diagnostic to Ollama"));
    assert!(stderr(&failed).contains("model request cancelled"));
    let passed = run(&workspace, "exit 0", b"y\n");
    assert!(passed.status.success(), "{}", stderr(&passed));
    assert!(stdout(&passed).contains("No retry was requested"));
    assert!(!stdout(&passed).contains("Send file and diagnostic to Ollama"));
    assert_eq!(fs::read_to_string(workspace.path().join("source.rs")).unwrap(), "fn main() {}\n");
    assert!(!workspace.path().join(".isa").exists());
}

#[test]
fn check_edit_reports_whitespace_hint_without_changing_files() {
    let workspace = Workspace::new();
    workspace.write("source.txt", "private text\nlet  x = 1;\n");
    workspace.write("proposal.txt", "File: source.txt\n<<<<<<< SEARCH\nlet x = 1;\n=======\nlet x = 2;\n>>>>>>> REPLACE");
    for command in ["check-edit", "apply-edit"] {
        let output = cli(&[command, "proposal.txt"], &workspace);
        assert!(!output.status.success()); assert!(stderr(&output).contains("candidate lines"));
        assert!(stderr(&output).contains("2-2")); assert!(!stderr(&output).contains("private text"));
        assert_eq!(fs::read_to_string(workspace.path().join("source.txt")).unwrap(), "private text\nlet  x = 1;\n");
        assert!(!workspace.path().join(".isa").exists());
    }
}

#[test]
fn duplicate_edit_reports_exact_locations_without_changing_files() {
    let workspace = Workspace::new();
    workspace.write("source.txt", "private\nold\nother\nold\n");
    workspace.write("proposal.txt", "File: source.txt\n<<<<<<< SEARCH\nold\n=======\nnew\n>>>>>>> REPLACE");
    for command in ["check-edit", "apply-edit"] {
        let output = cli(&[command, "proposal.txt"], &workspace);
        assert!(!output.status.success()); let err = stderr(&output);
        assert!(err.contains("Exact match candidate lines")); assert!(err.contains("2-2")); assert!(err.contains("4-4"));
        assert!(!err.contains("private")); assert!(!workspace.path().join(".isa").exists());
        assert_eq!(fs::read_to_string(workspace.path().join("source.txt")).unwrap(), "private\nold\nother\nold\n");
    }
}

#[cfg(unix)]
#[test]
fn mcp_stdio_requires_server_and_tool_approval() {
    use std::io::{Read, Write};
    use std::os::fd::FromRawFd;
    fn run(workspace: &Workspace, args: &[String], replies: &[u8]) -> Output {
        let mut master = -1; let mut slave = -1;
        assert_eq!(unsafe { libc::openpty(&mut master, &mut slave, std::ptr::null_mut(), std::ptr::null(), std::ptr::null()) }, 0);
        let mut terminal = unsafe { fs::File::from_raw_fd(master) };
        let input = unsafe { fs::File::from_raw_fd(slave) }; let output = input.try_clone().unwrap();
        let child = Command::new(env!("CARGO_BIN_EXE_isa-cli")).current_dir(workspace.path()).args(args)
            .stdin(Stdio::from(input)).stdout(Stdio::from(output)).stderr(Stdio::piped()).spawn().unwrap();
        terminal.write_all(replies).unwrap();
        let mut result = child.wait_with_output().unwrap(); let _ = terminal.read_to_end(&mut result.stdout); result
    }
    let workspace = Workspace::new();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp-server.cjs").to_string_lossy().into_owned();
    let marker = workspace.path().join("called").to_string_lossy().into_owned();
    let args = vec!["mcp".into(), "call".into(), "write".into(), "{\"value\":1}".into(), "--".into(), "node".into(), fixture, "valid".into(), marker.clone()];
    let denied_server = run(&workspace, &args, b"n\n"); assert!(!denied_server.status.success()); assert!(!workspace.path().join("called.started").exists());
    let denied_tool = run(&workspace, &args, b"y\nn\n"); assert!(!denied_tool.status.success()); assert!(workspace.path().join("called.started").exists()); assert!(!workspace.path().join("called").exists()); assert!(stdout(&denied_tool).contains("Call this exact tool"));
    let allowed = run(&workspace, &args, b"y\ny\n"); assert!(allowed.status.success(), "{}", stderr(&allowed)); assert!(stdout(&allowed).contains("fixture result"));
    let params: serde_json::Value = serde_json::from_slice(&fs::read(workspace.path().join("called")).unwrap()).unwrap(); assert_eq!(params["name"], "write"); assert_eq!(params["arguments"]["value"], 1);
}


// Run explicitly in CI after installing the locked official SDK test dependencies.
#[cfg(unix)]
#[test]
fn mcp_official_sdk_stdio_interoperability() {
    if std::env::var("ISA_MCP_SDK_TEST").as_deref() != Ok("1") { return; }
    use std::io::{Read, Write};
    use std::os::fd::FromRawFd;
    fn run(workspace: &Workspace, args: &[String], replies: &[u8]) -> Output {
        let mut master = -1; let mut slave = -1;
        assert_eq!(unsafe { libc::openpty(&mut master, &mut slave, std::ptr::null_mut(), std::ptr::null(), std::ptr::null()) }, 0);
        let mut terminal = unsafe { fs::File::from_raw_fd(master) };
        let input = unsafe { fs::File::from_raw_fd(slave) }; let output = input.try_clone().unwrap();
        let child = Command::new(env!("CARGO_BIN_EXE_isa-cli")).current_dir(workspace.path()).args(args)
            .stdin(Stdio::from(input)).stdout(Stdio::from(output)).stderr(Stdio::piped()).spawn().unwrap();
        terminal.write_all(replies).unwrap();
        let mut result = child.wait_with_output().unwrap(); let _ = terminal.read_to_end(&mut result.stdout); result
    }
    let workspace = Workspace::new();
    let server = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/mcp-sdk/server.mjs").to_string_lossy().into_owned();
    let marker = workspace.path().join("called").to_string_lossy().into_owned();
    let command = vec!["node".to_string(), server, marker.clone()];
    let mut list = vec!["mcp".into(), "list".into(), "--".into()]; list.extend(command.clone());
    let listed = run(&workspace, &list, b"y\n");
    assert!(listed.status.success(), "{}", stderr(&listed)); assert!(stdout(&listed).contains("sdk_echo")); assert!(stdout(&listed).contains("sdk_error")); assert!(!workspace.path().join("called").exists());
    let mut call = vec!["mcp".into(), "call".into(), "sdk_echo".into(), r#"{"message":"hello"}"#.into(), "--".into()]; call.extend(command.clone());
    let denied = run(&workspace, &call, b"y\nn\n"); assert!(!denied.status.success()); assert!(!workspace.path().join("called").exists());
    let accepted = run(&workspace, &call, b"y\ny\n"); assert!(accepted.status.success(), "{}", stderr(&accepted)); assert!(stdout(&accepted).contains("SDK echo: hello"));
    let data: serde_json::Value = serde_json::from_slice(&fs::read(&marker).unwrap()).unwrap(); assert_eq!(data["message"], "hello");
    let mut fail = vec!["mcp".into(), "call".into(), "sdk_error".into(), "{}".into(), "--".into()]; fail.extend(command);
    let error = run(&workspace, &fail, b"y\ny\n"); assert!(!error.status.success()); assert!(stdout(&error).contains("SDK tool failure")); assert!(stderr(&error).contains("MCP tool reported an error"));
}
