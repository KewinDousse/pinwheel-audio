# Architecture

## Two processes, one executable

| Mode | Started as | Role |
|:--|:--|:--|
| Background | `pinwheel.exe` | Tray icon, wheel hook, shortcuts, display. Single instance (mutex `Local\Pinwheel.Main`). |
| Settings | `pinwheel.exe --settings` | egui window. Single instance (mutex `Local\Pinwheel.Settings`). |

The settings window is a process of its own: the UI toolkit (egui, wgpu) is only loaded while it is open, and the background process stays light.

Communication:

- Settings → background: messages posted to the hidden window of class `Pinwheel.Main` (`crates/app/src/ipc.rs`). `WM_RELOAD` after each save, `WM_PREVIEW_OSD` for the preview.
- Background → settings: `status.json` (refused shortcuts, newer release), and `settings.json` when an output got a new ID. The window reads both again every second.

## Background process

All the state is in `App` (`crates/app/src/app.rs`), on the main thread, which owns a hidden top-level window. That window receives:

| Message | From | Effect |
|:--|:--|:--|
| `WM_VOLUME_NOTCHES` | hook thread | Changes the volume, shows the display |
| `WM_TOGGLE_MUTE` | hook thread | Mutes or unmutes, shows the display |
| `WM_HOTKEY` | `RegisterHotKey` | Changes the default output |
| `WM_UPDATE_CHECKED` | update thread | Writes the newer release in `status.json`, or schedules a retry when GitHub is out of reach |
| `WM_TRAY`, `TaskbarCreated` | Explorer | Menu, settings, icon added again |
| `WM_POWERBROADCAST` (resume), `WM_WTSSESSION_CHANGE` (unlock) | Windows | Hooks reinstalled |
| `WM_TIMER` | timers | Display fade, retry of refused shortcuts and of the icon, daily update check and its retry |

The mouse and keyboard hooks run on their own thread (`hook.rs`), see [ADR 0003](adr/0003-win-wheel-hook.md).

Low-level hooks need a message loop on their thread. `thread.rs` provides that thread (`MessageThread`): it lives until its owner drops it, and only ends after `WM_QUIT`, so that no message posted later reaches another thread that reused its ID.

### Path of a wheel notch

1. The hook sees `WM_MOUSEWHEEL` with Win alone held.
2. It adds up the delta (`WheelAccumulator`) and marks Win as used, so that its release does not open Start.
3. It swallows the event and adds the whole notches to the pending count, posting `WM_VOLUME_NOTCHES` unless one is already waiting.
4. `App` takes every pending notch (`hook::take_notches`), asks for the default multimedia output, computes the new level (`stepped_level`), applies it, and unmutes when the volume goes up.
5. The display draws the level (`core::osd::render`) and shows it, see [Display settings](#display-settings).

### Win + Shift + wheel

With Shift held too, the notches go to a count of their own, applied with steps of 1% (`settings::FINE_STEP`). The "Win + Shift + scroll" switch turns it off: the hook then leaves the event to other apps.

### Win + middle click

With Win alone held, the hook swallows the middle button press, marks Win as used, and posts `WM_TOGGLE_MUTE`. It swallows the matching release too, even when Win is already up, so that no window gets a release without its press. A middle press it lets through forgets a release it missed (the hooks reinstalled in between, or a release over an administrator window), so that the next release goes through. Turned off, both go through.

### Hook switches

The "Win + Shift + scroll" and "Win + middle click" switches reach the hook thread through atomics (`hook::configure`), set by the main thread at startup and after each reload.

### Path of a shortcut

1. `WM_HOTKEY` gives the index of the output in the settings.
2. `resolve` finds the output among those of Windows, see [ADR 0004](adr/0004-output-identity.md).
3. `IPolicyConfig::SetDefaultEndpoint` makes it the default output, see [ADR 0002](adr/0002-ipolicyconfig.md).
4. The default output is read back to check the switch, then shown.

### Next output shortcut

A shortcut of its own (`Slot::NextOutput`, identifier `0xBFFF`) makes the next output of the list the default one (`devices::next_output`): the first connected one after the current default, going round the list, or the first connected one when the default output is not in the list. An unplugged output that would resolve to a connected one of the list, through their shared name, counts as not connected: switching to it would give it that one's ID (see [ADR 0004](adr/0004-output-identity.md)), and the shortcut would stay on the same device. With none connected, the display says so. It is registered after the outputs' shortcuts, and not at all when one of them has the same combination: the settings window says so.

## Update check

See [ADR 0006](adr/0006-update-check.md). `crates/app/src/update.rs` makes the request on a thread of its own and posts `WM_UPDATE_CHECKED`; `crates/core/src/update.rs` reads GitHub's answer and compares the versions.

## Start menu entry

Pinwheel has no installer. The background process writes `Pinwheel.lnk` in the user's Start menu folder (`FOLDERID_Programs`) at each start, when it is missing or points at another executable (`crates/app/src/start_menu.rs`). Starting Pinwheel from Start while it runs opens its settings, like any second launch.

## Started notice

When the settings window does not open at startup (every start but the first), the display shows "Pinwheel started" with the app icon, so that a start is not silent. The icon comes from `res/pinwheel.ico` (`crates/app/src/icon.rs`), and the renderer draws the smallest of its images that covers the needed size.

## Settings window start

Each opening starts a process, so the window's start time is what the user waits for. The log gives it ("settings window shown N ms after the process started").

- wgpu is built with DirectX 12 only (`crates/app/Cargo.toml`): with its default backends, it also starts Vulkan and OpenGL at each opening. OpenGL alone (eframe's `glow`) is not an option, as Windows can lack OpenGL 3.3 (remote sessions, basic display driver, the CI runner). Without a DirectX 12 driver, wgpu falls back to WARP, Windows' software renderer, and the log says "Software rasterizer detected".

## Settings window place

The window opens where it was closed, with the same size (`crates/app/src/window_place.rs`). Windows' window placement gives the normal rectangle even while the window is minimized or maximized, where its current rectangle is off-screen or covers the screen. The window saves it at its first refresh, then once a second when it changed, and on exit, in `%LOCALAPPDATA%\Pinwheel\window.txt`; the file is local, as screens differ from one machine to the next. At the next opening, the rectangle is set back while the window is still hidden, twice so that a screen of another scale does not scale the size, and Windows moves it onto a screen if that one is gone. Only the normal rectangle is kept: a window closed maximized opens at its normal size, and a snapped one at its place from before the snap.

## Settings window look

`crates/app/src/settings_theme.rs` follows the Windows 11 Settings app: Segoe UI Variable (Segoe UI before Windows 11) read from the Windows fonts, controls on the right, on/off switches, the shortcut's keys drawn as keycaps, and the accent colour of Windows (`AccentLight2` in the dark theme, `AccentDark1` in the light one). Both themes are set, and egui follows the one of Windows.

## Settings window layout

The window shows little at once, so that it reads at a glance:

- Three groups, apart: the outputs, "Add an output" and "Next output"; a "Win + scroll" card (step, Win + Shift, Win + middle click) and an "On-screen display" card; a "General" card (language, startup, logs) then "About Pinwheel" (version, update check, links to the source code and to a new issue).
- Each card folds. Closed, an output's card shows what matters day to day: its name, its state, whether it is the default output of Windows (`Audio::default_output_id`, read every second) and its keys. The other cards show their title only: a summary of their values would repeat what opening them shows. A click on the card opens it, and scrolls it into view; one card is open at a time.
- Closed, the "Next output" card shows its keys, as an output's card does, or "Set a key" when it has none.
- The rare actions (change or clear a shortcut, the calls option, remove the output) are inside the open card. Remove is at its bottom, away from the header, so that a double click opening the card cannot remove the output.
- A shortcut's error or warning shows on its card, open or closed, and only when there is one.
- A newer release shows as a banner at the top, see [ADR 0006](adr/0006-update-check.md).
- During a capture, the box waiting for keys takes the place of the shortcut and the other cards are disabled.

## Shortcut capture

In the settings window, "Set a key", "Set" or "Change" starts `keys::Capture`, which reads the keyboard through Raw Input (`RIDEV_INPUTSINK`) on a message-only window of a `MessageThread`. Low-level keyboard hooks cannot do it: Windows does not call them, in any process, for the keys it sends to the settings window (seen on Windows 11, reproduced on Windows Server 2025, cause not found). Raw Input sees every key, Win included, but cannot stop one:

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

## Display settings

The "On-screen display" card sets how it shows (`OsdSettings`):

- **Show**: off, nothing shows, not even a notice such as a disconnected output or "Pinwheel started". "Try the display" is then disabled.
- **Screen**: the one under the mouse pointer, or the main screen (`MonitorFromPoint` at the origin).
- **Duration**: from 0.5 to 5 s before the 300 ms fade. **Opacity**: from 20 to 100%, the alpha the window starts its fade from.
- **Full screen**: when "Hide over full-screen apps and games" is on, nothing shows while `SHQueryUserNotificationState` reports a full-screen app, a Direct3D game or presentation mode. It is the state Windows gives apps that want to stay quiet, and covers a browser video in full screen.

Values out of range in a hand-edited file are brought back within it when the file is read.
