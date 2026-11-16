mod repository;
mod session;

use std::error::Error;
use std::io::{self, Write};
use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

const HELP: &str = "ISA CLI\n\n\
Usage:\n\
  isa-cli files [directory]                 List workspace files\n\
  isa-cli search <text> [directory]         Search text in workspace files\n\
  isa-cli context [directory]               Show repository and git context\n\
  isa-cli sessions [directory]              List saved sessions\n\
  isa-cli history <session-id> [directory]  Show a saved session\n\
  isa-cli chat <session-id> [directory]     Chat with a local Ollama model\n\n\
Set ISA_MODEL to select an Ollama model (default: llama3.2).\n\
Sessions are stored locally under .isa/sessions.\n\
No file writes or commands are executed on behalf of the model.";

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
        Some("sessions") => {
            let root = directory(&args, 1)?;
            for id in session::list(&root)? {
                println!("{id}");
            }
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
        if input.len() > 64 * 1024 {
            eprintln!("Message exceeds the 64 KiB limit.");
            continue;
        }
        messages.push(session::Message::new("user", input));
        session::save(root, id, &messages)?;
        match session::ollama_reply(model, &messages) {
            Ok(reply) => {
                println!("\nISA: {reply}\n");
                messages.push(session::Message::new("assistant", &reply));
                session::save(root, id, &messages)?;
            }
            Err(error) => eprintln!("Model unavailable: {error}"),
        }
    }
    Ok(())
}
