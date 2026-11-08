# ADR 0005: interface language

## Decision

The interface exists in English and French. The settings hold an optional language: when it is empty, the app follows the Windows display language (`GetUserDefaultUILanguage`), French for every French locale and English for any other language. The settings window has a picker: "Same as Windows", English, Français. Both processes read the language from the settings, so a change applies to the settings window, the tray menu and the on-screen display at once.

Every text lives in `crates/core/src/i18n.rs`, one table per language with the same fields. Texts with a value are functions of that value, so each language keeps its own word order and typography ("40%" in English, "40 %" in French).

Names of layout-dependent keys (letters, digits, punctuation) come from Windows (`GetKeyNameTextW`), in the language of the keyboard layout. The other keys have a name in each table.

The log stays in English.

## Reasons

- A table per language makes a missing translation a compile error, and needs no translation library or runtime file.
- The tables live in `crates/core` so they are tested on any OS.
- An English log can be read by anyone reporting a bug.

## Consequences

- Adding a language means adding a table, a `Lang` variant and its Windows language ID.
- The refusal of a shortcut is written to `status.json` as the Windows error; the settings window shows its own message in the current language.
