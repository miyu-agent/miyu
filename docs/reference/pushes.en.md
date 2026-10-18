# Pushes

Pushes are notifications the core sends on its own: `{"jsonrpc": "2.0", "method": <name>, "params": {…}}`, no `id`, no response expected. They only go to connections that subscribed (see [Connection](connection.en.md)); extensions also receive a few notifications meant for them, see [Extensions](extensions.en.md). Chinese version: [pushes.md](pushes.md).

| Push | Stream | `params` |
|---|---|---|
| `event` | `events` | `{"session", "event"}`: one event of the session, as-is (see [Events](events.en.md)); replays use it too |
| `view.add`, `view.update`, `view.append`, `view.hidden`, `view.remove`, `view.status` | `view` | See [View stream](view.en.md) |
| `sessions.changed` | `sessions` | `{"session", "entry"}`: one item of the session list, replaced whole (as in `session.list`); deleted ones are `{"session", "removed": true}` |
| `config.changed` | `config` | `{"layer", "via", "by"?, "version", "keys", "problems"}`: a config layer changed; `keys` as in `config.set`'s response; `via` is `set`, `edit`, `file` (edited by hand), `package` (after installing or removing packages) |
| `extension.changed` | `extensions` | `{"entry"}`: one extension's status, as in `extension.status` |
| `package.changed` | `packages` | `{"package", "entry"}`: one item of the package list; `entry` is `null` when gone |
| `package.progress` | No subscription: sent only to the connection that called `package.install {"source"}` | `{"package", "done", "total"}`: bytes downloaded so far and in all (once at the start and the end, at most every 200 ms in between) |
| `mcp.changed` | `packages` | `{"name", "server"}`: one MCP server in personal settings; `server` is `null` when removed |
| `widget.changed` | `widgets` | `{"package", "id", "widget"}`: one widget replaced whole; `null` when turned off or removed |
| `memory.event` | `memory` | `{"event", "persona"? | "session"?}`: a new line in one memory's log |
| `resync` | all | `{"stream", "session"?}`: fell behind, or the session stopped: this subscription stopped; subscribe again |

**Ordering**

1. Pushes a command causes always arrive before its response.
2. Pushes carry whole items (`entry`): repeated or late ones are fine when replaced by id.
3. A widget is pushed at most every 250 ms; only the newest content in between is kept.

**Falling behind**: the core doesn't wait for slow heads. A session keeps at most 1024 pushes per subscription, config 16, memory 64; beyond that it pushes `resync` and stops the subscription. Subscribe again: for sessions and memory, pass the last sequence number you saw as `after` to catch up; other streams' subscription responses are complete anyway.
