# Miyu core protocol reference

For programs that talk to the core: "heads" such as the terminal and web UIs, and extensions the core launches. It covers how to call each method that exists today, what comes back, and how it fails. It does not cover why things are the way they are or when they were built (that lives in `docs/blueprint/`). Chinese version: [README.md](README.md). This reference is under Apache-2.0 (`LICENSES/Apache-2.0.txt` at the repository root): you can write your own clients and extensions from it under any license.

## Conventions

- **One message per line**: the core and its peer exchange JSON-RPC 2.0 over one connection, one message per line (UTF-8, ending in `\n`), at most 1 MiB per line.
- **Hello first**: the first message after connecting must be `hello`; before that every other method gets `hello_first`.
- **Fields**: params are an object. Writing `null` is the same as leaving a field out. Unknown fields are ignored.
- **Errors**: JSON-RPC's own errors use its standard codes; Miyu's are all `-32010`, with the reason in `error.data.reason` and sometimes a few more fields (listed under each method's "Errors"). `error.message` is a sentence for people in the connection's language; don't branch on it.
- **Who can call**: each method says whether people (head connections) and extensions (connections of packages the core launched) can call it, and how they differ.
- **Protocol 1 is frozen** (2026-10-24): its methods are listed in [methods-v1.txt](methods-v1.txt), plus `hello` for the handshake and `subscribe`, `unsubscribe` and `provide`. From now on it is only added to: existing methods are not removed, renamed or given a new meaning, and replies only gain fields. New methods and fields say which core version they arrived in; an older core answers `unknown_method` to a method it doesn't know and ignores fields it doesn't know, so check the core version in the handshake reply before relying on them, or fall back on `unknown_method`. Removing, renaming or changing a method needs protocol 2: the handshake negotiates from the `[lowest, highest]` range you send, and fails with `protocol_mismatch` when there's no overlap.

## Contents

| Page | Methods |
|---|---|
| [Connection](connection.en.md) | Connecting, one message per line, `hello`, `subscribe`, `unsubscribe`, `account.*`, `login.*` |
| [Sessions](sessions.en.md) | `session.*`, `view.page`, `view.detail` |
| [View stream](view.en.md) | Ready-to-draw entries and session status |
| [Events](events.en.md) | Every kind of event in a session log |
| [Pushes](pushes.en.md) | What each stream pushes |
| [Attachments and file contents](blob.en.md) | `blob.*` |
| [Files](fs.en.md) | `fs.*` |
| [Config, secrets, models](config.en.md) | `config.*`, `secret.*`, `provider.*`, `model.*`, `usage.query` |
| [Personas and presets](personas.en.md) | `persona.*`, `preset.*` |
| [Projects](projects.en.md) | `project.*` |
| [Packages and commands](packages.en.md) | `package.*`, `extension.*`, `mcp.*`, `command.*`, `check` |
| [Extensions](extensions.en.md) | Launch, capabilities, `provide`, `tool.call`, `user.append`, `system.append`, widgets, `content.*`, `events.append` |
| [Venues](venues.en.md) | `venue.*`, `session.respond`, `session.note`, speaking for people on a chat platform |
| [Memory and knowledge bases](memory.en.md) | `memory.*`, `kb.*` |
| [Other](misc.en.md) | `job.*`, `human.get`, `link.preview`, `mermaid.render` |
| [Errors](errors.en.md) | Every reason code |

Tutorials for writing packages, extensions, scripts and skills are in [`docs/authors/`](../authors/README.en.md).
