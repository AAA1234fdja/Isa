# Changelog

## Unreleased changes since the last push

Comparison base: commit `30006e2` (`Use absolute Node path for limited MCP test`),
the last locally recorded pushed commit at the time this summary was written.
The entries describe changes made after that commit and do not constitute a release.
The summary covers both tracked modifications and newly added files.

### Frontend and appearance

- Replaced the monolithic page with a minimal `index.html` shell. The initial
  interface is now described in `frontend/src/layout-data.mjs` and created with
  DOM APIs by `frontend/src/layout.mjs`. JavaScript is required to display the
  workspace; the shell includes a no-script message. CSS and JavaScript are
  external, and the HTML has no inline styles, scripts or event handlers.
- Extracted `assets/interface.css` and feature-specific JavaScript assets for
  appearance, sidebar, notifications, attachments, projects, settings, agents,
  routing, chat and supervised actions. The Rust server and Node preview have
  allowlisted routes for the assets; Rust also embeds fallbacks. The preview
  monitor reloads after changes to the served interface assets.
- Added an esbuild-based frontend build with a locked npm dependency. Appearance,
  sidebar, notifications, the custom provider client, pool runner, project store
  and project presentation have ES module sources with injected dependencies.
  Provider, conversation, usage, attachment and diff helpers also have module
  sources. Checked-in bundles provide compatibility with the remaining scripts.
- **Migration is incomplete:** much of chat, supervision, agents, settings and
  application initialization still uses classic scripts, compatibility facades
  and shared state. The frontend has not been converted to TypeScript.
- Project metadata now uses a separate store. Failed persistence leaves project
  state unchanged; project UI drag state is isolated per module instance.
- Added 30 searchable themes to Appearance, for 33 total including the existing
  Forest, Moss and Sage palettes. New palettes support light, dark and system
  modes; selection is stored in the browser. Scale and reduced motion remain
  separate settings.
- Added the local `./isabel te amo/.` command and autocomplete. It saves a
  predefined response in the chat without calling a provider; this response is
  labeled **Ely Isa**, while other assistant messages remain labeled **Isa**.
  Previously saved versions of this particular response are displayed without
  the old `Isabel:` prefix.

### Local supervision and browser workflows

- Expanded the opt-in local supervisor and its browser dialogs for per-chat
  supervision: ask every time, ask occasionally for risky or unknown actions,
  or never ask after confirmation. Server-assigned risk, one-use approvals,
  expiry, workspace/chat/policy binding, cancellation and bounded admission
  apply as described in the README. Model data-sharing consent is separate
  from permission to execute a local action. CLI approvals are also separate.
- Added or expanded supervised shell action status, stop and action-history UI;
  server capability and workspace inspection; an authenticated workspace
  comparison; and manual, explicitly reviewed import of CLI sessions into new
  browser chats. Import does not synchronize or modify the CLI session.
- Added supervised browser access to the local work tracker and existing CLI
  semantic index, including explicit review and separate sharing of results.
  Work counts are not attribution to ISA. The work baseline and semantic index
  can contain sensitive workspace content in plaintext under `.isa/`.
- Added reviewed single-file SEARCH/REPLACE application and separate undo, plus
  experimental multi-file edit and undo transactions. Transactions accept 1–12
  proposals, at most 7,000 bytes total, and at most 1 MiB of original content.
  Approvals are bound to snapshots and file identity where supported. Writes
  and rollback are **not** a filesystem-wide atomic transaction and cannot
  prevent all races with outside processes.
- Added persistent, reusable undo records under the workspace's protected
  `.isa/web-undo`. Original file contents are **not encrypted**, remain readable
  to anyone with access, and have no automatic expiration. Undo references in
  the browser are separate from the records. Approval remains single-use.
- Added an opt-in supervised model tool loop, explicit goal-plan workflow with
  up to 12 proposed actions, and separate Chat MCP and Pool MCP interactions.
  MCP server startup, calls and model-sharing steps have distinct checks and
  consent as described in the README. Pool starts and calls have bounded scopes.
  Retrying a dispatched MCP tool is limited to tools explicitly declared
  idempotent; that declaration is not a guarantee of safety. Timeouts and
  uncertain outcomes are not automatically retried as completed tool errors.
- Added bounded MCP input-schema validation with no external or local reference
  resolution, plus stdio framing and partial-write handling. This is not full
  JSON Schema compatibility or a substitute for authorization.
- Added capability labels, local token handling, action-related commands and UI
  feedback for degraded audit storage. Browser tests use simulated servers for
  many of these workflows; see validation status below.

### Audit, execution profiles and platform work

- Added an opt-in, experimental encrypted action-detail path for arguments,
  results and provider-reported token counts using XChaCha20-Poly1305 and a
  workspace-specific key in the system credential store. Recognized credential
  fields are redacted, but arbitrary text can still contain sensitive data.
  Existing redacted event metadata remains plaintext where persisted; browser
  usage data and undo originals are not encrypted by this feature. Failed audit
  storage does not block actions and produces a warning without plaintext
  fallback for encrypted details. The implementation is **not validated**.
- Extended the Linux bubblewrap execution profile with a writable configured
  workspace and host networking. It applies to eligible spawned processes, not
  server-side edits, and is not a guarantee against all host or network risks.
- Added experimental Windows read-only workspace information and a separate
  opt-in for manual supervised PowerShell commands. Windows shell runs with the
  server user's privileges and host network; it is **not a sandbox**. Windows
  edits, undo, model-driven commands and MCP remain fail-closed. Limited and
  sandboxed profiles are not enabled on Windows.
- Added staged Windows primitives for process/job and pipe handling, suspended
  process creation, command-line and PowerShell handling, file identity, private
  ACLs, undo storage, MCP pipe writes and encrypted audit storage. These are
  experimental or test-only components, not evidence of a secure, working
  Windows integration; Windows compilation and runtime remain unverified.
- Documented a proposed Windows sandbox using a guest workspace copy and a
  separately approved import, with host/LAN and internet access, investigating
  both Hyper-V and Windows Sandbox. Added staged manifest, framing, export,
  import-plan and `.wsb` helpers. No VM, guest copy, execution or import is
  implemented. See `docs/windows-sandbox-design.md` for boundaries and risks.

### Repository, documentation and CI

- Added the Apache 2.0 project license, Cargo package metadata, `CONTRIBUTING.md`
  and `SECURITY.md`. Bundled third-party components retain their own licenses.
- Moved harness and skills Rust code to `src/harness/` and `src/skills/`; moved
  their documentation and the harness smoke-suite example to `docs/`. The
  third-party skill package under `Skills/web-design-engineer/` remains separate.
- Updated the README and provider documentation to describe current workflows,
  privacy and platform limitations rather than presenting staged functionality
  as fully implemented.
- Added `Cargo.lock`, pinned Rust 1.92.0 and separated the locked jsdom test
  dependencies from the MCP SDK fixture. Ignored local `node_modules` folders.
- Expanded the proposed CI configuration with locked builds, frontend bundle
  checks and browser tests, Linux Clippy and RustSec auditing, an advisory
  rustfmt check, and experimental Windows/macOS checks. These changed jobs have
  **not** been run on GitHub Actions for the current working tree.

### Verification and known limitations

- The latest local checks included 35 Node/jsdom test suites covering themes,
  asset loading, commands, projects, supervision and pools. Generated frontend
  bundles passed the reproducibility check. Earlier local checks also exercised
  the layout, project-store and module tests; jsdom is not a real browser.
- A local comparison of the JavaScript-created DOM against the former markup
  matched element order, text and attributes. This comparison does not establish
  visual equivalence across browsers.
- Standalone Linux harnesses exercised staged Windows sandbox planning, framing
  and `.wsb` template logic, not Windows guest execution or isolation.
- Full Rust compilation, real Rust-server end-to-end tests, real-browser tests,
  Windows/macOS runtime validation and the updated CI remain pending. The
  previous successful GitHub CI run applies only to the comparison base, not to
  the changes listed here.
- HTTP remains blocking; extracting the frontend does not resolve that
  architectural limitation. No release has been made for these changes.
