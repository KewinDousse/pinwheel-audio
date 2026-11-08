# Pinwheel

A small Windows 10/11 utility that lives in the notification area:

- **Win + mouse wheel** changes the volume of the default audio output, 10% per notch (adjustable).
- **One shortcut per audio output** makes it the default output. Any key or key combination works, media and macro keys included.
- **An on-screen display** shows the volume or the new output for 1 s, then fades out. Three styles: thin bar, pill, big number.

The interface is in English or French. It follows the Windows display language, and can be changed in the settings.

## Install

1. Download `pinwheel.exe` from the [releases](https://github.com/KewinDousse/pinwheel-audio/releases) and put it in a local folder, such as `%LOCALAPPDATA%\Programs\Pinwheel\`. Windows often blocks executables started from a network share.
2. Run it. The settings window opens on the first run.
3. Tick "Start Pinwheel when I sign in" to start it automatically.

The executable is not code-signed, so SmartScreen may warn about it on the first run: click "More info", then "Run anyway". Each release comes with a `pinwheel.exe.sha256` file to check the download.

If you use Volume2, quit it and remove it from startup: with both running, Win + wheel would change the volume twice.

## Use

- Click the tray icon to open the settings. Right-click for the menu (Settings, Quit).
- Running `pinwheel.exe` again while it runs also opens the settings.
- Changes are saved and applied right away.

## Files

| File | Content |
|:--|:--|
| `%APPDATA%\Pinwheel\settings.json` | Settings |
| `%LOCALAPPDATA%\Pinwheel\logs\pinwheel.log` | Log ("Open the log folder" button) |
| `%LOCALAPPDATA%\Pinwheel\status.json` | Shortcuts Windows refused, read by the settings window |

An unreadable settings file is renamed to `settings.json.<timestamp>.bak` instead of being overwritten.

## Development

Rust (edition 2024). The repository holds:

- `crates/core`: logic that does not depend on Windows (settings, shortcuts, interface texts, volume steps, wheel, output matching, drawing of the display). It builds and is tested on any OS.
- `crates/app`: the Windows app.
- `tools/make-icon`: generates `crates/app/res/pinwheel.ico`.

See [`docs/architecture.md`](docs/architecture.md), the decisions in [`docs/adr/`](docs/adr/), and [`CONTRIBUTING.md`](CONTRIBUTING.md).

### Tests

```sh
cargo test                     # crates/core
cargo run -p pinwheel-core --example osd-preview -- preview.png <font.ttf> [bold-font.ttf]
```

The `osd-preview` example draws every display style, in both languages, into one image. Tests on Windows follow [`docs/manual-tests.md`](docs/manual-tests.md).

### Build the Windows executable

On Windows: `cargo build -p pinwheel --release`.

From Linux, with [`cargo-xwin`](https://github.com/rust-cross/cargo-xwin) (which downloads the MSVC CRT and the Windows SDK, and accepts their license) and LLVM (`clang-cl`, `lld-link`, `llvm-rc`):

```sh
rustup target add x86_64-pc-windows-msvc
cargo install cargo-xwin --locked
scripts/build-windows.sh       # → target/x86_64-pc-windows-msvc/release/pinwheel.exe
```

`crates/app/build.rs` embeds the icon, the version information and the manifest (per-monitor DPI, no elevation) into the executable.

## License

[MIT](LICENSE)
