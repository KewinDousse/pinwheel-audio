# Architecture

## Two processes, one executable

| Mode | Started as | Role |
|:--|:--|:--|
| Background | `pinwheel.exe` | Tray icon, wheel hook, shortcuts, display. Single instance (mutex `Local\Pinwheel.Main`). |
| Settings | `pinwheel.exe --settings` | egui window. Single instance (mutex `Local\Pinwheel.Settings`). |

The settings window is a process of its own: the UI toolkit (egui, wgpu) is only loaded while it is open, and the background process stays light.

Communication:

- Settings → background: messages posted to the hidden window of class `Pinwheel.Main` (`crates/app/src/ipc.rs`). `WM_RELOAD` after each save, `WM_PREVIEW_OSD` for the preview.
- Background → settings: `status.json` (refused shortcuts), and `settings.json` when an output got a new ID. The window reads both again every second.

## Background process

All the state is in `App` (`crates/app/src/app.rs`), on the main thread, which owns a hidden top-level window. That window receives:

| Message | From | Effect |
|:--|:--|:--|
| `WM_VOLUME_NOTCHES` | hook thread | Changes the volume, shows the display |
| `WM_HOTKEY` | `RegisterHotKey` | Changes the default output |
| `WM_TRAY`, `TaskbarCreated` | Explorer | Menu, settings, icon added again |
| `WM_POWERBROADCAST` (resume), `WM_WTSSESSION_CHANGE` (unlock) | Windows | Hook reinstalled |
| `WM_TIMER` | timers | Display fade, retry of refused shortcuts and of the icon |

The mouse hook runs on its own thread (`hook.rs`), see [ADR 0003](adr/0003-win-wheel-hook.md).

Low-level hooks need a message loop on their thread. `thread.rs` provides that thread (`MessageThread`): it lives until its owner drops it, and only ends after `WM_QUIT`, so that no message posted later reaches another thread that reused its ID.

### Path of a wheel notch

1. The hook sees `WM_MOUSEWHEEL` with Win alone held.
2. It adds up the delta (`WheelAccumulator`) and sends the `vkE8` key so that Start does not open.
3. It swallows the event and posts the number of whole notches to the main window.
4. `App` asks for the default multimedia output, computes the new level (`stepped_level`), applies it, and unmutes when the volume goes up.
5. The display draws the level (`core::osd::render`) and shows it on the cursor's monitor.

### Path of a shortcut

1. `WM_HOTKEY` gives the index of the output in the settings.
2. `resolve` finds the output among those of Windows, see [ADR 0004](adr/0004-output-identity.md).
3. `IPolicyConfig::SetDefaultEndpoint` makes it the default output, see [ADR 0002](adr/0002-ipolicyconfig.md).
4. The default output is read back to check the switch, then shown.

## Shortcut capture

In the settings window, "Set…" starts `keys::Capture`, which installs a low-level keyboard hook on a `MessageThread`. While the capture lasts, the hook swallows every key: neither the shell nor the background process sees them, so Win+E does not open the Explorer and existing shortcuts do not fire. Any key, Win combinations included, can be chosen this way.

- The combination is complete at the first press of a key that is not a modifier. Ctrl, Alt, Shift and Win are tracked on the left and right side (`HeldModifiers`), so that releasing one does not forget the other.
- Escape alone cancels. The capture also ends by itself after 15 s, or as soon as another app takes the focus: the hook is global, and a key typed elsewhere must not be stolen.
- Injected keys are accepted, since macro software (G Hub, Synapse, AutoHotkey) sends its keys that way. Only `vkE8`, sent by the wheel hook, is ignored.
- `Hotkey::validate` refuses a combination with no main key, and F12, which Windows reserves. `Hotkey::warnings` flags those that work but get in the way: Win, or a combination without Ctrl, Alt or Win, whose key stops working in other apps (F13 to F24 excepted).

## Interface language

Texts come from `crates/core/src/i18n.rs`, in the language of the settings or else of Windows (`crates/app/src/language.rs`), see [ADR 0005](adr/0005-interface-language.md). The background process reads it for each display and each tray menu, so a change in the settings applies at once.

## Robustness

- **Audio**: only the Core Audio enumerator is kept. Each action asks for the default output again, and an error recreates the enumerator before a second try. An audio service that is slow at sign-in or restarted blocks nothing.
- **Icon**: retried every 2 s while Explorer is not ready, added again on `TaskbarCreated`.
- **Shortcuts**: a refused shortcut stays in the settings and is retried every 30 s.
- **Settings**: atomic write, unreadable file kept aside, a failed read keeps the current settings, never rewritten at startup once it exists. The settings window saves a dragged slider once, when it is released.
- **Log**: `%LOCALAPPDATA%\Pinwheel\logs\pinwheel.log`, panics included.

## Display

`crates/core/src/osd.rs` draws into a premultiplied RGBA image (tiny-skia for shapes, ab_glyph for text, with the Windows Segoe UI fonts). `crates/app/src/osd.rs` copies it into a `WS_EX_LAYERED` window set by `UpdateLayeredWindow`. The window never takes the focus (`WS_EX_NOACTIVATE`), lets clicks through (`WS_EX_TRANSPARENT`) and does not appear in Alt+Tab (`WS_EX_TOOLWINDOW`). It does not show above a game in exclusive fullscreen.
