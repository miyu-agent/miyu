# Extensions: how programs the core launches talk

An extension is a program a package brings. The core launches it from the manifest and speaks the same protocol with it over stdin and stdout. This page lists what is specific to extensions: what they get at launch, the methods only extensions may call, and what the core calls on them. A from-scratch tutorial is in `docs/authors/`. Chinese version: [extensions.md](extensions.md).

| Method | Direction | What it does |
|---|---|---|
| [`provide`](#provide) | extension → core | Register the tools it provides |
| [`tool.call`, `tool.cancel`](#toolcall-toolcancel) | core → extension | Call one of its tools; drop a call |
| [`package.methods`, `method.call`](#packagemethods-methodcall) | both | Register methods its page and widgets call; calls forwarded by the core |
| [`package.emit`](#packageemit) | extension → core | Push to people who have its page open |
| [`feature.offer`](#featureoffer) | extension → core | Offer features to this person's preset menu (construction IM-6) |
| [`port.claim`, `port.release`, `port.list`](#portclaim-portrelease-portlist) | extension → core (`port.list` for the owner) | Claim and release the ports it listens on; see the whole table (construction IM-5) |
| [`command.call`](#commandcall) | core → extension | A person typed a slash command it declared |
| [`user.append`](#userappend) | core → extension | At the start of a turn, ask whether it wants to add a few lines to the context |
| [`system.append`](#systemappend) | extension → core | Put a section at the end of the system prompt |
| [`widget.set`, `widget.clear`, `widget.watch`, `widget.unwatch`](#widgetset-widgetclear-widgetwatch-widgetunwatch) | both | Widget content |
| [`extension.config`](#extensionconfig) | core → extension | Its own settings changed |
| [`schedule.fired`](schedules.en.md#schedulefired) | core → extension | A schedule it added is due (construction J-4) |
| [`content.list`, `content.read`, `content.changed`](#contentlist-contentread-contentchanged) | both | Read content other packages bring for it |
| [`events.append`](#eventsappend) | extension → core | Record an event of its own in a session |

Other methods go through one table: those for people only (answering for the person, changing the permission level or workspace, deleting sessions, changing config, installing packages…) return `local_only`; those needing a capability return `needs_approval` when the manifest doesn't declare it (see "Capabilities" below); the rest work as usual. Each page's "Who can call" says which kind a method is; methods missing from the table count as people only.

## Launch

- **Program**: looked up in the package folder's `bin/` first, then next to `miyu`, never on `PATH`; arguments are the manifest's `[process] args`.
- **Environment**: the core's, plus `MIYU_HOME` (data root), `MIYU_RESOURCES` (resources directory), `MIYU_PACKAGE_DIR` (this package's folder), `MIYU_PACKAGE_STATE` (this package's state directory, already created), `MIYU_PROGRAM` (the real path of the main `miyu` program, G-15).
- **Working directory**: the package's state directory `state/packages/<id>/`.
- **Stderr**: appended to `state/logs/<id>.stderr`; over 1 MiB it is moved to `.old` first.
- **Handshake**: send `hello` as usual, without credentials. The response's `config` holds the final values of its own settings (secrets as real values).
- **Exit**: the core closing its stdin means "please exit"; it is killed if it hasn't exited after 5 seconds.
- **Crashes**: an exit with any code other than 1, being killed, or a failed handshake counts as a failure; it is restarted after 1, 2, 4, 8, 16 seconds, and stopped after 5 in a row (`failed_repeatedly`). **Exit code 1** means a problem a restart won't fix, like bad config or a taken port: it isn't restarted and stops at once (`config_error`). After 60 seconds of running, the failure count starts over.

## `feature.offer`

Offer a few features to this person's preset menu, such as "QQ history", with their tools:

```json
{"features": [{"id": "qq-history", "name": {"zh": "查询 QQ 群聊记录", "en": "QQ history"}, "summary": {"en": "…"}, "tools": ["qq_history"]}]}
```

- Replies `{}`. Each call sends the whole set, replacing what you last offered for this person; an empty list withdraws them all.
- Kept only in the core's memory: offer again every time you connect or your own switches change, and after a core restart. Exiting doesn't clear it.
- `id` is written like a package id and must not clash with this person's installed features or another package's offers; `name` and `summary` are one string or a table of languages (as in manifest `[features]`); `tools` must be tools you `provide`d. Anything wrong gets `bad_params` (`field` is `features`) and nothing changes.
- Offered features show up in this person's preset menu under your package; the tools they list belong to them, not to your package's own feature. Whether they're on follows the preset: the shipped "Everything" turns them on, the other shipped presets off, and people turn them on in their preset. Open sessions follow the new set from their next turn.
- In groups and private chats on your own platforms your whole package skips the preset (`venues.en.md`); offers only matter in local sessions and on other platforms.

The core also has `feature.set`, for people changing global switches; don't mix them up.

## `port.claim`, `port.release`, `port.list`

An extension that listens on a port of its own (like the one NapCat connects back to for im) claims it first, so a clash shows up at once instead of as a failed bind. The core only keeps a table and doesn't listen for you; the operating system still decides who gets the port.

- **`port.claim {"port": 1–65535}`**: an unclaimed port becomes yours and you get `{}`; claiming it again on the same connection also gets `{}`; one another extension holds gets `port_taken`, without saying who. `0` gets `bad_params`.
- **`port.release {"port"}`**: removes it if you claimed it, leaves it alone otherwise; always `{}`.
- **Released on exit**: when the process exits or the connection drops, its ports count as released; no need to release them yourself. The table is empty after a core restart; claim again when you start.
- **`port.list {}`**: only for the owner (others get `not_permitted`): `{"ports": [{"port", "package"}]}`, by port.

A connection the core didn't start gets `not_an_extension` from `port.claim` and `port.release`.

## Capabilities

The manifest's `[process] capabilities` declares what it needs. Packages you installed need your approval (`extension.enable` with `approve`); built-in ones count as approved. A package with unapproved capabilities isn't started. Approvals live in the switches file `extensions.json` under `approved` (package id to capabilities), and are removed when the package is removed.

| Capability | Allows |
|---|---|
| `tools` | Providing tools to the catalog (`provide`) |
| `commands` | Providing slash commands (manifest `[[slash]]`) |
| `widgets` | Showing its own content in the UI (widgets) |
| `context.inject` | Adding a few lines to the context at the start of a turn (`user.append`) |
| `context.system` | Putting a section at the end of the system prompt (`system.append`) |
| `packages.call` | Calling methods other packages export (`package.call`) |
| `search` | Being a search provider (manifest `[[search]]`, `search.query`) |
| `act_for_external` | Speaking for people on a platform (`session.send` and `command.run` with `as`), `session.note`, `venue.*`, recording `venue.*` events: for platform bridges only |
| `sessions.drive` | Driving sessions: `session.create`, `send`, `interrupt`, `revert`, `unrevert`, `redo`, `compact`, `clear`, `configure`, `fork`, `set_meta`, `respond`, `command.run`, `job.stop` |
| `events.read` | Reading sessions: subscribing to the `events`, `view` and `sessions` streams, `session.list`, `session.recap`, `view.page`, `view.detail`, `job.output` |
| `events.write` | Recording your own events in a session (`events.append`) |
| `memory.write` | Writing memories and knowledge bases: `memory.remember`, `update`, `forget`, `restore`, `dream`, `dream_stop`, `kb.add`, `create`, `update`, `mount`, `remove`, `delete`, `reindex` |
| `fs.read` | Reading files through the core (`fs.*`) |
| `network` | Fetching links through the core (`link.preview`) |
| `fs.write` | Writing given paths: granted once extensions run in a sandbox |
| `tool.guard`, `tool.rewrite` | Stopping a tool before it runs, rewriting its result afterwards: not available yet |

Calling a method whose capability isn't declared returns `needs_approval`, with the missing one in `data.capabilities`. `network` and `fs.*` cover what goes through the core; your program's own network and file access is only checked once extensions run in a sandbox.

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
| `plugin` | A venue plugin name, written like a package id: in groups and private chats the tool is only given where that venue has it on ([`venue.plugins`](venues.en.md#venueplugins)); local sessions are unaffected. Unset means always given |
| `timeout_ms` | 1000 to 600000, default 60000 |
| `need_load` | Retired (construction FL-1, tools are always sent whole): still accepted, ignored |
| `clicall` | `true`: the AI can also call it with `miyu call` inside a command (only read, network and execute tools can be); unset means it cannot. Put a structured result in the response's `data` and the script gets one line of JSON |
| `result` | For a tool with a structured result, its shape on one line, for example `{"memes":[{"id","tags"}]}`: tells the AI which fields a script can take directly (`miyu call --schema` prints it too). Unset means none |
| `display_name` | Name shown to people: `{"zh": "…", "en": "…"}` |
| `subject` | Which argument's value is shown after the name; must be one of `input_schema.properties` |

**Response**: `{"tools": <count>}`. If the connection drops, the tools stay in the catalog and answer "temporarily unavailable" when called.

**Errors**: `not_a_provider` (not an extension the core launched); `bad_tool`: `data.tool` is which one, `data.problem` is `duplicate`, `name`, `parameters`, `access`, `venues`, `timeout`, `clicall`, `result`, `display_name`, `subject`, `feature`, `plugin`.

## `tool.call`, `tool.cancel`

**`tool.call`** (core → extension, with an `id`, expects a response):

| Name | Notes |
|---|---|
| `session` | Which session |
| `call_id` | This call's id |
| `tool` | Tool name |
| `args` | Arguments |
| `cwd` | This turn's working directory |
| `pictures` | The session owner's picture folder (the XDG-style `pictures/` in their home, step Y-11): put images you draw or generate here, where people find them in the terminal and web UI. A real path that may not exist yet; create it (with any missing parents) before writing. Calls outside a session have no such field; keep using the workspace there. Only local sessions get it: venue sessions (groups, private chats) are kept inside their own workspace and don't have it either (step Y-11 fix) |
| `by` | Who asked her: the `by` of the message that triggered the turn; omitted for turns without a trigger |
| `owner` | Whether the core recognizes `by` as the owner |

**Response** `{"result": {"blocks": [...], "error"?, "end_turn"?, "sent"?, "detail"?, "data"?, "state"?}}`:

| Name | Notes |
|---|---|
| `blocks` | Content blocks for the model: `{"type": "text", "text"}`, `{"type": "image", "blob", …}`. Upload images with `blob.put` first |
| `error` | The call counts as failed |
| `end_turn` | Once this step's results are all in, don't ask the model again; the turn ends |
| `sent` | Images she "sends", at most 4: `[{"blob", "media_type", "width", "height", "name"?, "alt"?, "sticker"?}]`. Not shown to the model; heads draw them in the reply and chat platforms send them |
| `detail` | A structured result for heads, in one of the three common shapes below; not shown to the model. If the shape is wrong or `error` is true, the core drops this field and keeps the rest of the result |
| `data` | A structured result for scripts, any JSON: when the AI calls the tool with `miyu call` inside a command (`clicall`), the script gets it as one line of JSON. Not shown to the model, not kept in `tool.result`. Not passed on when `error` is true |
| `state` | Your package's state in this session, written as in **`session.set_state`, `session.state`** below: the core keeps the latest one per session, and undo and clearing the context follow it. A bad one is dropped whole and the result goes on; `null` means not written, `{}` clears; ignored when `error` is true |

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

## `session.set_state`, `session.state`

A package's state in one session (design 46): what you draw in widgets, what you leave the model at compaction, and your own data. Needs the `sessions.drive` capability; you only read and write your own package's state in sessions of the person you act for.

```json
{"widgets": {"todos": {"content": [<widget content>], "title"?: <title>}}, "note": "Todo list:\n- [ ] ...", "data": {...}}
```

| Field | Meaning |
|---|---|
| `widgets` | Widget id to `{"content", "title"?}`: the widget must be one your manifest marks `state = true` (only `scope = "session"` widgets). Content is written as for `widget.set` and checked against that widget's place. Heads draw it, whether or not your program is running |
| `note` | English text written verbatim into the checkpoint at compaction for the model (at most 2000 characters, under half CJK, no control characters except newlines). Not sent on ordinary turns |
| `data` | Your own data, any JSON up to 16 KiB as written: the core only stores it and `session.state` hands it back |

- All three are optional; `{}` or `null` clears. A bad state is refused whole: bad widget content gives `bad_widget_content` (the `path` starts at `widgets.<id>`), anything else `bad_params` with `field` (for example `state.widgets.other`: a widget you did not declare as a state widget).
- **`session.set_state {"session", "state"}`**: replaces your state, during a turn or between turns, without starting a turn; answers `{}` once on disk. Written during a turn, it belongs to that turn: undoing the turn goes back to the earlier one.
- **`session.state {"session"}`**: answers `{"state": {...} | null}`, your package's current state (with `note` and `data`).
- Clearing the context (`/clear`) clears every package's state; ordinary compaction does not.

## `package.methods`, `method.call`

**`package.methods`** (extension → core): `{"methods": [{"name", "timeout_ms"?}…]}` registers the methods its page and widgets call, replacing the previous set. Response `{"methods": <count>}`. Other connections get `not_an_extension`.

**`method.call`** (core → extension): `{"method", "params", "from"?}`: a person's head or another package called it through `package.call`; `from` is the calling package's id (absent when a person called). Whatever it answers is what `package.call` returns; a JSON-RPC error becomes `method_failed` on the other side.

To let other extensions call a method, list it in the manifest's `[export] methods`.

## `package.emit`

Extension → core (construction IM-1 lower-middle 2): `{"event", "data"?}` pushes to connections that subscribed with `subscribe {"stream": "package", "package": <its id>}`; they get `package.event {"package", "event", "data"}`. Pages update from it instead of polling. Response `{}`, also when nobody is subscribed. `event` follows the method-name rules (starts with a lowercase letter; lowercase letters, digits, `_`, `.`, `-`; at most 64), `data` is at most 256 KiB as JSON; otherwise `bad_params`. Other connections get `not_an_extension`. Not stored, not replayed.

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
