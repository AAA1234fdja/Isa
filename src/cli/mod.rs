mod context;
mod edit;
mod repository;
mod session;

use std::error::Error;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

const HELP: &str = "ISA CLI\n\n\
Usage:\n\
  isa-cli files [directory]                 List workspace files\n\
  isa-cli search <text> [directory]         Search text in workspace files\n\
  isa-cli context [directory]               Show repository and git context\n\
  isa-cli rank <query> [directory]          Rank relevant files with lexical search\n\
  isa-cli pack <query> [directory]          Output a bounded context pack as JSON\n\
  isa-cli replace <file> <old> <new> [directory]  Review and apply an exact replacement\n\
  isa-cli undo [directory]                  Review and undo the last replacement\n\
  isa-cli sessions [directory]              List saved sessions\n\
  isa-cli session-search <text> [directory] Search local session messages\n\
  isa-cli history <session-id> [directory]  Show a saved session\n\
  isa-cli chat <session-id> [directory]     Chat with a local Ollama model\n\
  isa-cli ask <session-id> <prompt> [directory]  Run one local model request\n\n\
Set ISA_MODEL to select an Ollama model (default: llama3.2).\n\
Sessions are stored locally under .isa/sessions.\n\
The model cannot execute commands or make file edits.";

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
        Some("rank") => {
            let query = argument(&args, 1, "query")?;
            let root = directory(&args, 2)?;
            for result in context::rank(&root, &query, 20)? {
                println!("{} ({:.3})\n{}\n", result.path.display(), result.score, result.excerpt);
            }
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
        Some("undo") => {
            let root = directory(&args, 1)?;
            edit::undo(&root)?;
        }
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
    let reply = session::ollama_stream(model, messages, |chunk| {
        let safe;
        let displayed = if terminal {
            safe = chunk.chars().map(|c| if c == '\n' || c == '\t' || !c.is_control() { c } else { '�' }).collect::<String>();
            safe.as_str()
        } else { chunk };
        print!("{displayed}");
        io::stdout().flush()?;
        Ok(())
    })?;
    println!();
    messages.push(session::Message::new("assistant", &reply));
    if let Err(error) = session::save(root, id, messages) {
        messages.pop();
        return Err(error);
    }
    Ok(())
}
