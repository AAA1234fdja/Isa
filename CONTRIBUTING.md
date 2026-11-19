# Contributing

Thanks for helping improve ISA. The project is licensed under the
[Apache License 2.0](LICENSE); third-party components may have separate licenses.
Only contribute material you have the right to submit under the project license.

## Development

Use Rust 1.92.0 (also specified in `rust-toolchain.toml`) and Node.js with npm.
From the repository root:

```sh
cargo check --locked --all-targets
cargo test --locked --all-targets
npm ci --ignore-scripts --no-audit --no-fund --prefix tests/mcp-sdk
npm ci --ignore-scripts --no-audit --no-fund --prefix tests/ui
node tests/web-supervision.cjs
```

The CI workflow contains the full test list and platform-specific cases. Some
integration tests require bubblewrap, a local MCP fixture, a Windows host or
other explicit configuration; inspect their prerequisites before running them.
A passing mock or Linux test does not validate Windows security behavior.
`Cargo.lock` pins the dependency resolution; update it intentionally with
`cargo update` and review changes. It does not by itself guarantee identical
build outputs across toolchains, platforms or environments. The pinned CI and
cross-platform checks have not yet been verified for the current changes.
Do not commit secrets, local `.isa` data or generated `node_modules` and
`target` directories.

Keep changes scoped, add tests for behavior and failure paths, and document
platform limitations and security assumptions. In particular, never claim that
an experimental sandbox, audit store or Windows path is validated without
appropriate real-platform testing. See [SECURITY.md](SECURITY.md) for reporting
suspected vulnerabilities.
