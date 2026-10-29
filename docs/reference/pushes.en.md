# Pushes

Pushes are notifications the core sends on its own: `{"jsonrpc": "2.0", "method": <name>, "params": {…}}`, no `id`, no response expected. They only go to connections that subscribed (see [Connection](connection.en.md)); extensions also receive a few notifications meant for them, see [Extensions](extensions.en.md). Chinese version: [pushes.md](pushes.md).

| Push | Stream | `params` |
|---|---|---|
| `event` | `events` | `{"session", "event"}`: one event of the session, as-is (see [Events](events.en.md)); replays use it too |
| `view.add`, `view.update`, `view.append`, `view.hidden`, `view.remove`, `view.status` | `view` | See [View stream](view.en.md) |
| `sessions.changed` | `sessions` | `{"session", "entry"}`: one item of the session list, replaced whole (as in `session.list`); deleted ones are `{"session", "removed": true}`. When a child changes, its parent and ancestors (whose `tokens` include their descendants) are pushed too, level by level |
| `config.changed` | `config` | `{"layer", "via", "by"?, "version", "keys", "problems"}`: a config layer changed; `keys` as in `config.set`'s response; `via` is `set`, `edit`, `file` (edited by hand), `package` (after installing or removing packages) |
| `extension.changed` | `extensions` | `{"entry"}`: one extension's status, as in `extension.status` |
| `package.changed` | `packages` | `{"package", "entry"}`: one item of the package list; `entry` is `null` when gone |
| `package.progress` | No subscription: sent only to the connection that called `package.install` (not a preview) | `{"package", "stage", "index", "count", "done"?, "total"?}`: `stage` is `download` (with `done` and `total`, bytes downloaded so far and in all, once at the start and the end, at most every 200 ms in between), `verify` (before checking) or `install` (before installing); `index` and `count` say which one of how many. A package folder or package file gets one `verify` and one `install`. Older cores push only `{"package", "done", "total"}`: treat a missing `stage` as `download` |
| `projects.changed` | `projects` | `{"project": <item>}`: a project, replaced whole (same as an item of `project.list`); a deleted one is `{"project": <id>, "deleted": true}` |
| `schedules.changed` | `schedules` | `{"id", "entry": <item>}`: a schedule, replaced whole (same as an item of `schedule.list`; also pushed after it runs); deleted ones and finished one-offs `{"id", "removed": true}` (construction J-1 lower) |
| `package.event` | `package` | `{"package", "event", "data"}`: what a package pushed to people with `package.emit` (construction IM-1 lower-middle 2); the package decides the shape. Not stored: connections not subscribed miss it; a lagging one gets `resync {"stream": "package", "package"}` |
| `notice` | `notices` | `{"kind", "at", …}`: a notice for people (construction J-3 middle); heads show a system notification by `kind`. `schedule.ran`: `{"schedule", "session", "ok"}`, a new-session schedule's run has ended; `ok` means it finished (errors and interruptions are `false`); schedules running in an existing session send none. `schedules.reset`: the schedule table broke, was set aside, and an empty one replaced it, so the schedules are gone. Not stored: connections not subscribed miss it |
| `kb.indexing` | `kb` | `{"kb", "done", "total"}`: a knowledge base is building its index; `done` of the `total` documents to rebuild are done. The first is 0, the last is always sent, at most one every 0.1 s in between |
| `kb.indexed` | `kb` | `{"kb"}`: the build finished; read that base again with `kb.list` |
| `mcp.changed` | `packages` | `{"name", "server"}`: one MCP server in the system config; `server` is `null` when removed |
| `widget.changed` | `widgets` | `{"package", "id", "widget"}`: one widget replaced whole; `null` when turned off or removed |
| `memory.event` | `memory` | `{"event", "persona"? | "session"?}`: a new line in one memory's log |
| `memory.dreaming` | `memory` | `{"upto", "kind", …, "persona"? | "session"?}`: while organizing, a revision or removal she just finished writing, or how far the summary is; not logged, not replayed (see [`memory.dream`](memory.en.md#memorydream)) |
| `resync` | all | `{"stream", "session"?}`: fell behind, or the session stopped: this subscription stopped; subscribe again |

**Ordering**

1. Pushes a command causes always arrive before its response.
2. Pushes carry whole items (`entry`): repeated or late ones are fine when replaced by id.
3. A widget is pushed at most every 250 ms; only the newest content in between is kept.

**Falling behind**: the core doesn't wait for slow heads. A session keeps at most 1024 pushes per subscription, config 16, memory 64; beyond that it pushes `resync` and stops the subscription. Subscribe again: for sessions and memory, pass the last sequence number you saw as `after` to catch up; other streams' subscription responses are complete anyway.
