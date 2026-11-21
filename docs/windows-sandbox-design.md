# Windows sandbox design (proposal, not implemented)

## Status and decision

ISA does **not** currently provide a Windows sandbox. The experimental Windows
PowerShell runner uses the user's privileges and a Job Object; it is not a
security boundary. `ISA_WEB_EXEC_PROFILE=sandboxed` and `limited` must continue
to fail closed on Windows. None of the controls below has been implemented or
validated. Do not advertise equivalence with Linux bubblewrap.

The recommended target for stronger isolation is an **ephemeral,
hypervisor-isolated Windows guest**, managed by a trusted host-side supervisor.
A regular Windows process container is not an acceptable substitute for a
hostile-code boundary: Microsoft distinguishes process isolation from
hypervisor-isolated containers for this purpose [1]. A VM reduces shared-kernel
exposure but does not make the exposed workspace, network or broker safe, and
is not a guarantee against hypervisor or host compromise.

The chosen direction is a guest workspace copy followed by a separately approved
import, with network access including host and LAN, and investigation of both a
Hyper-V VM and Windows Sandbox. These are targets, not implemented capabilities.
Host and LAN access increases the risk to local services and allows exfiltration
of guest-readable data. Each backend must independently demonstrate the requested
network behavior and isolation before it can be enabled; never silently substitute
one backend for the other. Supported hosts and deployment details remain open.

## Goals and scope

- Run shell commands, MCP servers and calls in an isolated guest with a
  writable copy of the configured workspace and network access. Honor the chat's
  existing always/risk/free supervision policy, obtaining one-use approvals
  where that policy requires them; the sandbox does not replace supervision.
  A host import is a separate operation requiring review and approval.
- Prevent direct guest access to unrelated host files, host credentials, host
  process handles and ISA's token, keyring, `.isa` audit and undo data, subject
  to the threat model and validated boundary. Secret values already present in
  the exported workspace can still be read and exfiltrated.
- Bound output, operation duration and host/guest resource consumption. Support
  an explicit stop request and report uncertain outcomes without automatic
  retries, except an independently approved retry of an MCP tool explicitly
  declared idempotent; that declaration does not guarantee safety.
- Keep normal Unix behavior unchanged. A Windows opt-in is not permission to
  weaken or silently switch the requested profile.

**Out of scope:** protection against a compromised host, administrator or
hypervisor, microarchitectural attacks, malicious network peers, deliberate
exfiltration of accessible workspace data, rollback of external side effects,
and proof that arbitrary tool output or generated code is safe. The guest and
its filesystem/network interfaces are hostile inputs to the host broker.

## Comparison and candidate backends

| Candidate | Strengths | Problems / decision |
| --- | --- | --- |
| Job Object and reduced environment | Useful for process/resource lifecycle | Not a filesystem, credential or network isolation boundary; reject as a sandbox. |
| AppContainer / LPAC | OS-enforced access checks, capabilities and reduced access to resources [2] | Shares host kernel; compatibility and precise access to a mutable workspace, descendants, networking and IPC require a separate threat model. Possible future lightweight profile, **not** a VM-equivalent claim. |
| Windows Sandbox | Disposable hypervisor-backed desktop; configurable mapped folders and network [3] | Writable host mappings persist changes and can expose host data; network may expose internal services; default clipboard and other redirections need explicit policy. Automation, per-action lifecycle and effective-policy attestation remain to be proven. Prototype only. |
| Hypervisor-isolated container or dedicated VM | Separate guest kernel and potentially automatable guest lifecycle [1] | Requires supported Windows edition, virtualization, runtime/image provisioning and patching; bind mounts, broker and network remain critical attack surfaces. Preferred research path, not implemented. |

Microsoft documents experimental `Experimental_CreateProcessInSandbox` APIs;
they are subject to change and currently document `inheritHandles` as
unsupported [4]. Investigate availability and stdio constraints separately;
do not assume they can replace ISA's current handle-list runner or that their
presence proves the policy is enforced.

## Proposed architecture

1. **Trusted host supervisor** authenticates loopback requests, checks workspace
   identity and chat policy, and obtains any approval required by that policy
   before starting the guest or making an MCP call. It owns credential-store
   keys and persistent audit/undo; never pass them to the guest.
2. **Guest lifecycle manager**, in a narrowly scoped service/process, verifies
   prerequisites, requested isolation mode and a pinned, patched guest image;
   creates an ephemeral instance; applies resource and network policy; waits for
   a readiness handshake; then dispatches the approved command. If any step
   fails, report unavailable and execute nothing on the host. Do not use the
   existing experimental PowerShell runner as a fallback.
3. **Authenticated per-instance broker** mediates control and bounded stdout,
   stderr and MCP stdio. It exposes a minimal message protocol with frame and
   total byte limits, deadline, per-instance nonce, version and strict state
   transitions. Use OS-authenticated endpoints plus per-instance credentials;
   a secret embedded inside the guest is not proof that guest code is trusted.
   Never expose the host control socket or execution token to the guest. The
   broker treats every guest message and path as untrusted.
4. **Execution boundary:** invoke a trusted guest-side launcher with independent
   program/argument fields or a separately labeled PowerShell script. Approval
   displays the exact command, backend, workspace sharing mode, network policy,
   limits and effect scope. No privileged guest daemon accepts arbitrary guest
   requests to alter host policy. Child processes remain inside the guest;
   sandbox teardown is the final containment action, not proof of rollback.
5. **Completion:** drain bounded output, distinguish exit, timeout, requested
   cancellation and unknown results. Stop requests guest termination and then
   destroys the guest within a bounded host deadline; if that cannot be verified,
   mark outcome unknown and alert. Do not claim descendants were killed or
   side effects reverted solely because a stop was requested.

Backend-specific APIs, image identity, authenticated transport, administrative
privileges and supported host versions are **work still to be verified**. A
manual Hyper-V lifecycle prototype is described below; it is not integrated
with ISA and there is no guest agent implementation.

### Backend investigation milestones

- **Windows Sandbox:** establish a supported way to launch, identify, stop and
  correlate a specific ephemeral instance from ISA, including when the UI is
  closed or the host crashes. Verify requested networking and redirection policy
  on each supported version and under managed-device policy. The documented
  `.wsb` settings are requests, not evidence of effective runtime isolation.
  The documented logon account should be an administrator [3], so guest-side
  code cannot be treated as a trusted authority after untrusted commands run.
  Do not map the host workspace as a shortcut; no suitable copy/return channel
  has been demonstrated yet.
- **Hyper-V VM:** validate provisioning a pinned guest image and per-instance
  disposable disk, authenticated broker endpoint and teardown by an immutable
  VM identifier. Microsoft documents administrative or Hyper-V Administrators
  membership for VM creation [6]; switch creation may require administrative
  privileges [7]. Do not mutate or delete existing switches, NATs, virtual
  machines or disks to make the environment work. Confirm DNS and host/LAN
  reachability on the selected switch without assuming guest loopback is host
  loopback. Verify image licensing and updates before distributing a guest.
- For both backends, do not mount a hostile guest-generated disk image directly
  on the host as an import shortcut. A bounded, authenticated transport and
  untrusted-data parser are still needed; peer authentication identifies an
  instance, not trustworthy file contents. Instance discovery by process name
  or timing alone is not sufficient to authorize cleanup or message exchange.

## Workspace model and host data

The selected workspace model is a guest copy and separately approved import.
For context, the alternative below has materially different behavior and must
not be presented as equivalent:

- **Writable host share:** changes are visible in the configured workspace
  during execution, closest to Linux's existing bind mount. A writable mapped
  folder can be modified by guest code and changes persist after guest disposal
  [3]. It can expose `.isa`, secrets, links and host filesystem behavior unless
  a separately verified sharing/broker policy enforces exclusions. A normal
  whole-workspace mapping is unacceptable if `.isa` must remain private.
- **Guest copy and controlled import:** the guest writes its own workspace and
  ISA presents a bounded change set for a separate approved host-side apply.
  No live host writes during execution; this changes semantics and requires
  explicit user agreement. Import must reject unsafe paths, reparse points,
  hard links, alternate data streams, short-name and case aliases, protected
  `.isa` and `.git` targets as appropriate, and revalidate file identity and
  contents at apply. Reject conflicts and oversized/nonregular objects. Never
  imply a multi-file import is atomic unless it really is.

For either path, preserve arbitrary data and executable bits/ACL semantics only
where explicitly specified and tested; do not blindly extract archives. Check
NTFS junctions, symlinks, reparse points, hard links, ADS, reserved device
names, UNC/device paths, short names, case-folding and trailing-dot aliases.
Handle-based checks and atomic operations must be chosen per filesystem; a
check-then-open path alone cannot prevent replacement races. A workspace that
is a drive root, a protected system location, a network share, or cannot be
safely identified must be rejected until deliberately supported. Define crash
recovery and cleanup without deleting another process's replacement by path.
Persistent undo remains plaintext in protected host `.isa` by the existing
user decision; it must not be exposed inside the guest.

## Network and resource policy

Network access is part of the requested sandbox behavior. It also permits
exfiltration of guest-readable data. Host-network equivalence is **not** a
safe default: loopback, LAN, cloud metadata endpoints, VPN, DNS and proxy access
can expose host or internal services. Define and test a default guest outbound
policy, including DNS, IPv4/IPv6 and hostname-to-address changes; display the
host/LAN scope during setup and any review required by supervision policy.
A guest loopback address is not
the host's loopback address. If network is required and cannot be enforced as
requested, reject the launch rather than silently disabling or broadening it.
No exposed host service should trust a request merely because it originates
from the VM network. Firewall/WFP integration, policy privileges and OS-version
support remain unproven; do not claim enforcement until measured.

Set explicit per-instance and aggregate limits for CPU, memory, disk, output,
processes, guest count, duration and concurrent actions, with cleanup after
host crashes. Apply limits before guest code can run and verify actual backend
behavior. Do not change the existing approval semantics or the 12-action plan
limit. Resource limits reduce risk but cannot prevent every denial of service.

## Audit, credentials and failure handling

- Keep credentials and audit keys in the host OS credential store, outside the
  guest; use existing encrypted audit interfaces for permitted arguments,
  results and provider-reported token counts. Redaction is heuristic and is
  not a guarantee that free text contains no secrets.
- If key access, encryption or storage fails, continue the approved action
  where policy permits, show the existing degraded-audit warning, and never
  write sensitive details to plaintext as a fallback. Metadata and logs may be
  incomplete; no tamper-proof or globally complete history is promised.
- Treat timeout, broken transport, VM crash, incomplete output and cancellation
  after dispatch as **unknown** when effects cannot be established. The host
  must not automatically replay shell commands or non-idempotent MCP calls.
- Startup, attach, network configuration, sharing, authentication, limit or
  cleanup failures must fail closed. Profile unavailable means no execution;
  explicit host-mode execution requires a fresh, clearly labeled user choice
  and separate approval, never an automatic fallback.

## Validation and release gates

All tests below require real supported Windows machines/VM hosts, in addition
to portable unit tests. Add CI jobs only after documenting prerequisites; do
not infer Windows runtime success from Linux mocks, AST parsing or compilation.

1. Capability probes, unsupported Windows editions, virtualization disabled,
   absent images, nested virtualization and policy/privilege failures: no host
   process dispatch, no silent fallback, clear unavailability.
2. Actual identity and escape tests: inaccessible unrelated host directories,
   registry, credentials, named pipes, privileged services and ISA token;
   inherited handles and ACLs; cross-guest access and resource cleanup. Include
   malicious paths, aliases, reparse points and concurrent replacement races.
3. Workspace tests for the chosen mode: writes, deletes, conflicts, `.isa`
   exclusion, hard links/junctions/ADS, long and Unicode paths, crash during
   import or share cleanup, and multiple concurrent actions.
4. Network tests for DNS, internet, host loopback, LAN, IPv4/IPv6, proxies and
   disallowed destinations. Verify effective rules on the supported OS/build;
   test fail-closed behavior if enforcement or network creation fails.
5. Real command and MCP fixtures: output flooding, malformed/oversized frames,
   stalled stdin/stdout, child and grandchild processes, cancellation at every
   dispatch stage, deadlines, VM crash, cleanup, and no duplicate tool calls.
6. End-to-end HTTP/UI: per-chat modes and approvals, no auto-fallback, readable
   profile and warnings, no token exposure, encrypted audit and key loss,
   degraded-audit behavior and restart recovery. Test real Windows Credential
   Manager and audit storage, not only mocks.
7. Review with Windows security expertise and independent adversarial testing;
   publish supported OS/runtime matrix, known gaps and recorded test evidence.

Roll out in stages: documentation and threat-model review; isolated prototype
behind a non-default development flag; Windows integration and adversarial
validation; user-facing opt-in only after release gates pass. Keep a kill switch
and fail closed. No rollout date or isolation percentage is promised.

## Implementation staging and open decisions

Staged components `src/cli/windows_sandbox_manifest.rs`,
`src/cli/windows_sandbox_import_plan.rs` and
`src/cli/windows_sandbox_export_plan.rs` provide a lexical path filter and
pure structural checks for proposed file changes and export inventories, with
portable unit tests. The import plan uses provisional limits of 1 MiB per file
and 4 MiB per plan and compares caller-supplied file identity, content digest
and size snapshots; it does not obtain identities or calculate digests. Identity
plus digest detects some same-content replacements but is not proof of handle
continuity: IDs may be reused and parent/root replacement still matters. The
export inventory rejects unsupported objects, protected
paths and missing directory parents, and uses provisional limits of 16 MiB per
file and 128 MiB total. Those limits are design defaults, not supported product
limits. The export traversal must exclude `.isa` and `.git`; encountering either
in an inventory rejects it.

`src/cli/windows_sandbox_frame.rs` also stages a length-prefixed frame encoder
and incremental decoder with provisional per-frame and per-stream limits and
rejection of truncated or invalid messages. The format is a prototype, not a
stable protocol. It does not provide authentication, transport, partial-write
handling, message validation or deadlines.

Two additional staged modules are `src/cli/windows_sandbox_lifecycle.rs`, a
pure state machine for approval, provisioning, execution and confirmed cleanup,
and `src/cli/windows_sandbox_protocol.rs`, a bounded parser for selected
versioned guest messages. An unconfirmed teardown or interruption after
execution starts is reported as an unknown outcome. Instance IDs and parsed
messages do not authenticate a guest or prove isolation; host-side verification
and a secure transport remain necessary. These modules and their tests have not
been compiled or run in the current environment.

These components are compiled as test-only CLI modules and are included in the
proposed Windows CI test list; they are **not connected to any execution, export
or import path**. `src/cli/windows_sandbox_wsb.rs` provides only a static,
test-only diagnostic `.wsb` template with network requested, redirections and
vGPU disabled, protected client requested, and no mappings or logon command [3].
It neither writes nor launches a configuration. Requested settings are not
runtime attestation, and this diagnostic template cannot execute ISA actions or
provide the workspace copy. The lexical filter rejects documented reserved
COM/LPT names including superscript digits [5], but is not a complete Windows
name resolver.
They do not touch the filesystem, establish trusted snapshots or object types,
handle non-ASCII filesystem aliases, or prevent reparse-point and replacement
races. They cannot authorize or safely perform a copy or import on their own.
Safe handle-based traversal, copying and rechecking, separately approved import,
filesystem conflict handling, undo integration and both integrated guest
backends remain unimplemented.

`desktop/windows/check-sandbox.ps1` is a read-only Windows prerequisite
report. It queries OS, virtualization, optional feature and Hyper-V switch
availability without creating or changing a VM, switch or guest. Unknown checks
are not failures or proof of availability, and positive indicators do not prove
that ISA's sandbox works. Run it on a Windows test machine with
`powershell -NoProfile -File desktop/windows/check-sandbox.ps1`. Its Windows CI
smoke check is configured but has not yet run for these changes.

Next, validate each backend's real prerequisites and automation, and design
handle-based export/import and identity checks before wiring in any execution.
The selected network scope includes host and LAN, but guest-to-host reachability
may depend on the backend and host configuration and must be explicitly tested.
Windows Sandbox automation and reliable per-action cleanup remain open research
questions; there is no promise that it can satisfy the release gates.

Open decisions:

1. Which Windows editions, machines and deployment permissions must be
   supported? A VM backend may be unavailable on some systems.
2. Which guest image/runtime, update strategy, package management and MCP
   dependencies will be supported, and who maintains them?
3. What exact change-review UI, conflict policy and size budgets are appropriate
   for a separately approved import?

## References

1. [Microsoft: Secure Windows containers](https://learn.microsoft.com/en-us/virtualization/windowscontainers/manage-containers/container-security).
2. [Microsoft: Launch an AppContainer](https://learn.microsoft.com/en-us/windows/win32/secauthz/implementing-an-appcontainer).
3. [Microsoft: Use and configure Windows Sandbox](https://learn.microsoft.com/en-us/windows/security/application-security/application-isolation/windows-sandbox/windows-sandbox-configure-using-wsb-file).
4. [Microsoft: Create Process in Sandbox (experimental)](https://learn.microsoft.com/en-us/windows/win32/secauthz/createprocessinsandbox).
5. [Microsoft: Naming files, paths, and namespaces](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file).
6. [Microsoft: Create a virtual machine in Hyper-V](https://learn.microsoft.com/en-us/windows-server/virtualization/hyper-v/get-started/create-a-virtual-machine-in-hyper-v).
7. [Microsoft: Create a Hyper-V virtual switch](https://learn.microsoft.com/en-us/windows-server/virtualization/hyper-v/get-started/create-a-virtual-switch-for-hyper-v-virtual-machines).

## Manual Hyper-V lifecycle prototype (not integrated)

`desktop/windows/hyperv-lifecycle.ps1` is a standalone, **untested** manual
prototype for creating and removing one Hyper-V VM; it is not the ISA sandbox
backend. It requires a trusted Windows test host, the Hyper-V PowerShell module,
appropriate administrator/Hyper-V permissions, a compatible licensed Generation
2 VHDX image, and an existing local directory controlled by the user. No guest
agent, identity-attested channel, workspace export/import, command dispatch,
resource enforcement or ISA approval integration is implemented. Starting a
VM does not establish that isolation or networking works as requested.

The `Create` operation checks a caller-supplied SHA-256 of the base VHDX,
creates a per-instance directory and differencing disk, creates a VM with a
random instance name and marker, and asks for confirmation before starting.
If an existing switch ID is explicitly supplied, it attaches that switch;
otherwise it uses no switch. An attached switch can expose host, LAN and
internet services; its effective reachability is not verified. It never creates
or changes a switch and does not attach the ISA workspace. Hash/path/reparse
checks are best-effort and do not pin a file handle or protect against local
races, image modification, aliases, malicious administrators, or untrusted local
processes. The base image and instance root must remain trusted and protected.

The `Destroy` operation requires both returned IDs, compares the exact VM ID,
name and marker, asks for confirmation, turns off that VM and removes it. The
marker guards against accidental mistakes, not malicious local impersonation.
Removal is not independently verified. **It deliberately leaves all disks and
folders behind**: they may contain guest data, occupy space, and require manual
inspection and cleanup by the user. If provisioning fails or the host crashes,
VMs and files may remain. Inspect Hyper-V Manager and the reported folder;
never bulk-delete based on a name prefix. No existing VM, switch or unrelated
disk should be altered. These protections have not been validated in Windows.

Example for a disposable test machine, using a VHDX and expected digest obtained
through a trusted channel (replace the example values). Run PowerShell with the
permissions needed by Hyper-V. Both operations request confirmation by default.

```powershell
$vm = & .\desktop\windows\hyperv-lifecycle.ps1 -Action Create `
    -BaseVhdx 'C:\ISA-Test\trusted-base.vhdx' `
    -BaseSha256 '<64-character trusted SHA-256>' `
    -InstanceRoot 'C:\ISA-Test\instances'
$vm | Format-List
# Keep both IDs. Review them before this destructive operation:
& .\desktop\windows\hyperv-lifecycle.ps1 -Action Destroy `
    -VmId $vm.VmId -InstanceId $vm.InstanceId
```

For networking, separately inspect `Get-VMSwitch` and pass `-SwitchId` with
the GUID of a specific existing switch during `Create`. Choosing a switch does
not ensure host/LAN/internet reachability and may expose internal services.
`-WhatIf` avoids the VM mutation steps; creation validation still reads the
base image and computes its hash. Do not pass API keys or workspaces to the VM.

The mock-only `desktop/windows/test-hyperv-lifecycle.ps1` tests identity checks,
rejection of a foreign VM, `-WhatIf`, and the teardown command sequence. CI is
configured to run these tests, but this checkout has not been tested in
PowerShell, against Hyper-V, or on Windows. It does not test actual VM creation,
resource limits, crashes, network, disk cleanup or adversarial path races. Those
checks and a design review are prerequisites to integrating the helper with ISA.

### PowerShell Direct diagnostic and transfer receiver

`desktop/windows/hyperv-channel-check.ps1` is an **untested standalone manual
connection diagnostic** for a running ISA-labeled Hyper-V VM. It checks the
instance marker, asks for confirmation and prompts locally for credentials of a
dedicated disposable guest account, then opens a PowerShell Direct session by
VM ID, runs one fixed diagnostic expression, and closes the session. It does
not save credentials, copy files or connect the session to ISA. Use Windows
PowerShell 5.1 on the host; the guest and host must support PowerShell Direct.
See [Microsoft's PowerShell Direct documentation](https://learn.microsoft.com/en-us/windows-server/virtualization/hyper-v/powershell-direct).

```powershell
powershell -NoProfile -File .\desktop\windows\hyperv-channel-check.ps1 `
    -VmId <vm-guid> -InstanceId <instance-guid>
```

Replace the two placeholders with the IDs produced by the lifecycle prototype.
Do not use a host or domain credential: a guest may be compromised. A successful
session authenticates an account to the guest; the VM marker and the guest's
reply do not attest guest integrity, authenticate arbitrary guest output, or
prove isolation. There is no bounded operation deadline, cleanup guarantee on
crash, or end-to-end validation yet. `test-hyperv-channel.ps1` supplies mock-only
checks for identity rejection, `-WhatIf` and session closure, with a Windows CI
step configured; those checks have not run for these changes. Never use this
diagnostic as a production channel or as authority to import files.

`src/cli/windows_sandbox_transfer.rs` adds a staged in-memory receiver for an
inventory and file chunks, with lexical path and resource limits and length and
digest checks. `src/cli/windows_sandbox_changes.rs` converts a completed
received inventory and a caller-supplied host baseline into a bounded,
deterministically ordered write/delete proposal and review digest; it can also
feed the existing conflict checker. Guest omissions become proposed deletions,
not automatic deletions. Directory operations are not represented. Tests cover
protected paths, duplicates, truncation, corrupted content, baseline conflicts,
case mismatches and review-digest changes. These components are not wired to
PowerShell Direct or the VM, are compiled only in tests and have not been run in
this environment. Digests supplied by an untrusted guest only detect transfer
errors; they do not establish trust.
`src/cli/windows_sandbox_wire.rs` connects framed, bounded, versioned transfer
messages to the in-memory receiver in tests; the instance identifier is not
peer authentication. The current frame-stream budget is 4 MiB, including JSON
and base64 overhead. This is a protocol exercise, not a VM connection.

PowerShell Direct's documented `New-PSSession -VMId` parameter set does not
include `-SessionOption`; the normal `MaximumReceivedObjectSize` and
`MaximumReceivedDataSizePerCommand` settings cannot simply be passed there.
We have not established that incoming guest serialization, operation deadlines,
and cancellation can be bounded for this transport. Until those properties
are demonstrated or a different authenticated bounded transport is used, no
guest workspace data is accepted by the runtime.
Successful reception is not approval to write files. A real copy/import still
requires handle-based source and destination validation, secure credential and
session lifecycle, separate review and approval, conflict rechecking, undo,
cleanup, and Windows adversarial testing. The runtime `sandboxed` profile
remains unavailable on Windows and must not fall back to host execution.
