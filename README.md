# ISA

ISA is an early-stage application. The application logic starts in Rust; the current interface uses HTML, CSS, and JavaScript because it is rendered in a browser. It does not yet connect to an AI agent, open local projects, or produce a desktop application.

## Run

Install the Rust toolchain, then run:

```sh
cargo run
```

Open `http://127.0.0.1:4174`. Set `ISA_ADDRESS=0.0.0.0:4174` when the preview must be accessible from another host. This minimal development server is not intended for public deployment.

## Interface scale

`k` in `index.html` is the app's own scale factor, initially `1`. A separate browser-zoom compensation preserves the apparent interface size when browser zoom changes during the session. Browsers do not expose absolute zoom independently of display scaling: the initial device pixel ratio is treated as the baseline, so zoom already applied before loading the page cannot reliably be detected. The app scale is independent and can later be exposed through settings. Mobile pinch zoom is left to the browser for accessibility.

## Automatic preview refresh

While the preview is open, ISA checks `index.html` and the logo for changes every two seconds and reloads when either changes. Projects and sent chat messages are stored in the browser and survive reloads. Unsent chat text is not saved; the active chat is restored in the same tab. Temporary server errors are retried. Run the Rust server from the project directory to serve live files; outside that directory it falls back to the embedded assets. Rust source changes require stopping and restarting `cargo run` to recompile the server.

## Extension catalog

The Extensions screen searches the real [Open VSX Registry](https://open-vsx.org/) through the Rust server's `GET /api/extensions?q=...` endpoint. Run `cargo run` with network access to `open-vsx.org` to use it; a static preview server cannot provide this endpoint. The server also needs access to crates.io when building its dependencies. The current sandbox may block the registry. Search results link to the registry; ISA does not yet install, load, execute, or manage extensions. Those capabilities require a compatible extension host and a security model.

Agents, folders, knowledge, and automations are interface placeholders, not connected services. Chats store user messages locally but do not yet connect to an AI provider. The activity chart counts distinct conversations with messages on each local calendar day. Search currently covers only preview projects stored in the browser.

## ISA CLI: initial backend

The Rust CLI is an initial foundation, not a complete coding agent. Run `cargo run --bin isa-cli -- help` for commands. It can list files, perform bounded literal text searches, show a limited Git status, rank files with lexical search, review exact text replacements and undo the most recent replacement, store named local chat sessions, and send chat messages to an Ollama server on `127.0.0.1:11434`. Set `ISA_MODEL` to choose an installed model. For example:

```sh
cargo run --bin isa-cli -- files .
cargo run --bin isa-cli -- search "TODO" .
cargo run --bin isa-cli -- context .
cargo run --bin isa-cli -- rank "authentication handler" .
cargo run --bin isa-cli -- pack "authentication handler" .
cargo run --bin isa-cli -- session-search "authentication" .
cargo run --bin isa-cli -- replace path/to/file "old text" "new text" .
cargo run --bin isa-cli -- undo .
cargo run --bin isa-cli -- chat my-session .
cargo run --bin isa-cli -- ask my-session "Explain Rust ownership" .
cargo run --bin isa-cli -- history my-session .
```

Sessions are stored in `.isa/sessions` and are not connected to the browser chat. The CLI does not automatically send repository contents to Ollama, execute model-generated commands, or modify project files. Exact replacements and undo require interactive approval; undo refuses to overwrite a file that has changed since the edit. This is not a sandbox or a race-free defense against a concurrently modified hostile workspace. File scanning respects Git ignore rules and excludes several common secret paths, but filtering is not a complete secret scanner. Session files contain conversation text; keep the workspace private. `context` invokes local Git with selected config settings disabled, but is not a security boundary against an untrusted repository.

Semantic indexing, AST-aware context, model tool calling, agent-driven editing, sandboxing, MCP, Git undo, background jobs, provider routing, and browser integration remain unimplemented. There is no automatic summarization; the Ollama request uses a bounded recent-message window.

### Design references

- [Cursor CLI headless and approvals](https://cursor.com/docs/cli/headless)
- [Qoder CLI permissions](https://docs.qoder.com/cli/permissions)
- [Qoder MCP reference](https://docs.qoder.com/cli/mcp-reference)
- [Hermes CLI and session management](https://github.com/NousResearch/hermes-agent/blob/main/website/docs/user-guide/cli.md)
- [MCP transports](https://modelcontextprotocol.io/specification/2025-03-26/basic/transports)

The Ollama chat streams responses to the terminal. `ask` runs one request without an interactive prompt and exits nonzero on failure; it does not execute agent tools. A user message is saved before the request, while an assistant message is saved only after the stream completes. Partial responses are not saved.

`pack` creates a bounded JSON context bundle using lexical ranking, not embeddings. Repository snippets are untrusted input and must not be treated as instructions. It does not transmit the bundle to a model; review sensitive contents before sharing it. `session-search` searches locally saved CLI sessions.
