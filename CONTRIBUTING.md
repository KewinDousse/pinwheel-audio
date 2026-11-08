# Contributing

Bug reports and pull requests are welcome. For a bug, the issue form asks for the Windows version, the outputs involved and the end of the log (`%LOCALAPPDATA%\Pinwheel\logs\pinwheel.log`).

## Before a pull request

CI runs these on Windows, and they must pass:

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

From Linux, the app crate is checked with `cargo xwin clippy --workspace --all-targets --target x86_64-pc-windows-msvc -- -D warnings` (see the README for the setup).

What the automated tests cannot cover (hooks, audio devices, the shell) is listed in [`docs/manual-tests.md`](docs/manual-tests.md): run the sections your change touches on a real Windows machine, and add a check when you add a behavior.

## Where things go

- Logic that does not need Windows goes in `crates/core`, with tests.
- Every interface text goes in both tables of `crates/core/src/i18n.rs`, English and French.
- A decision that is not obvious from the code gets an ADR in `docs/adr/`: the decision, its reasons and its consequences.
- Code comments explain why, not what.
