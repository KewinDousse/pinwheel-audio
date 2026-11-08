# Manual tests on Windows

What the automated tests do not cover. The log (`%LOCALAPPDATA%\Pinwheel\logs\pinwheel.log`) helps understand a failure.

## Startup

- [ ] First run: the icon appears, the settings window opens, and Start has a Pinwheel entry.
- [ ] Run the executable again: a single Pinwheel runs, the settings open.
- [ ] Startup ticked, then sign out and in again: Pinwheel runs.
- [ ] Turn Pinwheel off in Windows Settings > Apps > Startup: the settings window says so.
- [ ] Restart Explorer (Task Manager): the icon comes back.
- [ ] Startup ticked, sign in: a single icon, and "Quit" removes it (no ghost icon).
- [ ] First run, close the settings without changing anything, then run Pinwheel again: the settings do not open by themselves, and the display shows "Pinwheel started" with its icon.
- [ ] With Pinwheel running, start Pinwheel from the Start menu: the settings window comes to the front.
- [ ] Move `pinwheel.exe` to another folder and run it: the Start menu entry starts the moved one.

## Win + wheel

- [ ] In the Explorer and in a browser: the volume changes by 10%, the page does not scroll.
- [ ] Release Win after turning the wheel, quickly and after holding Win for a few seconds: the Start menu does not open.
- [ ] Win alone, without the wheel: the Start menu opens as usual.
- [ ] Ctrl + Win + wheel: Pinwheel does nothing.
- [ ] Volume at 33% (set in Windows), then one notch up: 40%.
- [ ] Muted, then one notch up: unmuted.
- [ ] Touchpad (two-finger scroll) and free-spinning wheel, when available.
- [ ] Task Manager (admin window) in the foreground: Win + wheel does nothing, a Windows limit (see ADR 0003). The window under the cursor scrolls.
- [ ] Spin the wheel fast from 0% to 100% and back several times: the display follows the volume without lagging behind.
- [ ] After sleep and wake, then after lock and unlock: it still works.
- [ ] After `Restart-Service audiosrv -Force` (admin PowerShell): it still works.

## Display

- [ ] Each style and each position, with the "Preview" button.
- [ ] In the settings, drag the "Step per wheel notch" slider (Volume section) back and forth: the shortcuts keep working afterwards, and the log shows no burst of saves.
- [ ] The display stays 1 s then fades out, and a new notch restarts the delay.
- [ ] It does not take the focus: typing in the active window keeps working.
- [ ] Two monitors with different scaling: it shows sharp and at the right size on the cursor's monitor.
- [ ] Change the accent colour (Settings > Personalization > Colors): the next display's gauge takes it.
- [ ] Display at the bottom: the panel stays the same distance above the taskbar, its shadow does not touch it.

## Settings window

- [ ] Windows in dark mode, then in light mode (Settings > Personalization > Colors): the window follows, text stays readable, switches and slider take the accent colour.
- [ ] Open each list (language, style, position, add an output): it shows at once, without a fade.
- [ ] Make the window narrow: titles and controls do not overlap.

## Output shortcuts

- [ ] Add two outputs with Ctrl+Alt+1 and Ctrl+Alt+2: each shortcut switches the output and shows it.
- [ ] Unplug or turn off an output (Bluetooth headset): it stays in the list, marked "disconnected", and its shortcut shows "disconnected".
- [ ] Plug it back: the shortcut works again.
- [ ] Same shortcut on two outputs: the window says so.
- [ ] Shortcut already taken by another app: the window says so within a second.
- [ ] "Also for calls": the communications output changes in Windows Settings > Sound.
- [ ] Restart the PC: every shortcut is still there.

## Shortcut capture

- [ ] Set Win+E: the Explorer does not open, the window shows the Win and E keys and the Win warning.
- [ ] Set a combination already given to another output: it does not switch outputs during the capture.
- [ ] Set right Ctrl + left Shift + a letter, releasing the modifiers in any order: the combination is complete.
- [ ] Set a single key (a letter, then F13): the warning shows for the letter, not for F13.
- [ ] A media key (play/pause) and a key sent by macro software: both accepted.
- [ ] Escape alone: the capture is cancelled, the previous shortcut stays.
- [ ] Type nothing for 15 s: the capture stops.
- [ ] During the capture, click another window and type: the capture stops and the keys reach that window.
- [ ] Close and reopen the settings: the captured shortcut is saved and works.

## Language

- [ ] Windows in French, language "Same as Windows": settings, tray menu and display in French. Windows in another language: in English.
- [ ] Pick English, then Français: the settings window, its title, the tray menu and the display follow at once.
- [ ] With the settings window open in one language, switch to the other, then run `pinwheel.exe --settings` again: the open window comes to the front.
- [ ] Key names: a shortcut with Shift reads "Shift" in English, "Maj" in French; a letter follows the keyboard layout.
