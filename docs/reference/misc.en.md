# Other: background jobs, human-readable text, link cards, diagrams

Chinese version: [misc.md](misc.md).

| Method | What it does |
|---|---|
| [`job.stop`](#jobstop) | Stop a background command or subagent |
| [`job.output`](#joboutput) | Read a background command's output |
| [`human.get`](#humanget) | Text for people: how tools look, message templates |
| [`link.preview`](#linkpreview) | A card for a link |
| [`mermaid.render`](#mermaidrender) | Render Mermaid source to SVG |

**Who can call**: people, extensions; extensions need the `events.read` capability for `job.output` and `network` for `link.preview`.

## `job.stop`

**Params**: `session` (the session that started it), `job` (job id, like `j1`).

**Response**: `{}`, once the report is on disk. A background command's whole process group is killed; a subagent stops its turn and what it spawned. Being stopped by a person wakes her, and she knows you stopped it.

**Errors**: `unknown_job` (no such job, or it already ended); `bad_params` (malformed id).

## `job.output`

**Params**

| Name | Type | Notes |
|---|---|---|
| `session` | string, required | The session that started it |
| `job` | string, required | Job id |
| `tail` | 1 to 2000, default 200 | The last this many lines |
| `after` | non-negative integer | You already have this many bytes; only what comes after. Only one of `after` and `tail` |
| `wait` | boolean, default `false` | With `after`: if nothing new and still running, answer when there's something new or it ends (answered in the background) |

**Response**

- Without `after`: `{"lines", "output", "running", "size", "truncated"}`: total lines, the last `tail` lines, whether it's still running, bytes read, whether earlier output was left out.
- With `after`: `{"more", "output", "running", "size"}`: pass `size` as the next `after`; `more` means there's more after this that the limit cut off.

At most 128 KiB per answer. To follow along: once without `after`, then with `after` and `wait`.

```json
{"id":"o1","jsonrpc":"2.0","result":{"lines":2,"output":"building\nhalf\n","running":true,"size":14,"truncated":false}}
```

**Errors**: `unknown_job`; `not_a_command` (it's a subagent: subscribe to its child session instead); `bad_params`.

## `human.get`

Text for drawing tools and hints in the UI, per language.

**Params**: `language` (the connection's when omitted).

**Response**: `{"language", "said", "tools", "packages"}`:

| Name | Notes |
|---|---|
| `said` | Message id → template text (the head fills in fields like `{name}`) |
| `tools` | Tool name → `{"name", "icon"?, "subject"?}`: display name, icon, which argument's value follows the name |
| `packages` | Tool name → which package it belongs to |

Read fresh every time, so resource changes during development need no restart. After installing or removing packages, or switching MCP servers, heads receive `package.changed`/`mcp.changed` and fetch it again.

```json
{"id":"h1","jsonrpc":"2.0","result":{"language":"en","said":{"core/tool-results/unattended":"Needs approval, nobody to approve"},"tools":{"read":{"icon":"→","name":"Read","subject":"file_path"}}}}
```

**Errors**: `bad_params` (malformed `language`).

## `link.preview`

Only when the networking package (`net`) is compiled in; otherwise `unknown_method`. Answered in the background.

**Params**: `url`.

**Response**: `{"card": {"url", "title", "description", "site", "kind", "image", "icon", "author"?, "duration"?}}`: `kind` is `video`, `article` or `page`; `image` and `icon` are `{"blob", "media_type"}` or `null` (read with `blob.get`); `duration` is in seconds. When no card can be made: `{"card": null, "why"}` with `not_a_url`, `unsupported_scheme`, `no_preview` or `unreachable`; that isn't an error.

## `mermaid.render`

Only when the Mermaid package is compiled in; otherwise `unknown_method`.

**Params**: `source`: Mermaid source.

**Response**: `{"svg", "marks": {"text", "line", "label"}}`: the SVG text, and the three marker colors used for text, lines and edge-label backgrounds, which heads replace with their own theme's colors.

**Errors**: `mermaid_too_long` (source over 64 KiB); `mermaid_failed` (couldn't render, `data.detail` has the library's message).
