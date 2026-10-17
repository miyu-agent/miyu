# Events

A session's truth is an append-only event log. Heads subscribed to the `events` stream receive them one by one (push `event`); `view.page` without `view` returns them too. Heads that only want to draw can use the [view stream](view.en.md) and skip working things out from events. Chinese version: [events.md](events.md).

## Envelope

```json
{"seq": 42, "at": "2026-10-01T06:03:12.345Z", "kind": "message.user", "turn": 41, "by": {"kind": "person", "account": "admin"}, "cause": "m1", "body": {…}}
```

| Name | Notes |
|---|---|
| `seq` | Sequence number from 1, no gaps or repeats within a session |
| `at` | Time, UTC, milliseconds |
| `kind` | Kind, see below |
| `turn` | Which turn (that turn's `turn.started` number); omitted when it belongs to none |
| `by` | Who: `person` (`account`; `via` when recognized on a platform), `external` (a person on a platform: `venue`, `id`, `role`, `account`?), `model`, `kernel`, `tool` (`call_id`), `module` (`id`, e.g. a package), `harness` (`name`), `session` (another session) |
| `cause` | Which command caused it: the request `id` |
| `body` | Fields of this kind |

The exact shape of each kind is in the repository's `docs/designs/samples/events/<kind>.jsonl`, one file per kind, each a real line.

## Recorded events

| `kind` | Meaning |
|---|---|
| `session.created` | The session was created (event 1): venue, working directory, persona, preset, model… |
| `session.policy_changed` | Permissions, model or the whole policy changed |
| `session.workspace_changed` | The working directory changed |
| `session.meta_changed` | Title or pin changed; titles the kernel gives unnamed sessions too |
| `session.recapped` | A recap (not in her context) |
| `session.sealed` | A forked session ran its turns and was sealed |
| `command.ran` | A person used a slash command (not in her context) |
| `turn.started`, `turn.ended` | A turn began or ended; `turn.ended`'s `reason` is `completed`, `interrupted`, `error`, `restarted` and so on |
| `turn.joined` | Recorded messages joined the running turn |
| `turn.reverted`, `turn.unreverted` | Turns were undone; the last undo was restored |
| `message.user` | A message from a person (or another session, another agent, someone on a platform) |
| `message.withdrawn` | Queued messages she hadn't heard yet were handed back |
| `message.assistant` | One complete model response: text, thinking, tool calls |
| `model.called` | One model request: which model, usage, cost, `error` on failure |
| `tool.result` | One tool call's result: `status` is `ok`, `error`, `denied`, `cancelled`, `skipped` |
| `tool.approval_requested`, `tool.approval_decided` | A person was asked to approve a call; their decision |
| `question.asked`, `question.answered` | She asked a set of questions; the person's answers |
| `context.injected` | A block of facts injected into the context (memory, something an extension returned…) |
| `context.compacted` | A compaction checkpoint; clearing the context is one too (`trigger` is `clear`) |
| `context.compaction_paused` | Automatic compaction was paused |
| `job.reported` | A background command ended |
| `child.reported` | A subagent's report |
| `peer.idle` | A session being waited on went idle, or can't be waited on |
| `image.described` | A description of an image (seen by the vision model for a model that can't see images) |
| `todos.cleared` | The to-do list was cleared |
| `goal.set`, `goal.changed`, `goal.paused`, `goal.resumed`, `goal.ended` | A standing goal was set, changed, paused, resumed, ended |
| `venue.recalled`, `venue.delivered` | Someone recalled a message on the platform; her message reached the platform |
| `ext.<package>.<name>` | Recorded by an extension (`events.append`) |

Undo, compaction and clearing never delete events: heads work out what's in effect themselves (or simply use the view stream).

## Transient events

Not recorded, no `seq`, only pushed to heads subscribed at that moment, never replayed (except that a model request streaming when you subscribe gets its `model.delta`s so far replayed once). Their shapes are in `docs/designs/samples/transient/`.

| `kind` | Meaning |
|---|---|
| `model.delta` | A piece of model output: `seen`, `index` (which block), plus one of `start` (with `name`, the block kind), `text`, `end` |
| `tool.progress` | A piece of a running tool's output: `call_id`, `text` |
| `status` | A request is retrying or switched provider |
| `model.changed` | The model or limits for the next requests changed: `why` is `failover` or `turn` |
| `compaction.started`, `compaction.progress`, `compaction.done` | A compaction started (one prepared in the background), how far the summary is, done |
| `todos.changed` | The whole current to-do list |
| `goal.changed` | The whole standing goal right now |
