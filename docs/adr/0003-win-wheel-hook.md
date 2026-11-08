# ADR 0003: Win + wheel through a low-level mouse hook

## Decision

Win + wheel is caught by a `WH_MOUSE_LL` hook, installed on a dedicated thread with its own message loop.

- **Trigger**: only with left or right Win held, without Ctrl, Alt or Shift. The horizontal wheel is ignored.
- **Wheel**: deltas add up, and each 120 (a classic notch) gives one step. A change of direction or a pause longer than 400 ms drops the fraction in progress. High-resolution wheels and touchpads send smaller deltas.
- **Steps**: the level lands on a multiple of the step (from 33%, a step of 10 leads to 40 or 30). Raising the volume unmutes.
- **Swallowed event**: the window under the cursor does not scroll.
- **Start menu**: on each caught event, the unassigned virtual key `0xE8` is sent (press then release), as the AutoHotkey documentation recommends (`A_MenuMaskKey`). Windows then considers that Win was used in a combination and does not open Start on release.
- **Callback's job**: it only computes the notches and posts `WM_VOLUME_NOTCHES` to the main window. The audio call happens on the main thread.
- **Reinstall**: the hook is reinstalled unconditionally every 5 minutes, on resume from sleep and on session unlock.

## Reasons

- Raw Input cannot swallow the event. `RegisterHotKey` does not handle the wheel.
- Windows silently removes a low-level hook whose callback exceeds `LowLevelHooksTimeout` (at most 1 s since Windows 10 1709), and the app cannot know it was removed ([documentation](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelmouseproc)). A dedicated thread that does nothing slow, plus a periodic reinstall, keep it from disappearing for good.

## Consequences

- When a window started as administrator is in the foreground, Windows (UIPI) does not pass the wheel to the hook of a non-elevated process: Win + wheel does not work there.
- If a touchpad's two-finger scroll does not reach the hook as `WM_MOUSEWHEEL`, it is ignored: to be checked on the hardware.
