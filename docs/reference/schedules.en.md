# Schedules: `schedule.*`

At a set time, hand one line to a session and let it run a turn: a morning briefing at 8, a look at the server every hour. There is one table, `root/schedules.db`, so schedules survive a core restart; times missed while the core was down are skipped, not made up. Design: `docs/designs/45-定时和循环.md`; how it runs: `docs/blueprint/schedules.md`. 中文版：[schedules.md](schedules.md).

| Method | Does |
|---|---|
| [`schedule.add`](#scheduleadd) | Add one |
| [`schedule.list`](#schedulelist) | List them all |
| [`schedule.cancel`](#schedulecancel) | Delete one |
| [`schedule.pause`](#schedulepause) | Pause |
| [`schedule.resume`](#scheduleresume) | Resume |
| [`schedule.update`](#scheduleupdate) | Change the time or the line |

**Who may call**: people; extensions need an approved `sessions.drive` (construction J-4), otherwise `needs_approval`. What an extension adds is recorded as added by its package, and it sees, cancels, pauses, resumes and updates only those: touching one it did not add is `unknown_schedule`. The row of a life loop (`loop` is `life`, the one a persona keeps running) is out of everyone's reach: it is not in the list or the `schedules` stream, and `cancel`, `pause`, `resume` and `update` on it get `unknown_schedule` as if there were no such id. To keep up, subscribe to the `schedules` stream ([connection](connection.en.md)); `schedules.changed` follows ([pushes](pushes.en.md)).

## An item

| Name | Meaning |
|---|---|
| `id` | `t1`, `t2`…, counting up; ids are never reused |
| `when` | When: `{"cron": "0 8 * * *", "recurring": true, "zone": "Asia/Tokyo"}` (standard five fields, read in `zone`: the one given when it was added, or the core machine's zone at that moment; older ones without `zone` use the core machine's), `{"at": "<time>"}` (once), `{"every": <seconds>}` (at least 60) |
| `prompt` | The line handed to the session, as written |
| `target` | Where: `{"session": <id>}`, or `{"new": {"persona", "preset", "cwd"}}` (a new session each time; `persona` `null` means none), or `{"package": {}}` (no session: when due, [`schedule.fired`](#schedulefired) goes to the extension that added it) |
| `by` | Who added it: a person `{"kind": "person"}`, the AI in a session `{"kind": "model", "session"}`, a package `{"kind": "package", "package"}` |
| `recurring` | Whether it stays after running: `false` for `at`, `true` for `every`, as written for `cron` |
| `paused` | Paused |
| `loop` | Loops only: `fixed` interval, or `dynamic` (the AI sets the pace); a life loop's `life` never shows up here, see above |
| `next` | Next time; absent while paused, or while a dynamic loop waits for the AI to set it |
| `last` | Last run: `{"at", "result", "session"?}`; `result` is `ran`, `queued` (the session was busy; it waits its turn), `sent` (pushed to the extension that added it) or `skipped:<why>` (`session_gone`: the session is gone and the schedule paused; `preset_off`: the AI added it and the session's preset turns it off; `not_opened`: the new session could not be opened; `package_unavailable`: the extension that added it is not running and could not be started) |
| `created` | When it was added |

Absent fields are left out. Times are written as elsewhere: `2026-10-26T08:00:00.000Z`.

## `schedule.add`

| Param | Type | Meaning |
|---|---|---|
| `when` | object, required | One of the three, as above; `recurring` and `zone` (an IANA time zone name such as `Asia/Tokyo`) only with `cron` |
| `prompt` | string, required | Not empty |
| `target` | object, optional | `{"session": <id>}`, `{"new": {"persona"?, "preset"?, "cwd"?}}` or `{"package": {}}`, one of them; omitted means a new session each time. Only extensions may write `package`; a person gets `bad_params` (no `data.problem`). Missing `persona`, `preset` are fixed to the current defaults; missing `cwd` is the home directory |

Response `{"schedule": <item>}`. A bad one adds nothing and gets `bad_params` with `data.problem`: `when` (not exactly one kind, or a bad time), `cron` (unreadable), `past` (the time has passed), `short` (under 60 seconds), `prompt` (empty), `session` (no such session, or this connection can't reach it: the same answer), `target` (both kinds written). An unknown persona or preset gets `unknown_persona`, `unknown_preset`, as when opening a session. Also `zone` (unknown time zone) and `cwd` (the new-session folder is missing, not a folder, or inside the data root outside the workspace folder). When there are already 200 schedules (added by people, the AI and packages together; a safety limit), adding one gets `too_many_schedules`: remove some first; existing ones keep running.

## `schedule.list`

Param `session`, optional: only those running on it. Response `{"schedules": [<item>…]}`: those with a next time first, by time; paused and waiting ones after.

## `schedule.cancel`

Param `id`. Response `{}`.

## `schedule.pause`

Param `id`. A paused one does not run; its next time is cleared. Response `{"schedule": <item>}`.

## `schedule.resume`

Param `id`. The next time is counted from now; missed ones are not made up. A one-off whose time has passed gets `bad_params` (`past`). Response `{"schedule": <item>}`.

## `schedule.update`

Change one's time or line. The id stays the same; `last`, the creation time, `target` and `by` are kept; anything not written stays as it is.

| Param | Type | Meaning |
|---|---|---|
| `id` | string, required | The schedule id |
| `when` | object, optional | Written and checked as in `schedule.add` |
| `prompt` | string, optional | Not empty |

Write at least one of `when` and `prompt`; neither gets `bad_params` (no `data.problem`). A new `when` recounts `next` from now; missed ones are not made up. A paused one stays paused, and `resume` counts from the new time. A loop keeps its kind: a `dynamic` one only takes `at` (which sets when it next wakes), a `fixed` one only `every`; anything else gets `bad_params` (`when`). `prompt` can change on both. A bad one changes nothing and gets `bad_params` with the same `data.problem` as `schedule.add` (`when`, `cron`, `past`, `short`, `prompt`, `zone`). Response `{"schedule": <item>}`; `schedules` subscribers get `schedules.changed` as usual.

```json
{"jsonrpc": "2.0", "id": "u1", "method": "schedule.update", "params": {"id": "t1", "when": {"every": 1800}}}
{"jsonrpc": "2.0", "id": "u1", "result": {"schedule": {"id": "t1", "when": {"every": 1800}, "prompt": "look at CI", "target": {"session": "<id>"}, "by": {"kind": "person"}, "recurring": true, "paused": false, "created": "2026-10-26T00:00:00.000Z", "next": "2026-10-26T00:50:00.000Z", "last": {"at": "2026-10-26T00:10:00.000Z", "result": "ran"}}}}
```

## `schedule.fired`

Core → extension, a notification (construction J-4): a schedule an extension added with `target` `{"package": {}}` is due. It goes to that package's process; no subscription, no reply.

| Name | Meaning |
|---|---|
| `id` | The schedule's id |
| `at` | When this run was due |
| `prompt` | What was written when it was added, handed back as is (the extension picks the format, e.g. JSON) |
| `missed` | How many runs were missed while the core was stopped, counting up to 1000; missed runs are not made up, only counted. Absent when none |

If the package isn't connected it isn't started. When it cannot be reached the run is recorded as `skipped:package_unavailable`; a one-off is removed as usual, a recurring one waits for its next time.
