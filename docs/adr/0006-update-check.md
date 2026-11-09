# ADR 0006: update check

## Decision

The background process asks GitHub for the latest release at startup, then every 24 hours while it runs. It only says so: it never downloads or installs anything.

- **Request**: `GET https://api.github.com/repos/KewinDousse/pinwheel-audio/releases/latest`, with WinHTTP on a thread of its own, the system proxy, a 15 s timeout and the `Pinwheel/<version>` user agent. That endpoint leaves out drafts and pre-releases.
- **Newer**: the release's tag, without its `v`, is a version `x.y.z` greater than the running one. Any other tag, a pre-release included, is ignored.
- **Page**: the release page (`html_url`) is shown only when it is on the repository's releases, since it opens in the browser.
- **Where it shows**: a banner at the top of the settings window ("Pinwheel x.y.z is available", "See the release"), and a first item in the tray menu that opens the page. The background process writes the release in `status.json` for the settings window.
- **Setting**: "Check for updates", in "About Pinwheel", on by default. Turned off, no request is made and the banner and menu item go away. Turned on, a check runs at once.
- **Failure**: a failed check (no network, a proxy that needs a login, a GitHub error, an answer that is not a release) is logged, and what an earlier check found still holds. It is tried again 15 minutes later, until a check succeeds: at sign-in, the network is often not ready yet.

## Reasons

- Pinwheel has no installer and runs from wherever the user put the exe. A notice, with the user downloading it, keeps it that way: replacing a running, unsigned exe would need an updater process and would trigger SmartScreen anyway.
- WinHTTP comes with Windows and follows its proxy and certificates. An HTTP and TLS crate would add to the exe's size for one request a day.
- Without a check, a user who downloaded Pinwheel once would never learn about a fix.

## Consequences

- GitHub sees one request a day from each running Pinwheel, with its version and the IP address. The README says so and how to turn it off.
- A new release is noticed within a day by a machine that stays on.
