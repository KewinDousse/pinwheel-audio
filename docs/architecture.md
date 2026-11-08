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
| `WM_POWERBROADCAST` (resume), `WM_WTSSESSION_CHANGE` (unlock) | Windows | Hooks reinstalled |
| `WM_TIMER` | timers | Display fade, retry of refused shortcuts and of the icon |

The mouse and keyboard hooks run on their own thread (`hook.rs`), see [ADR 0003](adr/0003-win-wheel-hook.md).

Low-level hooks need a message loop on their thread. `thread.rs` provides that thread (`MessageThread`): it lives until its owner drops it, and only ends after `WM_QUIT`, so that no message posted later reaches another thread that reused its ID.

### Path of a wheel notch

1. The hook sees `WM_MOUSEWHEEL` with Win alone held.
2. It adds up the delta (`WheelAccumulator`) and marks Win as used, so that its release does not open Start.
3. It swallows the event and adds the whole notches to the pending count, posting `WM_VOLUME_NOTCHES` unless one is already waiting.
4. `App` takes every pending notch (`hook::take_notches`), asks for the default multimedia output, computes the new level (`stepped_level`), applies it, and unmutes when the volume goes up.
5. The display draws the level (`core::osd::render`) and shows it on the cursor's monitor.

### Path of a shortcut

1. `WM_HOTKEY` gives the index of the output in the settings.
2. `resolve` finds the output among those of Windows, see [ADR 0004](adr/0004-output-identity.md).
3. `IPolicyConfig::SetDefaultEndpoint` makes it the default output, see [ADR 0002](adr/0002-ipolicyconfig.md).
4. The default output is read back to check the switch, then shown.

## Start menu entry

Pinwheel has no installer. The background process writes `Pinwheel.lnk` in the user's Start menu folder (`FOLDERID_Programs`) at each start, when it is missing or points at another executable (`crates/app/src/start_menu.rs`). Starting Pinwheel from Start while it runs opens its settings, like any second launch.

## Started notice

When the settings window does not open at startup (every start but the first), the display shows "Pinwheel started" with the app icon, so a start is never silent. The icon comes from `res/pinwheel.ico` (`crates/app/src/icon.rs`), and the renderer draws the smallest of its images that covers the needed size.

## Settings window look

`crates/app/src/settings_theme.rs` follows the Windows 11 Settings app: Segoe UI Variable (Segoe UI before Windows 11) read from the Windows fonts, one card per setting with its control on the right, on/off switches, the shortcut's keys drawn as keycaps, and the accent colour of Windows (`AccentLight2` in the dark theme, `AccentDark1` in the light one). Both themes are set, and egui follows the one of Windows. Animations are off, so lists and menus show at once.

## Shortcut capture

In the settings window, "Set…" starts `keys::Capture`, which reads the keyboard through Raw Input (`RIDEV_INPUTSINK`) on a message-only window of a `MessageThread`. Low-level keyboard hooks cannot do it: Windows does not call them, in any process, for the keys it sends to the settings window (seen on Windows 11, reproduced on Windows Server 2025, cause not found). Raw Input sees every key, Win included, but cannot stop one:

- the background process suspends its shortcuts while the capture lasts (`WM_CAPTURE_BEGIN`, `WM_CAPTURE_END`), and brings them back by itself after 20 s in case the settings window closes without saying so;
- a Win combination the shell reserves, such as Win+E, still does what Windows gives it while it is captured.

- The combination is complete at the first press of a key that is not a modifier. Ctrl, Alt, Shift and Win are tracked on the left and right side (`HeldModifiers`), so that releasing one does not forget the other.
- Escape alone cancels. The capture also ends by itself after 15 s, or as soon as another app takes the focus: Raw Input also receives the keys typed elsewhere.
- Injected keys are accepted, since macro software (G Hub, Synapse, AutoHotkey) sends its keys that way. Only `vkE8`, sent by the background process's keyboard hook, is ignored.
- Raw Input reports Ctrl, Alt and Shift without their side: the scan code and the `E0` flag give it back.
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

`crates/core/src/osd.rs` draws into a premultiplied RGBA image (tiny-skia for shapes, ab_glyph for text, with the Windows Segoe UI fonts). It follows the Windows 11 flyouts: dark surface, 8 px corners, a shadow drawn into the image's margin, and a gauge in the accent colour, read from Windows (`UISettings`, `AccentLight2`) each time it appears. `crates/app/src/osd.rs` copies it into a `WS_EX_LAYERED` window set by `UpdateLayeredWindow`. The window never takes the focus (`WS_EX_NOACTIVATE`), lets clicks through (`WS_EX_TRANSPARENT`) and does not appear in Alt+Tab (`WS_EX_TOOLWINDOW`). It does not show above a game in exclusive fullscreen.
