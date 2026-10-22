# Connection: connecting, hello, subscriptions

Chinese version: [connection.md](connection.md).

| Method | What it does |
|---|---|
| [`hello`](#hello) | Handshake: who you are, your credentials |
| [`subscribe`, `unsubscribe`](#subscribe-unsubscribe) | Subscribe to or leave a push stream |
| [`account.setup_code`, `account.setup`, `account.logout`](#accountsetup_code-accountsetup-accountlogout) | One-time code for web login, setting the username and password, revoking logins |
| [`account.invite`, `account.register` and the rest](#members) | Members: invite codes, registering, listing, disabling |
| [`account.invite`, `account.register` and the rest](#members) | Members: invite codes, registering, listing, disabling |

## Connecting

- **Local heads**: the core listens on a local socket only you can reach. Its path is in `run/socket` under the data root (one line, the path as-is) and the local token is in `run/token` (64 lowercase hex digits). Linux and macOS use a Unix domain socket, Windows a named pipe `\\.\pipe\miyu-<fingerprint>`. If no core is running, the head starts `miyu core` itself.
- **Extensions**: programs the core launches from a package manifest speak the same protocol over stdin and stdout, with no credentials (any given in `hello` are ignored).
- **Web**: the browser talks to the web app, which connects to the core like a local head. For a connection it opens on behalf of a remote person, the web app first writes the notification `{"jsonrpc":"2.0","method":"connection.remote","params":{"from":<address>}}` before the hello, then passes the browser's bytes through: that connection is remote (Y-1 下). It only goes one way: once marked it never becomes local again, and the notification is ignored after the hello.
- **Local machine only** (Y-1 下): reading files on the core's machine, handing over local paths and picking local folders are for local connections only; a remote one gets `local_machine_only` and nothing happens: `fs.*`, `session.terminal`, `session.set_terminal`, `account.setup_code`; `blob.put` and `package.install` with `path`, `kb.mount` with `folder`, `mcp.set` with `command`, `check` with `file` or `cwd`, `config.trust` and `config.get` with `cwd`; a session's `cwd` and `dirs`, an attachment's `path`, `command.run`'s `cwd` and `/workspace <path>`, a project's `workspace`. A session created without `cwd` works in its own default workspace. Remote connections get no `host` in the hello reply.

## One message per line

- Each message is one line of JSON, UTF-8, ending in `\n` (`\r\n` is accepted when reading). At most 1 MiB per line; a longer line gets `parse_error` and the connection closes.
- The core writes compact JSON with object fields sorted by name.

**Requests** are JSON-RPC 2.0: `jsonrpc` is `"2.0"`, `method` the method name, `params` an object (omitted means `{}`, an array gets `bad_params`), `id` a string of 1 to 128 bytes with no control characters. A message without `id` is a notification and the core ignores it, except `connection.remote` before the hello (see **Web** above). Unknown params are ignored; `null` means omitted.

**Responses**: success in `result`; refusal in `error`: `code` is the error code, `message` a sentence for people (in the connection's language), `data.reason` the reason code for programs, sometimes with more fields.

```json
{"id":"c7","jsonrpc":"2.0","result":{"events":[41]}}
{"error":{"code":-32010,"data":{"reason":"session_not_found"},"message":"Session not found."},"id":"c7","jsonrpc":"2.0"}
```

**Pushes** are notifications from the core: `{"jsonrpc":"2.0","method":<name>,"params":{…}}`, no `id`. See [Pushes](pushes.en.md) for what each stream pushes.

**Ordering**:

1. Requests on one connection are handled one at a time: the next line is read after the previous one is answered. A few slow ones are answered in the background without blocking later requests, matched by `id`: `model.call`, `memory.dream`, `link.preview`, `fs.find` and `job.output` with `wait`, and the slow cases of `command.run` (such as `/dream`).
2. Results before responses: the pushes a command causes always arrive before its response.
3. Sending the same `id` again (within a session's last 1024): you get the previous response and nothing is done twice. Refused ones are not remembered; sending again is judged again.

## `hello`

The handshake. It must be the first message; before it every other method gets `hello_first`. A connection that hasn't completed the handshake within 10 seconds is closed.

**Who can call**: people, extensions.

**Params**

| Name | Type | Notes |
|---|---|---|
| `protocol` | `[lowest, highest]`, required | The protocol major versions you support; the core only has `1` |
| `head` | `{"kind", "version"}`, required | What you are and which version; only logged |
| `locale` | string | Your system's language (`zh-CN`, `en_US.UTF-8` both work). Sets the connection's language when the `ui.language` setting is `auto` |
| `caps` | object | Only `input` (boolean) is read: whether you can let a person type |
| `token` | string | The local token |
| `code` | string | A one-time code (first web password, password reset) |
| `login` | string | A login token (a remembered web login) |
| `user`, `password` | string | Web username and password, together; the admin and members both use them |
| `invite` | string | Invite code: someone not registered yet connects with it and can only register |

Give exactly one kind of credential: `token`, `code`, `login`, `invite`, or `user` plus `password`. Extensions give none. The reply's `account` is the account recognized: the one the login token or username belongs to; a connection made with an invite code hasn't registered yet, so it is `null`, with `"register": true`.

**Response**

| Name | Notes |
|---|---|
| `protocol` | The chosen major version: `1` |
| `core` | `{"version"}`: the core's version |
| `account` | Who you are: local people are `admin`; an extension of a package that declares a system account is that account (named after the package), other extensions are `admin` too |
| `host` | `{"home", "platform", "workspace"}`: the system home directory (`null` if unknown), `linux`/`macos`/`windows`, this account's default workspace. Not given to remote connections (Y-1 下) |
| `language` | `zh`, `en` or `ja`: the language for text from now on. If the setting changes, the next message follows it without a new handshake |
| `view` | The view projection version, currently `1` (see `view.page` in [Sessions](sessions.en.md)) |
| `sandbox` | `{"usable": true}`, or `{"usable": false, "reason"}`: `helper_missing`, `helper_failed`, `no_mechanism` |
| `config_errors` | How many errors the config has right now; omitted when none |
| `config` | Extensions only: the final values of this package's own settings (secrets as real values). Later changes arrive as `extension.config` |
| `setup` | `true`: connected with a one-time code; can only set the username and password |
| `login` | `{"expires", "token"}`: only when connected with username and password; the web app keeps it for next time |

```json
→ {"jsonrpc":"2.0","id":"h1","method":"hello","params":{"protocol":[1,1],"head":{"kind":"tui","version":"0.1.0"},"locale":"en-US","token":"3f9a…"}}
← {"id":"h1","jsonrpc":"2.0","result":{"account":"admin","core":{"version":"0.0.1"},"host":{"home":"/home/me","platform":"linux","workspace":"/home/me/.miyu/home/admin/workspace"},"language":"en","protocol":1,"sandbox":{"usable":true},"view":1}}
```

Sending `hello` again after the handshake is fine: if it passes, the new language and capabilities apply; if it is refused and the connection stays open, the previous ones stay.

**Errors** (those marked "closes" close the connection after answering)

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | A required field is missing or has the wrong type; more than one kind of credential, or only one of `user`/`password` (closes) |
| `protocol_mismatch` | `1` is not in your range (closes) |
| `bad_token` | No credential, or a wrong local token (closes) |
| `bad_code`, `bad_login`, `bad_password` | Wrong one-time code, login token, username or password (closes); a wrong password for a disabled account is also `bad_password` |
| `account_disabled` | Right username and password, but the account is disabled (closes) |
| `bad_invite` | Wrong, expired, used or revoked invite code (closes) |
| `login_throttled` | Too many wrong passwords; wait a while (closes) |
| `hello_first` | Another method before the handshake (does not close) |
| `setup_first` | Connected with a one-time code and called something other than `human.get` and `account.setup` |
| `register_first` | Connected with an invite code and called something other than `human.get` and `account.register` |
| `not_permitted` | A member's connection called something other than `human.get` and `account.logout` (for now members can only manage their own account; permissions open up step by step) |

## `subscribe`, `unsubscribe`

Subscribe to a push stream: from now on its pushes go to this connection.

**Who can call**: people, extensions; extensions need the `events.read` capability for the `events`, `view` and `sessions` streams, and the `projects` stream is for people only (`local_only`).

**Params**

| Name | Type | Notes |
|---|---|---|
| `stream` | string, required | Which stream, see below |
| `session` | string | Required for `events` and `view`; not allowed for `config` and `sessions`; optional for `memory` (that session's memory) |
| `persona` | string | `memory` only: this persona's memory |
| `after` | non-negative integer | `events` and `memory` only: first replay everything with a higher sequence number; `0` means from the start |

| `stream` | Pushes | `subscribe` response |
|---|---|---|
| `events` | A session's events, as-is | The session's state right now: `model`, `limits`, `permission`, `usage`, `jobs`, `workspace`, `default_workspace` (the session's default workspace), `compactable` (whether there is anything to compact right now, that is, whether `session.compact` would answer `nothing_to_compact`; changes are pushed as `compactable.changed`), and when present `persona`, `preset`, `todos`, `goal`, `nested` (running `miyu call`s: `[{id, name}]`); with `after` also `upto` |
| `view` | A session's view projection (entries ready to draw) | The above, plus the latest page of entries and the full session `status` |
| `config` | Config changes | `{}` |
| `sessions` | One item of the session list changed | `{"sessions": [<item>…]}`, all of them |
| `extensions` | An extension's status changed | `{"extensions": […]}` |
| `packages` | One item of the package list changed | `{"packages": […]}` |
| `projects` | One of this account's projects changed | `{"projects": […]}`, all of them |
| `kb` | Index-building progress of this account's knowledge bases: `kb.indexing`, `kb.indexed` | `{}`: the current state comes from `kb.list` |
| `memory` | One memory's log | `{"upto": <last sequence number at that moment>}` |
| `widgets` | Widgets | See [Packages and commands](packages.en.md) |

What each stream pushes is in [Pushes](pushes.en.md).

1. A connection has one subscription per session (`events` and `view` count as the same); subscribing again replaces it. `sessions`, `config`, `extensions`, `packages`, `projects` and `widgets` are one each per connection.
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

Web login. There is one account, `admin`; the first time the web app opens, it sets a username and password for it, and from then on the browser logs in with them and stays logged in for 30 days.

**Who can call**: people. `account.setup_code` only on connections that showed the local token; `account.setup` on connections that used a one-time code; `account.logout` on any.

| Method | Params | Response |
|---|---|---|
| `account.setup_code` | none | `{"code", "expires", "first"}`: the one-time code, when it expires, whether no password has been set yet |
| `account.setup` | `username`, `password` | `{"username", "login": {"expires", "token"}}`: set, and a login token in exchange |
| `account.logout` | `all` (boolean, default `false`) | `{"revoked": <how many>}`: revokes this login token, or all of them |

A one-time code works once. Setting the password revokes every earlier browser login.

**Errors**

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | Wrong params; username or password not acceptable |
| `local_only` | Called by an extension, or `account.setup_code` on a connection that didn't use the local token |
## Members

Several people can use one Miyu: the admin hands out invite codes, and whoever has one registers an account of their own (a member). Each person's sessions are visible only to them.

**Who can call**: `account.register` on connections made with an invite code; the rest only the admin, members get `not_permitted`.

| Method | Params | Response |
|---|---|---|
| `account.invite` | `days` (1 to 30, default 7) | `{"code", "id", "expires"}`: the code is given only here, later only its id is listed |
| `account.invites` | none | `{"invites": [{"id", "created", "expires", "used"?}]}`: usable ones and recently used ones (`used` is `{"account", "at"}`) |
| `account.invite_revoke` | `id` | `{"revoked": true}`: revokes one not yet used |
| `account.register` | `username`, `password` | `{"account", "username", "login": {"expires", "token"}}`: registered; this connection is now that account |
| `account.list` | none | `{"accounts": [{"account", "kind", "username"?, "created"?, "disabled"?}]}`: `kind` is `admin`, `member` or `system` |
| `account.disable`, `account.enable` | `account` | `{"account", "disabled"}`: disabling revokes all its logins and closes its connections; data stays |

The username is the account id: starts with a lowercase letter, only lowercase letters, digits and `-`, at most 32; it can't be the admin's, an existing account's or a system account's, nor `users`. Invite codes are single-use; if registering fails (say the username is taken) the code stays usable.

**Errors**

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | Wrong params; username or password not acceptable |
| `username_taken` | The username is taken |
| `unknown_invite` | No such invite code, or already used |
| `unknown_account` | No such account, or it isn't a member |
| `not_permitted` | Not the admin |

## `login.ticket`, `login.redeem`

Hand-off tickets: a signed-in person jumps from one page to another program's page (from the web app to the messaging page) without signing in again. Before the jump, ask for a ticket and put it in the URL; the other side redeems it for a login token and connects to the core with that token (`connect_login` in `miyu-client`, over the local socket, `login` in the handshake; a page's own sign-in form uses `connect_password`, which returns the login token it got).

**Who can call**: `login.ticket` is for people only (connected with the local token, a login token or a password); extensions started by the core get `local_only`. Anyone holding a ticket can call `login.redeem`.

| Method | Params | Result |
|---|---|---|
| `login.ticket` | none | `{"ticket", "expires"}`: a 64-hex-digit ticket, valid for 60 seconds, bound to this connection's account |
| `login.redeem` | `ticket` | `{"account", "login": {"expires", "token"}}`: the ticket's account and a new login token (30 days) |

A ticket works once. At most 16 exist at a time; the oldest is dropped. Tickets live only in the core's memory and die with a restart.

**Errors**

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | `login.redeem` without `ticket` |
| `local_only` | An extension asked for a ticket |
| `bad_ticket` | No such ticket, expired, or already used |

