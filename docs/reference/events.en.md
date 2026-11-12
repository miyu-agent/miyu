# Events

A session's truth is an append-only event log. Heads subscribed to the `events` stream receive them one by one (push `event`); `view.page` without `view` returns them too. Heads that only want to draw can use the [view stream](view.en.md) and skip working things out from events. Chinese version: [events.md](events.md).

## Envelope

```json
{"seq": 42, "at": "2026-10-01T06:03:12.345Z", "kind": "message.user", "turn": 41, "by": {"kind": "person"}, "cause": "m1", "body": {…}}
```

| Name | Notes |
|---|---|
| `seq` | Sequence number from 1, no gaps or repeats within a session |
| `at` | Time, UTC, milliseconds |
| `kind` | Kind, see below |
| `turn` | Which turn (that turn's `turn.started` number); omitted when it belongs to none |
| `by` | Who: `person` (you; `via` when recognized on a platform), `external` (a person on a platform: `venue`, `id`, `role`; `owner: true` when the bindings recognize you), `model`, `kernel`, `tool` (`call_id`), `module` (`id`, e.g. a package), `harness` (`name`), `session` (another session) |
| `cause` | Which command caused it: the request `id` |
| `body` | Fields of this kind |

The exact shape of each kind is in the repository's `docs/designs/samples/events/<kind>.jsonl`, one file per kind, each a real line.

## Recorded events

| `kind` | Meaning |
|---|---|
| `session.created` | The session was created (event 1): venue, working directory, persona, preset, model… |
| `session.policy_changed` | Permissions, model, reasoning effort (`effort: {"level"}`, `null` goes back to the config), context limit (`context: {"tokens"}`, `null` removes it), vision model (`vision`, empty when removed), compaction line (`compaction_line: {"percent"}`, `null` removes it) or the whole policy changed |
| `session.workspace_changed` | The working directory changed |
| `session.meta_changed` | Title, pin or project changed (`project`: an id, empty when moved out); titles the kernel gives unnamed sessions too |
| `session.recapped` | A recap (not in her context) |
| `session.sealed` | A forked session ran its turns and was sealed |
| `command.ran` | A person used a slash command (not in her context) |
| `turn.started`, `turn.ended` | A turn began or ended; `turn.ended`'s `reason` is `completed`, `interrupted`, `error`, `restarted` and so on |
| `turn.joined` | Recorded messages joined the running turn |
| `turn.reverted`, `turn.unreverted` | Turns were undone; the last undo was restored |
| `message.user` | A message from a person (or another session, another agent, someone on a platform); a command the person typed with `!` has `shell: true`, its `blocks` being the command line. A path-only attachment (`session.send`, item 3) is a `{"type": "path", "name", "path", "size"?}` block: no blob, `size` being the byte count the head gave |
| `message.withdrawn` | Queued messages she hadn't heard yet were handed back |
| `message.assistant` | One complete model response: text, thinking, tool calls. When the model went on to make up the next turn after its answer (a role word at the start of a line, followed by an imitation of the user-side tags or the group message line of this request), that part was cut before the event was written and only what is left is here; the streamed `model.delta` pieces are unchanged, so treat this event as final |
| `model.called` | One model request: which model, usage, cost, `error` on failure. When the reply was cut it carries `cut`, one item per text block: `[{"block", "at", "bytes", "role"}]` (`block` is the block number, the same as `index` in `model.delta`; `at` is the byte offset in that block's original text where the cut starts; `bytes` is how many bytes were cut; `role` is the role word: `user`, `assistant`, `system`, `human`, `tool`). The cut text itself is not recorded; absent when nothing was cut |
| `tool.result` | One tool call's result: `status` is `ok`, `error`, `denied`, `cancelled`, `skipped` |
| `tool.nested` | A `miyu call` from a script finished: `id` (`call_12_1#3` under a foreground call, `j7#3` under a background job), `name`, `args`, `status` (`ok`, `error`, `denied`, `needs_approval`, `cancelled`), and when present `duration_ms`, `result` (blob), `bytes`, `reason`. Not in her context; ones under a job carry no `turn` |
| `tool.approval_requested`, `tool.approval_decided` | A person was asked to approve a call; their decision. A request with `expires_at` (a time) is denied automatically if no one confirms by then: a decision with `decision` `deny` and `by` `kernel` is recorded, and she gets a "no one confirmed" result and carries on; heads draw a countdown from it. With the config `permission.confirm.timeout` at `0` it waits forever and the field is absent |
| `question.asked`, `question.answered` | She asked a set of questions; the person's answers |
| `context.injected` | A block of facts injected into the context (memory, something an extension returned…) |
| `context.compacted` | A compaction checkpoint; clearing the context is one too (`trigger` is `clear`). When the summary request was too long and the oldest groups had to be cut, it carries `truncated` (how many groups: the earliest part is not in the summary); left out when nothing was cut |
| `context.compaction_paused` | Automatic compaction was paused |
| `job.reported` | A background command ended; `reason` `too_chatty` means a watched command printed 120 batches within an hour and was stopped |
| `shell.ran` | A command the person typed with `!` finished: `message` (the number of that message), `exit` or `signal`, `duration_ms`, `text` (the output, stdout and stderr together; a long one keeps the head and tail with a line in the middle saying how much was left out), `omitted`; plus `reason` if it ended abnormally: `timed_out` (ran for 120 seconds), `interrupted` (the person interrupted), `aborted` (the core restarted or crashed; not run again) |
| `job.lines` | A watched background command (`shell` with `watch`) printed a batch of lines: `job`, `text` (at most 4000 characters; a cut one has a line in the middle saying how many were left out), `lines`, `omitted`. No `turn` |
| `child.reported` | A subagent's report |
| `peer.idle` | A session being waited on went idle, or can't be waited on |
| `image.described` | A description of an image (seen by the vision model for a model that can't see images) |
| `todos.cleared` | The to-do list was cleared |
| `package.state` | A package's program set its state in this session (`{"package", "state"}`, see [Extensions](extensions.en.md)) |
| `goal.set`, `goal.changed`, `goal.paused`, `goal.resumed`, `goal.ended` | A standing goal was set, changed, paused, resumed, ended |
| `dream.ended` | A memory consolidation run finished: how many memories it added, revised and retired, whether it wrote the summary, whether it was stopped |
| `venue.recalled`, `venue.delivered` | Someone recalled a message on the platform; her message reached the platform |
| `ext.<prefix>.<name>` | Recorded by an extension (`events.append`; the prefix is the manifest's `prefix`, or the package id without it) |

Undo, compaction and clearing never delete events: heads work out what's in effect themselves (or simply use the view stream).

## Transient events

Not recorded, no `seq`, only pushed to heads subscribed at that moment, never replayed (except that a model request streaming when you subscribe gets its `model.delta`s so far replayed once). Their shapes are in `docs/designs/samples/transient/`.

| `kind` | Meaning |
|---|---|
| `model.delta` | A piece of model output: `seen`, `index` (which block), plus one of `start` (with `name`, the block kind), `text`, `end` |
| `tool.progress` | A piece of a running tool's output: `call_id`, `text` |
| `tool.nested_started` | A `miyu call` from a script passed the checks and is running: `id`, `name`; it ends with `tool.nested` |
| `status` | A request is retrying or switched provider |
| `model.changed` | The model, reasoning effort or limits for the next requests changed: `effort.from` is `session` (the session's own) or `system` (the config); `why` is `failover` or `turn` |
| `compaction.started`, `compaction.progress`, `compaction.done` | A compaction started (one prepared in the background), how far the summary is, done; `compaction.done` carries the same `truncated` as its `context.compacted` |
| `todos.changed` | The whole current to-do list |
| `package.state_changed` | What heads see of a package's state in this session changed: `{"package", "state": {"widgets"?}}`; a cleared one has `state` `{}` |
| `compactable.changed` | Whether there is anything to compact changed: `{"compactable": <bool>}`, same as `compactable` in the subscribe reply (whether `session.compact` would answer `nothing_to_compact`) |
| `goal.changed` | The whole standing goal right now |
