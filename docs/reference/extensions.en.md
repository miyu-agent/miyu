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
| [`tool.guard`, `guard.cancel`](#toolguard-guardcancel) | core → extension | Ask it before a tool call runs: allow, deny or ask the person; stop waiting |
| [`tool.after`](#toolafter) | core → extension | Tell it a tool call's result was written (read-only, no answer) |
| [`driver.call`, `driver.delta`, `driver.cancel`](#drivercall-driverdelta-drivercancel) | both ways | Its model driver: take one model call, stream deltas back; stop waiting |
| [`driver.encode`](#driverencode) | core → extension | Encode once without sending: checks the encoding is steady |
| [`driver.models`](#drivermodels) | core → extension | List the models it can use (optional) |
| [`system.append`](#systemappend) | extension → core | Put a section at the end of the system prompt |
| [`widget.set`, `widget.clear`, `widget.watch`, `widget.unwatch`](#widgetset-widgetclear-widgetwatch-widgetunwatch) | both | Widget content |
| [`extension.config`](#extensionconfig) | core → extension | Its own settings changed |
| [`schedule.fired`](schedules.en.md#schedulefired) | core → extension | A schedule it added is due (construction J-4) |
| [`content.list`, `content.read`, `content.changed`](#contentlist-contentread-contentchanged) | both | Read content other packages bring for it |
| [`events.append`](#eventsappend) | extension → core | Record an event of its own in a session |

Other methods go through one table: those for people only (answering for the person, changing the permission level or workspace, deleting sessions, changing config, installing packages…) return `local_only`; those needing a capability return `needs_approval` when the manifest doesn't declare it (see "Capabilities" below); the rest work as usual. Each page's "Who can call" says which kind a method is; methods missing from the table count as people only.

## Launch

- **Program**: looked up in the package folder's `bin/` first, then next to `miyu`, never on `PATH`; arguments are the manifest's `[process] args`.
- **Environment**: the core's, plus `MIYU_HOME` (data root), `MIYU_RESOURCES` (resources directory), `MIYU_PROGRAM` (the real path of the main `miyu` program, G-15), and this package's place in each part of the data root (design 55 section 4, all created and given as real paths): `MIYU_APP_DIR` (this package's folder), `MIYU_CONFIG_DIR`, `MIYU_STATE_DIR` (also the working directory), `MIYU_CACHE_DIR`, `MIYU_LOG_DIR`, `MIYU_TMP_DIR` (emptied before each start), `MIYU_RUNTIME_DIR`, and the four `MIYU_PICTURES_DIR`-style folders. The old names `MIYU_PACKAGE_DIR` and `MIYU_PACKAGE_STATE` are also set during the transition.
- **Working directory**: the package's state directory `var/lib/<id>/`.
- **Stderr**: appended to `var/log/<id>/stderr`; over 1 MiB it is moved to `.old` first.
- **Handshake**: send `hello` as usual, without credentials. The response's `config` holds the final values of its own settings (secrets as real values); with `provider.key` approved it has one more cell, `provider_key` (see "`provider_key`" below).
- **Exit**: the core closing its stdin means "please exit"; it is killed if it hasn't exited after 5 seconds.
- **When the core dies**: if the core crashes or is killed, you and the processes you started go with it (construction XH-1). On Unix you run in a process group of your own (you are not its leader) together with a process the core watches; when the core is gone, it kills the whole group. Processes that `setsid` or move to another group are not covered. On Windows you and your children are in the core's job object. A planned stop follows the previous item. You are not in the core's process group, so Ctrl+C in a terminal does not reach you.
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

The manifest's `[process] capabilities` declares what it needs. Packages you installed need your approval: given while installing (`package.install` and `package.upgrade` with `approve`, PK-3, see [Packages](packages.en.md)), and for anything still unapproved when opening (`extension.enable` with `approve`); built-in ones count as approved, except for `login.page` and `provider.key`, which need approval even for shipped packages (steps LP-1, PK-1). A package with unapproved capabilities isn't started. Approvals live in the switches file `extensions.json` under `approved` (package id to capabilities), keeping only what the current manifest declares; installing the same id another way (a local file over it, another source, another origin repository) does not keep earlier approvals; they are removed when the package is removed.

| Capability | Allows |
|---|---|
| `tools` | Providing tools to the catalog (`provide`) |
| `tools.commands` | Provided tools run commands (`runs_commands` in the `provide` spec): during a call, the commands you start can use `miyu call` to call the AI's tools as that session (step XG-1 part 2) |
| `commands` | Providing slash commands (manifest `[[slash]]`) |
| `widgets` | Showing its own content in the UI (widgets) |
| `context.inject` | Adding a few lines to the context at the start of a turn (`user.append`) |
| `context.system` | Putting a section at the end of the system prompt (`system.append`) |
| `packages.call` | Calling methods other packages export on the bus (`bus.call`); calling itself by package id (`package.call`) |
| `search` | Being a web provider (manifest `[[search]]`, `search.query`) |
| `model.driver` | Providing a model driver (manifest `[[driver]]`): when its models are used, everything sent to the model goes through it. Doesn't include network access |
| `provider.key` | Getting the key of one model provider the owner already set up (manifest `[provider_key]`, see "`provider_key`" below): shipped packages need approval for it too |
| `act_for_external` | Speaking for people on a platform (`session.send` and `command.run` with `as`), `session.note`, `session.respond` without `to`, `venue.*`, recording `venue.*` events: for platform bridges only |
| `login.page` | Getting sign-in tickets for pages opened on this machine (`login.ticket {"via": "page"}`, see "Signing in pages on this machine" below): shipped packages need approval for it too |
| `sessions.drive` | Driving sessions: `session.create`, `send`, `interrupt`, `revert`, `unrevert`, `redo`, `compact`, `clear`, `configure`, `fork`, `set_meta`, `retitle`, `respond`, `command.run`, `job.stop` |
| `events.read` | Reading sessions: subscribing to the `events`, `view` and `sessions` streams, `session.list`, `session.recap`, `view.page`, `view.detail`, `job.output`; receiving notices after tool calls (manifest `[after]`, `tool.after`) |
| `events.write` | Recording your own events in a session (`events.append`) |
| `memory.write` | Writing memories and knowledge bases: `memory.remember`, `import`, `update`, `forget`, `restore`, `dream`, `dream_stop`, `kb.add`, `create`, `update`, `mount`, `remove`, `delete`, `reindex` |
| `fs.read` | Reading files through the core (`fs.*`) |
| `network` | Fetching links through the core (`link.preview`) |
| `fs.write` | Writing given paths: granted once extensions run in a sandbox |
| `audio.record` | Recording from this computer's microphone: the extension opens the device itself, the core isn't involved; once extensions run in a sandbox, it opens the recording side of the sound card (design 62) |
| `audio.play` | Playing sound through this computer's speakers or headphones: the extension opens the device itself; once extensions run in a sandbox, it opens the playback side of the sound card |
| `tool.guard` | Allowing, denying or asking the person before a tool call runs (manifest `[guard]`, `tool.guard`) |
| `tool.rewrite` | Rewriting a tool's result after it runs: not available yet |

Calling a method whose capability isn't declared returns `needs_approval`, with the missing one in `data.capabilities`. `network` and `fs.*` cover what goes through the core; your program's own network and file access is only checked once extensions run in a sandbox.

## Signing in pages on this machine: `login.ticket`

Your package serves a web page (a bridge's admin page, say) and wants the owner to open it on this computer without typing the access password (step LP-1; the project owner decided on 2026-11-06: no password on this machine only). Declare `login.page`; once it is approved, call `login.ticket {"via": "page"}` on the extension connection and get `{"ticket", "expires"}`: valid for 60 seconds, once. The page signs in with it like any hand-off ticket: the `ticket` credential in the handshake (pages that talk to the core directly), or you redeem it for a device token on the extension connection with `login.redeem {"ticket"}` (see [Connection](connection.en.md), "`login.ticket`, `login.redeem`").

- Only `via` `"page"` is accepted: without `via`, or with another one, you get `local_only`; without `login.page` approved, `needs_approval` with `data.capabilities` `["login.page"]`. A shipped package that declares it needs approval too; it doesn't count as approved.
- The ticket only works on this machine: a remote connection passed on by the web software (one that sent `connection.remote` before the hello, whatever address it named) gets `bad_ticket` for it in the handshake or with `login.redeem`, and the ticket is spent. Your own `login.redeem` on the extension connection counts as this machine: the core can't see who opened your page or from where, so only you can keep the rules below.
- For every ticket the core's run log gets one line `ticket issued purpose=page by=extension package=<your id>`, never the ticket.

**Your obligations**:

1. Only ask for and redeem a ticket for a request that:
   - comes from a loopback address: IPv4 `127.0.0.0/8`, IPv6 `::1`; an IPv4 address mapped into IPv6 (`::ffff:127.0.0.1`) counts as IPv4;
   - carries none of the headers reverse proxies add (`Forwarded`, `X-Forwarded-*`, `X-Real-IP`, `Via`, `CF-Connecting-IP`, `True-Client-IP`, `Tailscale-*` and the like): a reverse proxy on this machine connects from loopback too, and these headers are the only way to tell; treat such requests as coming from elsewhere;
   - has a `Host` written the local way: `127.0.0.1`, `localhost` or `[::1]` with the port you listen on; accept only these even when you listen on the LAN (against DNS rebinding);
   - has an `Origin` exactly equal to `http://` plus the `Host` (same origin), or none.
2. Pages from elsewhere (the LAN, a reverse proxy) always sign in with the core's access password (the `password` credential, `connect_password` in `miyu-client`). Never hand them this ticket instead, and don't set up a password of your own and swap tickets after checking it.
3. Keep tickets out of your logs; hand one to the page in the URL fragment (`#`), and have the page redeem it at once and drop it from the address bar.
4. Recommended, as the web software's `/ticket` does: require a custom header on the request for a ticket (such as `X-Miyu-Local: 1`) and a body that isn't one of the three form types; pages on other sites can't send a custom header.

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
| `role` | What the tool is for, so the core recognizes it by role rather than by name. The only role now is `read_file` (reads a text file: the AI sees the skill list only when a tool like this is there). Unset means none |
| `scope` | Which sessions get it, narrowing further within `venues`: `any` (the default), `local` (sessions on this machine), `local_main` (the main session on this machine, not subagents), `local_main_attended` (the main session on this machine, with someone watching), `spawn` (this machine, while subagents may still go one level deeper) |
| `timeout_ms` | 1000 to 600000, default 60000 |
| `clicall` | `true`: the AI can also call it with `miyu call` inside a command (only read, network and execute tools can be); unset means it cannot. Put a structured result in the response's `data` and the script gets one line of JSON |
| `result` | For a tool with a structured result, its shape on one line, for example `{"memes":[{"id","tags"}]}`: tells the AI which fields a script can take directly (`miyu call --schema` prints it too). Unset means none |
| `display_name` | Name shown to people: `{"zh": "…", "en": "…"}` |
| `subject` | Which argument's value is shown after the name; must be one of `input_schema.properties` |
| `paths` | Which arguments are paths: `[{"parameter": "file_path", "write": true}]`. The argument must be a top-level one in `input_schema.properties` whose type is a string or an array of strings; `write` defaults to false (a read), and when true `access` must be `write`. The core checks the paths of each call against the permission rules (writing outside the workspace asks, as with the built-in `edit`), `tool.call` carries `checked`, and the file effects in the response are taken only for these paths. Without it (or with `null`) no paths are reported and no file effect is taken |
| `background_parameters` | Names of arguments that only exist when the session has a background: `["run_in_background"]`. Each must be a top-level one in `input_schema.properties` and not in `required`. In sessions with background running off, the AI's schema doesn't have them. `null` is the same as unset |
| `runs_commands` | `true`: this tool runs commands (step XG-1 part 2). The package must have `tools.commands` approved. The description the AI sees gets the section on how to use `miyu call` appended (as with the built-in `shell`); calls that get a channel carry `channel` in `tool.call` (see **Running commands** below). Unset, `null` or `false` means no |

**Response**: `{"tools": <count>}`. If the connection drops, the tools stay in the catalog and answer "temporarily unavailable" when called.

**Errors**: `not_a_provider` (not an extension the core launched); `bad_tool`: `data.tool` is which one, `data.problem` is `duplicate`, `name`, `parameters`, `access`, `venues`, `timeout`, `clicall`, `result`, `display_name`, `subject`, `feature`, `plugin`, `role`, `scope`, `paths`, `background_parameters`, `runs_commands` (set to anything but a boolean or `null`); `needs_approval`: a tool with `runs_commands` while the package doesn't have `tools.commands` approved, `data.capabilities` is `["tools.commands"]`, and nothing is registered.

## `tool.call`, `tool.cancel`

**`tool.call`** (core → extension, with an `id`, expects a response):

| Name | Notes |
|---|---|
| `session` | Which session |
| `call_id` | This call's id |
| `tool` | Tool name |
| `args` | Arguments |
| `cwd` | This turn's working directory: a real absolute path with `~` already expanded (as the session recorded it when the home directory is unknown) |
| `pictures` | The session owner's picture folder (the XDG-style `pictures/` in their home, step Y-11): put images you draw or generate here, where people find them in the terminal and web UI. A real path that may not exist yet; create it (with any missing parents) before writing. Calls outside a session have no such field; keep using the workspace there. Only local sessions get it: venue sessions (groups, private chats) are kept inside their own workspace and don't have it either (step Y-11 fix) |
| `by` | Who asked her: the `by` of the message that triggered the turn; omitted for turns without a trigger |
| `owner` | Whether the core recognizes `by` as the owner |
| `permission` | The permission in effect when this call was dispatched, `{"level", "read_only"}` (step TC-1): `level` is `full`; with `read_only` on it is read-only. Use it to decide how far you may reach; for example, a fully open session may send any file. Outsiders' sessions are confined separately (`owner` is false); apply your own rules for outsiders |
| `persona` | The session's persona for this turn (step TC-1); omitted when there is none. Use it to pick a voice or tone |
| `checked` | Only for tools that declare `paths`: the real locations the permission check approved, in the order of the paths in the arguments (one per array item). Write to these: after resolving a path from the arguments yourself, don't touch it unless it is in this list (a link on the way may have been swapped after the check) |
| `channel` | Only for tools that declare `runs_commands`, when this call opened a channel: `{"sock", "token", "bin"}`, see **Running commands** below |

**Response** `{"result": {"blocks": [...], "error"?, "end_turn"?, "sent"?, "detail"?, "data"?, "state"?, "effects"?}}`:

| Name | Notes |
|---|---|
| `blocks` | Content blocks for the model: `{"type": "text", "text"}`, `{"type": "image", "blob", …}`. Upload images with `blob.put` first |
| `error` | The call counts as failed |
| `end_turn` | Once this step's results are all in, don't ask the model again; the turn ends |
| `sent` | Images she "sends", at most 4: `[{"blob", "media_type", "width", "height", "name"?, "alt"?, "sticker"?}]`. Not shown to the model; heads draw them in the reply and chat platforms send them |
| `detail` | A structured result for heads, in one of the three common shapes below; not shown to the model. If the shape is wrong or `error` is true, the core drops this field and keeps the rest of the result |
| `data` | A structured result for scripts, any JSON: when the AI calls the tool with `miyu call` inside a command (`clicall`), the script gets it as one line of JSON. Not shown to the model, not kept in `tool.result`. Not passed on when `error` is true |
| `state` | Your package's state in this session, written as in **`session.set_state`, `session.state`** below: the core keeps the latest one per session, and undo and clearing the context follow it. A bad one is dropped whole and the result goes on; `null` means not written, `{}` clears; ignored when `error` is true |
| `effects` | Which files this call read, changed or trashed, and a new to-do list, in order: see **Effects** below. The core checks them before taking them; once taken they are the same as the built-in tools': the timeline shows lines added and removed, the diff can be viewed, and the AI doesn't have to read a file again after you changed it |

**Effects** (each item of `effects`, told apart by `kind`):

| `kind` | Other fields | What the core checks |
|---|---|---|
| `file.changed` | `path`; optionally `before`, `after` (content hashes `sha256:…`), `before_text`, `after_text` | `path` must be in `checked` as a write. The before is what the core read before sending `tool.call`, the after is what it reads when the response arrives; dropped when they are the same, when a hash or text you gave doesn't match, or over 20 MiB |
| `file.read` | `path`; `lines`: `[from, to]`, counted from 1, with from not above to (otherwise the item is dropped) | `path` must be in `checked` (read or write); the core hashes the whole file as it is now |
| `file.trashed` | `path`, `trash` (where it is in the trash, only shown to people, at most 4096 characters) | `path` must be in `checked` as a write and no longer exist |
| `todo.written` | `todos: [{"content", "status"}]`, `done`? (as the built-in `todowrite`; `null` is the same as unset) | Only the shape is checked; ignored when `error` is true |

- A relative `path` is taken from this call's `cwd`, one starting with `~` from the home directory. Only say *which* file: the core reads the contents itself, so what people see in the diff can't be made up.
- Other kinds, malformed items, paths that weren't checked, and a second item of the same kind for the same file are dropped one by one; the rest of the result goes on and the call doesn't fail. The core log gets a line `tool effect dropped` (`why` is one of `not_checked`, `unchanged`, `mismatch`, `too_big`, `kind`, `shape`).
- File effects are taken even when `error` is true: a change made before failing is real.
- Undo doesn't put files back (as with the built-in tools); it only rolls back what the AI has seen.

**Running commands** (tools that declare `runs_commands`, step XG-1 part 2): when the session has tools the AI can call from scripts, the call opens an endpoint just for it, and `tool.call` carries `channel`:

| Name | Notes |
|---|---|
| `sock` | The endpoint: a socket path on Unix, a named pipe name on Windows (`\\.\pipe\miyu-call-…`). Set it as the environment variable `MIYU_CALL_SOCK` |
| `token` | The token, 32 hex characters. Set it as `MIYU_CALL_TOKEN` |
| `bin` | A directory holding only `miyu`; put it first on `PATH` so the `miyu` a command calls is the same version as the running core. Left out if it couldn't be made |

- Set these three for the commands you start for this call. `miyu call <tool>` in those commands calls the AI's tools under the session's permissions and preset, with the same usage and exit codes as in the built-in `shell` (1 the tool failed or was denied, 2 bad usage or not one of yours, 3 the call is already over, 4 needs approval).
- Only descendants of your extension process get in: the connecting process's parents are followed up and must reach your process; your process itself, anything detached with `setsid` or a double `fork`, and other processes holding the token are refused (exit code 2). This guards against mistakes (a token leaked to another process, a command that detached and still tries to call); it doesn't stop your extension, which can call by starting a child process. The real gate is the package capability `tools.commands`, which a person approves.
- Once the call is answered, times out or is cancelled, the endpoint closes; a `miyu call` still waiting or connecting later exits with 3. To call tools from a command, wait for the command to finish before you respond.
- Commands people type in the session (`!`) are not handed to you, only to the built-in `shell`.

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

**`method.call`** (core → extension): `{"method", "params", "from"?}`: a person's head called it through `package.call` or `bus.call`, or another package through `bus.call`; `from` is the calling package's id (absent when a person called). Whatever it answers is what the caller gets; a JSON-RPC error becomes `method_failed` on the other side. An answer is one line of at most 1 MiB too: the core doesn't read a longer one, sends you `line_too_long` (with the `id` of that `method.call`), and the other side gets `method_failed` at once with `data.code` set to `line_too_long`; store large content as a blob (`blob.put`) and put only its hash in the answer.

To let other extensions call a method, declare a bus name under `[export]` (`provides` or `plugs`) and list the method in `methods` as `<name>.<method>` ([The bus](bus.en.md)). Other extensions can only call you by name; by package id they get `own_package_only`.

## `package.emit`

Extension → core (construction IM-1 lower-middle 2): `{"event", "data"?}` pushes to connections that subscribed with `subscribe {"stream": "package", "package": <its id>}`; they get `package.event {"package", "event", "data"}`. Pages update from it instead of polling. Response `{}`, also when nobody is subscribed. `event` follows the method-name rules (starts with a lowercase letter; lowercase letters, digits, `_`, `.`, `-`; at most 64), `data` is at most 256 KiB as JSON; otherwise `bad_params`. Other connections get `not_an_extension`. Not stored, not replayed.

## `search.query`

A person picked a source this package declares in `[[search]]` as the web provider, and the AI is searching the web (core → extension, with `id`, needs a response):

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

## `tool.guard`, `guard.cancel`

Asked before one of the AI's tool calls runs: allow, deny, or ask the person (core → extension, with an `id`, expects a response). For company policies (no writes under some folders, ask before some commands), auditing and the like.

**Who is asked**: extensions whose manifest declares `tool.guard` in `[process] capabilities` and has a `[guard]` table, approved by the owner and switched on. `[guard]` is a static table the core builds its lookup from at startup:

```toml
[guard]
tools = ["shell", "write", "edit"]   # tool names, optional
access = ["execute", "write"]        # access classes, optional
timeout_ms = 1000                    # how long to wait for you, optional
```

- You are asked about a call when its tool name is in `tools` or its access class is in `access`. A manifest with neither doesn't load (`bad_guard`, reported by `miyu check`): no "ask about everything". Access classes are `read`, `write`, `execute`, `network`, `outbound`, `venue`, `install`.
- Built-in tools, other packages' tools and MCP tools are all matched by name and access class; tools you `provide` yourself are not asked about. You can be a guard without providing any tool.
- `timeout_ms` defaults to 1000 and must be 100 to 5000.
- The permission policy decides first: calls it denies never reach you; allowed and ask-the-person calls do. Several guards are asked in parallel and the strictest wins: one deny denies; with no deny, one ask asks the person; when the policy asks, your allow still asks. You can only tighten, never loosen, and you don't change the sandbox.
- Tools called from scripts (`miyu call`) are asked about too; `call_id` is the nested call's id (like `call_12_1#3`).

**Params**: `{"session", "call_id", "tool", "args", "access", "cwd", "owner", "venue"}`: the session, this call's id, the tool name, the repaired arguments (exactly what will run), the access class, the turn's working directory (a real path, as `cwd` in `tool.call`), whether the speaker is the owner (as `owner` in `tool.call`), and where it was said (`local`, `private`, `group`).

**Response** `{"verdict": "allow" | "deny" | "ask", "reason"?}`:

- `allow`: go ahead. Nothing enters the context; the AI only sees the tool's result.
- `deny`: the tool doesn't run. The AI sees the result `Blocked by <your package id>: <reason>`: `reason` is trimmed, cut to 300 characters, and escaped into one line (newlines, quotes and angle brackets become `\n`, `\u0022` and so on); without one it says `no reason given`. The result on the timeline has `by` set to your package, and people see your original words.
- `ask`: ask the person through the usual approval (`tool.approval_requested` with `by` set to your package and `detail` `{"guard", "tool", "reason"?}`); they can allow once or deny. Sessions where no one can approve deny it.
- Extra fields, an unknown `verdict` or a `reason` that isn't text count as a deny with the reason `the guard answered in a wrong format`.

**No answer is a deny** (a failing guard fails closed):

| When | Reason the AI sees |
|---|---|
| No answer within `timeout_ms` | `the guard did not answer in time` |
| You are switched on but not running (starting, just crashed, can't start), or the connection dropped while asking | `the guard is not running` |
| A JSON-RPC error | `the guard returned an error` |

Extensions the owner switched off are not asked. Aim for a p95 under 20 ms: the person waits on this step.

**`guard.cancel`** (core → extension, notification): `{"session", "call_id"}`. The person interrupted, the turn was cancelled, or you timed out: the core no longer waits. The call ends as interrupted (or was already denied for the timeout), and an answer arriving later is ignored.

## `tool.after`

Tells you an AI tool call has a result and it was written to the session log (core → extension, notification: no `id`, no answer). For audits (recording every call in an outside log) and call statistics. Read-only: you get the written copy; you can't change the result, and nothing reaches the AI's context.

**Who is told**: extensions whose manifest declares `events.read` in `[process] capabilities` and has an `[after]` table, approved by the owner, switched on and running (handshake done). No new capability: whoever can read a session's event stream sees this anyway; this just saves subscribing to the whole stream to watch tool calls. `[after]` is written like `[guard]`, without a timeout:

```toml
[after]
tools = ["shell", "write"]   # tool names, optional
access = ["network"]         # access classes, optional; at least one of the two
```

- You're told about a call whose tool name is in `tools` or whose access class is in `access`. A manifest with neither, an unknown access class, a malformed tool name, or no `events.read` declared doesn't load (`bad_after`, reported by `miyu check`). A tool missing from the catalog (the AI got the name wrong) has no access class and only matches `tools`.
- Tools you `provide` yourself are included. Calls that were denied (by the permission policy, a guard, the kernel), skipped, cancelled or failed are all sent, with `status` as recorded.
- **Sessions follow the event-stream rule** (core1, 2026-11-07): you're not told about sessions you can't reach. An extension that isn't a platform bridge only hears about local sessions and the owner's own unconfined private chats; a platform bridge only hears about venue sessions on its own platforms (manifest `[connection] platform`).
- Tools called from scripts (`miyu call`) are sent too, in a different shape; see below.

**Params**:

| Field | Direct call (`tool.result`) | Called from a script (`tool.nested`) |
|---|---|---|
| `session` | Session id | Same |
| `call_id` | Call id (`call_12_1`) | Nested call id (`call_12_1#3`) |
| `tool` | Tool name | Same |
| `access` | Access class; left out for a tool missing from the catalog | Same |
| `status` | As in `tool.result`: `ok`, `error`, `denied`, `cancelled`, `skipped`… | As in `tool.nested`: `ok`, `error`, `denied`, `needs_approval`, `cancelled` |
| `duration_ms` | How long it ran; left out when it never ran | Same |
| `by` | The event's `by`, as in the event stream: the call itself when it ran, whoever denied it otherwise (`{"kind": "module", "id": "<package>"}`) | Same |
| `args` | The arguments as the model wrote them (a JSON string, the `args` of the `tool_call` block in the event stream) | The corrected arguments (the `args` of `tool.nested`) |
| `blocks` | The `blocks` of `tool.result`: what the AI sees; large blocks are references | Left out |
| `result`, `bytes` | Left out | Hash and size of the blob holding the result handed to the script (as in `tool.nested`), left out when nothing was stored; read it with `blob.get` |

`args` is a string, not an object (the `args` of `tool.guard` is a parsed object): you get it as the event stream records it. Tell the two shapes apart by whether `blocks` is there, or whether `call_id` has a `#`.

**No waiting, no blocking**:

- The core sends and moves on; the turn doesn't wait for you, and being slow or crashing doesn't hold up the session.
- Each extension has one queue of **at most 256**; when it's full new ones are dropped and a `WARN` line is logged (only the first of a run of drops, with the running total dropped for you; another line once there's room again).
- When the connection drops or you crash, the queue is thrown away and nothing is resent; after a restart you start from new calls. An extension switched on but not yet through the handshake isn't sent anything.
- **`tool.after` is not guaranteed delivery**: for a complete audit, subscribe to the event stream (`subscribe` `events`, `events.read`), which replays from the log.

## `driver.call`, `driver.delta`, `driver.cancel`

Your model driver takes one whole model call (core → extension, with an `id`, expects a response): Miyu hands over the unified request; you encode it in the wire format, sign in, send it, parse the response, classify errors, and stream deltas back. Retries, cooldowns, failover and pricing stay in Miyu.

**Who is called**: extensions whose manifest declares `model.driver` in `[process] capabilities` and has a `[[driver]]` table, approved by the owner and switched on. It is used only by providers whose config says `driver = "<your package id>/<driver name>"` (see `[[driver]]` in the [manifest](../authors/manifest.en.md)).

**Params** `{"call", "driver", "request", "fingerprint", "params", "key"?, "base_url"?, "blobs"?}`:

| Field | Meaning |
|---|---|
| `call` | This call's id; `driver.delta` and `driver.cancel` refer to it |
| `driver` | `<package id>/<driver name>`, to tell your drivers apart |
| `request` | The unified request as is: `tools`, `system`, `messages`, `stable`, and when present `continuation`, `described` (what each image shows, for endpoints that can't see images), `output_cap`, `tail`. The same state gives the same bytes; other drivers' private data is removed. A `hosted_tool` block (a tool the model ran on the provider's side, such as a search) that carries your driver's private data is handed over as is, and you decide whether to send it back to the provider; one from another driver, or with no private data, becomes a `text` block holding the text Miyu writes for the model (`<hosted-tool name="…">` wrapping the arguments and the result, both as they are except that every `</` becomes `<\/`; no trailing newline). Only the block's private data tells whether it is yours |
| `fingerprint` | `{"tools", "system", "messages": [...]}`: one hash (16 hex digits) for the tool surface, the system prompt and each message; the same content gives the same hash, so you can use it for your own cache |
| `params` | `{"model", "max_output"?, "effort"?, "temperature"?}` |
| `key` | The key this provider's config refers to; left out when you declared `key = "none"` or the provider has no key |
| `base_url` | The address this provider wrote; left out when it wrote none |
| `blobs` | Only if you declared `images = true`: the request's images as `{"<blob hash>": {"media_type", "data"}}`, `data` in base64. Otherwise left out, and the request carries `described` instead |

**Deltas** `driver.delta` (extension → core, notification): `{"call", "deltas": [...]}`, any number per notification, in order:

| Entry | Meaning |
|---|---|
| `{"start": {"index", "kind"}}` | Block `index` starts; `kind` is `"text"`, `"reasoning"`, `{"tool_call": {"name"}}` or `{"hosted_tool": {"name"}}` (a tool the provider ran on its own side, `name` being the provider's own tool name as is; Miyu doesn't run it). Blocks start from 0, one after another |
| `{"text": {"index", "text"}}` | A piece of that block's text: the reply, reasoning, or a piece of a tool call's or provider-run tool's arguments |
| `{"private": {"index", "data"}}` | Private data for a reasoning, tool call or provider-run tool block (a signature, the provider's own call or step id), any JSON, at most one per block. Later requests hand it back to you as is, never to another driver |
| `{"outcome": {"index", "text", "failed"?}}` | A provider-run tool's result: text both the model and a person can read (titles, URLs, excerpts, the page read), cut by the core at 64 KiB; `failed` is `true` when the provider says the run failed, `false` when left out. Only for a provider-run tool block, at most once per block and before it ends; leave it out when the run was interrupted and has no result |
| `{"end": {"index"}}` | That block is complete |
| `"truncated"` | The output limit was hit before the reply was finished |

Text only after a block starts, nothing after it ends. A wrong delta ends the call as `unclassified` (`the driver sent a bad delta: …`) and later ones are dropped. Deltas for an unknown `call` are dropped silently.

List the names of provider-run tools in the driver's `hosted` in the manifest ([manifest](../authors/manifest.en.md#driver)), so the owner knows at install time what the provider will run on its own. A name that isn't listed is still taken and shown, and the core logs a warning (once per driver and name each time the core runs; at most 64 names per driver are listed, and past that, or for a name longer than 64 bytes, one more line says there are more, without the name). Don't report tools that change things (running commands, writing files) this way; turn them off and go through the core's tools.

**Response** `{"usage"?, "error"?, "sent"?}`, after all of the call's `driver.delta`:

- `usage`: `{"uncached", "cache_read", "cache_write", "output", "reasoning"?}`. Left out means not reported; nothing is estimated.
- `error`: `{"class", "message", "retry_after_ms"?}`. `class` is one of `retryable`, `stalled` (stuck: nothing new for a long time; the kernel stops retrying after two in a row), `rate_limited`, `context_too_long`, `auth`, `quota`, `content_policy`, `unclassified`; anything else counts as `unclassified`. `retry_after_ms` is read only for `rate_limited`. `message` is cut to 500 characters and only shown to people. Don't retry a call yourself once you have sent deltas (the reply would repeat); Miyu retries, cools down and fails over by class.
- `sent`: the SHA-256 of the bytes you actually sent, as `sha256:<64 hex digits>`, recorded in `model.called`. If the same request is sent again (a retry) and your `sent` differs, your encoding looked at something besides the params and the cached prefix can't hit; Miyu logs a `WARN` (once per driver per run).

A response with extra fields or in a wrong format counts as `unclassified` (`the driver sent a bad answer: …`); so does a JSON-RPC error (`the driver refused the call: …`).

**Time limits**: the same as built-in drivers: from sending to the first delta, between deltas, and from the last delta to the response, no more than the idle limit (180 seconds, scaled by reasoning effort), and the whole call no more than ten times that. Past it the call is `retryable` (`the driver went quiet`). If the extension crashes or the connection drops, a waiting call is `retryable` (`the driver went away`). While you are backing off before a restart, a call is `retryable` with the wait from `retry_in`; a package that is off, not approved or stopped is not called, and the provider counts as unusable (a pool moves to its next member).

**`driver.cancel`** (core → extension, notification): `{"call"}`. The person interrupted, the turn was cancelled, the call timed out, or a delta was wrong: Miyu stopped waiting. Don't send anything more for this call; later deltas and the response are ignored.

## `driver.encode`

Encode once without sending (core → extension, with an `id`, expects a response): the params are those of `driver.call`; the response is `{"body"}`, where `body` is the bytes you would send for those params (as a string). `driver.check` (authors run `miyu dev driver-check`) uses it to check that your encoding is steady: the same params must give the same bytes, also after a restart, and adding one message at the end must not change any byte before the end of the stable region.

- Encode from the params only: no time, no random numbers, fields in a fixed order. `call` is this call's id; don't encode it.
- A driver that declares `cache = "contract"` must implement it; without it `driver.check` fails it. Others are reported as skipped. If you don't implement it, answer with JSON-RPC "method not found" (`-32601`).
- The owner's key isn't handed over (no `key`, no `base_url`), and no images.

## `driver.models`

List the models this driver can use (core → extension, with an `id`, expects a response). Optional: called when the owner's model list is refreshed (`model.list` with `refresh`, opening the settings page, after a model request answers 404). If you don't implement it, answer with JSON-RPC "method not found" (`-32601`); the owner then writes model names by hand.

**Params** `{"driver", "key"?, "base_url"?}`: `key` and `base_url` as for `driver.call` (handed over according to your `key` declaration, left out when not written).

**Response** `{"models": [...]}`, one entry per model:

| Field | Meaning |
|---|---|
| `id` | Model name; the owner writes it as `<provider>/<id>` |
| `window` | Context window, in tokens |
| `max_output` | Maximum output, in tokens |
| `tools` | Whether it can call tools |
| `vision` | It can see images: `true` counts as "inputs are text and images" |
| `reasoning` | Recognized, not used yet |

Each is looked up as "written by the owner > listed by you > catalog", so what the owner writes wins. The whole call waits at most 30 seconds; extra fields or a wrong format reject the whole list, and the previous list stays in use.

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

Its own settings (those starting with "package id plus dot") changed (core → extension, notification): `{"keys": {<key>: <new value> or null}}`, only the changed keys; secrets as real values; `null` when no longer set or unavailable. No subscription needed. With `provider.key` approved, changes to the `provider_key` cell are pushed the same way.

### `provider_key`

Step PK-1. An extension whose manifest has `[provider_key] catalog = "<catalog id>"`, that declares `provider.key` and was approved, gets one more cell in the handshake's `config` and in `extension.config`'s `keys`: `"provider_key": "<the real key>"` (a string). Only on extension connections the core started, never on head connections.

- **Which provider**: worked out from the owner's provider settings `providers.<id>` (`miyu_models::provider_key`): one with `catalog` written counts if it matches; without it, one whose literal address contains that provider's official address (only DeepSeek's `api.deepseek.com` for now; an address that is an environment-variable reference doesn't count); with neither, one whose id is that catalog id. If several count, the first by id; one without a key doesn't count.
- **Not handed over**: without `provider.key` approved (shipped packages need approval too), when the owner hasn't set up that provider, when its key can't be read. No other provider's key is ever handed over.
- **Changes**: a changed key (`secret.set`, an edited reference) is pushed as the new value; removing the provider or its key pushes `null`.
- The value never goes into the runtime log or events. `config.get` still only returns the reference `{"secret": "<name>"}`.

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

**Params**: `session`, `kind`, `body` (a JSON object, at most 16 KiB). Extensions can only record `ext.<own prefix>.<name>` (the prefix is the manifest's `prefix`, or the package id without it) (each segment starts with a lowercase letter; only lowercase letters, digits, `_`, `-`; at most 128 characters in all). Chat platform bridges can also record `venue.recalled` and `venue.delivered` (see [Venues](venues.en.md)).

**Response**: `{"seq": <number>}`.

**Errors**: `bad_params`; `session_not_found`.
