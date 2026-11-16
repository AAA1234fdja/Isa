# ISA

ISA is an early-stage application. The application logic starts in Rust; the current interface uses HTML, CSS, and JavaScript because it is rendered in a browser. It does not yet connect to an AI agent, open local projects, or produce a desktop application.

Chat provider configuration and credential limitations are documented in [docs/providers.md](docs/providers.md).

## Evaluation evidence

A user-provided evaluation reports results for two agent configurations on 20 software-engineering questions inspired by SWE-bench. Both received a reported average score of **9.75/10**. See the [evaluation summary](docs/evaluation.md) for the per-case scores and important limitations. These are reported assessment scores, not a verified SWE-bench resolution rate.

## Run

Install the Rust toolchain, then run:

```sh
cargo run
```

Open `http://127.0.0.1:4174`. Set `ISA_ADDRESS=0.0.0.0:4174` when the preview must be accessible from another host. This minimal development server is not intended for public deployment.

## Interface scale

`k` in `index.html` is the app's own scale factor, initially `1`. A separate browser-zoom compensation preserves the apparent interface size when browser zoom changes during the session. Browsers do not expose absolute zoom independently of display scaling: the initial device pixel ratio is treated as the baseline, so zoom already applied before loading the page cannot reliably be detected. The app scale is independent and can later be exposed through settings. Mobile pinch zoom is left to the browser for accessibility.

## Automatic preview refresh

While the preview is open, ISA checks `index.html` and the logo for changes every two seconds and reloads when either changes. Projects and sent chat messages are stored in the browser and survive reloads. Projects can be searched, edited, and linked to existing local conversations, either by dragging a conversation onto a sidebar project or by choosing it inside the project (including without a mouse); removing a conversation from a project or deleting a project does not delete the chat. These are local organizational links, not connected folders, repositories, or agent context. Conversation avatars, names, and user-written descriptions are stored with local chats. View chats searches titles, descriptions, linked project descriptions, and saved message text; results identify matching sources and can jump to a matching message. Utility responses that are not saved as chat messages are not searchable. Older conversations without a creation timestamp show their first recorded message instead. Unsent chat text is not saved; the active chat is restored in the same tab. Temporary server errors are retried. Run the Rust server from the project directory to serve live files; outside that directory it falls back to the embedded assets. Rust source changes require stopping and restarting `cargo run` to recompile the server.

## Extension catalog

The Extensions screen searches the real [Open VSX Registry](https://open-vsx.org/) through the Rust server's `GET /api/extensions?q=...` endpoint. Run `cargo run` with network access to `open-vsx.org` to use it; a static preview server cannot provide this endpoint. The server also needs access to crates.io when building its dependencies. The current sandbox may block the registry. Search results link to the registry; ISA does not yet install, load, execute, or manage extensions. Those capabilities require a compatible extension host and a security model.

The browser can route chats to configured Ollama or OpenAI-compatible custom providers and run bounded agent-pool workflows; these do not execute tools. Knowledge and automations remain placeholders. Token activity shows only provider-reported usage saved in this browser; see [provider and usage details](docs/providers.md). Search covers local projects and conversations.

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

Sessions are stored in `.isa/sessions` and are not connected to the browser chat. CLI chat does not automatically send repository contents to Ollama, execute model-generated commands, or modify project files. The separate `propose-edit` command sends a selected file and applies a reviewed change only after explicit approvals. Exact replacements and undo require interactive approval; undo refuses to overwrite a file that has changed since the edit. This is not a sandbox or a race-free defense against a concurrently modified hostile workspace. File scanning respects Git ignore rules and excludes several common secret paths, but filtering is not a complete secret scanner. Session files contain conversation text; keep the workspace private. `context` invokes local Git with selected config settings disabled, but is not a security boundary against an untrusted repository.

The initial CLI tools do not provide autonomous editing, sandboxing, model-directed MCP tool calling or a resolved repository graph. See the later sections for the separately implemented semantic index, context summaries, browser routing and read-only agent loop.

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

Start Ollama and pull the specified model first. The browser sends recent text messages to the same-origin `/api/chat` endpoint; the server invokes the Rust response harness and local Ollama. Without `ISA_WEB_MODEL`, requests go to the decoy and return the no-provider error, not a fabricated answer. `ISA_HARNESS=0` disables the extra probe calls. The Node development preview only supports the decoy, not the real model endpoint. Supported attachments can be sent to the configured local model as described below. Stop cancels the browser request but does not yet cancel an in-progress Ollama computation on the server. Do not expose this unauthenticated local server to untrusted networks: same-origin checks are not a substitute for authentication, especially behind a public preview proxy. No API credentials are needed for the local model.

The response harness supports explicit call budgets (`ISA_HARNESS_MODE=quick`, `balanced`, or `thorough`) and optional project retrieval (`ISA_HARNESS_CONTEXT=lexical` or `semantic`, disabled by default). See [Harness Isa](Harness%20Isa/README.md) for privacy, workspace selection, and safety limitations.

For repeatable, explicitly defined harness checks, use `isa-cli harness-eval <suite.json> <quick|balanced|thorough>`. This sends each case to the local model and reports simple assertion results, not a general quality score. See [Harness Isa](Harness%20Isa/README.md) for the suite format and limitations.

ISA supports reviewed, text-only local skills for the response harness. See [Skills](Skills/README.md) for creation, editing, enable/disable, model-generated proposals, and on-demand selection. Skills do not run tools or grant permissions.

### Command approval interface

The composer stores a **per-chat supervision preference**: ask every time,
ask occasionally (risky or unknown actions), or never ask. New and legacy
chats default to ask every time; the old global preference is not migrated.
Selecting never ask requires an explicit confirmation for the current chat.
These are **UI preferences only**, not enforced permissions: chat cannot run
commands, MCP tools or skills, and there is no authenticated approval protocol.
CLI commands, MCP and skills are separate; their existing approvals are not
changed by this selector. Model data-sharing consent is also separate.

Before enabling chat execution, a trusted backend must enforce the selected
policy for commands, tools and skill actions, conservatively treat unknown
operations as risky, scope any remembered approvals to the exact action and
workspace, and handle denial and changes while requests are pending. Browser
state alone is not authorization. The Node preview and decoy cannot create real
approval requests. No mode currently makes the browser a sandbox.

### Attachments

The composer stages up to eight files (10 MiB each, 25 MiB in total), including pasted images. It previews supported images, renders PDF pages locally, extracts readable text from DOCX, displays UTF-8 text and code, and plays audio or video when the browser supports the codec. Unsupported or binary formats remain downloadable. Image markings can be saved as a PNG copy; the original is preserved.

With the **Rust server** and `ISA_WEB_MODEL` configured, the chat sends UTF-8 text, extracted PDF and DOCX text, and up to two images to local Ollama. Images require a model that reports the `vision` capability; each image must be at most 2 MiB after conversion or marking. Other formats, scanned PDFs without extractable text, password-protected documents, documents over 40 pages, and more than 20 KiB of combined extracted document text are rejected with an explanation rather than silently omitted. DOCX input is limited to 2 MiB and bounded archive metadata; preview and extraction are not a malware sandbox. Audio, video, spreadsheets, and other binaries can be previewed or downloaded where supported but cannot yet be sent to the model. The Node development preview still uses only the decoy and does not send files to an AI provider.

Document and image contents are treated as untrusted input, but prompt injection remains possible. The contents are sent to local Ollama during response probes and composition; the harness's planner may also see part of the message text. Files are held in memory only: sent attachment chips can be reopened while they remain in the session's bounded in-memory cache, but not after reload or eviction. Chat history stores attachment names and temporary references, not file contents. Subsequent turns do not resend earlier files automatically. Review sensitive documents before attaching them. Do not expose the unauthenticated local server to untrusted networks.

PDF previews and text extraction use locally bundled [PDF.js](https://github.com/mozilla/pdf.js) (Apache-2.0). DOCX text extraction uses [Mammoth.js](https://github.com/mwilliamson/mammoth.js) (BSD-2-Clause, including the licenses listed in its bundle). Copies of their licenses are in `assets/vendor`.

### Task overview and Git changes panels

The upper-right Task overview panel shows the active browser conversation's latest request, actual provider status, latest response or error, and the plan generated by `./goal/.` for review. Plans do not run steps or tools, and tool activity is not connected. The Changes panel displays read-only staged and unstaged Git diffs, split by file, when the **Rust** web server is explicitly started with a workspace:

```sh
ISA_WEB_WORKSPACE=/absolute/path/to/repository cargo run --bin isa
```

Drag the left edge of either panel to resize it; the width is saved in the browser. With the divider focused, use the left/right arrow keys (Shift for larger steps), or Home/End to reach the limits. Use Refresh to reload changes. It shows tracked Git diffs only; untracked and ignored files are not included. Binary patches may not show file contents. Each diff is capped at 2 MiB, and display is capped at 200 files per section and 2,000 lines per file. Changes are not attributed specifically to the agent, and revert is not implemented. The browser's folder picker does not select the server workspace. The Node development preview has no Git endpoint and displays an unavailable message.

The configured directory may contain sensitive changes. The server does not authenticate users; same-origin checks are not a substitute for authentication or protection against DNS rebinding. Enable `ISA_WEB_WORKSPACE` only on a trusted local interface, not a public preview or untrusted network. Without it, no workspace diff is exposed.

### Local background processes

`isa-cli processes [directory]` opens an interactive, Unix-only process supervisor. Use `start <name> <bash command>`, `list`, `logs <name>`, `stop <name>`, `remove <name>` (after exit), or `quit`. It starts real background processes in the selected working directory and captures the most recent 64 KiB of combined output in memory. Each new command requires terminal approval; “allow always” applies only to the exact command for the current supervisor session. Denial does not run the command. The supervisor attempts to stop its process groups on exit. Commands run with the user's permissions, without a sandbox, and can access files and the network. Processes that daemonize, escape their process group, or outlive an already-exited group leader may not be stopped automatically. Do not use this as a security isolation mechanism.

The browser has a display component for genuine command and process activity records, but the chat backend **does not yet execute tools or stream those records**. It therefore does not show simulated activity, cannot start or stop these CLI-supervised processes from the browser, and cannot yet automatically suggest an alternative after denial. Browser approval preferences are not connected to this supervisor. The CLI supervisor must stay open to manage its processes; it is not integrated with the Arena live preview system.

### Read-only agent loop (experimental)

The Rust Ollama runtime can run a bounded model–search–model loop. For the web server, set both `ISA_WEB_MODEL` and `ISA_WEB_AGENT_WORKSPACE` to a trusted directory before starting ISA, then enable **Read-only agent loop** in the chat settings. Each message requires confirmation before workspace excerpts can be sent to Ollama. The browser folder picker does not configure this server directory. For the CLI, set `ISA_AGENT_LOOP=1`; the chat/ask directory is the search workspace. The normal harness is bypassed during loop turns. Resume and goal helper requests do not use the loop.

The model can request at most three lexical searches, declaration lookups or call-site lookups combined before returning a final answer. Search results are untrusted and bounded, and invalid decisions or exhausted limits fail rather than executing another action. The UI shows completed search queries, not a live progress stream. The loop cannot edit files, run commands, use MCP, invoke plugins, or execute the task plan. Custom providers and agent pools do not support it. It does not use approval policies for write operations because none are exposed; it is not a sandbox. Existing exclusion rules are not a secret scanner, and filesystem changes or symlinks can create races: only expose a trusted directory whose contents may be sent to the configured model. Model instructions can be influenced by untrusted content. Browser Stop cancels the request, but does not guarantee server-side model or search work stops. Ollama must support the required JSON decision protocol; a noncompliant response is an error. Reported Ollama chat usage is included when available; embedding, failed and cancelled requests may be missing.

### Reviewed SEARCH/REPLACE proposals

The CLI can review one exact replacement from a UTF-8 proposal file:

    File: src/example.rs
    <<<<<<< SEARCH
    old text
    =======
    new text
    >>>>>>> REPLACE

Optionally run `cargo run --bin isa-cli -- check-edit proposal.txt /path/to/workspace`
first. This non-interactive, read-only check prints a JSON review (including the
submitted search and replacement) to stdout and exits nonzero on failure. It
does not contact a model, write files, create undo records, or authorize an edit.
The proposal file is resolved from the current directory; its target path is
relative to the workspace. Treat JSON output as potentially sensitive and avoid
sharing it indiscriminately. A successful check reflects only that moment: the
file can change later, and `apply-edit` still rechecks and requires approval.

Run `cargo run --bin isa-cli -- apply-edit proposal.txt /path/to/workspace`.
This is a manually initiated CLI operation, not an agent tool or browser
integration. The proposal accepts exactly one block and an existing, relative
file path. The delimiter line breaks are not part of the search or replacement;
add a blank line immediately before a delimiter to include a trailing newline.
An empty replacement deletes the matched text. Marker lines in the content are
ambiguous and rejected; control and bidirectional formatting characters in the
replacement or search are rejected to make review safer. The search must match exactly once, and both sides are
limited to 4,000 characters. ISA displays the change, requires terminal approval,
checks the file again after approval, and records the prior contents for the
existing single-edit `undo` command. It refuses unattended execution, symlink
paths, protected paths and oversized or binary targets. This mechanism is not a
sandbox or a race-free defense against a concurrently modified workspace, and
`.isa/undo.json` can contain file contents. Multi-file transactions, AST edits,
automatic model invocation and web approval remain unimplemented.

### Local symbol lookup

`isa-cli symbols <name> [directory]` finds syntax declarations in supported
Rust, Python, JavaScript, TypeScript and TSX files using Tree-sitter. The optional read-only
Ollama agent loop can request the same lookup within its shared three-lookup
budget. Results include names, kinds, paths and line numbers, and are treated as
untrusted data. On the web route, the existing workspace confirmation covers
sending these results to Ollama. The browser folder picker is not connected to
the server workspace.

`isa-cli callers <name> [directory]` finds syntactic call sites whose callee
name exactly matches the query (case-sensitive), including direct and named
member calls. It reports the nearest named enclosing function when available.
The read-only Ollama loop can request the same lookup. Matches are not resolved:
identically named methods can refer to unrelated declarations, and dynamic,
computed or unsupported calls may be missed. It is not a complete call graph.
Agent Pools can also request call sites through the optional symbol-context
setting described below.

This is an experimental syntax lookup, not a resolved dependency graph,
reference finder, or a guarantee of correct localization. It does not support
all syntax or languages; wrapped or destructured function assignments and
other constructs may be missed. It respects the existing ignore/exclusion rules,
skips symlinks and bounds individual files, aggregate bytes, visited AST nodes,
and output; results may be incomplete, including after parse errors or skips.
The workspace file listing itself has a separate cap. File names and symbols
can contain sensitive information, and exclusions are not a secret scanner. It
is not a sandbox or a race-free defense against a changing workspace. For Agent Pools, set an optional **Pool symbol context** query in chat settings;
each message then asks for permission before looking up that name on the Rust
server and sharing at most 15 matching declarations or unresolved call sites
and their paths with pool providers. Choose the lookup type in chat settings;
call-site queries match the callee name exactly and do not resolve identity.
A lookup failure stops the pool, and goal/summary helpers do not use this
context. Clear the query to opt out. The model cannot invoke this lookup itself
in the pool. Direct custom-provider chats do not use it. The endpoint requires
same-origin JSON and `ISA_WEB_AGENT_WORKSPACE`; like the rest of the development
server it is not suitable for public deployment or untrusted local clients.

### Model-generated edit proposal in the CLI

`cargo run --bin isa-cli -- propose-edit src/example.rs "Describe the change" /path/to/workspace`
asks the configured local Ollama model (`ISA_MODEL`) for one SEARCH/REPLACE
proposal for the selected existing file. ISA first asks permission to send the
complete file and instruction to Ollama, then separately displays the proposed
replacement and asks permission to apply it. A malformed proposal, another file
path, a non-unique match, a file changed since inference, or denial stops the
operation. Successful edits use the same single-file undo record as `apply-edit`.

The file is limited to 24 KiB and the instruction to 4,000 bytes; the regular
proposal and edit limits also apply. This command runs only when explicitly
invoked from an interactive terminal. The browser has a separate read-only
proposal flow; neither flow is an autonomous agent-loop tool. Ollama receives the full file, which may contain
secrets; exclusion rules are not a secret scanner. The model may produce an
incorrect or unsafe change even when formatting is valid, so review it before
approval. There is no sandbox or race-free protection against a concurrently
modified workspace, no multi-file transaction, and no automatic test execution.

The `check-edit` JSON, web review, and interactive CLI replacement review
include an advisory Tree-sitter syntax comparison for Rust, Python, JavaScript, TypeScript and TSX files. It
checks the in-memory before and proposed-after snapshots (each at most 64 KiB)
with a best-effort 100 ms parse budget per snapshot. Unsupported, oversized or
unfinished parses are reported as unavailable, not as passing. A parse without
errors is not a compiler, type checker, test suite or correctness guarantee;
existing errors also make it impossible to infer from the two booleans whether
a particular error is new. For each snapshot ISA reports up to eight approximate
error or missing-syntax locations (1-based lines and UTF-8 byte columns), with
an explicit incomplete flag if the bounded traversal is cut short. It does not
return surrounding source text. The comparison never writes the proposed result.
In `replace`, `apply-edit` and `propose-edit`, ISA prints this advisory before
requesting final approval; detected errors do not automatically apply or block a
change. The user remains responsible for the decision and for running tests.

### MCP tools over stdio (CLI)

ISA supports an explicitly user-driven subset of MCP protocol `2025-06-18`
over stdio on Unix. From an interactive terminal, use:

```text
isa-cli mcp list -- <server-program> [server-arguments...]
isa-cli mcp call <tool-name> '<json-object>' -- <server-program> [server-arguments...]
isa-cli mcp propose <task> -- <server-program> [server-arguments...]
isa-cli mcp loop <task> -- <server-program> [server-arguments...]
```

ISA asks before starting the server, performs initialization and bounded
`tools/list` discovery, displays the server-provided tool metadata and exact
arguments, and asks separately before `tools/call`. It does not enforce the
server-provided JSON Schema. With `propose`, after separate consent ISA sends the task and complete discovered
metadata (up to 16 KiB combined) to the configured Ollama model. The model can
suggest at most one call or decline; ISA checks that the name uniquely matches
and the arguments are an object, then shows the proposal for separate approval.
After a successful call, the full result is displayed. You can separately approve
sending the original task, tool name, arguments and result to Ollama for a final
answer; this is skipped if their serialized size exceeds 32 KiB. Declining does
not undo the call. ISA performs no further tool calls in this final-answer step,
and a failed model request does not undo the tool's effects. Tool-reported errors
are shown and do not trigger a summary. Results can contain secrets or prompt
injection, and the model's answer may be inaccurate; inspect them before sharing
or relying on them. Server metadata and model proposals may contain prompt
injection or unsafe arguments: review them. The browser cannot invoke MCP. Only stdio and this protocol version
are supported; HTTP, OAuth, resources, prompts, sampling, server requests other than ping, and
persistent sessions are not implemented. Server pings receive an empty response
within the same bounded operation; unsupported server requests fail closed. Discovery is limited to three pages and
100 tools; calls are refused when discovery is incomplete or the name is not unique. Messages and operation time are bounded. A call can have completed
before a timeout or interruption, so do not blindly retry it.

For a bounded multi-tool workflow, `mcp loop` lets the model propose up to
three sequential calls to the same stdio server. ISA displays the full context
and asks before **every** model request, then displays the selected tool and
arguments and asks before **every** call. The next context includes the full
results of all prior calls; inspect it for secrets and untrusted instructions
before approving. The context is capped at 32 KiB without truncation. After
three calls, the model may give a final answer but cannot call another tool.
Invalid responses, denial, oversized context, protocol failures, and tool errors
stop the loop. Prior calls and their effects are never rolled back. A failed or
timed-out call may already have run: do not blindly retry. Tool descriptions
and results can change or be malicious; discovery and approval are not a
sandbox or a guarantee of server behavior. This is CLI-only and uses Ollama,
not the web agent or a general tool-calling API.

**MCP servers are executable programs, not sandboxes.** Even initialization or
listing may change files, expose environment variables, access credentials or
the network. Tool descriptions, schemas and results are untrusted. Only start
trusted servers and review arguments carefully; inherited permissions and process
groups do not provide isolation, and escaped descendants may survive. Server
stderr is discarded. ISA terminates the server process on normal client exit,
but cannot guarantee cleanup of all descendants or effects.

Protocol reference: [MCP stdio transport and lifecycle](https://modelcontextprotocol.io/specification/2025-06-18/basic/transports).

CI also runs the interactive CLI against a local fixture built using the official
MCP TypeScript SDK (pinned in `tests/mcp-sdk/package-lock.json`). It exercises
real protocol initialization, discovery, approval, calls and tool errors. To
run that optional interoperability test locally, install Node.js and Rust, then:

```sh
npm ci --ignore-scripts --no-audit --no-fund --prefix tests/mcp-sdk
ISA_MCP_SDK_TEST=1 cargo test --test cli mcp_official_sdk_stdio_interoperability
```

This test does not establish compatibility with every MCP server or test an
actual Ollama model.


### Bounded test-feedback loop (CLI)

From the workspace, run `isa-cli feedback <file> <instruction> -- <program> [args...]`.
ISA shows the exact program and arguments and requires terminal approval before
each run. It uses the same user-selected command for at most three test runs
and at most two model proposals. It does not insert a shell; explicitly choosing a shell still
allows arbitrary commands. The test runs in the real workspace with the user's
permissions, inherited environment and network access: **there is no sandbox**.
It can modify files, leak data or leave background processes. The 60-second
best-effort timeout kills the process group, but escaped descendants may survive.

ISA shows bounded stdout/stderr tails, exit status, timeout and truncation. On
failure or incomplete output it displays the exact diagnostic to share, then separately asks permission
to send it and the current complete target file (at most 24 KiB) to Ollama for
one SEARCH/REPLACE proposal. Logs and source may contain secrets or adversarial
instructions. ISA displays the proposal and syntax advisory, then requires a
separate approval to apply it, checks the file snapshot again, and asks again
before each test rerun. Denial or an error stops the loop; previously applied
changes are not automatically reverted. The single `undo` record is overwritten
by each approved edit and can only restore the latest one, if unchanged. The
result may be wrong, a file can change after checking, and a zero exit code is
not proof of correctness. This is a bounded, manually approved CLI loop, not an
autonomous web agent. Unix only. There is no race-free filesystem protection.

### Read-only web proposal generation

The composer’s **Propose edit** button opens a separate dialog. Enter an existing
relative file in the server workspace and an instruction, then explicitly select
**Send file to Ollama**. The Rust server requires `ISA_WEB_AGENT_WORKSPACE` and
`ISA_WEB_MODEL`; it sends the entire selected UTF-8 file (up to 24 KiB) and the
instruction to its configured Ollama model. Secret filtering is incomplete. The
browser folder picker, selected chat provider and pools are not used. A valid
single-file proposal can be opened in the separate review dialog; it is not
saved in chat and no file is edited. If the file changed during the request,
ISA rejects the response. Cancellation may not stop server-side model work.
Generated proposals can be wrong or unsafe, path checks are not a sandbox or
race-free, and local clients can access the endpoint without the browser consent
flow. Only use a trusted workspace and do not expose the server to untrusted
clients. This flow does not report model usage in the chat heatmap.

### Read-only web proposal review

Assistant messages containing `<<<<<<< SEARCH` offer **Review proposal**. Paste or edit one complete `File: …` SEARCH/REPLACE block in the dialog and select **Check proposal**. The Rust server checks the existing UTF-8 file in `ISA_WEB_AGENT_WORKSPACE`, validates the same format and exact unique match used by the CLI, and displays the submitted search and replacement literally. No file or undo record is written; this does not apply a patch. A successful check is a snapshot, not a guarantee that the file remains unchanged. If the search has no exact match, the review, `check-edit`, and CLI replacement operations can report up to five approximate line ranges found by collapsing whitespace (files up to 64 KiB). These are hints only, may be partial or misleading, and contain no source excerpts; inspect the file and correct the proposal manually. They never relax the exact-match rule. If SEARCH matches more than once, including overlapping matches, ISA reports up to five exact-match line ranges for files up to 64 KiB; add context to make the search unique. It never chooses a match automatically. A single complete block surrounded by explanations or Markdown fences is extracted for review. When several unambiguous blocks are detected, choose and load one explicitly; only that block is checked, not the combined changes. Loading replaces any text already in the editor. Ambiguous or incomplete blocks must be edited manually. The review can optionally reveal spaces, tabs and line breaks. After a successful check, **Download proposal** saves the checked text to the browser device as `isa-proposal.txt`; it may contain sensitive content. Transfer it to the CLI host if necessary and run `check-edit` or `apply-edit` there, with the correct workspace. Downloading does not authorize or apply anything, and later checks may fail if the file changed. The browser folder picker does not configure the server workspace. The Node development preview does not implement this endpoint. Path exclusions and symlink checks are not a sandbox or race-free security boundary; use a trusted workspace and server, and do not expose it to untrusted clients.
