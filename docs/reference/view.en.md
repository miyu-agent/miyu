# View stream: ready-to-draw entries and session status

The core turns a session log into entries a UI can draw directly (the view projection), so heads don't have to work it out from events. Get them by paging with `view.page {view: true}` or by subscribing to the view stream with `subscribe {stream: "view"}`. Chinese version: [view.md](view.md).

## Subscribing

The response to `subscribe {"session", "stream": "view"}` carries the latest page of entries (as `view.page {view: true}`: `entries`, `more`, `first`, `last`, `capped`), the full session `status`, and the fields of an events subscription response. Then it pushes:

| Push | Meaning |
|---|---|
| `view.add {session, entry, after}` | A new entry, placed after `after` (`null` means first) |
| `view.update {session, entry}` | An entry changed (its state); `after` when its position changed |
| `view.append {session, id, text}` | Text appended to a reply or thought being written, or to a tool's `args` being written |
| `view.hidden {session, ids, hidden}` | Undo or restore: these entries hide or show again |
| `view.remove {session, id}` | An entry goes away (half-written parts dropped on interrupt, a compaction unfinished when the turn ended) |
| `view.status {session, status}` | The session status changed: the whole of it |
| `resync {session, stream: "view"}` | Fell behind: subscribe again |

The view stream doesn't push raw events. Text comes in the connection's language; if the language changes, later entries follow it.

## Entries

One object each, with the common fields `id`, `kind`, `turn` (which turn), `hidden` (hidden by an undo), `at` (when it began).

**Ids** follow positions in the log, so paging and subscribing agree: `m<number>` a message, `b<seen>.<block>` a block of a reply, `g…` a stretch of the timeline, `c<number>` a compaction, `e<number>` a turn's end and asides, `s<number>.<n>` an image she sent, `x<number>.<n>` an aside that exists only in the stream.

| `kind` | Fields |
|---|---|
| `user` | `text`; `attachments` (`kind`, `name`, `media_type`, `blob`, images with `width`, `height`); `from` (where a message from elsewhere came from); `level`; `queued: true` (queued, she hasn't heard it yet); `withdrawn: true` (a queued message handed back: put the text back in the input box); `round` (which round of a standing goal) |
| `reply` | `text` (Markdown source; parsing and rendering are the head's job); `open: true` (still being written); `persona`: the persona that spoke this turn (its id, `null` for none). After a mid-session persona change, earlier replies keep the old persona: the change counts from the turn it actually takes effect (the next turn start) |
| `media` | An image she sent: `image`, `alt`, `sticker` |
| `group` | A stretch of the timeline (thoughts and tool steps): `steps`, `open`, `summary`, `summary_en` (the collapsed line), `took_ms`, `failed` |
| `thought` | A thought step: `group`, `text`, `open`, `took_ms` |
| `tool` | A tool step: `group`, `call`, `name`, `title`, `state`, `args`, `diff` (`{added, removed}`), `images`, `job`, `approval`, `took_ms`, `files` (`[{path, action}]`, `action` is `created`, `changed`, `trashed`), `skill`, `detail` (structured details from the tool, e.g. the items it found; one with `kind` is one of the three common shapes `links`, `table`, `images`, see [Extensions](extensions.en.md#toolcall-toolcancel)), `output` (the start of the result text: at most 100 lines or 8 KiB; when cut, `output_cut: true`, and `view.detail` gives the whole), `questions` (the set asked by a question step, present as soon as it is asked), `nested` (a `shell` whose script called tools: `{count, last}`, `last` being the highest-numbered `{n, name, title, state, started?}`; heads append `· #n display name` to the line and fetch the full list with `view.detail`) |
| `end` | A turn's end: `reason`, `model`, `endpoint`, `took_ms`, `usage`; on error `error`, `explain` |
| `notice` | An aside: `what` below |

**A `tool`'s `state`**: `preparing` (still writing arguments), `running`, `ok`, `error`, `denied`, `cancelled`, `skipped`.

**A `tool`'s `title`**: `{"name", "object", "said", "via"}`: the tool's display name, its object (a path, a command's short title…), the result sentence, and who did it (e.g. the web search source `Exa`, shown in parentheses after the name; absent when there is none).

**The collapsed line** (`summary`): segments `{"text", "tone"?}`, `tone` being `added`, `removed`, `error`. For example `Ran 2 commands · 1 edit +3 -1 · 5s`.

**A `notice`'s `what`**:

| `what` | Fields |
|---|---|
| `compaction` | `trigger` (`auto`, `manual`, `overflow`), `state` (`running`, `done`, `failed`), `written`, `expected`, `prepared`, `before`, `after`, `took_ms`, `usage`, `instructions`; fetch the summary text with `view.detail` |
| `cleared` | The context was cleared |
| `paused` | Automatic compaction paused: `reason`, `failures` |
| `reverted` | Turns were undone: `turns`, `said`, `jobs` |
| `job` | A background job finished: `job`, `job_kind`, `title`, `mark` (`done`, `failed`, `stopped`), `report`, `command`, `output` (`{blob, chars}`), `exit_code`, `signal`, `took_ms`, `nested` (when the command's script called tools, as on `tool`) |
| `answered` | A set of questions was answered: `questions`, `answers`, `by` |
| `persona` | The persona or preset changed mid-session, at the moment it takes effect: `persona: {from, to}`, `preset: {from, to}`, only the ones that changed; ids, `null` for none |
| `recap` | A recap: `text`, `covers` |
| `model` | The model changed: `endpoint`, `model`, `from`, `why` (`failover`, `replaced`) |
| `workspace` | The workspace changed: `cwd`, `dirs` |
| `peer` | A session being waited on went idle or can't be waited on: `session`, `reason`, `status` |

## Session `status`

| Name | Notes |
|---|---|
| `state` | `idle`, `running`, `waiting` (an approval or question is open) |
| `waiting` | `[{what: "approve" | "ask", entry, call}]` |
| `since` | When this turn started |
| `waited_ms` | How long this turn has waited on a person (an approval, an answer) in finished stretches, in milliseconds, overlapping waits counted once; absent when it never waited. Waiting does not count as time spent |
| `waiting_since` | When the current wait on a person began; absent when not waiting. Time spent is now − `since` − `waited_ms` − (now − `waiting_since`), so the clock stops while waiting; the `took_ms` of entries excludes waiting too |
| `doing` | What's happening: `thinking`, `writing`, `tool` (with `entry`); `retrying` (`attempt`, `limit`, `at`, `class`, `message`); `compacting` (`trigger`, `written`, `expected`) |
| `context` | `{window, compaction_line, used}`: context window, compaction line, latest usage (that request's input plus output; right after compaction, the compacted size) |
| `speed` | `{output, ms}`: the latest output tokens and milliseconds |
| `cooling_until` | When every candidate is cooling down, the earliest recovery time |
| `preparing` | Compacting ahead in the background: `{seen, since}` |
| `usage` | This session's total usage |
| `model`, `permission`, `todos`, `workspace`, `persona`, `preset` | As in the subscription response; a persona or preset changed mid-session is pushed. A no-tools session has `preset: null` plus `no_tools: true`, as in the session list (step P-16) |
| `todos_done` | When the to-do list was cleared because everything was done, the list before clearing (heads tick everything, pause, then collapse) |
| `goal` | The standing goal: `{text, limit, state, reason, round, since, usage}`, `state` being `running`, `paused`, `completed`, `blocked`, `waiting` |
| `jobs` | Jobs this session started directly (running ones and the last 20 finished): `{job, what, title, state, started, ended, session, command, exit_code, signal, why, nested}` (`nested` as on `tool`, only for a command whose script called tools), `state` being `running`, `done`, `failed`, `stopped`, `aborted` |
