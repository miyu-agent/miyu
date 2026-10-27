# Sessions: `session.*`, `view.*`

A session is one conversation: an append-only event log, from which the core works out the current state. Heads create sessions, speak into them, and subscribe to their events to draw. Chinese version: [sessions.md](sessions.md).

| Method | What it does |
|---|---|
| [`session.create`](#sessioncreate) | Create a session |
| [`session.list`](#sessionlist) | List sessions |
| [`session.send`](#sessionsend) | Say something |
| [`session.interrupt`](#sessioninterrupt) | Interrupt the running turn |
| [`session.withdraw`](#sessionwithdraw) | Take back one queued message without interrupting |
| [`session.tools`](#sessiontools) | The tools she can use next turn |
| [`session.context`](#sessioncontext) | What the context is made of |
| [`session.usage_parts`](#sessionusage_parts) | Cumulative usage split by context part |
| [`session.answer`](#sessionanswer) | Answer an approval request or a set of questions |
| [`session.revert`, `session.unrevert`](#sessionrevert-sessionunrevert) | Undo; restore the last undo |
| [`session.redo`](#sessionredo) | Redo the last turn |
| [`session.compact`](#sessioncompact) | Compact the context now |
| [`session.clear`](#sessionclear) | Clear the context |
| [`session.recap`](#sessionrecap) | Get a one-line recap |
| [`session.set_permission_level`](#sessionset_permission_level) | Change the permission level, the read-only switch |
| [`session.set_workspace`](#sessionset_workspace) | Change the working directory |
| [`session.set_meta`](#sessionset_meta) | Rename, pin |
| [`session.terminal`, `session.set_terminal`](#sessionterminal-sessionset_terminal) | The terminal session: get it, point it elsewhere |
| [`session.configure`](#sessionconfigure) | Change the model, persona or preset |
| [`session.fork`](#sessionfork) | Fork a child session |
| [`session.delete`](#sessiondelete) | Delete a session |
| [`view.page`](#viewpage) | Read history page by page |
| [`view.detail`](#viewdetail) | The full diff of a tool call; a compaction's summary |

Sessions in venues (group chats and private chats on a chat platform) have a few more methods; see [Venues](venues.en.md).

## Common

- **Session ids** are UUIDv7 strings, ordered by creation.
- **Who can call**: people can call all of these. Extensions call them on sessions their account can see and need a capability: reading (`session.list`, `session.recap`, `session.tools`, `session.context`, `session.usage_parts`, `view.page`, `view.detail`) needs `events.read`; creating, sending, interrupting, reverting, unreverting, redoing, compacting, clearing, configuring, forking and renaming need `sessions.drive`; `session.answer`, `session.set_permission_level`, `session.set_workspace`, `session.terminal`, `session.set_terminal` and `session.delete` are for people only (`local_only`). What an extension says without `as` is recorded as itself (`harness`, named after the package); it can't set `from`.
- **Sessions that aren't running are loaded**: a session the core restarted past or that retired after idling is loaded on first use; heads don't have to care.
- **Watch pushes for changes**: most methods return `{}` or a few event numbers; the change itself arrives as events to heads subscribed to the session ([Pushes](pushes.en.md)).
- **Finding the session** can fail on every method with a `session`:

| `data.reason` | When |
|---|---|
| `session_not_found` | No such session; deleted ones too |
| `session_broken` | The session can't be loaded: its log is broken or unreadable |
| `session_stopped` | The session stopped (couldn't write, a bug); sending again reloads it |
| `session_sealed` | A forked session ran its turns and was sealed; commands that change the conversation are refused |
| `restoring` | A command arrived while an undo is still finishing (normally unreachable) |

## `session.create`

**Params**

| Name | Type | Notes |
|---|---|---|
| `cwd` | string | The head's working directory as a person sees it, e.g. `~/src/miyu`. Omitted means the session's own default workspace; remote connections cannot give it (Y-1 下) |
| `persona` | string or `null` | Which persona; `null` means none; omitted follows the `persona.default` setting |
| `preset` | string or `null` | Which preset; omitted follows `preset.default`, else `full`; `null` means no tools: the session gets none (step P-16; the old way to say it, still accepted. New sessions pick "Nothing", `none`, step P-17) |
| `model` | string | `<provider>/<model>` or `@<pool>`; omitted follows `models.chat` |
| `memory` | `persona`, `session`, `off` | Memory follows the persona, stays in this session, or is off; omitted follows the persona's setting |
| `dirs` | array of strings | Extra directories, readable and writable like the working directory |
| `chosen` | boolean, default `false` | Accepted and ignored (it used to mark a choice by the person): a too-wide `cwd` (root; on Windows also home) is always used and the response has `wide`; what the session may freely write is then its own default workspace |
| `oneshot` | boolean, default `false` | A one-shot session (the `miyu ask` kind) |
| `images` | boolean, default `true` | Whether you can display images |
| `titles` | boolean, default `true` | Whether the session gets automatic titles: with `false` it never does; renaming by hand still works |
| `project` | string | Create it in this project ([Projects](projects.en.md)): `persona` and `preset` default to the project's, and a `session.meta_changed` follows (`events` is `[1, 2]`) |
| `default_workspace` | The session's default workspace, an absolute path: a main session has its own `…/home/<account>/workspace/<session id>/`; children and forks use their main session's. When `cwd` equals it, it can be shown as "default workspace" |

**Response**

| Name | Notes |
|---|---|
| `session` | The new session's id |
| `events` | `[1]`: the `session.created` event; `[1, 2]` with `project` |
| `cwd` | Where the session actually works, as people write it (`~/…` under the home directory, same as `session.list`) |
| `wide` | `true`: the directory is too wide (root; on Windows also home or a folder holding the data directory) and is used because the person chose it; the session may freely write only its own default workspace |
| `untrusted_project` | The directory has a project config not yet trusted or rejected: where it is |

```json
→ {"jsonrpc":"2.0","id":"c1","method":"session.create","params":{"cwd":"~/src/miyu"}}
← {"id":"c1","jsonrpc":"2.0","result":{"cwd":"~/src/miyu","events":[1],"session":"01a14aca-d2c4-768c-acc6-a67e35106d22"}}
```

Sending the same `id` again returns the session created the first time.

**Errors**

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | Malformed persona or preset id; `memory` not one of the three; `model` not a string |
| `unknown_persona`, `persona_invalid` | No such persona; its files are wrong (`data.problem` says which layer, file and line) |
| `unknown_preset`, `preset_invalid` | The same, for presets |
| `unknown_model` | `model` can't be resolved: malformed, unknown provider, unknown pool |
| `dir_too_wide` | A `dirs` entry is too wide (home, root, contains the data directory) |
| `internal_error` (`-32603`) | The core couldn't create it |

## `session.list`

**Params**: `oneshot` (boolean, `true` lists only one-shot sessions), `dream` (boolean, `true` lists only consolidation sessions, `false` leaves them out, all when omitted), `limit` (at most this many; all when omitted).

**Response** `{"sessions": [<item>…]}`, newest first. An item:

| Name | Notes |
|---|---|
| `session` | Id |
| `cwd` | Working directory as a person sees it |
| `last_active` | Time of the last event |
| `oneshot` | Whether it's one-shot |
| `parent` | A child session's parent; `null` for main sessions |
| `title` | Title; omitted when there is none |
| `named` | `true`: a person gave the title |
| `preview` | When there's no title: the first line of the first message, at most 50 characters |
| `pinned` | `true`: pinned |
| `busy` | `true`: a turn is running |
| `persona`, `preset` | Which persona and preset; omitted when none. With no tools `preset` is `null` and `no_tools: true` is added (step P-16) |
| `tokens` | Tokens used by it and every session it spawned, all the way down: input (uncached, cache read, cache write) plus output of every model call (main, title, compaction alike), the same sum as the terminal's `Σ`; omitted while 0 |
| `context` | `{"used", "window"?}`: tokens in context (the input of the latest main request: uncached, cache read, cache write, the same figure as the terminal footer and the session status); omitted before the first request and right after a compaction. `window` is the context window of that request's model under the current config, omitted when unknown |
| `project` | Which project it is in; omitted when none |
| `kind`, `dream` | Only on consolidation sessions (the sessions memory consolidation runs in): `kind` is `"dream"`, `dream` is `{"persona"?, "session"?, "manual"?, "from"?}`: whose memory (a persona or a session), `manual: true` when a person asked for it, `from` the session it was asked from; a finished run adds `result`: `{"given", "revised", "retired", "summary", "stopped"?}`, how many memories it added, revised and retired, whether it wrote the summary, `stopped: true` when it was stopped; a run that did not finish has none. A main session list usually leaves them out (pass `dream: false`); a memory page lists those of its memory by `dream` |
| `forked` | Only on forked subagent sessions: `true`. Heads show a fork icon for it. Ordinary subagents and main sessions don't have it |
| `origin` | Only on sessions the shell integration opened (`session.terminal`): `"terminal"`. Heads show "shell" for it in the session list. Older ones don't have it |

Only local sessions are listed; venue sessions aren't (see `venue.sessions` in [Venues](venues.en.md)). To keep up to date, subscribe to the `sessions` stream ([Connection](connection.en.md)).

**Errors**: `bad_params`; `internal_error` when the session directory can't be read.

## `session.send`

**Params**

| Name | Type | Notes |
|---|---|---|
| `session` | string, required | Which session |
| `text` | string, required | What to say |
| `attachments` | array | Attachments, each the `blob`, `name`, `media_type` from a `blob.put` response, optionally `path` (where it was on this machine) |
| `urgent` | boolean, default `false` | Cut in: if she's busy, she sees it at the next step |
| `from` | string | You are another agent program: your name; the message is recorded as yours, not the person's |
| `cwd` | string | Move to this working directory first (shell integration: each turn's directory follows the shell): checked like [`session.set_workspace`](#sessionset_workspace), `wide` included; the same directory records nothing, a change pushes `session.workspace_changed`. Not for venue sessions |
| `as`, `venue` | object | Venue sessions only; see [Venues](venues.en.md) |

**Response**: `events` (this message's number), `cwd` (after any move), `untrusted_project` (as in `session.create`); `wide: true` when moved somewhere too wide. A bad `cwd` (`path_unreadable`, `not_a_directory`, `path_forbidden`) refuses the whole message.

1. If she's idle, the message starts a turn; if busy, it's queued (`urgent` ones cut in at the next step). The turn's `turn.started` has this request's `id` as `cause`, so you can tell it's yours.
2. It returns once the message is on disk, without waiting for her answer; watch the pushes for that.
3. Attachments are checked before the session is looked up: the blob must exist and is recognized again from its content (see [Attachments](blob.en.md)).

```json
→ {"jsonrpc":"2.0","id":"m1","method":"session.send","params":{"session":"01a14aca-…","text":"Look at this image","attachments":[{"blob":"sha256:9f86d0…","name":"shot.png","media_type":"image/png"}]}}
← {"id":"m1","jsonrpc":"2.0","result":{"cwd":"~/src/miyu","events":[42]}}
```

**Errors**

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | An attachment is missing fields or malformed; `from` not a string or empty after removing control characters |
| `empty_message` | No text and no attachments |
| `unknown_attachment` | An attachment's blob isn't in the core |
| `attachment_too_big` | An attachment is an image over the limits |
| `venue_session` | Speaking into a venue session without `as` |

## `session.interrupt`

**Params**: `session`; `queued` (required): what to do with queued messages not yet sent: `send` sends them right after, `return` hands them back, `keep` keeps them without sending.

**Response**: `{"events": [<the events appended>…]}`.

**Errors**: `not_running`: no turn is running.

## `session.withdraw`

Takes back one queued message she hasn't heard yet, without interrupting the turn.

**Params**: `session`; `message` (optional): which one, the seq of its `message.user`; leave it out for the latest.

**Response**: `{"events": [<the seq of the message.withdrawn>]}`. The push is the same as when `session.interrupt` returns messages: one `message.withdrawn` whose `messages` holds just this one; the view marks that message `withdrawn: true`. The turn keeps running and, when it's done, the remaining queued messages open the next one.

**Errors**: `already_heard`: the named message isn't queued, a request already took it (or it opened the next turn); `nothing_queued`: no message named and nothing queued.

## `session.tools`

The tools her next request will carry.

**Params**: `session` (optional; without it, what a new session would get); without `session` you may pass `persona` (`null` for none) and `preset` (`null` for no tools), as in `session.create`.

**Response**: `{"tools": [{"name", "source": {"kind", "id", "name"}, "summary"?, "loaded"?}]}`, in the order sent to the model (byte order of names). `kind` is `builtin`, `package`, `mcp` or `script`; `source.name` follows the connection's language. Only scripts have `summary`. Tools that must be loaded first and show only their name have `loaded: false`, and then `load_tools` is listed too.

```json
{"tools":[{"name":"edit","source":{"kind":"builtin","id":"basesystem","name":"Base system"}},{"name":"mcp__notes__search","source":{"kind":"mcp","id":"notes","name":"notes"},"loaded":false}]}
```

**Errors**: `bad_params` (`session` together with `persona` or `preset`); the session lookup errors.

## `session.context`

What this session's context is made of right now: how much the system prompt, tool definitions, example dialogue, checkpoint summary and conversation each take. Computed only when called; it doesn't affect the session.

**Params**: `session` (optional; without it, what a new session would get); without `session` you may pass `persona` (`null` for none) and `preset` (`null` for no tools), as in `session.create`.

**Response**: `{"window"?, "line"?, "used"?, "basis", "anchor"?, "estimated_total", "parts": [...]}`. `window` and `line` are the context window and the compaction line (absent before limits are known); `used` is how much is in use, as in the status bar; `basis` is `measured` (counted from the provider-reported `anchor` of the latest request) or `estimated` (all local estimates). Each of `parts` is `{"part", "label", "tokens", "children"?}`: `part` is a stable id, `label` follows the connection's language, and parts of 0 are left out. With `used`, the parts are already scaled to add up to exactly `used` (parts scaled to 0 are left out); `estimated_total` is the sum of the local estimates before scaling.

Part ids: `system` (in composing order: `system.persona`, `system.venue`, `system.group`, `system.profile`, `system.project`, `system.core`, `system.clicall`, `system.show`, `system.skills`, `system.knowledge`, `system.style_lock`, `system.appended:<package>`; in older sessions, stretches that no longer match the current text go into `system.other`), `tools` (with `tools.builtin`, `tools.package:<package>`, `tools.mcp:<service>`, `tools.scripts`, `tools.deferred`), `demos`, `summary`, `messages` (with `messages.user`, `messages.assistant`, `messages.memory` (with `messages.memory.resident`, `messages.memory.recall`), `messages.knowledge`, `messages.facts`, `messages.project_docs` (the repository's AGENTS.md), `messages.reports` (reports from subagents and background commands), `messages.tool_results`). Packages and services are labelled by name.

```json
{"window":128000,"line":102400,"used":45231,"basis":"measured","anchor":41000,"estimated_total":43800,"parts":[{"part":"system","label":"System prompt","tokens":6100,"children":[{"part":"system.persona","label":"Persona","tokens":4300},{"part":"system.core","label":"Core instructions","tokens":900},{"part":"system.skills","label":"Skills","tokens":900}]},{"part":"messages","label":"Conversation","tokens":37700,"children":[{"part":"messages.user","label":"Your messages","tokens":4200},{"part":"messages.assistant","label":"AI replies","tokens":33500}]}]}
```

**Errors**: `bad_params` (`session` together with `persona` or `preset`); the session lookup errors; `internal` when a new session can't be composed.

## `session.usage_parts`

How much this session has used so far, split into the same parts as `session.context`, plus a part for AI output. Main requests only; compaction, subagents, recaps and the like are in `usage.query` by use.

**Params**: `session` (required); `tree` (boolean, optional) to include the subagent sessions it dispatched.

**Response**: `{"requests", "input", "output", "parts": [...]}`. `requests` is how many main requests, `input` the reported input (uncached, cache read, cache write) added up, `output` the output added up. `parts` are as in `session.context` with a final `output` part; they add up to exactly `input + output`. Each request's input is split by its parts' shares at that moment and then summed; cached and uncached input are not split separately, so some cached input lands on newer content.

```json
{"requests":503,"input":80000000,"output":203822,"parts":[{"part":"system","label":"System prompt","tokens":220000},{"part":"tools","label":"Tool definitions","tokens":1900000},{"part":"messages","label":"Conversation","tokens":77880000},{"part":"output","label":"AI output","tokens":203822}]}
```

**Errors**: `bad_params`; the session lookup errors.

## `session.answer`

When she does something that needs approval she stops and asks (pushed as `tool.approval_requested`); when she asks with `ask_user` she waits too (pushed as `question.asked`). This answers both.

**Params**

| Name | Type | Notes |
|---|---|---|
| `session` | string, required | Which session |
| `call` | string, required | That call's `call_id` |
| `decision` | `once`, `session`, `deny` | Answer an approval: allow once, allow for this session, deny |
| `reason` | string | Why it was denied, shown to her; only with deny |
| `attachments` | array | Images or files attached to a denial, same shape as in `session.send` (`blob`, `name`, `media_type` from `blob.put`, optional `path`); she gets them with the reason; denials only |
| `answers` | array | Answer questions: one item per question in order, `{"picked": [chosen option labels…], "text": <written answer>, "notes": <note>, "attachments": [<as above>]}`, all optional; attachments go to her with that question |
| `nested` | positive integer | When a tool she calls with `miyu call` inside a command needs approval, the request carries `nested` (its number): send it back. Leaving it out also works and answers the one waiting under that command |

Give exactly one of `decision` and `answers`. **Response**: `{"events": [numbers…]}`.

**What `session` remembers**: the pushed request's `rule` (without one, `session` can't be chosen), compact JSON: `tool` is which tool, `write` the folders whose writes are allowed (with everything below). When a command wants to run outside the sandbox, `write` is the working folder, and if the tool can tell the command's prefix there is also `prefix` (for example `"docker ps"`, `"npm run"`): once chosen, in this session, while the working folder is that folder or below it, commands with the same prefix run outside the sandbox without asking again. Chained commands whose parts have different prefixes, commands starting with a variable assignment, and commands wrapped in `sudo`, `env`, `sh -c` and the like have no `prefix` and are asked every time. A head can show `prefix` as "Don't ask again for commands starting with `docker ps` in this folder".

```json
{"prefix":"docker ps","tool":"shell","write":["/home/me/proj"]}
```

**Errors**

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | Both or neither given; `reason` with questions; `decision` not one of the three |
| `not_asking` | The call isn't waiting: already answered, finished, or waiting for the other kind |
| `no_rule` | `session` chosen, but this approval has no scope that can be remembered |
| `unexpected_reason` | Allowed, but with a reason |
| `bad_answer` | The answers don't match the questions: wrong count, unknown option, several picks on a single-choice question |

## `session.revert`, `session.unrevert`

Undo takes the last turn (or every turn from one on) out of the conversation; files she changed are left as they are. Restore takes back the last undo.

**Params**: `session`; `revert` also takes `turn`: undo from this turn on (that turn's `turn.started` number); omitted undoes the last turn.

**Response**

| Name | Notes |
|---|---|
| `events` | The events: `turn.reverted` or `turn.unreverted` |
| `cwd` | The session's working directory (real path) |
| `turns` | How many turns were undone (restored) |
| `said` | The first line of what the person said in the first one; omitted when none |
| `compactions`, `clears` | Undo only: compactions and clears inside the undone turns; the context is back to before them. Omitted when 0 |
| `jobs` | Undo only: background jobs stopped, `[{"job", "what", "title"}]`; omitted when none |

**Errors**

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | `turn` is 0 |
| `turn_running` | A turn is running: interrupt first |
| `nothing_to_revert` | There are no turns |
| `unknown_turn` | That turn isn't in the current history |
| `nothing_to_unrevert` | Nothing to restore: no undo, or a new turn or compaction since |

## `session.redo`

Undo the last turn and send the messages that started it again, starting a new turn. Editing your last message in the web UI goes through this.

**Params**: `session`; `text`: replace the text of the message that started the turn (omitted keeps it); `attachments`: replace its attachments (omitted keeps them; an empty array removes them).

**Response**: as for undo; `events` ends with the numbers of the resent messages. The new turn's `turn.started` has this request's `id` as `cause`.

**Errors**: `not_redoable` (the last turn wasn't started by a person, or there are no turns), `turn_running`, `empty_message` (nothing left after the replacement), and the attachment errors of `session.send`.

## `session.compact`

**Params**: `session`; `instructions`: what the person wants kept, e.g. "keep the database design discussion".

**Response**: `{"events": [<that turn's turn.started number>]}`, returned once it starts, not when it's done. Progress arrives as `compaction.progress`, `compaction.done`, `context.compacted`, `turn.ended`.

**Errors**: `turn_running`; `nothing_to_compact` (nothing new since the last compaction).

## `session.clear`

Clear the context: a turn that only records an empty checkpoint, with no model request. The to-do list is cleared too.

**Params**: `session`. **Response**: as for `session.compact`; by the time it arrives, that turn's start, empty checkpoint and end have already been pushed. Undoing that turn brings the context back.

**Errors**: `turn_running`; `nothing_to_clear` (already empty).

## `session.recap`

A one-line recap: what this session is doing, what's done, what's stuck. A separate model request fed only the last few turns.

**Params**: `session`. **Response** `{"text", "upto", "cached"}`: the line, up to which event, and whether it's the previous one returned again (nothing new since, no request).

When it's written, every subscribed head first receives `model.called` (`purpose` is `recap`) and `session.recapped`.

**Errors**: `nothing_to_recap` (she hasn't said anything yet); `recap_failed` (the request failed or returned no text).

## `session.set_permission_level`

**Params**: `session`; `level`: `workspace` or `full` (fully open); `read_only`: the read-only switch. Give at least one.

**Response**: `{}`. A change is pushed as `session.policy_changed`. Tightening applies at once; loosening applies from the next request. Whether to ask the person before going fully open is up to the head.

**Errors**: `bad_params`: neither given; `level` not one of the two.

## `session.set_workspace`

**Params**: `session`; `cwd`: the new working directory (absolute or starting with `~`); `dirs`: the new extra directories, replacing them all (empty removes all). Give at least one.

A `cwd` that is the account workspace itself (the handshake's `host.workspace`) means going back to this session's own default workspace: it is replaced by it (`default_workspace` in the session list and the subscribe reply).

**Response**: `{"cwd", "dirs"}`, plus `wide: true` when too wide. A change is pushed as `session.workspace_changed`; this turn keeps the old one, the next starts with the new one.

**Errors**

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | Neither given |
| `path_unreadable` | The directory doesn't exist or can't be read |
| `not_a_directory` | It's a file |
| `path_forbidden` | It's inside Miyu's data directory (except the account's own default workspace) |
| `dir_too_wide` | A `dirs` entry is too wide |

## `session.terminal`, `session.set_terminal`

The terminal session: the one long-running session that what you say in the shell goes into (shell integration, design `22-命令行.md` section 7). One per account, and one per terminal for calls that carry a `terminal` key (construction T-22); the core decides which one it is and when a new one starts.

**`session.terminal`**: param `cwd` (optional): where a new one starts; without it, the default workspace. `fresh` (optional, default false): when true, start a new one as in rule 3 and point at it, whatever is pointed at now (`/new`). `terminal` (optional): which terminal, a key the head makes up, 1 to 128 characters with no control characters (otherwise `bad_params`); with it the pointer is kept per key, without it the account's original pointer is used. `create` (optional, default true): when false, only ask: in cases 2 and 3 below the response is `{"session": null}`, nothing starts and the pointer doesn't move (to show which session a freshly opened terminal is in before anything is said); together with `fresh: true` it's `bad_params`. Response `{"session"}`:

1. The session pointed at still exists and is this account's own local main session (not a child, venue or one-shot session): it's that one.
2. It has been idle longer than `terminal.idle_new_after` minutes (0 never replaces): a new one starts, the pointer moves, and the response adds `renewed: {"last"}` (when the old one last had an event); the old one is left alone. Being pointed at (`session.set_terminal`, or being started) counts as activity: switching to a session that has been quiet for hours starts the idle time from the switch.
3. Nothing pointed at yet, or it was deleted or doesn't count: a new one starts and the response adds `created: true`.

A new one uses `terminal.persona` and `terminal.preset` (unset follows `persona.default` and `preset.default`). Several connections asking at once (with the same key) get one session. Each key remembers when it was last used; keys unused for 7 days are dropped when a pointer is written, and at most 64 are kept (least recently used go first). A dropped key starts over as if never pointed.

**`session.set_terminal`**: param `session`: point the terminal session at it (what `/sessions` picked). `terminal` (optional): whose pointer to change, as above. Only this account's own local main sessions; anything else is refused with `session_not_found`. Response `{"session"}`.

## `session.set_meta`

**Params**: `session`; `title`: the new title, 1 to 200 characters, `null` removes it; `pinned`: `true` pins, `false` unpins; `project`: a project id moves it in, `null` moves it out (only local main sessions can join: venue sessions, subagents and forks get `bad_params` with `data.field` `project`). Give at least one. Pin and project are separate: a head that unpins on moving into a project sends `pinned` itself.

**Response**: `{}`. A change is pushed as `session.meta_changed`. Sessions without a name get a title from the core after the first turn, also pushed as `session.meta_changed` (`by` is the kernel).

**Errors**: `bad_params`: nothing given; an empty title or one over 200 characters; an empty `project` string, or a session that cannot join (`data.field` is `project`). `unknown_project`: no such project.

## `session.configure`

Change the session's model, persona or preset; it applies from the next turn.

**Params**: `session`; at least one of:
- `model`: `<provider>/<model>` or `@<pool>`; `null` stops pinning a model and follows the configured `models.chat` again (read every turn, so a config change follows).
- `persona`: a persona id; `null` means no persona.
- `preset`: a preset id; `null` switches to no tools (step P-16).

Changing the persona or preset compacts first: the core summarizes the conversation so far under the old persona right away, and the next turn switches over and continues from the summary. Memory that follows the persona is kept in the new persona's room from then on; earlier memories stay where they were.

**Response**: `{}`. A change is pushed as `session.policy_changed`; when a model change takes effect, `model.changed`. `persona` and `preset` in `session.list` and the `subscribe` response show the new values right away.

**Errors**: `bad_params` (nothing given, an empty string, a wrong type; a subagent changing persona or preset, with `data.field` naming it); `unknown_model`; `unknown_persona`; `unknown_preset`; `turn_running`: a turn is running while changing persona or preset (wait for it to finish).

## `session.fork`

Fork a child session from a main session: the child carries on from its history up to the fork point, with a byte-identical request prefix, so the cache is reused.

**Params**: `session` (the parent, a main session); `seal_after`: seal after this many turns (from 1); omitted never seals.

**Response**: `{"session": <child id>, "upto": <fork point number>}`. If the parent is in the middle of a turn, the fork point is just before that turn.

**Errors**: `bad_params`: the parent is a subagent or a fork; `seal_after` isn't an integer from 1.

## `session.delete`

Moves it to the trash, kept for 7 days. Child sessions it spawned are deleted with it.

**Params**: `session`. **Response**: `{}` once moved. Heads subscribed to it get `resync`.

**Errors**: `turn_running`: a turn is running (interrupt first); `internal_error`: couldn't move it.

## `view.page`

Read history page by page, from the newest back; page edges fall between turns.

**Params**

| Name | Type | Notes |
|---|---|---|
| `session` | string, required | Which session |
| `before` | positive integer | Only numbers below this; omitted is the newest page. To page back, pass the previous page's `first` |
| `turns` | 1 to 50, default 20 | At most this many turns |
| `view` | boolean, default `false` | `true` returns entries ready to draw (the view projection) instead of raw events |

**Response**: `events` (or `entries`) and `more` (whether there's more before), plus `first` and `last` when not empty; `capped: true` when fewer turns came back to stay under 1 MiB; `jobs` for jobs that finished on this page but were started earlier.

The newest page's `last` is the last event right now: then `subscribe {"after": last}` gets only what's new.

**Errors**: `bad_params`; `session_not_found`; `session_broken`.

## `view.detail`

**Params**: `session`; `call` (a tool call's `call_id`), `job` (a background job's id) or `compaction` (the number a compaction replaced up to), exactly one; optionally `nested_after` (an integer): only the nested calls numbered above it, plus earlier ones still running.

**Response**

- A call: `{"files": [...], "output"}`. `output` is the full result text; `files` has one item per file: `path`, `action` (`write` or `trash`), `diff` (unified, 3 lines of context), `added`, `removed`; when no diff can be computed, `skipped` (`too_big`, `binary`, `missing`).
- A compaction: `{"summary"}`: its summary text.
- When a script called tools (`miyu call`): a call's response also has `nested`, and a job's response is just `{"nested"}`. One item per call, by number: `n`, `name`, `title` (`{name, object}`, the display name in the connection's language), `args` (the raw arguments), `state` (`running`, `ok`, `error`, `denied`, `needs_approval`, `cancelled`), and when present `started` (when a running one started), `took_ms`, `reason` (the refusal or error), `result` (`{blob, bytes}`, what the script got back, fetched with `blob.get`). A `shell` still running with no result yet also answers `{"nested"}`.

A call's result never changes; asking once is enough. Nested calls grow while they run; use `nested_after` to get only the new ones.

**Errors**: `bad_params`; `session_not_found`; `unknown_call` (no result for that call, or no such compaction); `session_broken`.
