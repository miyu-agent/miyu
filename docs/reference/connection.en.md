# Connection: connecting, hello, subscriptions

Chinese version: [connection.md](connection.md).

| Method | What it does |
|---|---|
| [`hello`](#hello) | Handshake: who you are, your credentials |
| [`subscribe`, `unsubscribe`](#subscribe-unsubscribe) | Subscribe to or leave a push stream |
| [`account.setup_code`, `account.setup`, `account.logout`](#accountsetup_code-accountsetup-accountlogout) | Access password: the one-time code for setting it from the web, setting and changing it, revoking devices |
| [`account.set_avatar`, `account.avatar`](#your-avatar) | Your avatar |
| [`core.stop`](#corestop) | Ask the core to stop |

## Connecting

- **Local heads**: the core listens on a local socket only you can reach. Its path is in `run/socket` under the data root (one line, the path as-is) and the local token is in `run/token` (64 lowercase hex digits). Linux and macOS use a Unix domain socket, Windows a named pipe `\\.\pipe\miyu-<fingerprint>`. If no core is running, the head starts `miyu core` itself.
- **Extensions**: programs the core launches from a package manifest speak the same protocol over stdin and stdout, with no credentials (any given in `hello` are ignored).
- **Web**: the browser talks to the web app, which connects to the core like a local head. For a connection it opens on behalf of a remote person, the web app first writes the notification `{"jsonrpc":"2.0","method":"connection.remote","params":{"from":<address>}}` before the hello, then passes the browser's bytes through: that connection is remote (Y-1 下). It only goes one way: once marked it never becomes local again, and the notification is ignored after the hello.
- **Local machine only** (Y-1 下): reading files on the core's machine, handing over local paths and picking local folders are for local connections only; a remote one gets `local_machine_only` and nothing happens: `fs.*`, `session.terminal`, `session.set_terminal`, `account.setup_code`, `core.stop`; `blob.put` and `package.install` with `path`, `kb.mount` with `folder`, `mcp.set` with `command`, `check` with `file` or `cwd`, `config.trust` and `config.get` with `cwd`; a session's `cwd` and `dirs`, an attachment's `path`, `command.run`'s `cwd` and `/workspace <path>`, a project's `workspace`. A session created without `cwd` works in its own default workspace. Remote connections get no `host` in the hello reply.

## One message per line

- Each message is one line of JSON, UTF-8, ending in `\n` (`\r\n` is accepted when reading). At most 1 MiB per line; a longer line gets `parse_error` and the connection closes.
- The core writes compact JSON with object fields sorted by name.

**Requests** are JSON-RPC 2.0: `jsonrpc` is `"2.0"`, `method` the method name, `params` an object (omitted means `{}`, an array gets `bad_params`), `id` a string of 1 to 128 bytes with no control characters. A message without `id` is a notification and the core ignores it, except `connection.remote` before the hello (see **Web** above). Unknown params are ignored; `null` means omitted.

**Just one person**: Miyu is for the one person who installed it (the owner); there are no accounts, members or admin mode (since 2026-10-28). A `"scope"` that old heads put in the request envelope is accepted and ignored.

**Responses**: success in `result`; refusal in `error`: `code` is the error code, `message` a sentence for people (in the connection's language), `data.reason` the reason code for programs, sometimes with more fields.

```json
{"id":"c7","jsonrpc":"2.0","result":{"events":[41]}}
{"error":{"code":-32010,"data":{"reason":"session_not_found"},"message":"Session not found."},"id":"c7","jsonrpc":"2.0"}
```

**Pushes** are notifications from the core: `{"jsonrpc":"2.0","method":<name>,"params":{…}}`, no `id`. See [Pushes](pushes.en.md) for what each stream pushes.

**Ordering**:

1. Requests on one connection are handled one at a time: the next line is read after the previous one is answered. A few slow ones are answered in the background without blocking later requests, matched by `id`: `model.call`, `memory.dream`, `link.preview`, `job.stop`, `fs.find` and `job.output` with `wait`, and the slow cases of `command.run` (such as `/dream`).
2. Results before responses: the pushes a command causes always arrive before its response.
3. Sending the same `id` again (within a session's last 1024): you get the previous response and nothing is done twice. Refused ones are not remembered; sending again is judged again. This is for resending after a dropped connection, and it **does not depend on the connection**: a new connection that reuses an `id` used before is treated as a resend, gets the earlier response, and nothing happens (for example `session.send` answers, but the message never reaches the session). So every connection's `id`s must differ from all earlier ones; the simplest way is a random prefix per connection followed by an increasing number. `session.create` also recognizes resends by `id`: creating again with the same `id` gets back the session made the first time, even after the core restarts.

## `hello`

The handshake. It must be the first message; before it every other method gets `hello_first`. A connection that hasn't completed the handshake within 10 seconds is closed.

**Who can call**: people, extensions.

**Params**

| Name | Type | Notes |
|---|---|---|
| `protocol` | `[lowest, highest]`, required | The protocol major versions you support; the core only has `1` |
| `head` | `{"kind", "version"}`, required | What you are and which version; only logged |
| `locale` | string | Your system's language (`zh-CN`, `en_US.UTF-8` both work). Sets the connection's language when the `ui.language` setting is `auto` |
| `caps` | object | `input` (boolean): whether you can let a person type. `prefix_ask` (boolean): you ask the person when changed files are about to change the prefix (see **Prefix changing** under [`session.send`](sessions.en.md#sessionsend)) |
| `token` | string | The local token |
| `code` | string | A one-time code (setting the access password from the web the first time) |
| `login` | string | A device token (a remembered web login) |
| `password` | string | The access password, for connecting from the local network |
| `ticket` | string | A hand-off ticket (from `login.ticket`) brought over by another page; it is exchanged for a device token on the spot |

Give exactly one kind of credential: `token`, `code`, `login`, `ticket` or `password`. Extensions give none. A remote connection passed on by the web software (`connection.remote`) that gives no credential while no access password is set gets `password_not_set`.

**Response**

| Name | Notes |
|---|---|
| `protocol` | The chosen major version: `1` |
| `core` | `{"version"}`: the core's version |
| `avatar` | The version of your avatar, only when it has one (construction AV-1): heads use it to refresh their cache and fetch the picture with `account.avatar` |
| `host` | `{"home", "platform", "workspace", "dirs"}`: the system home directory (`null` if unknown), `linux`/`macos`/`windows`, the default workspace, and the XDG-style folders `{"pictures", "videos", "documents", "downloads"}` (the real path when it exists and as is otherwise, step Y-11; on Windows without the `\\?\` prefix, step WP-1). Not given to remote connections (Y-1 下); extensions get no `dirs` |
| `language` | `zh`, `en` or `ja`: the language for text from now on. If the setting changes, the next message follows it without a new handshake |
| `view` | The view projection version, currently `1` (see `view.page` in [Sessions](sessions.en.md)) |
| `sandbox` | `{"usable": true}`, or `{"usable": false, "reason"}`: `helper_missing`, `helper_failed`, `no_mechanism` |
| `config_errors` | How many errors the config has right now; omitted when none |
| `config` | Extensions only: the final values of this package's own settings (secrets as real values). Later changes arrive as `extension.config` |
| `setup` | `true`: connected with a one-time code; can only set the access password |
| `password_set` | Whether an access password is set (boolean): heads decide from it whether "allow LAN access" can be turned on |
| `login` | `{"expires", "token"}`: only when connected with the access password or a hand-off ticket; the web app keeps it for next time. A ticket used this way is spent |

```json
→ {"jsonrpc":"2.0","id":"h1","method":"hello","params":{"protocol":[1,1],"head":{"kind":"tui","version":"0.1.0"},"locale":"en-US","token":"3f9a…"}}
← {"id":"h1","jsonrpc":"2.0","result":{"core":{"version":"0.0.1"},"host":{"home":"/home/me","platform":"linux","workspace":"/home/me/.miyu/home/me/workspace"},"language":"en","protocol":1,"sandbox":{"usable":true},"view":1}}
```

Sending `hello` again after the handshake is fine: if it passes, the new language and capabilities apply; if it is refused and the connection stays open, the previous ones stay.

**Errors** (those marked "closes" close the connection after answering)

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | A required field is missing or has the wrong type; more than one kind of credential (closes) |
| `protocol_mismatch` | `1` is not in your range (closes) |
| `bad_token` | No credential, or a wrong local token (closes) |
| `bad_code`, `bad_login`, `bad_password` | Wrong one-time code, device token or access password (closes) |
| `password_not_set` | No access password yet: a `password` was given, or a remote connection gave nothing (closes). Set one on this computer first |
| `bad_ticket` | No such hand-off ticket, expired, or already used (closes) |
| `login_throttled` | Five wrong passwords from the same place within 60 seconds, or 30 tries from all places together; wait a while (closes). `data.retry_after_ms` says how long. A place is an address without its port; IPv6 addresses in the same /64 count as one |
| `hello_first` | Another method before the handshake (does not close) |
| `setup_first` | Connected with a one-time code and called something other than `human.get` and `account.setup` |

## `subscribe`, `unsubscribe`

Subscribe to a push stream: from now on its pushes go to this connection.

**Who can call**: people, extensions; extensions need the `events.read` capability for the `events`, `view` and `sessions` streams, and the `projects`, `schedules`, `notices` and `package` streams are for people only (`local_only`).

**Params**

| Name | Type | Notes |
|---|---|---|
| `stream` | string, required | Which stream, see below |
| `session` | string | Required for `events` and `view`; not allowed for `config` and `sessions`; optional for `memory` (that session's memory) |
| `persona` | string | `memory` only: this persona's memory |
| `after` | non-negative integer | `events` and `memory` only: first replay everything with a higher sequence number; `0` means from the start |

| `stream` | Pushes | `subscribe` response |
|---|---|---|
| `events` | A session's events, as-is | The session's state right now: `model`, `limits`, `permission`, `usage`, `jobs`, `workspace`, `default_workspace` (the session's default workspace), `compactable` (whether there is anything to compact right now, that is, whether `session.compact` would answer `nothing_to_compact`; changes are pushed as `compactable.changed`), and when present `persona`, `preset`, `todos`, `goal`, `nested` (running `miyu call`s: `[{id, name, args, at}]`, `at` being when each started); with `after` also `upto` |
| `view` | A session's view projection (entries ready to draw) | The above, plus the latest page of entries and the full session `status` |
| `config` | Config changes | `{}` |
| `sessions` | One item of the session list changed | `{"sessions": [<item>…]}`, all of them |
| `extensions` | An extension's status changed | `{"extensions": […]}` |
| `packages` | One item of the package list changed | `{"packages": […]}` |
| `projects` | A project changed | `{"projects": […]}`, all of them |
| `schedules` | A schedule changed or ran (construction J-1 lower) | `{"schedules": […]}`, all of them |
| `notices` | Notices for people (construction J-3 middle): a new-session schedule's run has ended, the schedule table broke and was replaced by an empty one. Heads show system notifications from it | `{}`; earlier ones are not replayed |
| `package` | With `package`: what that package pushes to people (construction IM-1 lower-middle 2); its pages update from it instead of polling. A package that isn't installed gets `unknown_package` | `{}`; earlier ones are not replayed |
| `kb` | Index-building progress of knowledge bases: `kb.indexing`, `kb.indexed` | `{}`: the current state comes from `kb.list` |
| `memory` | One memory's log | `{"upto": <last sequence number at that moment>}` |
| `widgets` | Widgets | See [Packages and commands](packages.en.md) |

`model` in the `events` response is what the session asks next: `ref` (its model or `@pool`), `endpoint` and `model` (who gets the next request; a rotating pool has neither), and `effort` `{"level", "from"}`: the reasoning effort that model uses, `from` being `session` (set with `session.configure`, step SM-1) or `system` (the config). A rotating pool, or a request that carries no effort, has no `effort`.

What each stream pushes is in [Pushes](pushes.en.md).

1. A connection has one subscription per session (`events` and `view` count as the same); subscribing again replaces it. `sessions`, `config`, `extensions`, `packages`, `projects`, `schedules`, `notices`, `kb` and `widgets` are one each per connection; `package` is one per package.
2. **Replay**: with `after`, the events with a higher sequence number that were on disk when you subscribed are pushed one by one, then the response (with `upto`), then everything from `upto + 1` on, with nothing lost or repeated. If a model request is streaming at that moment, its deltas so far (`model.delta`) are pushed once more after the replay so the live ones that follow line up. Transient events without sequence numbers are not replayed.
3. **Falling behind**: if you read too slowly, a session keeps at most 1024 pushes per subscription (16 for config, 64 for memory); beyond that it pushes `resync` and the subscription stops. A stopped session also pushes `resync`. Subscribe again, with the last sequence number you saw as `after`, to catch up.
4. Being subscribed to a session counts as watching it: in a session nobody is watching, a subagent's report is recorded without waking her.
5. `unsubscribe`: same `stream` and `session`/`persona`; stops forwarding and returns `{}`, also when you weren't subscribed.

```json
→ {"jsonrpc":"2.0","id":"s1","method":"subscribe","params":{"session":"01a14aca-…","stream":"events","after":30}}
← (events 31 to 42 are pushed first)
← {"id":"s1","jsonrpc":"2.0","result":{"limits":{"compaction_line":850000,"window":1000000},"model":{"endpoint":"deepseek","model":"deepseek-v4","ref":"deepseek/deepseek-v4"},"upto":42,…}}
```

**Errors**

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | Unknown `stream`; `session` missing where required or given where not allowed; `after` not a non-negative integer, or not accepted by this stream |
| `session_not_found`, `session_broken`, `session_stopped` | The session doesn't exist, can't be read, or stopped (subscribing again reloads it) |
| `memory_not_installed`, `memory_unavailable` | `memory` stream: memory isn't installed, or that place has no memory |

## `account.setup_code`, `account.setup`, `account.logout`

The access password is only needed to connect from the local network (a phone, a browser on another computer); local heads use the local token. Sign in once from the network with the access password and the browser keeps a device token for 30 days.

**Who can call**: people. `account.setup_code` only on a connection made with the local token; `account.setup` and `account.logout` on any (a device-token connection changing the access password must give the current one, see below).

| Method | Params | Result |
|---|---|---|
| `account.setup_code` | none | `{"code", "expires", "first"}`: the one-time code, when it expires, and whether no access password has been set yet |
| `account.setup` | `password`, `old`? | `{}` on a local-token connection; otherwise `{"login": {"expires", "token"}}`: a new device token, which this connection goes by from then on |
| `account.logout` | `all` (boolean, default `false`) | `{"revoked": <how many>}`: revokes this device token, or all of them with `all` |

A one-time code works once. Setting or changing the access password revokes every device and closes their connections (except the one making the change). The password is at most 1024 bytes and must not be empty or only whitespace; there is no minimum length. A device-token connection (a web page opened from `miyu web`, or a LAN login) must give the current password as `old` once one is set; it is throttled by source, the same as password login. Before a password is set it needn't. Local-token and one-time-code connections never need it.

**Errors**

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | Wrong params; the password is not acceptable; a device-token connection changing the access password without `old` |
| `bad_password`, `login_throttled` | A device-token connection changing the access password gave the wrong `old`; this source got it wrong 5 times within 60 seconds, or all sources together tried 30 times (`data.retry_after_ms`) |
| `local_only` | An extension called it; `account.setup_code` not on a local-token connection |

## `core.stop`

Asks the core to stop (DT-4 下): the desktop app's tray "Quit" uses it to close the core too.

**Who can call**: people, only on a connection made with the local token; a remote one gets `local_machine_only`.

| Method | Params | Result |
|---|---|---|
| `core.stop` | none | `{}`: once the reply is out, the core stops as on a stop signal: a turn halfway through is recorded as restarted and carries on next time it loads, running background commands are recorded `restarted` and then killed, extensions exit, and the socket is let go. The run log says `stopped reason=request` |

When the core stops, every connection closes. Whoever connects next (a terminal, a web tab reconnecting) starts the core again as usual.

**Errors**

| `data.reason` | When |
|---|---|
| `local_only` | An extension called it; a connection not made with the local token (device token, access password, handoff ticket) called it |
| `local_machine_only` | A remote connection called it |

## Your avatar

One PNG, JPEG or WebP, at most 1 MiB and 1024 pixels wide or tall (as for persona avatars).

| Method | Params | Result |
|---|---|---|
| `account.set_avatar` | `blob` (upload with `blob.put` first) or `unset: true`, optionally `expect` (the version you last read, `null` for none) | `{"avatar": <version or null>}`; a mismatched `expect` gets `avatar_conflict` (`data` carries the current version); not a picture `avatar_not_image`, too big `avatar_too_big` |
| `account.avatar` | none | `{"avatar", "media_type", "data"}` (version, media type, base64), or `null` when there is none |

## `login.ticket`, `login.redeem`

Hand-off tickets: a signed-in person jumps from one page to another program's page (from the web app to the messaging page, or `miyu web` opening the web app) without signing in again. Before the jump, ask for a ticket and put it in the URL; the other side connects with it in one of two ways:

- Give `ticket` in the handshake (see the credentials above): connected in one step; the reply carries the new device token. Pages that connect to the core straight from the browser use this.
- Redeem it with `login.redeem` for a device token and connect to the core with that token (`connect_login` in `miyu-client`, over the local socket, `login` in the handshake; a page's own sign-in form uses `connect_password`, which returns the login token it got). The older way; it still works.

**Who can call**: `login.ticket` is for people only (connected with the local token, as the terminal does, or a device token, a password or a hand-off ticket); extensions started by the core get `local_only`. Anyone holding a ticket can call `login.redeem`.

| Method | Params | Result |
|---|---|---|
| `login.ticket` | none | `{"ticket", "expires"}`: a 64-hex-digit ticket, valid for 60 seconds |
| `login.redeem` | `ticket` | `{"login": {"expires", "token"}}`: a new device token (30 days) |

A ticket works once, whether given in a handshake or redeemed. At most 16 exist at a time; the oldest is dropped. Tickets live only in the core's memory and die with a restart.

**Errors**

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | `login.redeem` without `ticket` |
| `local_only` | An extension asked for a ticket |
| `bad_ticket` | No such ticket, expired, or already used |

