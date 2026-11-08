# ADR 0004: output identity and keeping the settings

## Decision

Each chosen output is saved with its Windows endpoint ID and its name (`PKEY_Device_FriendlyName`, such as "Headphones (Realtek USB Audio)"). When its shortcut is pressed, the output is looked up among the active, unplugged or disabled outputs:

1. Same ID and active: that is the one.
2. Otherwise, if **exactly one** active output has the same name: that is the one. Its new ID and name replace the old ones in the settings, and the change is logged.
3. Otherwise (unplugged, not found, or several outputs with the same name): nothing changes, and the display says "disconnected".

A missing output stays in the settings with its shortcut until the user removes it.

The settings file (`%APPDATA%\Pinwheel\settings.json`, JSON with a version number):

- is only written after a user action: a change in the settings window, or a shortcut press that triggers case 2. The one exception is the first run, when the background process writes the defaults: the file then tells later starts that the first run happened;
- is written atomically, through a temporary file named after the writing process, then a rename: both processes write it;
- is kept as it is in memory when a read fails (file locked, for example), so a failed read never becomes saved defaults;
- when unreadable, is renamed to `settings.json.<timestamp>.bak` before the defaults are used;
- tolerates missing fields (defaults) and ignores unknown ones.

A shortcut Windows refuses stays saved: the refusal is shown in the settings and registration is retried every 30 s.

## Reasons

- The endpoint ID survives restarts and unplugging, but Microsoft states that it changes when the driver is updated or the device reinstalled ([documentation](https://learn.microsoft.com/en-us/windows/win32/coreaudio/endpoint-id-strings)). Pairing a Bluetooth device again also gives a new one. The name finds the output again in those cases.
- With two outputs of the same name, there is no safe way to choose.
- Never rewriting at startup keeps an output that is missing at that moment from vanishing from the settings.
