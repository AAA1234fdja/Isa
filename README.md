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
cargo run --bin isa-cli -- diff .
cargo run --bin isa-cli -- diff --staged .
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

`diff` explicitly prints tracked unstaged changes; `diff --staged` prints staged changes. Neither includes untracked files or sends changes to a model. Output is limited to 2 MiB and external diff programs and text conversion are disabled. Git access is not a sandbox.

## Subagent contracts and goals

The CLI can manage local contract and goal records:

```sh
cargo run --bin isa-cli -- contract create reviewer "Review changes for defects"
cargo run --bin isa-cli -- contract set reviewer instructions "Do not modify files"
cargo run --bin isa-cli -- contract set reviewer deliverables "A prioritized review"
cargo run --bin isa-cli -- contract set reviewer status ready
cargo run --bin isa-cli -- contract list
cargo run --bin isa-cli -- contract show reviewer
cargo run --bin isa-cli -- goal create audit "Review the current change"
cargo run --bin isa-cli -- goal bind audit reviewer
cargo run --bin isa-cli -- goal status audit completed
cargo run --bin isa-cli -- goal list
cargo run --bin isa-cli -- goal show audit
```

Records live in `.isa/contracts` and `.isa/goals`, are local to the workspace, and are excluded from Git. Supported contract statuses are `draft`, `ready`, and `archived`; goal statuses are `open`, `paused`, and `completed`. A binding is a reference, not a running task. The CLI does not yet start subagents or enforce contract instructions, tool restrictions, budgets, or approvals. Do not rely on these records as a security boundary. The registry is not designed for concurrent writers or hostile processes modifying its files.

## Chat composer

The browser chat can save up to four UTF-8 text attachments (64 KiB each) with a message in browser local storage. They are not sent to a model or uploaded; do not attach sensitive files on a shared browser. Folder selection grants browser read access for the current page session only. You can browse a folder and explicitly attach individual UTF-8 text files to a chat. Folder access is not restored after reload, and no files are sent to an agent. Browsers without directory handles can use the folder file picker; only the first 2,000 entries are shown. Voice input uses the browser's Speech Recognition API when available; the browser or its provider may process audio.

Several `./command/.` shortcuts open sections of the browser interface. CLI-backed commands, model selection, approval controls, and AI responses are not connected to the browser chat yet. Unsupported controls remain disabled.

Provider brand icons are sourced from [SVGL](https://github.com/pheralb/svgl) and [Simple Icons](https://simpleicons.org/) (Google Docs). Brand names and logos belong to their respective owners; their display does not imply a connection or affiliation.

## Work change counts

`isa-cli work start [directory]` records a baseline of the workspace files at the start of a work session. `isa-cli work status [directory]` reports added and removed text lines relative to that baseline; it does not count chat responses. `isa-cli work end [directory]` deletes the baseline. The counts include any qualifying file edits made after the start, regardless of who made them, not only edits by an agent.

The snapshot stores file contents unencrypted under `.isa/work` until ended. The scan respects ignore rules and excludes common sensitive paths, but is not a complete secret filter. Binary files and text files over 512 KiB or 10,000 lines are skipped and reported; the total captured text is limited to 16 MiB. This tracker is currently CLI-only: it is not yet connected to the browser, GitHub, or PR creation. Concurrent changes during a scan and hostile workspace modification are not isolated.

For summary instances and non-chat utility commands, see [Context instances](docs/context-instances.md). Ollama summaries require two additional requests after each completed CLI response; the browser does not yet access CLI summaries.

## Local decoy provider

Run the decoy alongside the Rust web server:

```sh
cargo run --bin isa-decoy
cargo run --bin isa
```

The decoy listens only on `127.0.0.1:4180`. When no real model provider is configured, the browser sends message text to the same-origin `/api/decoy` endpoint on the Rust web server, which forwards it to the decoy. The decoy returns a structured `no_provider` error, which the chat displays as an error from Isa. Attachments are saved locally with the chat but are not transmitted to the decoy. If the decoy or web server is unavailable, the user message is kept and an error is displayed. A configured provider must not silently fall back to the decoy. The static Python preview does not implement this API.

When Rust is unavailable, a development-only JavaScript bridge can run the same decoy flow for the preview. Start `node dev/decoy.mjs` and `node dev/preview.mjs` in separate terminals. This bridge is not the production Rust server and does not implement the extension catalog endpoint.

Chat messages display local timestamps. Settings lets users choose United States or Colombia date and time formatting; the clock uses the device time zone. While a request is pending, the send button becomes a stop button and aborts the browser request.

GitHub CLI authorization and planned Google Drive, Gmail, and Notion integrations are documented in [Connector authorization](docs/connectors.md). The browser connector menu is not yet connected to these flows.

## Settings

The Settings screen groups functional controls into Appearance, Chat, and Data. Appearance stores the light/dark/system mode, an ISA green workspace palette, interface scale, and reduced-motion preference in the browser. Chat controls regional date formatting. Data can export or delete browser-local chats; exports include attached text. These preferences and conversations are not synchronized between devices or connected to CLI profiles.

## Local semantic indexing

The CLI can index workspace text with real local Ollama embeddings and search chunks by cosine similarity. Install and start Ollama on `127.0.0.1:11434`, then pull an embedding model, for example `ollama pull nomic-embed-text`:

```sh
isa-cli semantic-index nomic-embed-text /path/to/workspace
isa-cli semantic-search "where are user sessions saved?" /path/to/workspace
```

The index is stored in `.isa/semantic-index.json`, ignored by this repository's `.gitignore`, and contains source excerpts and embeddings in plaintext. Protect it as sensitive project data and exclude `.isa/` in other repositories. Indexing respects ignore rules and the CLI's excluded paths, skips binary and large files, and limits index size; it cannot reliably identify every secret embedded in source files. Review your workspace and ignore rules before indexing. Source text is sent to the local Ollama service, not an external provider. Output includes paths, line ranges, similarity scores, and excerpts marked as untrusted. Deleted, ignored, or changed chunks are omitted from searches until the index is rebuilt; rebuild after edits to include new content. Unchanged chunks are reused when the model digest matches. Changing the installed model requires rebuilding. This is CLI-only: the web UI and agent do not yet consume this index. `rank` and `pack` remain lexical.

The CLI's initial response adaptation harness is documented in [Harness Isa](Harness%20Isa/README.md). It is inspired by the propose–probe–compose pattern in Harness², not an integration of the upstream research framework. It performs up to four model calls (two probes, a review, and a composition). Set `ISA_HARNESS=0` to disable the extra calls.

### Browser chat with the response harness

The Rust web server can explicitly use a local Ollama model for browser chat:

```sh
ISA_WEB_MODEL=llama3.2 cargo run --bin isa
```

Start Ollama and pull the specified model first. The browser sends recent text messages to the same-origin `/api/chat` endpoint; the server invokes the Rust response harness and local Ollama. Without `ISA_WEB_MODEL`, requests go to the decoy and return the no-provider error, not a fabricated answer. `ISA_HARNESS=0` disables the extra probe calls. The Node development preview only supports the decoy, not the real model endpoint. Attached files are not yet supported for model calls. Stop cancels the browser request but does not yet cancel an in-progress Ollama computation on the server. Do not expose this unauthenticated local server to untrusted networks: same-origin checks are not a substitute for authentication, especially behind a public preview proxy. No API credentials are needed for the local model.

The response harness supports explicit call budgets (`ISA_HARNESS_MODE=quick`, `balanced`, or `thorough`) and optional project retrieval (`ISA_HARNESS_CONTEXT=lexical` or `semantic`, disabled by default). See [Harness Isa](Harness%20Isa/README.md) for privacy, workspace selection, and safety limitations.

For repeatable, explicitly defined harness checks, use `isa-cli harness-eval <suite.json> <quick|balanced|thorough>`. This sends each case to the local model and reports simple assertion results, not a general quality score. See [Harness Isa](Harness%20Isa/README.md) for the suite format and limitations.
