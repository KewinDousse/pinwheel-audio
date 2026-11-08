# ADR 0002: switching the default output with IPolicyConfig

## Decision

The default output is changed with the undocumented COM interface `IPolicyConfig` (IID `f8679f50-850a-41cf-9c72-430f290290c8`, class `CPolicyConfigClient`, CLSID `870af99c-171d-4f9e-af0d-e63df40c2bc9`), method `SetDefaultEndpoint(id, role)`, the eleventh method after those of `IUnknown`.

A shortcut sets the `eConsole` and `eMultimedia` roles, as the Windows Sound settings do. The `eCommunications` role (calling apps) is only set when the output has the "Also for calls" option, off by default.

The wheel changes the volume of the `eMultimedia` default output.

After a switch, the default output is read back: a mismatch is logged and shown on screen.

## Reasons

- Windows offers no public API to change the default output. Tools of the same kind (AudioSwitcher, SoundSwitch, EarTrumpet) use this interface, stable since Windows 7.
- Setting the communications default can switch a Bluetooth headset to its "hands-free" profile, mono and of lower quality.

## Consequences

- A Windows update could break the interface with no visible error: reading the default back after a switch detects it.
- The interface is isolated in `crates/app/src/audio.rs` (module `policy`). Only the position of `SetDefaultEndpoint` in the vtable matters: the methods before it are declared without parameters and never called.
