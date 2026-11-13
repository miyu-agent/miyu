# Life sessions: `life.*`

Give a persona a "life session" that keeps running a loop: she decides when to wake next, lives a stretch of her own day when she wakes, and what she saves goes into the persona's memory. One per persona. A life session is an ordinary session opened with the shipped preset `life` (Life Simulation); it wakes on the life row of the schedule table (`loop` is `life`), which nobody else can touch (it is not in `schedule.list`, see [Schedules](schedules.en.md)): only these methods change it. Design: `docs/designs/70-推演人生.md`; how it runs: `docs/blueprint/life.md`. 中文版：[life.md](life.md).

| Method | Does |
|---|---|
| [`life.start`](#lifestart) | Open a life session for a persona; she wakes for the first time right away |
| [`life.status`](#lifestatus) | Where the persona's life session is and whether it is running |
| [`life.pause`](#lifepause) | Pause: she does not wake |
| [`life.resume`](#liferesume) | Resume: she wakes once right away, then on her own schedule |
| [`life.kill`](#lifekill) | End this life: she never wakes again, the session is archived read-only, and its memories are cleared |
| [`life.new`](#lifenew) | End the current life if there is one, then start a new one |

**Who may call**: people only (local and remote heads); an extension started by the core gets `local_only`.

All of them take one parameter: `persona` (string, required), the persona's id. An unknown persona is `unknown_persona`.

**Telling a life session apart**: its log holds one [`life.started`](events.en.md) event; in the session list (`session.list`, the `sessions` stream) its item has `life: true`, which other sessions do not carry. An ended one also holds a [`life.ended`](events.en.md) event, and its item also has `life_ended: true`.

## State

`life.status`, `life.pause` and `life.resume` all answer with:

| Name | Meaning |
|---|---|
| `state` | `none` (the persona has no life session), `running`, `paused`, or `ended` (the latest one has ended and no new one was started) |
| `session` | The life session's id; absent for `none` |
| `next` | When she wakes next; only for `running` |
| `doing` | What she is doing now, as she wrote it the last time she set her next wakeup in the life session: `{"text", "at"}`, where `text` is that line (line breaks turned into spaces, at most 200 characters) and `at` is when she wrote it. Absent if she never wrote one or left it out the last time. Present while paused too |

If the schedule row is gone (for instance the table was broken and replaced by an empty one), the state is `paused`: `life.resume` rebuilds the row from the session log.

What she is doing now is also handed to her other sessions of the same persona (local ones, groups and private chats, forked branches; not subagents) at the start of their next turn, only while the life is running and only once per line.

## `life.start`

Opens a new session with the default model of this moment, the preset `life` and this persona (the title is generated as usual), sets its context cap to 32000 tokens (or to the least the current model needs, if that is more; it can be changed later with `context` in `session.configure`), records the identity in the log, and adds the life row with its first time set to now, so she wakes for the first time right away. Answers `{"session": <id>}`.

- If the persona already has a life session: `life_exists`, with that one in `data.session`. When two connections start the same persona at once, only one session is opened and the other gets `life_exists`. A latest life that has ended does not count: a new one is opened (the old one stays as an archive, its memories already cleared).
- If the session cannot be opened, the refusal is the one `session.create` gives, and nothing is left behind. If setting the context cap or recording the identity fails, the new session is deleted and the refusal is passed on.
- If the identity is recorded but the schedule row cannot be added (for instance there are already 200 schedules: `too_many_schedules`), the refusal carries the session in `data.session`: it already is the persona's life session, `life.status` says `paused`, and `life.resume` picks it up once there is room.

```json
{"jsonrpc": "2.0", "id": "l1", "method": "life.start", "params": {"persona": "miyu"}}
{"jsonrpc": "2.0", "id": "l1", "result": {"session": "<id>"}}
```

## `life.status`

Answers with the state above.

```json
{"jsonrpc": "2.0", "id": "s1", "method": "life.status", "params": {"persona": "miyu"}}
{"jsonrpc": "2.0", "id": "s1", "result": {"state": "running", "session": "<id>", "next": "2026-11-14T10:00:00.000Z"}}
```

## `life.pause`

Pause: she does not wake, and the next time is cleared. Pausing a paused one is fine. Answers with the state above. Without a life session: `no_life`; an ended one: `life_ended`.

## `life.resume`

Resume: the next time becomes now, so she wakes once right away and then on her own schedule; the note she left for her next wakeup is kept. A running one is left as it is; a missing schedule row is rebuilt from the log. Answers with the state above. Without a life session: `no_life`; an ended one: `life_ended`.

## `life.kill`

End this life, and answer once it is done: a `life.ended` event goes into the log (ended by a person, no last words) and a running turn is interrupted; the session is archived read-only (sending answers `session_sealed`) and can still be read and browsed; the life row is deleted, so she never wakes again; the persona's memories whose sources all lie in this session are cleared (this cannot be undone), and the session is taken out of the index of past conversations; heads subscribed to the `notices` stream get a `life.ended` notice (see [Pushes](pushes.en.md)). There is no way to continue it. Answers `{"state": "ended", "session": <id>}`.

- Without a life session: `no_life`.
- An ended one succeeds as well: whatever was left uncleared (for instance when the core stopped half way) is finished, and no second notice is sent.

## `life.new`

End a running or paused life as `life.kill` does, then start a new one as `life.start` does. Answers `{"session": <new id>, "ended": <the one that ended>}`; `ended` is absent when there was nothing to end. The new life does not remember the old one (its memories were cleared).

## Errors

| `reason` | When | `data` |
|---|---|---|
| `unknown_persona` | No such persona | |
| `life_exists` | `life.start`: the persona already has a life session | `session` |
| `no_life` | `life.pause`, `life.resume`, `life.kill`: the persona has no life session | |
| `life_ended` | `life.pause`, `life.resume`: the persona's latest life has ended | `session` |
| `too_many_schedules` | `life.start`, `life.resume`: there are already 200 schedules, so the life row cannot be added | `session` for `life.start` |
