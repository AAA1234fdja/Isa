# Windows desktop application: staged plan

ISA runs as a local Rust server and browser interface. An experimental Windows
window and installer prototype are available under `desktop/windows/`; neither
has yet been validated on Windows. The desktop package is intended to be an
additional way to use ISA, not a replacement for `cargo run` from a source
checkout. The existing development mode must continue serving live UI files.

## Foundation in this change

`ISA_ASSET_MODE=embedded` selects UI assets compiled into the Rust binary
instead of reading UI files relative to the process working directory. The
normal default remains `live`, with embedded fallbacks. This switch does not
restrict API access, select a workspace, authenticate requests, or create a
window. A dynamically selected port can be requested with
`ISA_ADDRESS=127.0.0.1:0`, and the actual address is logged after binding.
The log is not an authenticated desktop handshake. Different ports are
separate browser origins, so local storage continuity needs a deliberate
solution before desktop packaging.

## Remaining work

1. Prototype a Windows native window and evaluate the WebView integration,
   backend lifecycle, local origin and safe navigation. Preserve same-origin
   controls and explicitly prevent untrusted pages from receiving privileged
   access. Do not assume loopback, embedded assets or a WebView are a sandbox.
2. Design desktop-to-backend authentication, startup and shutdown, including
   port ownership, multiple instances, abnormal exits, network requests and
   user-visible errors. Inherited environment variables must not silently
   enable experimental actions. Do not put secrets in URLs or logs.
3. Choose stable application storage, credential handling and an explicit
   migration/export story for existing browser data. A folder chosen in the
   interface is not automatically a server workspace.
4. Package and install the app, its assets and dependencies; add uninstall and
   upgrade behavior, release builds, versioning and signing decisions. Define
   whether a WebView runtime must be installed separately or provisioned.
5. Test installation, startup, offline behavior, updates and removal on real
   Windows systems. Test development mode independently on every supported
   platform. Keep unimplemented Windows capabilities fail-closed.

The prototype's current build and installer instructions and limitations are
documented in `desktop/windows/README.md`. No validated installer, security
review, Windows desktop execution, or production-readiness claim is implied.
