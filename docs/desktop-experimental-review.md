# Windows desktop experimental features: manual review

## Scope and current status

This document covers the experimental Windows desktop prototype and the
changes in this working tree. It is a review checklist, not a claim that these
features are secure, complete, or validated. The current environment cannot
compile or run .NET/Windows or Rust. Existing CI results for earlier commits do
not validate these changes. Do not use this prototype for sensitive or
production workloads until it has been reviewed and tested on Windows.

The wrapper now sets the backend's existing Windows read-only, manual
PowerShell, and encrypted-audit switches to `1`. This does **not** bypass the
existing token, explicit workspace, chat supervision policy, or separate
provider-sharing confirmations. It also does not turn unimplemented components
into working integrations. The desktop still binds only `127.0.0.1:4175`.

## Enabled switches and prerequisites

| Switch set by the desktop wrapper | What it enables | Remaining prerequisites / limitations |
| --- | --- | --- |
| `ISA_WEB_WINDOWS_READ_ONLY=1` | Existing Windows workspace-information action path. | Requires a valid execution workspace and token. It is not a sandbox or a full coding agent. |
| `ISA_WEB_WINDOWS_SHELL_EXPERIMENTAL=1` | Existing manually entered supervised PowerShell command path. | Same workspace and token; per-chat approval policy remains in effect. Runs with the user's privileges and network, not in a sandbox. Model-driven tool loops, edits and MCP are not enabled on Windows by this switch. |
| `ISA_WEB_AUDIT_ENCRYPTED=1` and `ISA_WEB_WINDOWS_AUDIT_EXPERIMENTAL=1` | Attempts existing encrypted action details and usage storage. | An audit key must first be initialized for the workspace in the OS credential store. Failures do not block actions; the interface warns of incomplete audit. Metadata remains volatile on the Windows path. Encryption and filesystem checks are experimental; do not rely on them to protect sensitive data. |

The desktop forwards **only** these explicit inherited settings when complete
and valid: `ISA_WEB_EXEC_WORKSPACE` plus `ISA_WEB_EXEC_TOKEN`, and optionally
`ISA_WEB_AGENT_WORKSPACE` plus `ISA_WEB_MODEL`. An explicit
`ISA_WEB_EXEC_PROFILE` is forwarded with a valid execution pair; unsupported
profiles, including `sandboxed` on Windows, fail closed instead of silently
falling back to host PowerShell. When either pair is provided,
the desktop asks for confirmation at startup; declining stops launch.
Workspaces must be absolute
existing directories; tokens must be 32–256 printable non-space ASCII
characters. All other inherited `ISA_` variables are removed, including MCP
command configuration. The agent workspace/model pair configures the existing
read-only Ollama agent path; enabling it per chat and confirming context sharing
are still required. The browser folder picker does not configure these paths.

### Example for a trusted Windows test workspace

Run in PowerShell after building the desktop prototype. Replace the path with a
real test workspace, not a valuable production directory. The generated token is
shown because it must be entered into the action dialog; keep the console and
process environment private and do not share or save the token in a repo.

```powershell
$env:ISA_WEB_EXEC_WORKSPACE = 'C:\path\to\your\test-workspace'
$bytes = New-Object byte[] 32
$rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
try { $rng.GetBytes($bytes) } finally { $rng.Dispose() }
$env:ISA_WEB_EXEC_TOKEN = -join ($bytes | ForEach-Object { $_.ToString('x2') })
Write-Host "Enter this temporary token in the ISA action dialog: $env:ISA_WEB_EXEC_TOKEN"
.\desktop-prototype\Isa.Desktop.exe
```

For the optional read-only Ollama agent loop, additionally set
`ISA_WEB_AGENT_WORKSPACE` to a trusted existing directory and `ISA_WEB_MODEL`
to an installed local Ollama model before launching. Ollama must be running.
Do not assume selecting a browser project changes either server workspace.

To test encrypted audit, the workspace key must be initialized using the
separately built `isa-cli audit-key init <workspace>` as described in the main
README. The desktop installer currently does not bundle that CLI. Never assume
an audit record exists merely because the switch is on.

## Experimental features and boundaries to inspect

- **Manual PowerShell:** commands can read, change or delete files, use the
  network and spawn processes. Job Object termination is best effort;
  descendants and side effects may survive. Encoded scripts may appear in
  process command lines. Changing a chat policy to never ask permits execution
  according to that policy; this change does not set that policy.
- **Workspace info and Ollama loop:** workspace paths are server configuration,
  distinct from browser project records. Context can be sent to the selected
  model after the existing consent flow; model output is untrusted.
- **Encrypted audit:** check Credential Manager access, workspace ACLs, key
  initialization, warnings, missing-record behavior, and recovery. Arguments
  and outputs can contain secrets despite limited redaction. Key loss prevents
  recovery; old plaintext metadata is not automatically encrypted.
- **xKiro bridge:** fixed upstream destination, loopback and same-origin
  restrictions, bounded response and SSE forwarding have been implemented but
  not yet compiled or tested end to end. See `docs/providers.md`; cancelling in
  the browser does not guarantee remote computation stops.
- **Desktop icon, external links and blob export:** code exists but requires
  interactive Windows verification. HTTPS links still require confirmation;
  exports require a file choice. WebView permissions remain denied.
- **Desktop startup and isolation:** the wrapper blocks arbitrary inherited ISA
  options and does not authenticate against hostile local processes. An
  environment-supplied bearer token can be observed by sufficiently privileged
  local processes. The app is not a sandbox.

## Not enabled or not implemented

- Windows web file edits/undo, model-driven tool loop, MCP stdio and pool MCP
  remain blocked or disconnected: there is no completed Windows integration to
  activate. Staged ACL, file identity, undo and named-pipe components are not a
  complete feature. Do not remove their platform gates merely to make the UI
  claim support.
- Hyper-V and Windows Sandbox execution/import remain designs or staged
  primitives, not working sandboxes.
- Windows CLI process supervision and feedback execution have separate
  `ISA_WINDOWS_PROCESSES_EXPERIMENTAL` and
  `ISA_WINDOWS_FEEDBACK_EXPERIMENTAL` opt-ins for `isa-cli`; the desktop
  installer does not bundle or launch that CLI. See the main README before
  separately enabling either.
- No automatic model, workspace, MCP server, audit key or secrets are invented
  or silently configured. Existing per-chat and separate approval flows remain.

## Suggested manual checks

1. Compile the Rust backend and Windows desktop project; run the Windows CI
   suites and installer smoke test. Verify the icon on the executable, window,
   shortcuts and installer.
2. Start with no workspace/token: verify actions fail closed. Try malformed,
   relative and nonexistent paths and invalid tokens; verify none are forwarded.
3. Configure a disposable workspace and token. Test workspace info and manual
   PowerShell under each chat approval mode, including denial and cancellation.
   Confirm commands run with the expected account and no sandbox is implied.
4. Verify audit-key setup and explicit incomplete-audit warnings on failures;
   inspect storage and recovery with non-sensitive test data only.
5. Configure Ollama and test context consent. Verify MCP, edits and unsupported
   Windows features still fail closed rather than reporting success.
6. Test xKiro non-streaming, SSE, error status, cancellation, Agent Pools, and
   rejection of cross-origin/forwarded requests. Test ordinary providers still
   use their existing transport. Never share an API key or raw network export.
7. Verify HTTPS navigation and blob export, shutdown, occupied port behavior,
   missing WebView runtime and installer/uninstaller behavior interactively.
