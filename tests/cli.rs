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
