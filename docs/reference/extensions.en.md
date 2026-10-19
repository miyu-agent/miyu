# Extensions: how programs the core launches talk

An extension is a program a package brings. The core launches it from the manifest and speaks the same protocol with it over stdin and stdout. This page lists what is specific to extensions: what they get at launch, the methods only extensions may call, and what the core calls on them. A from-scratch tutorial is in `docs/authors/`. Chinese version: [extensions.md](extensions.md).

| Method | Direction | What it does |
|---|---|---|
| [`provide`](#provide) | extension → core | Register the tools it provides |
| [`tool.call`, `tool.cancel`](#toolcall-toolcancel) | core → extension | Call one of its tools; drop a call |
| [`package.methods`, `method.call`](#packagemethods-methodcall) | both | Register methods its page and widgets call; calls forwarded by the core |
| [`command.call`](#commandcall) | core → extension | A person typed a slash command it declared |
| [`user.append`](#userappend) | core → extension | At the start of a turn, ask whether it wants to add a few lines to the context |
| [`system.append`](#systemappend) | extension → core | Put a section at the end of the system prompt |
| [`widget.set`, `widget.clear`, `widget.watch`, `widget.unwatch`](#widgetset-widgetclear-widgetwatch-widgetunwatch) | both | Widget content |
| [`extension.config`](#extensionconfig) | core → extension | Its own settings changed |
| [`content.list`, `content.read`, `content.changed`](#contentlist-contentread-contentchanged) | both | Read content other packages bring for it |
| [`events.append`](#eventsappend) | extension → core | Record an event of its own in a session |

Extensions call other methods under the same rules as people (`session.*`, `blob.*`, `model.call`…); methods for people only, such as changing config or installing packages, return `local_only`. The list is in [Packages and commands](packages.en.md) and in each page's "Who can call".

## Launch

- **Program**: looked up in the package folder's `bin/` first, then next to `miyu`, never on `PATH`; arguments are the manifest's `[process] args`.
- **Environment**: the core's, plus `MIYU_HOME` (data root), `MIYU_RESOURCES` (resources directory), `MIYU_PACKAGE_DIR` (this package's folder).
- **Working directory**: the package's state directory `state/packages/<id>/`.
- **Stderr**: appended to `state/logs/<id>.stderr`; over 1 MiB it is moved to `.old` first.
- **Handshake**: send `hello` as usual, without credentials. The response's `account` is its account (the system account if it declares one, otherwise `admin`), and `config` holds the final values of its own settings (secrets as real values).
- **Exit**: the core closing its stdin means "please exit"; it is killed if it hasn't exited after 5 seconds.
- **Crashes**: an exit with any code other than 1, being killed, or a failed handshake counts as a failure; it is restarted after 1, 2, 4, 8, 16 seconds, and stopped after 5 in a row (`failed_repeatedly`). **Exit code 1** means a problem a restart won't fix, like bad config or a taken port: it isn't restarted and stops at once (`config_error`). After 60 seconds of running, the failure count starts over.

## Capabilities

The manifest's `[process] capabilities` declares what it needs. Packages the admin installed need a person's approval (`extension.enable` with `approve`); built-in ones count as approved. A package with unapproved capabilities isn't started.

| Capability | Allows |
|---|---|
| `tools` | Providing tools to the catalog (`provide`) |
| `commands` | Providing slash commands (manifest `[[slash]]`) |
| `widgets` | Showing its own content in the UI (widgets) |
| `context.inject` | Adding a few lines to the context at the start of a turn (`user.append`) |
| `context.system` | Putting a section at the end of the system prompt (`system.append`) |
| `packages.call` | Calling methods other packages export (`package.call`) |
| `search` | Being a search provider (manifest `[[search]]`, `search.query`) |
| `act_for_external` | Speaking for people on a chat platform: for chat platform bridges only |
| `tool.guard`, `tool.rewrite`, `events.read`, `events.write`, `sessions.drive`, `network`, `fs.read`, `fs.write` | Declared and approved now; what they unlock comes in later versions |

## `provide`

Register every tool this package provides now, replacing the previous registration. Only accepted from extensions the core launched.

**Params** `{"tools": [...]}`. A tool:

| Name | Notes |
|---|---|
| `name` | Letters, digits, `_`, `-`, 1 to 64 characters; must not clash with the core's or another package's tools |
| `description` | For the model; short English sentences |
| `input_schema` | `{"type": "object", …}` |
| `access` | What it touches: `read` (only looks, stays inside Miyu), `write`, `execute`, `network`, `outbound` (sends things out), `venue` (actions on a chat platform: recall, mute), `install` |
| `venues` | Which sessions: one or more of `local`, `private`, `group` |
| `timeout_ms` | 1000 to 600000, default 60000 |
| `need_load` | `true`: must be loaded first; the tool list only carries its name and description |
| `display_name` | Name shown to people: `{"zh": "…", "en": "…"}` |
| `subject` | Which argument's value is shown after the name; must be one of `input_schema.properties` |

**Response**: `{"tools": <count>}`. If the connection drops, the tools stay in the catalog and answer "temporarily unavailable" when called.

**Errors**: `not_a_provider` (not an extension the core launched); `bad_tool`: `data.tool` is which one, `data.problem` is `duplicate`, `name`, `parameters`, `access`, `venues`, `timeout`, `need_load`, `display_name`, `subject`, `feature`.

## `tool.call`, `tool.cancel`

**`tool.call`** (core → extension, with an `id`, expects a response):

| Name | Notes |
|---|---|
| `session` | Which session |
| `call_id` | This call's id |
| `tool` | Tool name |
| `args` | Arguments |
| `cwd` | This turn's working directory |
| `by` | Who asked her: the `by` of the message that triggered the turn; omitted for turns without a trigger |
| `owner` | Whether the core recognizes `by` as the owner |

**Response** `{"result": {"blocks": [...], "error"?, "end_turn"?, "sent"?, "detail"?}}`:

| Name | Notes |
|---|---|
| `blocks` | Content blocks for the model: `{"type": "text", "text"}`, `{"type": "image", "blob", …}`. Upload images with `blob.put` first |
| `error` | The call counts as failed |
| `end_turn` | Once this step's results are all in, don't ask the model again; the turn ends |
| `sent` | Images she "sends", at most 4: `[{"blob", "media_type", "width", "height", "name"?, "alt"?, "sticker"?}]`. Not shown to the model; heads draw them in the reply and chat platforms send them |
| `detail` | A structured result for heads, in one of the three common shapes below; not shown to the model. If the shape is wrong or `error` is true, the core drops this field and keeps the rest of the result |

The three shapes of `detail` (at most 64 KiB as JSON):

| `kind` | Other fields | Limit |
|---|---|---|
| `links` | `links: [{"title", "url", "snippet"?, "site"?, "date"?}]`; `url` must be `http` or `https` | 50 links |
| `table` | `columns: ["name", …]`, `rows: [[cell, …], …]`; each row has one cell per column, cells are strings, numbers, booleans or `null` | 20 columns, 200 rows |
| `images` | `images: [{"blob", "media_type", "caption"?}]`; `media_type` starts with `image/`; upload images with `blob.put` first | 20 images |

Extra or missing fields are refused. Heads that know the shape draw it as a card; it is kept in the `detail` of `tool.result` (see [Events](events.en.md)).

A JSON-RPC error or a malformed answer counts as a failed call. With no answer by `timeout_ms`, the call reports "didn't finish within N seconds".

**`tool.cancel`** (core → extension, notification): `{"session", "call_id"}`. Sent on timeout, interruption, or when the core is shutting down: drop that call.

Who may ask for what, and who may be acted on, is the providing side's job: check `by` and `owner`, and when refusing, say so in the result with `error` set.

## `package.methods`, `method.call`

**`package.methods`** (extension → core): `{"methods": [{"name", "timeout_ms"?}…]}` registers the methods its page and widgets call, replacing the previous set. Response `{"methods": <count>}`. Other connections get `not_an_extension`.

**`method.call`** (core → extension): `{"method", "params", "from"?}`: a person's head or another package called it through `package.call`; `from` is the calling package's id (absent when a person called). Whatever it answers is what `package.call` returns; a JSON-RPC error becomes `method_failed` on the other side.

To let other extensions call a method, list it in the manifest's `[export] methods`.

## `search.query`

A person picked a source this package declares in `[[search]]` as the search provider, and the AI is searching the web (core → extension, with `id`, needs a response):

`{"source", "query", "want", "allowed_domains", "blocked_domains"}`: the source (`<package>/<id>`), what to search, at most how many results, only these domains, not these domains (the core filters by them again afterwards).

**Response** `{"hits": [{"title", "url", "snippet"?, "date"?}]}`: entries without a title or URL are dropped, excerpts are shortened and extras ignored, as for the shipped providers. When rate limited, answer with a JSON-RPC error whose `data.reason` is `limited`: the core cools you down step by step (from 30 minutes) and doesn't ask meanwhile. Other errors, or no answer within 20 seconds, fall back to the shipped providers for that search. The extension must be running and have registered its connection (`provide`; the tool list may be empty).

## `command.call`

A person typed a slash command declared in this package's `[[slash]]` (core → extension):

`{"command", "argument", "text", "session", "by", "owner", "language"}`: the command name, the words after it, the whole line typed, the session, who typed it, whether that's the owner, the connection's language.

**Response** `{"said": "<reply, may span lines>"}`; the core records a `command.ran` and hands the reply to the person as-is. No answer within a minute, an error, or no `said` reaches the person as `command_unavailable` or `command_failed`.

## `user.append`

Asked at the start of a turn (core → extension). Only extensions that declare `context.inject`, are running, and have a tool in this session's tool set are asked; they are asked in parallel with **a 300 ms limit**.

`{"session", "venue", "text"?}`: `venue` is `local`, `private` or `group`; `text` is the person's message that started the turn (absent for turns not started by a person).

**Response** `{"content": [{"kind", "text"}…]}`, at most 3 blocks: `kind` is lowercase letters, digits, `-`, `_`; `text` 1 to 4000 characters. What it returns is recorded in the session after the triggering message and replayed as-is later. Empty, late or malformed answers count as nothing this turn.

## `system.append`

Put a section at the end of the system prompt (extension → core). Requires `context.system`.

**Params**: `content` (1 to 4000 characters; `null` removes it), `session` (only this session; all when omitted). **Response**: `{}`.

Only sessions whose tool set includes this package's tools get it; a change applies from the next turn. Kept only in the core's memory: set it again after the core restarts.

**Errors**: `not_a_provider`; `needs_approval` (`data.capabilities` is `["context.system"]`); `bad_params`.

## `widget.set`, `widget.clear`, `widget.watch`, `widget.unwatch`

A widget is a block of content declared in the manifest's `[[widget]]` and shown on a head: the home screen (`home`), the sidebar (`sidebar`, can follow the session), the status bar (`status`), or a full page (`page`, opened by a slash command). Requires `widgets`.

**Extension → core**

| Method | Params | Response |
|---|---|---|
| `widget.set` | `widget`, `session` (required for widgets that follow the session), `title` (replaces the manifest's), `content` | `{"kept": true}`; `{"kept": false}` for a session widget nobody is watching in that session |
| `widget.clear` | `widget`, `session` | `{}` |

**Core → extension** (notifications): `widget.watch {"widget", "session"?}`: someone started looking at this widget; fetch data and push content. `widget.unwatch`: the last viewer left. After the handshake the core sends a `widget.watch` for each widget being watched right then.

**Content** is an array, one section per item:

| Section | Fields |
|---|---|
| `heading` | `text` |
| `text` | `text` (may span lines, at most 2000 characters), `tone` |
| `pairs` | `items`: `{"label", "value", "tone"?}`, at most 50 |
| `progress` | `value` (0 to 1), `text`, `mark` (0 to 1, a point marked on the bar) |
| `list` | `items`: `{"title", "detail"?, "right"?, "state"?, "action"?}`, at most 200 |
| `buttons` | `items`: `{"label", "action"}`, at most 8 |
| `sections` | Pages only: `items`: `{"id", "title", "subtitle"?, "badge"?, "state"?, "blocks"}`, `add`: `{"label", "call", "options"?}` |
| `form` | Pages only: `id`, `fields` (`{"key", "label", "hint"?, "control", "value", …}`), `submit` (`{"label", "call", "params"?}`) |

- Text is a string, or a language-to-string table (`{"zh": "…", "en": "…"}`) the core picks from by the connection's language. No Markdown, no HTML.
- `tone`: `normal`, `dim`, `accent`, `good`, `warn`, `bad`. A list item's `state`: `todo`, `doing`, `done`, `failed`, `off`; a section's `state`: `good`, `warn`, `bad`, `off`.
- `action`: `{"call": <method>, "params"?}` calls a method it registered; `{"open": <widget>}` opens one of its pages; `{"copy": <text>}` copies text to the clipboard.
- Form controls: `toggle`, `select` (with `options: [{"value", "label"}]`), `text`, `textarea`, `list`, `number` (`min`, `max`, `integer`); a `value` of `null` means the default; `default` is `{"text", "value"?}`: the line shown to people, and what the default actually is (heads draw a `null` field from it). On save the head calls `submit.call` with `submit.params` plus `{"values": {only the changed fields}}`; the extension answers `{"saved": true}`, or `{"saved": false, "problems": [{"key", "message"}]}`, or `{"saved": false, "message"}`. The `add` call gets `{"value": <the option picked>}` and may answer `{"id": <the new item>}`.
- At most 64 sections and 64 KiB per widget; `status` must be a single one-line `text` at most 40 columns wide; `home` at most 8 sections. A widget is pushed to heads at most every 250 ms; only the newest content in between is kept.

**Errors**: `unknown_widget` (`data.widget`); `bad_widget_content`: `data.path` says where (e.g. `content[4].items[2].title`), `data.problem` is one of `type`, `missing`, `too_long`, `too_many`, `too_big`, `control_char`, `tone`, `state`, `value`, `action`, `place`, `unknown`, `duplicate`; `bad_params` (`session` missing for a session widget, or given for a global one).

## `extension.config`

Its own settings (those starting with "package id plus dot") changed (core → extension, notification): `{"keys": {<key>: <new value> or null}}`, only the changed keys; secrets as real values; `null` when no longer set or unavailable. No subscription needed.

## `content.list`, `content.read`, `content.changed`

Another package can write `[content.<its id>] dir = "…"` in its manifest to bring a folder of content for it (that's how sticker content packs work).

| Method | Params | Response |
|---|---|---|
| `content.list` | none | `{"packages": [{"package", "version"?}…]}`: packages bringing it content, by id |
| `content.read` | `package`, `path`, `offset` | `{"data", "size", "eof"}`, at most 512 KiB at a time |
| `content.changed` (core → extension, notification) | `{}` | A package bringing it content was installed, removed or upgraded: list again |

**Errors**: `not_a_provider`; `unknown_package` (no such package, or it brings nothing for it); `bad_params`; `not_found`.

## `events.append`

Record an event without a turn number in a session: it doesn't start a turn, doesn't interrupt, and isn't shown to the model.

**Params**: `session`, `kind`, `body` (a JSON object, at most 16 KiB). Extensions can only record `ext.<own package id>.<name>` (each segment starts with a lowercase letter; only lowercase letters, digits, `_`, `-`; at most 128 characters in all). Chat platform bridges can also record `venue.recalled` and `venue.delivered` (see [Venues](venues.en.md)).

**Response**: `{"seq": <number>}`.

**Errors**: `bad_params`; `session_not_found`.
