# ADR 0003: Win + wheel through low-level hooks

## Decision

Win + wheel is caught by a `WH_MOUSE_LL` hook. A `WH_KEYBOARD_LL` hook keeps Start closed. Both are installed on a dedicated thread with its own message loop.

- **Trigger**: only with left or right Win held, without Ctrl or Alt. The horizontal wheel is ignored.
- **Win + Shift + wheel**: steps of 1%, for a fine setting. It can be turned off, and Win + Shift + wheel is then left to other apps.
- **Win + middle click**: mutes or unmutes the default output. The press and its release are swallowed, and Win counts as used, as with the wheel. It can be turned off.
- **Wheel**: deltas add up, and each 120 (a classic notch) gives one step. A change of direction or a pause longer than 400 ms drops the fraction in progress. High-resolution wheels and touchpads send smaller deltas.
- **Steps**: the level lands on a multiple of the step (from 33%, a step of 10 leads to 40 or 30). Raising the volume unmutes.
- **Swallowed event**: the window under the cursor does not scroll.
- **Start menu**: when Win is released after a caught wheel event, the keyboard hook swallows the release and sends in its place the unassigned virtual key `0xE8` (press then release) followed by the release of Win, as the AutoHotkey documentation recommends (`A_MenuMaskKey`). Windows then considers that Win was used in a combination and does not open Start. The mask is sent at release, not with each wheel event: a held key repeats, and Windows only looks at what follows the last press of Win. The release is swallowed only once `SendInput` has taken all three keys, so Win cannot stay down. A first press of Win (not a repeat) clears the state, so a release the hook missed cannot block the next Start.
- **Callback's job**: the mouse callback only computes the notches and adds them to a shared count. It posts `WM_VOLUME_NOTCHES` only when no message is waiting, and the main window takes the whole count at once: a fast spin applies several notches in one change and one display, instead of queueing one display per notch. The audio call happens on the main thread.
- **Reinstall**: the hooks are reinstalled unconditionally every 5 minutes, on resume from sleep and on session unlock.

## Reasons

- Raw Input cannot swallow the event. `RegisterHotKey` does not handle the wheel.
- Windows silently removes a low-level hook whose callback exceeds `LowLevelHooksTimeout` (at most 1 s since Windows 10 1709), and the app cannot know it was removed ([documentation](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelmouseproc)). A dedicated thread that does nothing slow, plus a periodic reinstall, keep the hooks from disappearing for good.

## Consequences

- When a window started as administrator is in the foreground, Windows (UIPI) does not pass the wheel to the hook of a non-elevated process: Win + wheel does not work there.
- If a touchpad's two-finger scroll does not reach the hook as `WM_MOUSEWHEEL`, it is ignored: to be checked on the hardware.
