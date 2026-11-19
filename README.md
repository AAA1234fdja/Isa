# ISA

ISA is an early-stage application with a Rust server and a browser interface. It supports provider routing, limited agent workflows and opt-in local actions; many integrations remain experimental or incomplete. Browser project records are not connected server workspaces, and there is no desktop application.

Chat provider configuration and credential limitations are documented in [docs/providers.md](docs/providers.md). See [CONTRIBUTING.md](CONTRIBUTING.md) and [SECURITY.md](SECURITY.md) for development and vulnerability reporting.

The project is licensed under [Apache License 2.0](LICENSE). Bundled third-party components may have separate licenses; see their accompanying notices.

## Evaluation evidence

A user-provided evaluation reports results for two agent configurations on 20 software-engineering questions inspired by SWE-bench. Both received a reported average score of **9.75/10**. See the [evaluation summary](docs/evaluation.md) for the per-case scores and important limitations. These are reported assessment scores, not a verified SWE-bench resolution rate.

## Run

Install the Rust toolchain, then run:

```sh
cargo run
```

Open `http://127.0.0.1:4174`. Set `ISA_ADDRESS=0.0.0.0:4174` when the preview must be accessible from another host. This minimal development server is not intended for public deployment.

## Frontend development

`index.html` is a minimal shell. `frontend/src/layout-data.mjs` describes the initial interface as a declarative DOM tree, and `frontend/src/layout.mjs` mounts it using DOM APIs before the application scripts run. JavaScript is required to display the workspace; the HTML shell contains a no-script notice. Rebuild the layout bundle after changing either source file.

Appearance, sidebar, notifications, the custom provider client, the agent-pool runner, the project store and project presentation are ES module factories in `frontend/src/` with explicit dependencies; provider endpoint validation, usage aggregation, and bounded conversation, attachment and diff helpers are independent utility functions in the same directory. Their checked-in `assets/*.js` bundles contain temporary compatibility bridges for the remaining classic scripts. The rest of the frontend still shares global state and has not yet been migrated to ES modules. After editing these modules, run `npm ci --ignore-scripts --prefix frontend` and `npm run build --prefix frontend`; run `npm run check --prefix frontend` to detect stale generated bundles. Generated bundles are kept in the repository because the Rust server embeds them and does not require Node at runtime. Rebuild before checking changes in the preview.

## Appearance

Appearance settings offer 33 searchable palettes, including the original Forest, Moss and Sage themes. Each theme works with light, dark or system color mode; the selected palette is saved in browser storage. Interface scale and reduced motion are configured separately.

## Interface scale

The appearance controller in `frontend/src/appearance.mjs` owns the app's scale factor, initially `1`. A separate browser-zoom compensation preserves the apparent interface size when browser zoom changes during the session. Browsers do not expose absolute zoom independently of display scaling: the initial device pixel ratio is treated as the baseline, so zoom already applied before loading the page cannot reliably be detected. The app scale is independent and can be adjusted in Appearance settings. Mobile pinch zoom is left to the browser for accessibility.

## Automatic preview refresh

While the preview is open, ISA checks `index.html`, `assets/interface.css`, `assets/preview.js`, `assets/layout.js`, `assets/appearance.js`, `assets/sidebar.js`, `assets/notifications.js`, `assets/provider-utils.js`, `assets/conversation-utils.js`, `assets/provider-client.js`, `assets/pool-runner.js`, `assets/project-store.js`, `assets/project-ui-module.js`, `assets/supervised-actions.js`, `assets/attachments.js`, `assets/projects.js`, `assets/settings.js`, `assets/agents.js`, `assets/routing.js`, `assets/chat.js`, `assets/interface.js` and the logo for changes every two seconds and reloads when any changes. Projects and sent chat messages are stored in the browser and survive reloads. Projects can be searched, edited, and linked to existing local conversations, either by dragging a conversation onto a sidebar project or by choosing it inside the project (including without a mouse); removing a conversation from a project or deleting a project does not delete the chat. These are local organizational links, not connected folders, repositories, or agent context. Conversation avatars, names, and user-written descriptions are stored with local chats. View chats searches titles, descriptions, linked project descriptions, and saved message text; results identify matching sources and can jump to a matching message. Utility responses that are not saved as chat messages are not searchable. Older conversations without a creation timestamp show their first recorded message instead. Unsent chat text is not saved; the active chat is restored in the same tab. Temporary server errors are retried. Run the Rust server from the project directory to serve live files; outside that directory it falls back to the embedded assets. Rust source changes require stopping and restarting `cargo run` to recompile the server. Changes to `dev/preview.mjs`, including asset routes, also require restarting the Node preview; an older running server can otherwise return 404 for newly extracted assets.

## Extension catalog

**Windows status:** Windows support is incomplete and has not been validated. Supervised edits and undo, MCP stdio, and secure undo storage require Unix; Windows CLI process supervision and feedback test execution are separately available only through unvalidated experimental opt-ins described below; the sandbox profile requires Linux. An experimental Windows mode permits workspace-information inspection when `ISA_WEB_WINDOWS_READ_ONLY=1`. A separate, more dangerous opt-in `ISA_WEB_WINDOWS_SHELL_EXPERIMENTAL=1` also permits **manual** supervised PowerShell commands; model tool loops, edits and undo remain blocked. Both modes require the existing local token, configured workspace and chat supervision policy. **Do not use the Windows shell mode in production:** it has not been compiled or tested in Windows. Commands run with the server user's privileges, host network and no sandbox. Audit event metadata is volatile and degraded; encrypted details have a separate unvalidated experimental opt-in. Base64-encoded scripts are not confidential and may appear in process command lines. The experimental process uses a Job Object, bounded output and best-effort termination; descendants and side effects may survive. Briefly inheritable handles could leak through concurrent launchers that inherit all handles. Windows CI is configured but has not run. CLI edit and workspace filters reject some ambiguous Windows names and reparse points, but can omit legitimate files and are not a security boundary or secret scanner. Experimental file-identity, owner-only ACL and plaintext undo-storage components are staged but unvalidated and disconnected from web edits. They reject some hard links, reparse points and unsafe ACLs but do not close path races. Windows web edits and undo remain disabled. ACLs, aliases and filesystem races still need review. A named-pipe primitive and cancellable overlapped writer for a future Windows MCP stdin are staged only and not connected to MCP. Both are uncompiled and untested on Windows. If cancellation cannot be confirmed, the writer conservatively retains the operation's memory and handles; a global limit rejects further operations after too many such cases. Cancellation or timeout can leave the tool outcome unknown.

The Extensions screen searches the real [Open VSX Registry](https://open-vsx.org/) through the Rust server's `GET /api/extensions?q=...` endpoint. Run `cargo run` with network access to `open-vsx.org` to use it; a static preview server cannot provide this endpoint. The server also needs access to crates.io when building its dependencies. The current sandbox may block the registry. Search results link to the registry; ISA does not yet install, load, execute, or manage extensions. Those capabilities require a compatible extension host and a security model.

The browser can route chats to Ollama or OpenAI-compatible custom providers and run bounded agent-pool workflows. Separate opt-in Chat MCP, Pool MCP and supervised tool-loop interactions can invoke supported actions; ordinary pool messages do not. Knowledge and automations remain placeholders. Token activity shows only provider-reported usage saved in this browser; see [provider and usage details](docs/providers.md). Search covers local projects and conversations.

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

CLI sessions are stored in `.isa/sessions`; the browser can import a read-only copy as described below, but there is no synchronization. CLI chat does not automatically send repository contents to Ollama, execute model-generated commands, or modify project files. The separate `propose-edit` command sends a selected file and applies a reviewed change only after explicit approvals. Exact replacements and undo require interactive approval; undo refuses to overwrite a file that has changed since the edit. This is not a sandbox or a race-free defense against a concurrently modified hostile workspace. File scanning respects Git ignore rules and excludes several common secret paths, but filtering is not a complete secret scanner. Session files contain conversation text; keep the workspace private. `context` invokes local Git with selected config settings disabled, but is not a security boundary against an untrusted repository.

The basic CLI chat does not autonomously edit files, execute tools or provide a resolved repository graph. Separate, opt-in CLI MCP commands can ask the model to propose tool calls with approvals; the experimental web supervisor offers additional tools and a Linux-only sandbox profile, subject to the limitations below. See the later sections for semantic search, context summaries, browser routing and the read-only agent loop.

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

The browser can stage attachments and send supported text and images to compatible
providers, subject to the limits and consent described in [Attachments](#attachments).
Chats and their exports may contain sensitive data; do not use a shared browser
for sensitive work. The folder picker grants browser read access only for the current
page session. It can browse and attach selected files, but it does not configure
a server workspace. Voice input uses the browser's Speech Recognition API when
available; the browser or its provider may process audio.

`./command/.` shortcuts open browser sections and supervised local-action dialogs.
Custom provider routing and per-chat supervision are available, with the limits
and server requirements described below. CLI process management and other CLI-only commands are not connected to browser
chat. CLI sessions can be imported as separate browser chats.

### Supervised web work tracker (experimental)

`./work/.` opens the CLI work tracker for the configured server execution
workspace. Start creates its plaintext baseline, status reports added/removed
lines and changed/skipped file counts, and end deletes the baseline. Starting
and ending are high-risk actions; status is read-only. Each follows the chat
supervision policy and uses a one-use prepared action. Before end, the server
binds approval to the baseline contents and checks it again. Web operations are
serialized with one another, but not with CLI commands or external writers.

The browser displays counts locally and can explicitly insert them into an empty
composer draft; it does not automatically send them to a model, GitHub or a PR.
The counts are not attribution to ISA: they include all qualifying changes
since the baseline. File and size exclusions and concurrent changes can make
them incomplete. The baseline contains unencrypted text under `.isa/work`;
exclude `.isa/` from version control and protect it. Ending is irreversible.
Filesystem operations run in the server, not the command sandbox, and closing
the dialog does not guarantee cancellation of an in-progress scan. The Rust
backend and real integration test have not been compiled or executed in this
environment; JavaScript tests simulate the supervisor.

### Import CLI sessions into browser chat (experimental)

Run `./sessions/.` in the composer on the direct local Rust server, enter the
local execution token and list sessions from `.isa/sessions` under
`ISA_WEB_EXEC_WORKSPACE`. Select a session, load and review it, then explicitly
import it as a **new** browser chat. Import is read-only: it never modifies the
CLI session, merges with an existing chat or synchronizes later changes. Original
CLI timestamps, images and other metadata are not imported. All imported text
is saved in browser local storage and may be included in later provider calls;
review it for sensitive data and prompt injection. The browser folder picker and
`ISA_WEB_AGENT_WORKSPACE` do not choose the session directory.

The list returns at most 100 IDs from at most 1,000 directory entries, with a
truncation indicator. Imported session files must be version 1, at most 256 KiB,
and contain 1–200 user/assistant text messages, at most 64 KiB per message and
128 KiB total. Oversized or malformed files are refused rather than truncated.
The requests require the local execution capability, exact loopback connection
and same origin. They do not grant action approvals; the browser never sends the
session to a model as part of import. Node and static previews cannot use this
bridge. The Rust backend and real end-to-end test remain uncompiled and untested
in this environment; browser tests use a simulated server.

### Server and workspace inspection

Chat settings identifies a compatible Rust server or the Node development
preview using `/api/chat-options`. The declaration reports whether the server's
platform supports the full current supervised-action implementation; on Windows it
reports unavailable and separately indicates whether the experimental read-only
or manual PowerShell mode is enabled. A supported platform is only a hint, not evidence that a
workspace, profile or token is configured, nor authorization or proof of sandbox
support. Older servers may not report platform support. Custom providers may be
called directly by the browser even in the Node preview, and an unknown or
static server cannot be assumed to support local actions.
**Review server workspaces** requires a direct `127.0.0.1` connection and the
local execution token. It compares the canonical execution and search/proposal
workspaces without recording them in the chat or sending them to a provider.
It reports when the two differ, or when the search workspace is not configured.
The execution workspace and token must already be configured on the Rust server;
this is read-only and does not alter settings or policy. The browser folder name
is shown separately and is not a server path. Reconfigure the environment and
restart the server to change server workspaces. Paths can be sensitive; the
comparison endpoint is authenticated but is not a defense against a compromised
browser or same-user process. Backend changes and end-to-end tests remain
uncompiled and unverified in this environment.


Provider brand icons are sourced from [SVGL](https://github.com/pheralb/svgl) and [Simple Icons](https://simpleicons.org/) (Google Docs). Brand names and logos belong to their respective owners; their display does not imply a connection or affiliation.

## Work change counts

`isa-cli work start [directory]` records a baseline of the workspace files at the start of a work session. `isa-cli work status [directory]` reports added and removed text lines relative to that baseline; it does not count chat responses. `isa-cli work end [directory]` deletes the baseline. The counts include any qualifying file edits made after the start, regardless of who made them, not only edits by an agent.

The snapshot stores file contents unencrypted under `.isa/work` until ended. The scan respects ignore rules and excludes common sensitive paths, but is not a complete secret filter. Binary files and text files over 512 KiB or 10,000 lines are skipped and reported; the total captured text is limited to 16 MiB. The tracker is also available in the browser as described below; it is not connected to GitHub or PR creation. Concurrent changes during a scan and hostile workspace modification are not isolated.

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

The index is stored in `.isa/semantic-index.json`, ignored by this repository's `.gitignore`, and contains source excerpts and embeddings in plaintext. Protect it as sensitive project data and exclude `.isa/` in other repositories. Indexing respects ignore rules and the CLI's excluded paths, skips binary and large files, and limits index size; it cannot reliably identify every secret embedded in source files. Review your workspace and ignore rules before indexing. Source text is sent to the local Ollama service, not an external provider. Output includes paths, line ranges, similarity scores, and excerpts marked as untrusted. Deleted, ignored, or changed chunks are omitted from searches until the index is rebuilt; rebuild after edits to include new content. Unchanged chunks are reused when the model digest matches. Changing the installed model requires rebuilding. The browser can explicitly search this index as described below; the agent does not automatically consume it. `rank` and `pack` remain lexical.

The CLI's initial response adaptation harness is documented in [Harness Isa](docs/harness.md). It is inspired by the propose–probe–compose pattern in Harness², not an integration of the upstream research framework. It performs up to four model calls (two probes, a review, and a composition). Set `ISA_HARNESS=0` to disable the extra calls.

### Browser chat with the response harness

The Rust web server can explicitly use a local Ollama model for browser chat:

```sh
ISA_WEB_MODEL=llama3.2 ISA_WEB_EXEC_TOKEN="<32-or-more-printable-characters>" cargo run --bin isa
```

Start Ollama and pull the specified model first. Open the server directly on `http://127.0.0.1` and use `./local-token/.` to enter the configured token in tab memory. The browser sends recent text messages to the same-origin `/api/chat` endpoint; the server invokes the Rust response harness and local Ollama. Without `ISA_WEB_MODEL`, requests go to the decoy and return the no-provider error, not a fabricated answer. `ISA_HARNESS=0` disables the extra probe calls. The Node development preview only supports the decoy, not the real model endpoint. Supported attachments can be sent to the configured local model as described below. Stop cancels the browser request but does not yet cancel an in-progress Ollama computation on the server. Do not expose this unauthenticated local server to untrusted networks: same-origin checks are not a substitute for authentication, especially behind a public preview proxy. No API credentials are needed for the local model.

The response harness supports explicit call budgets (`ISA_HARNESS_MODE=quick`, `balanced`, or `thorough`) and optional project retrieval (`ISA_HARNESS_CONTEXT=lexical` or `semantic`, disabled by default). See [Harness Isa](docs/harness.md) for privacy, workspace selection, and safety limitations.

For repeatable, explicitly defined harness checks, use `isa-cli harness-eval <suite.json> <quick|balanced|thorough>`. This sends each case to the local model and reports simple assertion results, not a general quality score. See [Harness Isa](docs/harness.md) for the suite format and limitations.

ISA supports reviewed, text-only local skills for the response harness. See [Skills](docs/skills.md) for creation, editing, enable/disable, model-generated proposals, and on-demand selection. Skills do not run tools or grant permissions.

### Local action supervision

The composer stores a **per-chat supervision preference**: ask every time,
ask occasionally (risky or unknown actions), or never ask. New and legacy
chats default to ask every time; the old global preference is not migrated.
Selecting never ask requires an explicit confirmation for the current chat.
For the opt-in local actions below, the Rust server enforces the registered
per-chat policy. All shell commands are high-risk: both ask modes require
individual approval; never ask starts them without another approval. The built-in
workspace-info action is read-only and reveals only the configured workspace
path and whether it is a directory; ask every time requires approval, while ask
occasionally and never ask do not. The server assigns risk, not the client.
Ordinary Agent Pool messages and chat sends without the optional tool loop do not
invoke these actions. The opt-in Tool loop, Chat MCP and Pool MCP can invoke MCP;
manual, chat and pool MCP actions use the same web policy. Skill actions are not connected. CLI approvals are separate. Model
data-sharing consent is also separate. No mode makes the browser a sandbox.

### Supervised web semantic search (experimental)

After building the CLI index, `./semantic-search/.` opens a read-only search of
`.isa/semantic-index.json` in `ISA_WEB_EXEC_WORKSPACE`. It requires the direct
local Rust server and execution token. The query is embedded by the configured
local Ollama model; each search follows the chat supervision policy and requires
**separate explicit consent** to send the query to Ollama even in never-ask mode.
The search does not rebuild the index and does not use the browser folder or
`ISA_WEB_AGENT_WORKSPACE`. Missing indexes or incompatible embedding models fail instead of falling back;
stale or excluded source chunks are omitted from results.

Results are shown locally as untrusted excerpts, never automatically saved or
sent to the chat provider. A separate button can insert them into an empty
composer draft; sending that draft is another user action. Excerpts can contain
secrets or prompt injections. Closing the dialog cannot guarantee cancellation
of dispatched server work or Ollama computation. The web backend and a real
Ollama/index end-to-end test have not been compiled or executed in this
environment; JavaScript tests use a simulated supervisor and provider.

### Local workspace access (experimental hardening)

Read-only workspace endpoints `/api/changes`, `/api/symbols`,
`/api/edit-proposal` and `/api/edit-preview` require the same local capability
and exact loopback origin/host checks as supervised actions. Ollama `/api/chat` also requires it: the harness can read the configured
project context, independently of browser settings.
Configure `ISA_WEB_EXEC_TOKEN` and enter it with `./local-token/.` from direct
`http://127.0.0.1`; the token remains in tab memory. This does not authenticate
all other endpoints or defend against a compromised local browser or same-user
process. Remote previews cannot use these operations. Do not expose the server
to untrusted networks. Rust HTTP tests for this hardening have been added but
have not run in this environment.

### Server capability check

`./capabilities/.` reads the same-origin server's capability declaration after
the chat's supervision decision. It distinguishes the limited Node development
preview from the Rust server, reports which configured capabilities the server
claims, and warns when the browser origin cannot use the local executor. It is
informational: a declaration does not verify the execution token, sandbox,
workspace, Ollama or provider. The check does not send chat content or the
execution token. A remote preview must never be given the local token.

### Local tool commands

The composer exposes these actions as commands rather than toolbar buttons. Type
`./` for suggestions, use Tab to complete, and send the exact command to open its
existing dialog. Commands do not themselves approve or execute an action, and
require an existing chat. Attachments are not accepted with these commands.

| Command | Opens |
| --- | --- |
| `./workspace-info/.` | Workspace info |
| `./mcp-tools/.` | Manual MCP tools |
| `./tool-loop/.` | Supervised model tool loop |
| `./chat-mcp/.` | Chat MCP |
| `./pool-mcp/.` | Pool MCP |
| `./action-history/.` | Action history |
| `./edit-transaction/.` | Multi-file edit transaction |
| `./undo-transaction/.` | Transaction undo |
| `./undo-edit/.` | Single-file edit undo |
| `./propose-edit/.` | Generate a read-only edit proposal |
| `./semantic-search/.` | Search an existing CLI semantic index |
| `./work/.` | Start, inspect or end a work baseline |
| `./running-actions/.` | Check and request stop of registered web shell actions |
| `./process-snapshot/.` | Review a manually copied CLI process snapshot |
| `./capabilities/.` | Check the server capability declaration |
| `./local-token/.` | Set or clear the local workspace token in this tab |

The dialogs retain their local-server requirements, policy checks, approvals
and separate provider-sharing consent. `Run command` remains on the toolbar:
it opens a supervised shell command form; its effects, output and Stop limitations
are described below. These commands do not bypass those checks.

### Applying one reviewed edit from web chat

For a checked SEARCH/REPLACE proposal, **Apply in local workspace** can prepare
and apply one replacement through the opt-in authenticated local supervisor.
The target is an existing file in `ISA_WEB_EXEC_WORKSPACE`, which may differ
from the read-only preview workspace. The server independently checks the exact
proposal, protected paths and a SHA-256 snapshot; it rejects a file changed
since preparation. In both ask modes the exact action requires approval; never
ask runs after the user starts the prepared action. The write uses a same-file
atomic rename, but path and content checks can race hostile concurrent changes.
Web edit writes are serialized within this server process, but not against shell
commands or other processes. This is not a multi-file transaction, an AST edit,
or a sandboxed write. Before a write, ISA stores the complete original in a
persistent, unencrypted record under `ISA_WEB_EXEC_WORKSPACE/.isa/web-undo`.
The directory and records are restricted to the server user on Unix; this is
not protection against the same user or hostile filesystem races. Ensure `.isa/`
is excluded from version control and backups as appropriate. Records do not
expire or get deleted automatically and may consume disk space; delete sensitive
records manually when no longer needed. A write is refused if the undo record
cannot be stored. A failed write may leave an unused record on disk.

**Undo edit** separately prepares a restore under the chat policy. It restores
the original file only if the current content matches the post-edit snapshot;
if it already matches the original, undo reports a no-op. Other changes are not
overwritten. Records are reusable, including after restart, but approvals remain
single-use. Browser local storage keeps record references and filenames (not
file contents), scoped by chat; clearing browser storage removes those UI
references, not the server records. Undo does not restore prior metadata or
permissions or reverse test commands and side effects. This undo is independent
of the CLI undo record. The optional bubblewrap
profile applies to spawned Bash/MCP processes, not this server-side file write.
An optional test command supplied before applying is prepared automatically
after a successful edit through the separate command supervisor; ask modes require
a separate approval, while never-ask executes it without another prompt. A failed
test does not roll back the edit. Alternatively, after a successful edit the dialog
can prepare a separate supervised test command, or prepare a reverse
SEARCH/REPLACE when the replacement is nonempty.
The reverse is not a reliable undo: it may be ambiguous, match different text,
or fail after changes; it must be reviewed and authorized as a new edit. ISA does not select test commands automatically; commands may have side effects.
Inspect and test changes, and use a trusted workspace. The read-only preview is
not authorization, and the model-driven search loop cannot invoke this action.
The current Rust backend and end-to-end integration require validation before
relying on this functionality.

### Supervised multi-file edits (experimental)

**Edit transaction** accepts a JSON array of one to twelve existing-file
SEARCH/REPLACE proposal strings (7,000 bytes total; HTTP request limit 8,192
bytes). The original contents of all target files together are limited to 1 MiB.
Prepare reviews every file against its snapshot. This action is high risk:
always-ask and occasionally-ask require approval; never-ask does not. An approval
is single-use and bound to the chat, workspace and policy. Protected paths,
symlinks, duplicate paths and hard-linked files are rejected. Unix file identities
are checked for aliases; non-Unix systems fail closed. The snapshots are checked
again before writing.

**Undo transaction** separately restores all original contents only if every
file matches its post-edit snapshot; if all already match their original
snapshots, it reports a no-op. Mixed or modified states are rejected. The undo
record is persistent and reusable like single-file undo, but every prepared
action is single-use and follows the chat policy. Browser local storage retains
references and filenames for the same chat. Metadata other than the current
file permissions and external side effects are not restored.

The operations stage files and use conditional best-effort rollback after a write
failure. Errors report whether nothing was replaced, rollback completed, or
rollback was partial, including unresolved paths; inspect files before retrying.
This is not a filesystem-wide atomic transaction: other processes can observe
intermediate states, race checks or modify files, and a crash or failed rollback
can leave partial changes. The UI is manual and is not connected to the model tool
loop. The Rust backend and real end-to-end tests have not been compiled or run in
this environment; isolated engine tests and simulated UI tests are not a substitute.

### Supervised model tool loop (experimental)

**Tool loop** is an explicitly started interaction in a chat routed to a custom
OpenAI-compatible provider. You can start it using the toolbar, or opt in per
chat with **Supervised tool loop on send** in Chat settings. In the latter mode,
sending a message opens the loop for that exact message; it requires a local
token and still requests consent for every provider call. It does not accept
attachments or include earlier chat history. The manual toolbar interaction
instead starts with the most recent message as an editable task. Using a bounded JSON
protocol (not native provider tool calling), the model may propose up to twelve
workspace-info, Bash, one-file SEARCH/REPLACE, MCP startup and MCP tool-call
actions, then must return a final answer. The model can also propose a concrete
sequential plan of one to twelve of these actions, subject to the same total
twelve-action budget for the loop. The whole plan is checked for decision shape, per-field size and duplicates
before its first step; MCP tool availability and supervisor checks are evaluated
again at each step. Steps run in order without replanning between them, and stop
on denial, failure, cancellation or context change. The plan is displayed but is
not a separate approval: each step follows the chat supervision policy, including
never-ask. Results are not sent to the provider until a later separately approved
request. The 16 KB context limit still applies; a long plan or large outputs
can prevent a subsequent provider request after actions have already run. MCP uses the separately configured stdio server and one call per
discovered session. Duplicate logical actions are refused. Action preparation, risk classification, policy and one-use approval
are enforced by the existing server supervisor. In never-ask mode actions run
without further action approval; each outgoing model request still requires
separate explicit consent. The exact task, action history and untrusted outputs
are shown before sending. Context and output limits fail closed; a failed,
unknown or incomplete action stops the loop. MCP's declared-idempotent retry
can duplicate effects. Tool output is not automatically saved in chat; the final answer may be added
explicitly, or is saved automatically for a successful tool-loop-on-send run.
The answer may contain sensitive information and later requests may include it. Filesystem writes are not sandboxed and
are not a multi-file transaction. Stop requests cancellation of a running Bash
command but does not guarantee termination of MCP, provider work, escaped
processes or effects already performed. A single-file edit can create a
persistent undo record, subject to the limitations above.

This loop has only been exercised with simulated providers and supervisor. The
Rust backend and real end-to-end integration remain uncompiled and untested in
this environment. It is not integrated with the Ollama read-only loop, agent
pools, the separate goal-plan runner or native OpenAI/Anthropic tool calling, and
it does not run automated tests unless the model proposes a supervised command.

### Explicit goal-plan commands

A generated goal plan can be opened with **Run explicit plan steps**. The user
must supply one to eight named Bash commands as JSON; ISA does not translate
model text into commands or autonomously decide whether the plan is correct.
Steps run sequentially in the authenticated local supervisor. Each command is
prepared with the chat's current policy; both ask modes require approval for
each step, while never-ask runs the explicitly supplied steps without another
prompt. The run stops on denial, changed chat/plan/policy, nonzero exit, timeout,
stop or incomplete output. Completed commands are not rolled back, and the
stop request is best effort. Output is shown in the dialog and is not forwarded
to a model or saved to the chat. This is not an autonomous agent loop, a
multi-file transaction or a proof that the goal succeeded. The Rust supervisor
and end-to-end behavior remain unverified in this environment.

Example input:

```json
[{"step":"Inspect status","command":"git status --short"},{"step":"Run tests","command":"cargo test"}]
```

### Local action history

**Action history** displays recent supervisor events for the active chat when
using the opt-in Rust local executor directly on `127.0.0.1`. It requires the
same execution token and local origin checks as the action API. Events contain
only an independent random action reference, action type, status, manual/pool
source, and server timestamp; commands, arguments, results, approval IDs and
tokens are not stored in the metadata log. When explicitly enabled, separate
encrypted details may contain arguments and results; see below. The server attempts to append redacted events to the
execution workspace at `.isa/web-audit/events.jsonl`, with restrictive Unix
permissions and fsync. The UI retrieves at most 100 recent events for the chat;
the log is limited to 8 MiB and has no automatic rotation. When persistence
fails or the log is unsafe/corrupt/full, the UI reports degraded status and
shows only this process's in-memory events. Once degraded, the status remains
until server restart; missed events cannot be recovered automatically. A restart
cannot establish whether a previous failure created gaps. Operators should
protect, archive and manage the log themselves; do not delete it while the
server is running. The log is unencrypted, can reveal chat identifiers, action
kinds and times, and is not tamper-proof, a complete transactional journal, or
proof of human approval. Crashes and external writers can cause gaps or races;
actions may proceed even if audit persistence fails. An unknown outcome may
have produced side effects; do not blindly retry. The interface is read-only and
does not share events with models. Persistent audit has isolated storage tests
but Rust server/restart integration has not been executed in this environment.
Pool events describe supervised action API use, not model-sharing consent.

### Opt-in local supervised actions

On Unix, a user can manually open **Run command** or **Workspace info**
in an existing chat. An untested Windows-only experimental mode is described
in the Windows status above; it uses PowerShell, not Bash.
This is disabled by default. Start the Rust server on `127.0.0.1` with an
explicit trusted workspace and a randomly generated secret, for example:

```sh
export ISA_WEB_EXEC_WORKSPACE="$PWD"
export ISA_WEB_EXEC_TOKEN="$(openssl rand -hex 32)"
cargo run
```

Enter the token in the dialog on the same local machine; it is retained only in
page memory until reload and is not saved in the chat. Requests require the
secret, exact loopback IP/port Host and Origin, local peer, and reject forwarded
headers. The executor intentionally does **not** work through public preview
proxies or non-loopback access. Treat the token as a local bearer capability:
any process or script that obtains it can set policies and authorize commands.
This is not user authentication against a compromised browser or same-user
process, and does not replace OS isolation. Do not expose the server or token to
untrusted clients. Restart the server to clear policies and pending actions.

The server keeps each chat policy in memory and issues a one-use, 120-second
approval bound to the exact typed action, chat, workspace and policy version. A policy
change invalidates pending actions. Denial consumes/cancels an action. In
occasional mode all shell commands require approval because risk cannot be
reliably inferred from shell syntax. Never ask executes the manually entered
command after preparation without a second prompt. The manual Run command form executes user-entered text. The separate opt-in
tool loop can prepare model-proposed commands under the same policy. Commands
run as Bash with the server's permissions and environment, without a sandbox;
they can read or modify files, access the network, and may spawn descendants.
The execution token is removed from the child environment, but this does not
protect it from a hostile same-user process. Output is capped and saved in the
chat as untrusted text, which later model requests may include. While a command
is running, **Stop command** asks the authenticated server to terminate its
process group; a stop before dispatch cancels the pending action. The request
is best effort: already completed side effects and escaped descendants may
remain, and a stop request is not proof of termination. Closing the dialog alone
does not request stop. This applies only to supervised shell commands, not MCP,
model inference, or the main chat Stop button. The server integration has not
been compiled or tested in this environment. Do not blindly retry after a lost response. The Node preview and decoy
cannot run these actions. The server handles at most two concurrent actions.

Workspace info saves its result as untrusted chat text, which later model
requests may include; it does not list files or run programs. Its read-only
classification is specific to this implemented action, not a general claim
about other tools. The typed action API rejects unknown actions and client-supplied
risk labels. The legacy command API uses the same supervisor. Adding MCP or
other tools requires explicit implementations and their own security review.

The browser's local setting is sent to the authenticated server when a manual
action is prepared; it is not itself an authorization boundary. The server
cannot distinguish a human click from another bearer using the token. The experimental model-driven tool loop uses this gate for supported actions;
skills are not connected to it.

### Attachments

The composer stages up to eight files (10 MiB each, 25 MiB in total), including pasted images. It previews supported images, renders PDF pages locally, extracts readable text from DOCX, displays UTF-8 text and code, and plays audio or video when the browser supports the codec. Unsupported or binary formats remain downloadable. Image markings can be saved as a PNG copy; the original is preserved.

With the **Rust server** and `ISA_WEB_MODEL` configured, the chat sends UTF-8 text, extracted PDF and DOCX text, and up to two images to local Ollama. Images require a model that reports the `vision` capability; each image must be at most 2 MiB after conversion or marking. Other formats, scanned PDFs without extractable text, password-protected documents, documents over 40 pages, and more than 20 KiB of combined extracted document text are rejected with an explanation rather than silently omitted. DOCX input is limited to 2 MiB and bounded archive metadata; preview and extraction are not a malware sandbox. Audio, video, spreadsheets, and other binaries can be previewed or downloaded where supported but cannot yet be sent to the model. The Node development preview still uses only the decoy and does not send files to an AI provider.

Document and image contents are treated as untrusted input, but prompt injection remains possible. The contents are sent to local Ollama during response probes and composition; the harness's planner may also see part of the message text. Files are held in memory only: sent attachment chips can be reopened while they remain in the session's bounded in-memory cache, but not after reload or eviction. Chat history stores attachment names and temporary references, not file contents. Subsequent turns do not resend earlier files automatically. Review sensitive documents before attaching them. Do not expose the unauthenticated local server to untrusted networks.

PDF previews and text extraction use locally bundled [PDF.js](https://github.com/mozilla/pdf.js) (Apache-2.0). DOCX text extraction uses [Mammoth.js](https://github.com/mwilliamson/mammoth.js) (BSD-2-Clause, including the licenses listed in its bundle). Copies of their licenses are in `assets/vendor`.

### Task overview and Git changes panels

The upper-right Task overview panel shows the active browser conversation's latest request, actual provider status, latest response or error, and the plan generated by `./goal/.` for review. Generated plans do not run steps or tools automatically; the separate explicit-command runner above can run user-supplied steps, and its activity is not connected to this panel. The Changes panel displays read-only staged and unstaged Git diffs, split by file, when the **Rust** web server is explicitly started with a workspace:

```sh
ISA_WEB_WORKSPACE=/absolute/path/to/repository cargo run --bin isa
```

Drag the left edge of either panel to resize it; the width is saved in the browser. With the divider focused, use the left/right arrow keys (Shift for larger steps), or Home/End to reach the limits. Use Refresh to reload changes. It shows tracked Git diffs only; untracked and ignored files are not included. Binary patches may not show file contents. Each diff is capped at 2 MiB, and display is capped at 200 files per section and 2,000 lines per file. Changes are not attributed specifically to the agent, and revert is not implemented. The browser's folder picker does not select the server workspace. The Node development preview has no Git endpoint and displays an unavailable message.

The configured directory may contain sensitive changes. Workspace diffs, symbol lookup, edit proposal and preview now require the local execution token and exact direct loopback checks. Ollama chat also requires them because the harness can access project context. Set `ISA_WEB_EXEC_TOKEN` and use `./local-token/.` on direct `http://127.0.0.1` to enter it into tab memory. The general chat server is not user-authenticated; do not expose it to untrusted networks. Without `ISA_WEB_WORKSPACE`, no workspace diff is exposed. These changes have not been compiled or tested against a real Rust server in this environment.

### Supervised shell action status (experimental)

`./running-actions/.` checks the number of registered web shell actions for the
current chat, after the chat's normal supervision decision. It requires the
local Rust execution supervisor and token. The count includes actions that may
be waiting for capacity; it is a point-in-time count, not a process list. It
contains action IDs but no commands, logs, process IDs or other chats' actions. A selected web action can receive a separate supervised stop request,
even after its original dialog is closed. It does not read, start or stop the
independent CLI process manager; a zero count does not prove that child
processes have stopped. Stop is cooperative and does not guarantee terminating
descendants or undoing effects. Nothing is sent to the model. JavaScript tests simulate the
backend; the Rust integration test has not been run in this environment.

### Encrypted action details (experimental; not validated)

The existing redacted metadata log remains plaintext. A separate, **opt-in and
unvalidated** implementation can store action arguments and results in
`.isa/web-audit/sealed.jsonl` using XChaCha20-Poly1305, with a workspace-specific
key in the operating system credential store. It is disabled by default. The
implementation and crypto tests have not been compiled or tested end to end in
this environment. Do not rely on this feature to protect sensitive data until
it has been validated on the target system.

Once validated, initialize the key interactively with `isa-cli audit-key init
/path/to/workspace`, then start the server with `ISA_WEB_AUDIT_ENCRYPTED=1` and
the same `ISA_WEB_EXEC_WORKSPACE`. Linux requires a working unlocked Secret
Service session. On Windows, the configured `windows-native` keyring backend
uses Windows Credential Manager; a named mutex serializes cooperating Windows
initializers for the same resolved account, but aliases and non-cooperating
writers remain risks. Credential-store recovery and the Windows encrypted store have not been validated.
For experimental testing only, set both `ISA_WEB_AUDIT_ENCRYPTED=1` and
`ISA_WEB_WINDOWS_AUDIT_EXPERIMENTAL=1`, plus the existing Windows read-only or
shell opt-in. Encrypted details and reported token usage can then be stored,
but event metadata remains volatile and reports degraded persistence. ACL and
path checks do not eliminate concurrent workspace/ancestor replacement races.
Do not use this untested mode to protect sensitive data.
On other platforms do not initialize the same key concurrently. Loss of the key
makes encrypted details unreadable; do not delete or replace it. Moving the
workspace changes the keychain lookup and requires a future migration mechanism.
The old redacted plaintext metadata is not automatically encrypted or deleted.

Arguments are recorded at proposal and results at completion, after limited
credential-field redaction. **Arbitrary text may still contain credentials or
other sensitive data.** Review your inputs and restrict access. Entries over
16 KiB are truncated; the viewer shows at most 20 details, and the encrypted
log has an 8 MiB limit without rotation. If the key, encryption or storage
fails, actions continue as requested; no sensitive details fall back to plaintext.
The action dialog displays an audit warning and the history dialog reports degraded
status. The experimental Windows path revalidates existing encrypted records before
each append; this can be slow and the check is not atomic with external changes.
Failures and crashes can leave
gaps, and the status is not independently tamper-proof. For normal chat sends
and manual context compression, reported numeric input/output/total/cached token
counts are also sent to the local supervisor for encrypted storage when the
browser has a local execution token. At most 32 calls per batch are saved and
truncation is marked. Missing provider usage, other workflows, remote previews,
closed tabs, absent tokens and failed requests can leave gaps; client-supplied
counts are not independently verified. Existing browser token-activity counts
remain in localStorage and are not encrypted by this feature. The key, execution
token and other deliberately recognized credential fields are not intentionally
recorded.

### Local background processes

`isa-cli processes [directory]` opens an interactive process supervisor. On Unix, use `start <name> <bash command>`, `list`, `snapshot`, `logs <name>`, `stop <name>`, `remove <name>` (after exit), or `quit`. It starts real background processes in the selected working directory and captures the most recent 64 KiB of combined output in memory. Each new command requires terminal approval; “allow always” applies only to the exact command for the current supervisor session. Denial does not run the command. The supervisor attempts to stop its process groups on exit. Commands run with the user's permissions, without a sandbox, and inherit the user's environment (which can include credentials); they can access files and the network. Processes that daemonize, escape their process group, or outlive an already-exited group leader may not be stopped automatically. Do not use this as a security isolation mechanism.

On Windows, `ISA_WINDOWS_PROCESSES_EXPERIMENTAL=1` opts into an **unvalidated**, non-production PowerShell variant with the same approval flow and an eight-process limit. It uses the experimental Job Object runner, with only `SystemRoot` in the child environment and a 24-hour maximum runtime. `logs` becomes available only when the process exits, retaining up to 8 KiB per stream. Stop and job cleanup are best effort and cannot undo side effects; escaped or indirectly launched processes may survive. The script is Base64-encoded, not encrypted, and can be visible in the command line. Handle-inheritance races remain. Windows tests are configured but have not run.

For a manual, read-only bridge, enter `snapshot` in the interactive CLI process
manager and copy its single JSON line into `./process-snapshot/.` in a browser
chat. This is an unverified point-in-time copy, not a live connection or a
process control interface. It includes process names, state of the tracked
leader and elapsed time since start, but no commands, logs, PIDs or workspace
paths. Names can still be sensitive. The browser validates and reviews it
locally using the chat's supervision preference; nothing is automatically saved
or sent to a provider. Inserting it into an empty draft and sending that draft
are separate choices. A snapshot cannot prove that descendants stopped.

The CLI process manager runs in its own interactive process and has no shared registry with the web server. The browser has a display component for genuine command and process activity records. The manual chat executor and experimental model tool loop are separate from the CLI process manager; they do not stream the CLI manager’s records. It therefore does not show simulated activity, cannot start or stop these CLI-supervised processes from the browser, and cannot yet automatically suggest an alternative after denial. Browser approval preferences are not connected to this supervisor. The CLI supervisor must stay open to manage its processes; it is not integrated with the Arena live preview system.

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
invoked from an interactive terminal. The browser has a separate proposal review and supervised single-file apply flow; neither flow is an autonomous agent-loop tool. Ollama receives the full file, which may contain
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

### Optional Linux resource profile

For web-launched Bash commands and MCP servers, Linux users can opt into a
restricted **resource profile** before starting ISA:

```sh
export ISA_WEB_EXEC_PROFILE=limited
```

The default profile is `host`. An unknown profile or `limited` on an unsupported
platform disables local actions rather than falling back. The selected profile
is shown in the action review and bound to pending approvals and MCP sessions.
This does not alter the separately approved CLI commands or MCP sessions.

The limited profile starts web child processes with a minimal environment
(`PATH=/usr/bin:/bin`, `LANG=C.UTF-8`) and applies Linux per-process hard and
soft limits before execution: no core dumps, 30 seconds of CPU time, 4 billion
bytes of virtual address space, 32 MiB per file, and 128 open file descriptors.
Existing stricter inherited limits are not raised. If the OS rejects a limit,
process startup fails; ISA does not retry without the limits. These settings may
prevent software or MCP servers that depend on a custom environment, larger
address space, or extra file descriptors from working.

**This is not a sandbox.** The child still has the server user's filesystem,
network, and operating-system permissions, can access sensitive files and may
start descendants. Limits apply per process, not as a total process-tree,
network, disk-space or memory budget; they do not prevent all denial of service
or data exfiltration. Privileged processes may be able to change limits. The
minimal environment does not protect against hostile processes running as the
same user. Existing wall-time, output, concurrency and best-effort process-group
cleanup remain separate and do not guarantee stopping descendants or undoing
side effects. Use a separate OS-level isolation boundary for untrusted code.

### Experimental Linux bubblewrap profile

On Linux, `ISA_WEB_EXEC_PROFILE=sandboxed` uses `/usr/bin/bwrap` for web-launched
commands and MCP servers and applies the limited profile's resource limits. It
requires bubblewrap and usable unprivileged user namespaces; it does not fall
back to host execution if either is unavailable. The selected workspace is
writable at `/workspace` inside the child, so configure MCP programs and paths
accordingly. The host root directory cannot be used as the workspace. `/usr`,
`/bin`, library directories, DNS configuration and certificates are exposed
read-only to support runtimes and networking; `/tmp` is private. Networking is
**enabled**, including access to local services, as requested. This allows data
exfiltration and does not prevent access through services or Unix sockets in the
workspace. Runtime directories and certificates are visible; this is not complete
host confidentiality, a VM, or protection against kernel vulnerabilities or all
resource exhaustion. Choose a trusted workspace and MCP server. This profile has
only isolated local component tests so far; full Rust/server integration has not
been validated for these changes.

Windows does not currently have a sandboxed execution profile. See the
[Windows sandbox design proposal](docs/windows-sandbox-design.md) for candidate
isolation, workspace and network tradeoffs; it is not an implemented feature.

### Manual MCP tools in web chat

Unix local sessions can use **MCP tools** in an existing chat after configuring
the opt-in local action supervisor above. Set `ISA_WEB_MCP_COMMAND` before
starting the Rust server to a JSON array containing an executable and its
arguments, for example:

```sh
export ISA_WEB_MCP_COMMAND='["/usr/bin/node","/absolute/path/to/trusted-server.js"]'
```

Do not include secrets in the arguments: they appear in the review dialog. The
server command comes only from backend configuration, never from model output.
Starting and discovering a server is a high-risk action. Both ask modes require
approval; never ask permits it without supervisor approval, after the user
explicitly starts the prepared action. Each tool call is a separate high-risk
action, with its exact name, JSON arguments, server command, workspace, chat,
policy and live discovery session bound to a one-use approval. Schema validation
is not implemented. Incomplete discovery and duplicate names are rejected.

The session expires after five minutes and permits **one logical tool action** (up to two attempts for a declared-idempotent tool). Closing
or changing policy invalidates a session; after a call or an error, start and
approve a new session. Up to four sessions and two concurrent actions are
allowed. Before starting MCP, the supervisor retries admission up to three times
when execution capacity or the pool serial gate is busy, with 150 ms between
attempts. The same one-use authorization covers these admission attempts; the
policy, workspace, profile and expiry are checked again before dispatch. A tool whose discovered metadata has `annotations.idempotentHint: true` is
automatically called a second time after a complete `isError: true` response,
subject to the remaining pool budget and a fresh policy/configuration check.
The approval review shows the maximum attempt count. The server supplies this
hint and may be wrong: attempts can duplicate effects. Timeouts, lost responses,
transport/protocol errors and server starts are not retried. A failed or
interrupted call may already have had side effects. A running process or descendants may outlive a
close or timeout. Server metadata and results are untrusted, shown locally in
the dialog, and are not automatically saved in chat or sent to a model.
Regular message sends and ordinary pool messages do not invoke MCP. The separate
**Chat MCP** and **Pool MCP** interactions described below support single-agent and opt-in
workflow calls with staged consent.

MCP servers are executable programs and **not sandboxes**: startup, discovery
and calls may access files, inherited environment and network or modify data.
The execution capability and server command configuration are removed from the
child environment, but hostile same-user processes are not isolated. Use only
trusted servers and workspaces. CLI MCP has separate approvals and sessions.
The remote Node preview cannot use this feature. The same bearer-capability
limitations described above apply; possession of the token is not proof of a
human approval.

### MCP input schema validation (experimental)

CLI proposals and calls and web MCP call preparation check arguments against
server-provided input schemas. Schemas are untrusted: validation is not a safety
or authorization guarantee. This initial implementation rejects schemas over
16 KiB, excessive depth or node counts, all references (including local ones),
and custom vocabularies. Network and file reference retrieval are disabled;
patterns use the linear-time Rust regex engine, which does not support all
ECMA-262 patterns. Thus legitimate schemas may be rejected. Standard JSON Schema
format behavior and unknown annotation keywords depend on the selected draft and
validator; this is not full MCP schema coverage. The Rust implementation and
end-to-end tests have not yet been compiled or executed in this environment.

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
arguments, and asks separately before `tools/call`. Experimental input-schema
validation rejects invalid arguments before calling the tool; see limitations
below. With `propose`, after separate consent ISA sends the task and complete discovered
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
injection or unsafe arguments: review them. This CLI flow is separate from manual web MCP above. Only stdio and this protocol version
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
permissions and network access: **there is no sandbox**. On Unix it inherits
the environment and the 60-second best-effort timeout kills the process group,
but escaped descendants may survive. It can modify files, leak data or leave
background processes.

On Windows, CLI feedback test execution is **experimental, unvalidated, and off
by default**. Set `ISA_WINDOWS_FEEDBACK_EXPERIMENTAL=1` to opt in; the selected
program must be an absolute path to an existing `.exe`, with no shell inserted
or PATH lookup. The child environment contains only `SystemRoot`, so tests
requiring other environment variables or PATH may fail. Execution uses the
experimental Job Object runner and a 60-second timeout. Termination and
cancellation are best effort and do not undo side effects; handle-inheritance
races remain. The interactive loop does not provide a separate Stop control.
CLI edit/undo security limitations still apply. Do not rely on this mode in
production; Windows runtime tests have not been run.

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

Assistant messages containing `<<<<<<< SEARCH` offer **Review proposal**. Paste or edit one complete `File: …` SEARCH/REPLACE block in the dialog and select **Check proposal**. The Rust server checks the existing UTF-8 file in `ISA_WEB_AGENT_WORKSPACE`, validates the same format and exact unique match used by the CLI, and displays the submitted search and replacement literally. The check itself writes no file or undo record and does not apply a patch. A successful check is a snapshot, not a guarantee that the file remains unchanged. If the search has no exact match, the review, `check-edit`, and CLI replacement operations can report up to five approximate line ranges found by collapsing whitespace (files up to 64 KiB). These are hints only, may be partial or misleading, and contain no source excerpts; inspect the file and correct the proposal manually. They never relax the exact-match rule. If SEARCH matches more than once, including overlapping matches, ISA reports up to five exact-match line ranges for files up to 64 KiB; add context to make the search unique. It never chooses a match automatically. A single complete block surrounded by explanations or Markdown fences is extracted for review. When several unambiguous blocks are detected, choose and load one explicitly; only that block is checked, not the combined changes. Loading replaces any text already in the editor. Ambiguous or incomplete blocks must be edited manually. The review can optionally reveal spaces, tabs and line breaks. After a successful check, **Apply in local workspace** offers the supervised single-file write described above, and **Download proposal** saves the checked text to the browser device as `isa-proposal.txt`; it may contain sensitive content. Transfer it to the CLI host if necessary and run `check-edit` or `apply-edit` there, with the correct workspace. Downloading does not authorize or apply anything, and later checks may fail if the file changed. The browser folder picker does not configure the server workspace. The Node development preview does not implement this endpoint. Path exclusions and symlink checks are not a sandbox or race-free security boundary; use a trusted workspace and server, and do not expose it to untrusted clients.

### Chat MCP: opt-in one-tool interaction

For a chat routed to a configured custom provider, **Chat MCP** offers a
separate supervised one-tool interaction. It pre-fills the latest user message
as an editable task; conversation history and attachments are not sent. It uses
the same bounded pool scope internally, but no agent pool needs to be configured.
Startup and the proposed tool call follow the chat supervision policy. Sending
the task and discovered metadata to the provider, and later sending the tool
result, require separate explicit clicks even in never-ask mode. The result is
not saved automatically: an optional button adds the provider's interpretation
to the chat, where later requests may include it. A change to the chat history,
route, provider or supervision stops the interaction. Startup and tools can have
side effects even if no data is shared. This uses MCP stdio, not native provider
tool calling; schema validation has the limitations below. Regular message sends invoke MCP
only when the separate supervised tool loop is explicitly enabled. Server-side integration remains unverified until the
Rust backend and end-to-end tests can be compiled and run.

### Pool MCP supervisor and opt-in workflow

The local supervisor provides an internal, opt-in pool-run scope for MCP
orchestration. It binds a run to a chat, workspace, policy version and a set of
agent identifiers, and enforces at most eight server starts and eight calls per
run, serially, with one-use approvals and bounded pre-dispatch admission retries.
Sessions created
under a pool scope are bound to the originating agent identifier and cannot be
used by another agent or as unscoped manual sessions. Expiration,
closing the run or changing policy invalidates pending actions and sessions;
in-flight processes can still have effects. The run and agent identifiers are
supplied by the authenticated capability holder, not verified identities, and
that holder can use the independent manual API; this is not a security boundary
against a compromised browser or same-user process.

**Pool MCP** is a separate, opt-in interaction for a single chosen agent in the
active chat's selected pool. It is not part of the normal multi-agent workflow.
The user supplies a task and token, reviews the server start according to the
chat policy, and separately consents to send the displayed task, agent
instructions and tool metadata to the named provider endpoint. That provider
may propose one discovered tool and JSON arguments, which are prepared and
reviewed under the same policy before execution. A second explicit consent is
required to send the displayed tool result and task to that same provider for
interpretation. Nothing is automatically added to chat or forwarded to other
pool agents. Alternatively, **Use result in pool workflow** runs the existing
workflow with the task and result as untrusted context. Every outgoing request,
including retries and requests containing other agents’ responses, is shown
with the recipient and requires separate consent. Denial stops further sends;
requests already dispatched cannot be recalled. This workflow and its trace
remain in the dialog and are not saved to chat. Metadata and results over 16 KB
are not shared. Declared-idempotent tool errors may be retried once; other
dispatched failures are not. Schema validation has the limitations below. Provider instructions
and metadata are untrusted; inspect the exact action. A provider may retain or
redistribute data it receives. The user can close the dialog to stop further
steps and release the scope, but aborting or closing cannot undo dispatched
side effects or guarantee remote work stops. Provider usage from this separate
interaction is not included in chat totals.

Alternatively, **Run pool workflow with MCP** starts the pool workflow from this
dialog with MCP enabled for its active agents. Before an agent's first ordinary
response, the supervisor starts and discovers MCP for that agent, asks its
provider for a single JSON tool proposal, and executes a valid discovered call
under the chat's supervision policy. Planning and synthesis requests do not
initiate tool calls. Starts and calls are serialized and limited to eight per
run; agents after the start budget is exhausted continue without MCP, with that
limitation reported in pool details. Server startup can have side effects even
when the agent declines a call. The user's task and untrusted metadata go to the
requesting provider only after consent; tool results and responses can reach
other agents only through individually consented provider requests. Denial
stops further requests; in-flight operations may already have effects. The
workflow result is displayed in the dialog and is not saved to chat. This opt-in
path is separate from ordinary chat messages and does not enable tools for them.
An idempotent-declared tool may be called twice after a complete tool error;
timeouts, lost responses, transport errors and server starts are never repeated.

The backend and its Rust tests for these changes have not yet been validated in
the local environment. `tests/pool-mcp-e2e.cjs` exercises the real Rust server
and MCP fixture with simulated model providers after building the server; it is
configured in CI, but has not yet been run on these changes.
