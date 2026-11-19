# ISA Windows desktop prototype

This is an **unsigned experimental desktop and installer prototype**, not a
production release. Neither has yet been validated on Windows. It is an
additional desktop entry point; `cargo run` from the project folder remains
the separate live-development server.

## Build on Windows x64

Install Rust 1.92.0, the .NET 8 SDK, and the Microsoft Edge WebView2 Runtime.
From the repository root:

```powershell
cargo build --locked --release --bin isa
if ($LASTEXITCODE -ne 0) { throw 'Backend build failed' }
dotnet publish desktop/windows/Isa.Desktop.csproj -c Release -r win-x64 --self-contained true -o desktop-prototype
if ($LASTEXITCODE -ne 0) { throw 'Desktop build failed' }
Copy-Item target/release/isa.exe desktop-prototype/isa.exe
.\desktop-prototype\Isa.Desktop.exe
```

The desktop executable needs its published files and the matching `isa.exe` in
the same folder. WebView2 Runtime is a separate prerequisite; neither the app
nor installer downloads it automatically. If the runtime is missing, install
Microsoft Edge WebView2 Runtime separately and restart the app.

## Build the prototype installer

Install Inno Setup 6 on Windows and build the published folder as above. Copy
`LICENSE` and this README into `desktop-prototype`, then compile the installer:

```powershell
Copy-Item LICENSE desktop-prototype/LICENSE
Copy-Item desktop/windows/README.md desktop-prototype/README.md
& "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe" desktop/windows/installer.iss
if ($LASTEXITCODE -ne 0) { throw 'Installer build failed' }
```

The resulting `desktop-installer/ISA-Desktop-Prototype-Setup.exe` is unsigned
and installs for the current Windows user without administrator access. It
creates a Start menu shortcut, offers an optional desktop shortcut, and has an
uninstaller. It does not start the app automatically. Uninstall retains the
WebView profile and its local data; remove that data separately if desired.
There is no automatic update mechanism. Do not distribute it as a production
release.

CI is configured to build the portable folder and installer, then smoke-test
installation, missing-backend and occupied-port failures, WebView startup,
frontend mounting, backend shutdown, uninstall, and profile retention. The
smoke mode uses a temporary WebView profile and closes automatically. Its
artifacts are short-lived and unsigned. These checks have not yet run on
Windows for this revision and do not replace interactive use, broader
compatibility testing, or a security review.

## Prototype behavior and limitations

- The wrapper starts its own bundled backend at `127.0.0.1:4175` in embedded
  asset mode. It checks the backend's startup message and refuses to connect if
  the port is already occupied. It does not reuse the development server.
- All inherited `ISA_` environment settings are removed. Workspace access,
  experimental shell and local-model server configuration are therefore not
  enabled by this wrapper; a future reviewed configuration flow is required.
  Custom providers configured directly in the UI remain separate.
- Navigation and frames are limited to that local origin, new windows and
  downloads are blocked, and WebView permission requests are denied. Links to
  external sites, chat export, microphone/voice input and any other functionality
  depending on those features are consequently unavailable in this prototype.
  No privileged native object or messaging bridge is exposed to the page.
- WebView data is stored separately under `%LOCALAPPDATA%\ISA\DesktopPrototype`.
  Browser chats are not imported. The fixed origin helps retain WebView storage
  between launches but local data is not encrypted by the wrapper. A separate
  development browser uses its own storage.
- Closing the window attempts to terminate the backend and its descendants.
  Abnormal termination can leave a backend running; stop it before relaunching.
  Termination does not undo command effects. The fixed port, loopback connection
  and startup message are not a secure authenticated channel against malicious
  local processes. Do not use this prototype for sensitive workloads or expose
  the local server to untrusted networks.
- Running and testing the desktop app and installer on Windows, dependency and
  security review, handling backend crashes and upgrades, migration, signing
  and production-ready packaging are still required. Existing Windows capability
  restrictions remain in place.
