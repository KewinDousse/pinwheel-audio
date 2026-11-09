# ADR 0001: Rust, windows-rs and egui

## Decision

The app is written in Rust, with `windows-rs` for the Windows APIs, `egui`/`eframe` for the settings window, and `tiny-skia` and `ab_glyph` to draw the on-screen display. It ships as a single native executable, with no runtime to install, and builds from Linux (`cargo-xwin`) as well as from Windows.

## Reasons

- The app runs all the time and must start reliably at sign-in. A native executable starts instantly, depends on no runtime (statically linked CRT, see `.cargo/config.toml`) and extracts nothing into `%TEMP%`.
- The delicate parts (mouse hook, `RegisterHotKey`, Core Audio, `IPolicyConfig`) are direct Win32 and COM calls, as simple in Rust as elsewhere.
- The logic and the drawing of the display, kept in `crates/core`, are tested on any OS.

## Consequences

- The settings window does not look native (egui).
- The executable weighs about 13 MB, mostly because of egui's renderer (wgpu). The background process does not use it.
- The display is drawn by hand rather than described in a UI language.
- The settings window runs in a separate process, because egui (through winit) wants to own the main thread's message loop, which the background process already uses.
