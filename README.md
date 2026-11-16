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
