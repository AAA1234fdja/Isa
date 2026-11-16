#[path = "../../Harness Isa/mod.rs"]
mod harness;
#[path = "../../Skills/mod.rs"]
mod skills;
mod connectors;
mod context;
mod agent_loop;
mod semantic;
mod edit;
mod registry;
mod repository;
mod processes;
mod session;
mod symbols;
mod summary;
mod work;

use std::error::Error;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

const HELP: &str = "ISA CLI\n\n\
Usage:\n\
  isa-cli files [directory]                 List workspace files\n\
  isa-cli search <text> [directory]         Search text in workspace files\n\
  isa-cli context [directory]               Show repository and git context\n\
  isa-cli diff [--staged] [directory]       Show a bounded Git diff\n\
  isa-cli processes [directory]             Supervise approved local processes\n\
  isa-cli skill <command> ...               Manage reusable skills\n\
  isa-cli harness-eval <suite.json> <mode>   Evaluate response harness checks\n\
  isa-cli semantic-index <model> [directory] Build local embedding index\n\
  isa-cli semantic-search <query> [directory] Search the embedding index\n\
  isa-cli rank <query> [directory]          Rank relevant files with lexical search\n\
  isa-cli callers <name> [directory]       Find syntactic call sites (no name resolution)\n\
  isa-cli symbols <name> [directory]       Find syntax declarations (Rust, Python, JS, TS)\n\
  isa-cli pack <query> [directory]          Output a bounded context pack as JSON\n\
  isa-cli replace <file> <old> <new> [directory]  Review and apply an exact replacement\n\
  isa-cli apply-edit <proposal-file> [directory]  Review one SEARCH/REPLACE block\n\
  isa-cli propose-edit <file> <instruction> [directory]  Generate and review one edit\n\
  isa-cli undo [directory]                  Review and undo the last replacement\n\
  isa-cli connector github <command>        Manage the GitHub connection\n\
  isa-cli contract <command> ...            Manage subagent contracts\n\
  isa-cli goal <command> ...                Manage goals\n\
  isa-cli work <start|status|end> [directory]  Track file changes during work\n\
  isa-cli sessions [directory]              List saved sessions\n\
  isa-cli session-search <text> [directory] Search local session messages\n\
  isa-cli history <session-id> [directory]  Show a saved session\n\
  isa-cli resume <session-id> <AC|LP> [directory]  Show a saved summary\n\
  isa-cli chat <session-id> [directory]     Chat with a local Ollama model\n\
  isa-cli ask <session-id> <prompt> [directory]  Run one local model request\n\n\
Set ISA_MODEL to select an Ollama model (default: llama3.2).\n\
Set ISA_HARNESS=0 to disable response adaptation.\n\
Set ISA_AGENT_LOOP=1 to enable a bounded read-only workspace search loop.\n\
Sessions are stored locally under .isa/sessions.\n\
The model cannot execute commands or edit files without explicit CLI review and approval.";

fn argument(args: &[String], index: usize, name: &str) -> Result<String> {
    args.get(index)
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .ok_or_else(|| format!("missing {name}").into())
}

fn directory(args: &[String], index: usize) -> Result<PathBuf> {
    let path = match args.get(index) {
        Some(path) => PathBuf::from(path),
        None => std::env::current_dir()?,
    };
    let root = path.canonicalize()?;
    if !root.is_dir() {
        return Err("workspace must be a directory".into());
    }
    Ok(root)
}

pub fn run(args: Vec<String>) -> Result<()> {
    match args.first().map(String::as_str) {
        None | Some("help" | "--help" | "-h") => println!("{HELP}"),
        Some("files") => {
            let root = directory(&args, 1)?;
            for file in repository::files(&root)? {
                println!("{}", file.display());
            }
        }
        Some("search") => {
            let query = argument(&args, 1, "search text")?;
            let root = directory(&args, 2)?;
            let matches = repository::search(&root, &query)?;
            for hit in &matches {
                println!("{}:{}:{}", hit.path.display(), hit.line, hit.text);
            }
            eprintln!("{} match(es)", matches.len());
        }
        Some("context") => {
            let root = directory(&args, 1)?;
            let files = repository::files(&root)?;
            println!("Workspace: {}\nFiles: {}", root.display(), files.len());
            for path in files.iter().take(100) {
                println!("  {}", path.display());
            }
            if files.len() > 100 {
                println!("  ... {} more", files.len() - 100);
            }
            match repository::git_context(&root) {
                Ok(context) => println!("\nGit status:\n{context}"),
                Err(error) => eprintln!("Git context unavailable: {error}"),
            }
        }
        Some("diff") => {
            let staged = args.get(1).is_some_and(|arg| arg == "--staged");
            let root = directory(&args, if staged { 2 } else { 1 })?;
            print!("{}", repository::git_diff(&root, staged)?);
        }
        Some("processes") => { if args.len() > 2 { return Err("usage: isa-cli processes [directory]".into()); } let root = directory(&args, 1)?; processes::interactive(&root)?; }
        Some("skill") => skill_command(&args[1..])?,
        Some("harness-eval") => {
            let suite = argument(&args, 1, "suite file")?;
            let mode = argument(&args, 2, "mode: quick, balanced, or thorough")?;
            if args.len() != 3 { return Err("usage: isa-cli harness-eval <suite.json> <quick|balanced|thorough>".into()); }
            let model = std::env::var("ISA_MODEL").unwrap_or_else(|_| "llama3.2".to_string());
            if !harness::evaluate(std::path::Path::new(&suite), &model, &mode)? { return Err("one or more harness evaluation cases failed".into()); }
        }
        Some("semantic-index") => {
            let model = argument(&args, 1, "embedding model")?;
            let root = directory(&args, 2)?;
            semantic::build(&root, &model)?;
        }
        Some("semantic-search") => {
            let query = argument(&args, 1, "query")?;
            let root = directory(&args, 2)?;
            println!("{}", serde_json::to_string_pretty(&semantic::search(&root, &query)?)?);
        }
        Some("rank") => {
            let query = argument(&args, 1, "query")?;
            let root = directory(&args, 2)?;
            for result in context::rank(&root, &query, 20)? {
                println!("{} ({:.3})\n{}\n", result.path.display(), result.score, result.excerpt);
            }
        }
        Some("callers") => {
            let query = argument(&args, 1, "call name")?;
            let root = directory(&args, 2)?;
            println!("{}", serde_json::to_string_pretty(&symbols::callers(&root, &query)?)?);
        }
        Some("symbols") => {
            let query = argument(&args, 1, "symbol name")?;
            let root = directory(&args, 2)?;
            println!("{}", serde_json::to_string_pretty(&symbols::lookup(&root, &query)?)?);
        }
        Some("pack") => {
            let query = argument(&args, 1, "query")?;
            let root = directory(&args, 2)?;
            let packed = context::pack(&root, &query)?;
            println!("{}", serde_json::to_string_pretty(&packed)?);
        }
        Some("replace") => {
            let file = argument(&args, 1, "file")?;
            let old = argument(&args, 2, "search text")?;
            let new = args.get(3).ok_or("missing replacement text")?;
            let root = directory(&args, 4)?;
            edit::replace(&root, std::path::Path::new(&file), &old, new)?;
        }
        Some("apply-edit") => {
            let proposal = argument(&args, 1, "proposal file")?;
            let root = directory(&args, 2)?;
            edit::apply_proposal(&root, std::path::Path::new(&proposal))?;
        }
        Some("propose-edit") => {
            if args.len() > 4 { return Err("usage: isa-cli propose-edit <file> <instruction> [directory]".into()); }
            let file = argument(&args, 1, "file")?;
            let instruction = argument(&args, 2, "instruction")?;
            let root = directory(&args, 3)?;
            let model = std::env::var("ISA_MODEL").unwrap_or_else(|_| "llama3.2".to_string());
            edit::propose_edit(&root, std::path::Path::new(&file), &instruction, &model)?;
        }
        Some("undo") => {
            let root = directory(&args, 1)?;
            edit::undo(&root)?;
        }
        Some("connector") => {
            if args.get(1).map(String::as_str) != Some("github") { return Err("only GitHub CLI authorization is implemented; Google Drive, Gmail, and Notion are not connected yet".into()); }
            match args.get(2).map(String::as_str) {
                Some("connect") => connectors::connect_github()?,
                Some("status") => connectors::github_status()?,
                Some("enable") => connectors::set_github_enabled(true)?,
                Some("disable") => connectors::set_github_enabled(false)?,
                Some("whoami") => connectors::github_whoami()?,
                Some("disconnect") => connectors::disconnect_github()?,
                _ => return Err("usage: connector github <connect|status|enable|disable|whoami|disconnect>".into()),
            }
        }
        Some("work") => {
            let action = argument(&args, 1, "work action")?;
            let root = directory(&args, 2)?;
            match action.as_str() {
                "start" => { work::start(&root)?; println!("Work started. Existing files are the baseline."); }
                "status" => {
                    let status = work::status(&root)?;
                    println!("{}", serde_json::to_string_pretty(&serde_json::json!({"added":status.added,"removed":status.removed,"changed_files":status.changed_files,"skipped_files":status.skipped_files}))?);
                }
                "end" => { work::end(&root)?; println!("Work ended; baseline removed."); }
                _ => return Err("usage: work <start|status|end> [directory]".into()),
            }
        }
        Some("contract") => contract_command(&args)?,
        Some("goal") => goal_command(&args)?,
        Some("sessions") => {
            let root = directory(&args, 1)?;
            for id in session::list(&root)? {
                println!("{id}");
            }
        }
        Some("session-search") => {
            let query = argument(&args, 1, "search text")?;
            let root = directory(&args, 2)?;
            let matches = session::search(&root, &query)?;
            for hit in &matches {
                println!("{} [{}] {}", hit.id, hit.role, hit.excerpt);
            }
            eprintln!("{} match(es)", matches.len());
        }
        Some("history") => {
            let id = argument(&args, 1, "session ID")?;
            let root = directory(&args, 2)?;
            let messages = session::load(&root, &id)?;
            for message in messages {
                println!("{}: {}\n", message.role, message.content);
            }
        }
        Some("resume") => {
            let id = argument(&args, 1, "session ID")?;
            let variant = summary::Variant::parse(&argument(&args, 2, "summary variant")?)?;
            let root = directory(&args, 3)?;
            println!("{}", summary::read(&root, &id, variant)?);
        }
        Some("chat") => {
            let id = argument(&args, 1, "session ID")?;
            let root = directory(&args, 2)?;
            let model = std::env::var("ISA_MODEL").unwrap_or_else(|_| "llama3.2".to_string());
            chat(&root, &id, &model)?;
        }
        Some("ask") => {
            let id = argument(&args, 1, "session ID")?;
            let prompt = argument(&args, 2, "prompt")?;
            let root = directory(&args, 3)?;
            let model = std::env::var("ISA_MODEL").unwrap_or_else(|_| "llama3.2".to_string());
            let mut messages = session::load(&root, &id)?;
            run_turn(&root, &id, &model, &mut messages, &prompt)?;
        }
        Some(other) => return Err(format!("unknown command: {other}\n\n{HELP}").into()),
    }
    Ok(())
}

fn skill_command(args: &[String]) -> Result<()> {
    match args.first().map(String::as_str) {
        Some("list") if args.len() <= 2 => {
            let root = directory(args, 1)?;
            for (id, enabled) in skills::list(&root)? { println!("{} [{}]", id, if enabled { "enabled" } else { "disabled" }); }
        }
        Some("show") if (2..=3).contains(&args.len()) => {
            let id = argument(args, 1, "skill ID")?; let root = directory(args, 2)?;
            println!("{}", skills::show(&root, &id)?);
        }
        Some(action @ ("create" | "edit")) if (3..=4).contains(&args.len()) => {
            let id = argument(args, 1, "skill ID")?; let path = argument(args, 2, "skill text file")?; let root = directory(args, 3)?;
            let content = skills::read_input(std::path::Path::new(&path))?;
            skills::save(&root, &id, &content, action == "create")?;
            println!("Skill {id} saved disabled. Review and enable it before use.");
        }
        Some(action @ ("enable" | "disable" | "delete")) if (2..=3).contains(&args.len()) => {
            let id = argument(args, 1, "skill ID")?; let root = directory(args, 2)?;
            if action == "delete" { skills::remove(&root, &id)?; println!("Skill {id} deleted."); }
            else { skills::set_enabled(&root, &id, action == "enable")?; println!("Skill {id} {}.", if action == "enable" { "enabled" } else { "disabled" }); }
        }
        Some("plan") if (2..=3).contains(&args.len()) => {
            let task = argument(args, 1, "task")?; let root = directory(args, 2)?;
            let model = std::env::var("ISA_MODEL").unwrap_or_else(|_| "llama3.2".into());
            harness::task_plan(&root, &model, &task)?;
        }
        Some("propose") if (3..=4).contains(&args.len()) => {
            let id = argument(args, 1, "skill ID")?; let task = argument(args, 2, "task")?; let root = directory(args, 3)?;
            let model = std::env::var("ISA_MODEL").unwrap_or_else(|_| "llama3.2".into());
            skills::propose(&root, &id, &task, &model)?;
        }
        _ => return Err("usage: skill plan <task> [dir] | list [dir] | show <id> [dir] | create|edit <id> <text-file> [dir] | enable|disable|delete <id> [dir] | propose <id> <task> [dir]".into()),
    }
    Ok(())
}

fn contract_command(args: &[String]) -> Result<()> {
    use registry::Kind;
    match args.get(1).map(String::as_str) {
        Some("create") => {
            let id = argument(args, 2, "contract ID")?;
            let objective = argument(args, 3, "objective")?;
            let root = directory(args, 4)?;
            registry::create_contract(&root, &id, &objective)?;
            println!("Created contract {id} (draft; no agent runtime connected).");
        }
        Some("set") => {
            let id = argument(args, 2, "contract ID")?;
            let field = argument(args, 3, "field")?;
            let value = argument(args, 4, "value")?;
            let root = directory(args, 5)?;
            registry::set_contract(&root, &id, &field, &value)?;
            println!("Updated contract {id}.");
        }
        Some("show") => {
            let id = argument(args, 2, "contract ID")?;
            let root = directory(args, 3)?;
            println!("{}", serde_json::to_string_pretty(&registry::read(&root, Kind::Contract, &id)?)?);
        }
        Some("list") => {
            let root = directory(args, 2)?;
            for id in registry::list(&root, Kind::Contract)? { println!("{id}"); }
        }
        _ => return Err("usage: contract create <id> <objective> [directory] | set <id> <field> <value> [directory] | show <id> [directory] | list [directory]".into()),
    }
    Ok(())
}

fn goal_command(args: &[String]) -> Result<()> {
    use registry::Kind;
    match args.get(1).map(String::as_str) {
        Some("create") => {
            let id = argument(args, 2, "goal ID")?;
            let objective = argument(args, 3, "objective")?;
            let root = directory(args, 4)?;
            registry::create_goal(&root, &id, &objective)?;
            println!("Created goal {id}.");
        }
        Some("status") => {
            let id = argument(args, 2, "goal ID")?;
            let status = argument(args, 3, "status")?;
            let root = directory(args, 4)?;
            registry::set_goal_status(&root, &id, &status)?;
            println!("Updated goal {id}.");
        }
        Some("bind") => {
            let id = argument(args, 2, "goal ID")?;
            let contract = argument(args, 3, "contract ID")?;
            let root = directory(args, 4)?;
            registry::bind_goal(&root, &id, &contract)?;
            println!("Linked goal {id} to contract {contract}.");
        }
        Some("show") => {
            let id = argument(args, 2, "goal ID")?;
            let root = directory(args, 3)?;
            println!("{}", serde_json::to_string_pretty(&registry::read(&root, Kind::Goal, &id)?)?);
        }
        Some("list") => {
            let root = directory(args, 2)?;
            for id in registry::list(&root, Kind::Goal)? { println!("{id}"); }
        }
        _ => return Err("usage: goal create <id> <objective> [directory] | status <id> <open|paused|completed> [directory] | bind <id> <contract-id> [directory] | show <id> [directory] | list [directory]".into()),
    }
    Ok(())
}

fn chat(root: &std::path::Path, id: &str, model: &str) -> Result<()> {
    let mut messages = session::load(root, id)?;
    println!("ISA · local model: {model}\nType /quit to exit. Model output cannot run tools.");
    loop {
        print!("> ");
        io::stdout().flush()?;
        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            break;
        }
        let input = input.trim();
        if input == "/quit" {
            break;
        }
        if input.eq_ignore_ascii_case("./help/.") {
            println!("Commands: ./help/. shows this help; ./resume-AC/. shows the all-chat summary; ./resume-LP/. shows the latest-prompt summary; /quit exits. These commands do not add messages to the session. Summaries are generated after completed model responses.");
            continue;
        }
        let variant = if input.eq_ignore_ascii_case("./resume-AC/.") { Some(summary::Variant::AllChat) } else if input.eq_ignore_ascii_case("./resume-LP/.") { Some(summary::Variant::LastPrompt) } else { None };
        if let Some(variant) = variant {
            match summary::read(root, id, variant) {
                Ok(text) => println!("{text}"),
                Err(_) => eprintln!("No saved summary is available for this session and variant."),
            }
            continue;
        }
        if input.is_empty() {
            continue;
        }
        if let Err(error) = run_turn(root, id, model, &mut messages, input) {
            eprintln!("\nModel request failed: {error}");
        }
    }
    Ok(())
}

fn run_turn(root: &std::path::Path, id: &str, model: &str, messages: &mut Vec<session::Message>, input: &str) -> Result<()> {
    if input.trim().is_empty() { return Err("message must not be empty".into()); }
    if input.len() > 64 * 1024 { return Err("message exceeds the 64 KiB limit".into()); }
    messages.push(session::Message::new("user", input));
    if let Err(error) = session::save(root, id, messages) {
        messages.pop();
        return Err(error);
    }
    let terminal = io::stdout().is_terminal();
    let display = |chunk: &str| -> Result<()> {
        let safe;
        let displayed = if terminal {
            safe = chunk.chars().map(|c| if c == '\n' || c == '\t' || !c.is_control() { c } else { '�' }).collect::<String>();
            safe.as_str()
        } else { chunk };
        print!("{displayed}");
        io::stdout().flush()?;
        Ok(())
    };
    let reply = if std::env::var("ISA_AGENT_LOOP").as_deref() == Ok("1") {
        let outcome = agent_loop::run(root, model, messages)?;
        for step in &outcome.steps { eprintln!("ISA agent loop: {}", step); }
        display(&outcome.answer)?;
        outcome.answer
    } else if std::env::var("ISA_HARNESS").as_deref() == Ok("0") {
        session::ollama_stream(model, messages, display)?
    } else {
        harness::respond(root, model, messages, display)?
    };
    println!();
    messages.push(session::Message::new("assistant", &reply));
    if let Err(error) = session::save(root, id, messages) {
        messages.pop();
        return Err(error);
    }
    for variant in [summary::Variant::AllChat, summary::Variant::LastPrompt] {
        if let Err(error) = summary::generate(root, id, model, variant, messages) {
            eprintln!("Summary update failed: {error}");
        }
    }
    Ok(())
}

// Accounting is scoped to the server request's thread and includes nested harness calls.
pub fn track_web_usage<T>(work: impl FnOnce() -> Result<T>) -> (Result<T>, Vec<Option<serde_json::Value>>) {
    session::track_usage(work)
}

// This loop is separately opted in and is restricted to read-only workspace search.
pub fn web_agent_loop(root: &std::path::Path, model: &str, entries: &[(String, String)], images: Vec<String>) -> Result<(String, Vec<serde_json::Value>)> {
    let mut messages: Vec<_> = entries.iter().map(|(role, content)| session::Message::new(role, content)).collect();
    if let Some(last) = messages.last_mut() { last.images = images; }
    let outcome = agent_loop::run(root, model, &messages)?;
    Ok((outcome.answer, outcome.steps))
}

// The browser server passes only validated conversation roles to this entry point.
pub fn web_response(root: &std::path::Path, model: &str, entries: &[(String, String)], images: Vec<String>) -> Result<String> {
    let mut messages: Vec<_> = entries.iter().map(|(role, content)| session::Message::new(role, content)).collect();
    if let Some(last) = messages.last_mut() { last.images = images; }
    if std::env::var("ISA_HARNESS").as_deref() == Ok("0") {
        session::ollama_stream(model, &messages, |_| Ok(()))
    } else {
        harness::respond(root, model, &messages, |_| Ok(()))
    }
}

pub fn web_preview_edit(root: &std::path::Path, proposal: &str) -> std::result::Result<serde_json::Value, &'static str> {
    edit::preview_proposal(root, proposal)
}

// Server workspace is selected by trusted configuration, never by the request.
pub fn web_symbols(root: &std::path::Path, query: &str) -> Result<serde_json::Value> {
    symbols::lookup(root, query)
}

pub fn web_callers(root: &std::path::Path, query: &str) -> Result<serde_json::Value> {
    symbols::callers(root, query)
}

// Diff access is read-only and only exposed by the server after explicit configuration.
pub fn web_changes(root: &std::path::Path) -> Result<serde_json::Value> {
    let staged = repository::git_diff(root, true)?;
    let unstaged = repository::git_diff(root, false)?;
    Ok(serde_json::json!({"staged": staged, "unstaged": unstaged}))
}

#[cfg(test)]
mod web_changes_tests {
    use super::*;
    use std::process::Command;
    #[test]
    fn reports_real_staged_and_unstaged_changes() {
        let dir = std::env::temp_dir().join(format!("isa-web-diff-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&dir).unwrap();
        let git = |args: &[&str]| {
            let result = Command::new("git").current_dir(&dir).env("GIT_CONFIG_NOSYSTEM", "1").env("GIT_CONFIG_GLOBAL", "/dev/null").args(args).output().unwrap();
            assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
        };
        git(&["init", "-q"]); git(&["config", "user.name", "ISA Test"]); git(&["config", "user.email", "test@example.invalid"]);
        std::fs::write(dir.join("file.txt"), "original\n").unwrap(); git(&["add", "file.txt"]); git(&["commit", "-qm", "initial"]);
        std::fs::write(dir.join("file.txt"), "staged\n").unwrap(); git(&["add", "file.txt"]);
        std::fs::write(dir.join("file.txt"), "unstaged\n").unwrap();
        std::fs::write(dir.join("untracked.txt"), "not included\n").unwrap();
        let data = web_changes(&dir).unwrap();
        assert!(data["staged"].as_str().unwrap().contains("+staged"));
        assert!(data["unstaged"].as_str().unwrap().contains("+unstaged"));
        assert!(!data.to_string().contains("untracked.txt"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
