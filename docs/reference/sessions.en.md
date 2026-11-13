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
| [`session.retitle`](#sessionretitle) | Have the model write a new title |
| [`session.viewing`](#sessionviewing) | A head tells the core which session it has open |
| [`session.terminal`, `session.set_terminal`](#sessionterminal-sessionset_terminal) | The terminal session: get it, point it elsewhere |
| [`session.configure`](#sessionconfigure) | Change the model, vision model, persona, preset, reasoning effort or compaction line |
| [`session.fork`](#sessionfork) | Fork a child session |
| [`session.delete`](#sessiondelete) | Delete a session |
| [`session.prompts`](#sessionprompts) | List what was asked in a session |
| [`session.export`](#sessionexport) | Export a session as a raw bundle or Markdown |
| [`session.import`](#sessionimport) | Import a raw bundle |
| [`view.page`](#viewpage) | Read history page by page |
| [`view.detail`](#viewdetail) | The full diff of a tool call; a compaction's summary |

Sessions in venues (group chats and private chats on a chat platform) have a few more methods; see [Venues](venues.en.md).

## Common

- **Session ids** are UUIDv7 strings, ordered by creation.
- **Who can call**: people can call all of these. Extensions call them on sessions they can see (a bridge sees the venue sessions of its platforms; other extensions see local sessions and your own private chats) and need a capability: reading (`session.list`, `session.recap`, `session.tools`, `session.context`, `session.usage_parts`, `session.prompts`, `view.page`, `view.detail`) needs `events.read`; creating, sending, interrupting, reverting, unreverting, redoing, compacting, clearing, configuring, forking, renaming and retitling need `sessions.drive`; `session.answer`, `session.set_permission_level`, `session.set_workspace`, `session.terminal`, `session.set_terminal`, `session.delete`, `session.export` and `session.import` are for people only (`local_only`). What an extension says without `as` is recorded as itself (`harness`, named after the package); it can't set `from`.
- **Sessions that aren't running are loaded**: a session the core restarted past or that retired after idling is loaded on first use; heads don't have to care.
- **Watch pushes for changes**: most methods return `{}` or a few event numbers; the change itself arrives as events to heads subscribed to the session ([Pushes](pushes.en.md)).
- **Finding the session** can fail on every method with a `session`:

| `data.reason` | When |
|---|---|
| `session_not_found` | No such session; deleted ones too |
| `session_broken` | The session can't be loaded: its log is broken or doesn't parse |
| `session_read_only` | A line in the middle of the session log is broken: what came before it can still be read with `view.page`, but the session can't be loaded or continued; start a new session to go on. `data.segment` is which segment (file name only), `data.line` which line |
| `session_unavailable` | The session can't be read right now: an I/O error while reading its log. The log is intact; try again later |
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
| `model` | string | `<provider>/<model>` or `@<pool>`; omitted records `models.chat` as it is right now (a pool stays a pool), and later changes to `models.chat` leave this session alone |
| `vision` | string | Which model describes images for this session, written like `model`; omitted follows the configured `models.vision`. Recorded as a `session.policy_changed` right after creation, like `vision` in `session.configure` |
| `compaction_line` | integer | This session's compaction line: a percent of the window, 20 to 95; omitted follows the configured `compaction.line`. Recorded as a `session.policy_changed` right after creation, like `compaction_line` in `session.configure` |
| `memory` | `persona`, `off` | Memory follows the persona, or is off; omitted follows the persona. Anything else (the old `session` included) is `bad_params` |
| `dirs` | array of strings | Extra directories, readable and writable like the working directory |
| `chosen` | boolean, default `false` | Accepted and ignored (it used to mark a choice by the person): a too-wide `cwd` (root; on Windows also home) is always used and the response has `wide`; what the session may freely write is then its own default workspace |
| `oneshot` | boolean, default `false` | A one-shot session (the `miyu ask` kind) |
| `images` | boolean, default `true` | Whether you can display images |
| `titles` | boolean, default `true` | Whether the session gets automatic titles: with `false` it never does; renaming by hand still works |
| `project` | string | Create it in this project ([Projects](projects.en.md)): `persona` and `preset` default to the project's, and a `session.meta_changed` follows (`events` is `[1, 2]`) |
| `default_workspace` | The session's default workspace, an absolute path: a main session has its own `…/workspace/<session id>/`; children and forks use their main session's. When `cwd` equals it, it can be shown as "default workspace" |

**Response**

| Name | Notes |
|---|---|
| `session` | The new session's id |
| `events` | `[1]`: the `session.created` event; `[1, 2]` with `project`; one more each with `vision` and `compaction_line` (in that order, after the project's) |
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
| `bad_params` (`-32602`) | Malformed persona or preset id; `memory` not one of the three; `model` or `vision` not a string; `compaction_line` not an integer from 20 to 95 |
| `unknown_persona`, `persona_invalid` | No such persona; its files are wrong (`data.problem` says which layer, file and line) |
| `unknown_preset`, `preset_invalid` | The same, for presets |
| `unknown_model` | `model` or `vision` can't be resolved: malformed, unknown provider, unknown pool |
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
| `viewed` | `true`: at least one head has it open ([`session.viewing`](#sessionviewing)); that head shows its background notices |
| `persona`, `preset` | Which persona and preset; omitted when none. With no tools `preset` is `null` and `no_tools: true` is added (step P-16) |
| `tokens` | Tokens used by it and every session it spawned, all the way down: input (uncached, cache read, cache write) plus output of every model call (main, title, compaction alike), the same sum as the terminal's `Σ`; omitted while 0 |
| `context` | `{"used", "window"?}`: tokens in context (the input of the latest main request: uncached, cache read, cache write, the same figure as the terminal footer and the session status); omitted before the first request and right after a compaction. `window` is the context window of that request's model under the current config, omitted when unknown |
| `project` | Which project it is in; omitted when none |
| `kind`, `dream` | Only on consolidation sessions (the sessions memory consolidation runs in): `kind` is `"dream"`, `dream` is `{"persona"?, "session"?, "manual"?, "from"?}`: whose memory (the persona; `session` only appears on older sessions), `manual: true` when a person asked for it, `from` the session it was asked from; a finished run adds `result`: `{"given", "revised", "retired", "summary", "stopped"?}`, how many memories it added, revised and retired, whether it wrote the summary, `stopped: true` when it was stopped; a run that did not finish has none. A main session list usually leaves them out (pass `dream: false`); a memory page lists those of its memory by `dream` |
| `forked` | Only on forked subagent sessions: `true`. Heads show a fork icon for it. Ordinary subagents and main sessions don't have it |
| `archived` | Only on imported read-only archives: `true`. Heads list them in their own section and replace the input box with a line saying it's an archive |
| `origin` | Where it was opened: `"terminal"` on sessions the shell integration opened (`session.terminal`), which heads show as "shell" in the session list; `"advisor"` on advisor sessions she opened by asking the advisor (child sessions, with `parent`). Other and older ones don't have it |

Only local sessions are listed; venue sessions aren't (see `venue.sessions` in [Venues](venues.en.md)). To keep up to date, subscribe to the `sessions` stream ([Connection](connection.en.md)).

**Errors**: `bad_params`; `internal_error` when the session directory can't be read.

## `session.send`

**Params**

| Name | Type | Notes |
|---|---|---|
| `session` | string, required | Which session |
| `text` | string, required | What to say |
| `attachments` | array | Attachments, in order. Each is the `blob`, `name`, `media_type` from a `blob.put` response, optionally `path` (where it was on this machine); or a path-only item `{"path", "name", "size"?}` with no `blob` (item 3 below) |
| `urgent` | boolean, default `false` | Cut in: if she's busy, she sees it at the next step |
| `shell` | boolean, default `false` | A command the person typed with `!` in the input box: `text` is the command line, run by the core in the session's working directory (item 4 below) |
| `skills` | array of strings | Skills the person named with `$` in the input box, in order (pick names from [`command.catalog`](packages.en.md#commandcatalog)'s `skills`): each skill's body goes before what was said, wrapped in `<skill name=… path=…>`; the timeline shows `text` as typed. Unknown skills or ones people can't name: `unknown_skill`; turned off by the session's preset: `command_not_allowed` (`data.why` names the preset); listed twice, with `shell`, in a venue session or said by someone else: `bad_params` |
| `prefix` | `"keep"` or `"compact"` | What the person chose when the prefix is changing: send as is, or compact and send (see **Prefix changing** below) |
| `from` | string | You are another agent program: your name; the message is recorded as yours, not the person's |
| `cwd` | string | Move to this working directory first (shell integration: each turn's directory follows the shell): checked like [`session.set_workspace`](#sessionset_workspace), `wide` included; the same directory records nothing, a change pushes `session.workspace_changed`. Not for venue sessions |
| `as`, `venue` | object | Venue sessions only; see [Venues](venues.en.md) |

**Response**: `events` (this message's number), `cwd` (after any move), `untrusted_project` (as in `session.create`); `wide: true` when moved somewhere too wide. A bad `cwd` (`path_unreadable`, `not_a_directory`, `path_forbidden`) refuses the whole message.

1. If she's idle, the message starts a turn; if busy, it's queued (`urgent` ones cut in at the next step). The turn's `turn.started` has this request's `id` as `cause`, so you can tell it's yours.
2. It returns once the message is on disk, without waiting for her answer; watch the pushes for that.
3. Attachments are checked before the session is looked up: the blob must exist and is recognized again from its content (see [Attachments](blob.en.md)).
   - **Path-only items**: a file over the upload limit (20 MiB) is not uploaded; you hand over only its path on the core's machine. `path`: an absolute path (starting with `/`, `~/`, a Windows drive letter or `\\`), no control characters, at most 4096 bytes; `name`: the file name (no path, at most 255 bytes); `size`: bytes, a non-negative integer, optional. Malformed ones get `bad_params`.
   - The core doesn't read it or check that it exists: she sees one line with the name, path and size, the content isn't included, and she reads it from that path with her tools when she needs it (permissions apply then). They can be mixed with `blob` items; order is kept.
   - It is recorded as a `{"type": "path", "name", "path", "size"?}` block in `message.user` (see [Events](events.en.md)); in the view it is still an attachment, without `blob` and `media_type`, with `path_only: true` (see [View](view.en.md)).
   - Only `session.send` and `session.redo` take them: venue sessions (messages with `as`) and the attachments of [`session.answer`](#sessionanswer) don't, `bad_params`. `path` is a path on the core's machine, so a remote connection sending one still gets `local_machine_only` (see [Connection](connection.en.md)).
4. `shell: true`: only from a person typing in a head; no permission check, no sandbox, no confirmation. It is recorded as a `message.user` with `shell`, and the reply comes once that is on disk. It runs when she hears it (first thing in a turn it starts when she's idle, or before the next model request when she's busy), in the session's working directory at that moment; the result is recorded as `shell.ran` (see [Events](events.en.md)) and she answers. Queued ones can be handed back like any message, and those never run. It is killed with its whole process group after 120 seconds; interrupting (`session.interrupt`) while it runs kills it and she doesn't answer. With `attachments` or `venue` it is `bad_params`. In the view stream the `user` entry gets a `shell` field (see [View](view.en.md)).

**Prefix changing** (only for heads that sent `caps.prefix_ask: true` in the hello): when files in your tool description and prompt overrides, personas or presets change (skills don't count: a mid-session skill change goes into the end of the messages and leaves the prefix alone), the next request's prefix changes and the cache is lost. This is judged per session: a change to another persona, to pictures such as avatars and backgrounds, or to overrides the session doesn't use leaves its prefix untouched and isn't asked about. Starting a new turn in an idle session (`session.send`, `session.redo`, or a `command.run` that starts a turn) without `prefix` is refused with `prefix_changing`; `data.files` lists the changed files that go into its prefix (paths relative to the data root, at most 20) and `data.more` counts the rest. A session that just switched persona or preset was compacted then and isn't asked again. Offer the person "Send", "Compact and send" and "Cancel", then send again with `"prefix": "keep"` or `"compact"`. `compact` compacts first; if it can't, you get the compaction's refusal (for example `nothing_to_compact`) and the message is not sent. A batch that was let through is not asked again.

```json
→ {"jsonrpc":"2.0","id":"m1","method":"session.send","params":{"session":"01a14aca-…","text":"Look at this image","attachments":[{"blob":"sha256:9f86d0…","name":"shot.png","media_type":"image/png"}]}}
← {"id":"m1","jsonrpc":"2.0","result":{"cwd":"~/src/miyu","events":[42]}}
```

Path only:

```json
→ {"jsonrpc":"2.0","id":"m2","method":"session.send","params":{"session":"01a14aca-…","text":"Have a look at this image file","attachments":[{"path":"/home/alice/Downloads/disk.iso","name":"disk.iso","size":31457280}]}}
← {"id":"m2","jsonrpc":"2.0","result":{"cwd":"~/src/miyu","events":[47]}}
```

**Errors**

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | An attachment is missing fields or malformed; a path-only attachment sent to a venue session; `from` not a string or empty after removing control characters |
| `empty_message` | No text and no attachments |
| `unknown_attachment` | An attachment's blob isn't in the core |
| `attachment_too_big` | An attachment is an image over the limits |
| `venue_session` | Speaking into a venue session without `as` |
| `shell_not_allowed` | `shell` not typed by the person in a head: with `from` or `as`, from an extension, or into a venue session |
| `shell_unavailable` | `shell`: no installed package has a tool that runs commands (not installed, or turned off) |

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

**Response**: `{"tools": [{"name", "source": {"kind", "id", "name"}, "summary"?}]}`, in the order sent to the model (byte order of names). `kind` is `builtin`, `package`, `mcp` or `script`; `source.name` follows the connection's language. Only scripts have `summary`. Tools are always sent whole (construction FL-1), so the old `loaded: false` for name-only tools no longer appears.

```json
{"tools":[{"name":"edit","source":{"kind":"builtin","id":"basesystem","name":"Base system"}},{"name":"mcp__notes__search","source":{"kind":"mcp","id":"notes","name":"notes"}}]}
```

**Errors**: `bad_params` (`session` together with `persona` or `preset`); the session lookup errors.

## `session.context`

What this session's context is made of right now: how much the system prompt, tool definitions, example dialogue, checkpoint summary and conversation each take. Computed only when called; it doesn't affect the session.

**Params**: `session` (optional; without it, what a new session would get); without `session` you may pass `persona` (`null` for none) and `preset` (`null` for no tools), as in `session.create`.

**Response**: `{"window"?, "line"?, "context"?, "used"?, "basis", "anchor"?, "estimated_total", "parts": [...]}`. `window` and `line` are the context window (the window in effect when a context limit is set) and the compaction line (absent before limits are known); `context` is the session's context limit (absent when not set); `used` is how much is in use, as in the status bar; `basis` is `measured` (counted from the provider-reported `anchor` of the latest request) or `estimated` (all local estimates). Each of `parts` is `{"part", "label", "tokens", "children"?}`: `part` is a stable id, `label` follows the connection's language, and parts of 0 are left out. With `used`, the parts are already scaled to add up to exactly `used` (parts scaled to 0 are left out); `estimated_total` is the sum of the local estimates before scaling.

Part ids: `system` (in composing order: `system.persona`, `system.venue`, `system.group`, `system.profile`, `system.project`, `system.core`, `system.clicall`, `system.show`, `system.skills`, `system.knowledge`, `system.style_lock`, `system.appended:<package>`; in older sessions, stretches that no longer match the current text go into `system.other`), `tools` (with `tools.builtin`, `tools.package:<package>`, `tools.mcp:<service>`, `tools.scripts`), `demos`, `summary`, `messages` (with `messages.user`, `messages.assistant`, `messages.memory` (with `messages.memory.resident`, `messages.memory.recall`), `messages.knowledge`, `messages.facts`, `messages.project_docs` (the repository's AGENTS.md), `messages.reports` (reports from subagents and background commands), `messages.tool_results`). Packages and services are labelled by name.

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

When she does something that needs approval she stops and asks (pushed as `tool.approval_requested`); when she asks with `ask_user` she waits too (pushed as `question.asked`). This answers both. An approval request with `expires_at` is denied automatically if no one answers by then (the config `permission.confirm.timeout`, 5 minutes by default); questions wait forever.

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

**What `session` remembers**: the pushed request's `rule` (without one, `session` can't be chosen), compact JSON: `tool` is which tool, `read` and `write` the folders whose reads and writes are allowed (with everything below). Once chosen, this tool touching those folders in this session is not asked again. A request for a command carries no `rule`: it can only be allowed once or denied.

```json
{"read":["/home/me/.ssh"],"tool":"read"}
```

**Errors**

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | Both or neither given; `reason` with questions; `decision` not one of the three |
| `not_asking` | The call isn't waiting: already answered, finished, waiting for the other kind, or past the request's `expires_at` and already denied automatically |
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

**Params**: `session`; `text`: replace the text of the message that started the turn (omitted keeps it); `attachments`: replace its attachments (omitted keeps them; an empty array removes them); `prefix`: as in `session.send` (**Prefix changing**).

**Response**: as for undo; `events` ends with the numbers of the resent messages. The new turn's `turn.started` has this request's `id` as `cause`.

**Errors**: `not_redoable` (the last turn wasn't started by a person, or there are no turns), `turn_running`, `empty_message` (nothing left after the replacement), and the attachment errors of `session.send`, and `prefix_changing`.

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

**Params**: `session`; `level`: only `full` (fully open); `read_only`: the read-only switch. Give at least one.

**Response**: `{}`. A change is pushed as `session.policy_changed`. Tightening applies at once; loosening applies from the next request. Whether to ask the person before going fully open is up to the head.

**Errors**: `bad_params`: neither given; `level` not one of the two.

## `session.set_workspace`

**Params**: `session`; `cwd`: the new working directory (absolute or starting with `~`); `dirs`: the new extra directories, replacing them all (empty removes all). Give at least one.

A `cwd` that is the workspace folder itself (the handshake's `host.workspace`) means going back to this session's own default workspace: it is replaced by it (`default_workspace` in the session list and the subscribe reply).

**Response**: `{"cwd", "dirs"}`, plus `wide: true` when too wide. A change is pushed as `session.workspace_changed`; this turn keeps the old one, the next starts with the new one.

**Errors**

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | Neither given |
| `path_unreadable` | The directory doesn't exist or can't be read |
| `not_a_directory` | It's a file |
| `path_forbidden` | It's inside Miyu's data directory (except the workspace folder `workspace/`) |
| `dir_too_wide` | A `dirs` entry is too wide |

## `session.terminal`, `session.set_terminal`

The terminal session: the one long-running session that what you say in the shell goes into (shell integration, design `22-命令行.md` section 7). One, and one per terminal for calls that carry a `terminal` key (construction T-22); the core decides which one it is and when a new one starts.

**`session.terminal`**: param `cwd` (optional): where a new one starts; without it, the default workspace. `fresh` (optional, default false): when true, start a new one as in rule 3 and point at it, whatever is pointed at now (`/new`). `terminal` (optional): which terminal, a key the head makes up, 1 to 128 characters with no control characters (otherwise `bad_params`); with it the pointer is kept per key, without it the original pointer is used. `create` (optional, default true): when false, only ask: in cases 2 and 3 below the response is `{"session": null}`, nothing starts and the pointer doesn't move (to show which session a freshly opened terminal is in before anything is said); together with `fresh: true` it's `bad_params`. Response `{"session"}`:

1. The session pointed at still exists and is a local main session (not a child, venue or one-shot session): it's that one.
2. It has been idle longer than `terminal.idle_new_after` minutes (0 never replaces): a new one starts, the pointer moves, and the response adds `renewed: {"last"}` (when the old one last had an event); the old one is left alone. Being pointed at (`session.set_terminal`, or being started) counts as activity: switching to a session that has been quiet for hours starts the idle time from the switch.
3. Nothing pointed at yet, or it was deleted or doesn't count: a new one starts and the response adds `created: true`.

A new one uses `terminal.persona` and `terminal.preset` (unset follows `persona.default` and `preset.default`). Several connections asking at once (with the same key) get one session. Each key remembers when it was last used; keys unused for 7 days are dropped when a pointer is written, and at most 64 are kept (least recently used go first). A dropped key starts over as if never pointed.

**`session.set_terminal`**: param `session`: point the terminal session at it (what `/sessions` picked). `terminal` (optional): whose pointer to change, as above. Only local main sessions; anything else is refused with `session_not_found`. Response `{"session"}`.

## `session.set_meta`

**Params**: `session`; `title`: the new title, 1 to 200 characters, `null` removes it; `pinned`: `true` pins, `false` unpins; `project`: a project id moves it in, `null` moves it out (only local main sessions can join: venue sessions, subagents and forks get `bad_params` with `data.field` `project`). Give at least one. Pin and project are separate: a head that unpins on moving into a project sends `pinned` itself.

**Response**: `{}`. A change is pushed as `session.meta_changed`. Sessions without a name get a title from the core after the first turn, also pushed as `session.meta_changed` (`by` is the kernel).

**Errors**: `bad_params`: nothing given; an empty title or one over 200 characters; an empty `project` string, or a session that cannot join (`data.field` is `project`). `unknown_project`: no such project.

## `session.retitle`

Have the model write a new title from the latest part of the conversation. It uses the same instruction and model as the title the core writes by itself, and usage is recorded the same way (`model.called` with `purpose` `title`), but it reads the last three answered turns at most (the automatic title reads only the first turn). Each turn carries what the person said and her final text answer; tool calls and reasoning are left out, and a turn she has not finished answering is not included. A title a person gave is replaced too; sessions created with `titles: false`, one-shot sessions, subagent sessions and venue sessions are titled all the same.

**Params**: `session`.

**Response**: `{}`, as soon as the request is sent; it does not wait for the model. The new title is pushed as `session.meta_changed` (`by` is the kernel), and the session-list item no longer carries `named`. If a title request is already on its way, nothing more is sent and the response is still `{}`. A turn in progress is left out and not interrupted; if her first turn has not finished, the title is written when that turn ends. If the model fails or writes no title, the title stays and only the failed `model.called` is recorded; if a person sets a title with `session.set_meta` after asking and before it arrives, theirs is kept.

**Errors**: `bad_params`: no `session`; nothing to title (she has no reply with text yet and no turn is running). `session_not_found` when there is no such session.

## `session.viewing`

A head tells the core which session it has open: send it on switching, on opening a new session, and on start. For people's heads only; an extension gets `needs_approval`.

**Params**: `session`: the open session; `null` or omitted means none.

**Response**: `{}`. The core keeps one per connection and forgets it when the connection drops. A session-list item carries `viewed: true` while at least one head has it open, and a change is pushed in `sessions.changed` like `busy`. The head that has a background session open shows its "answered / asking / needs approval" notice as usual; other heads skip it when they see `viewed`, so several heads don't each pop one.

**Errors**: `bad_params`; `session_not_found` when there is no such session.

## `session.configure`

Change the session's model, vision model, persona, preset, reasoning effort or compaction line, which applies from the next turn, or its context limit, which applies from the next request.

**Params**: `session`; at least one of:
- `model`: `<provider>/<model>` or `@<pool>`; `null` stops pinning a model and follows the configured `models.chat` again (read every turn, so a config change follows).
- `vision`: the model that describes images, written like `model`: `<provider>/<model>` or `@<pool>`; `null` stops pinning one and follows the configured `models.vision` again. When the session's model can't view images, the core has this one turn each image into text (without a pin, `models.vision`). If the pin later stops resolving (its provider or pool removed), that image counts as not described; the core does not fall back to `models.vision`.
- `persona`: a persona id; `null` means no persona.
- `preset`: a preset id; `null` switches to no tools (step P-16).
- `effort`: a reasoning effort level (step SM-1), checked against the levels of the session's model right now (`facts.reasoning` in `model.list`; when the same call changes the model, the new one's). A session on a pool cannot set one: you get `bad_params` (`data.field` is `effort`), and each member of the pool follows its own config; `null` goes back to following the config. It affects only this session: `providers.<id>.models.<model>.effort` in the config is the default for new sessions. A model without that level uses the config; switching back uses the level again.
- `context`: a context limit in tokens; `null` removes it. The window in effect is the smaller of the model's window and this limit (the limit alone when the model reports no window), and the compaction line follows that window. The limit stays with the session through model changes and pool member changes. One too small to give a compaction line gets `bad_params` (`data.field` is `context`, `data.min` is the least it accepts), and nothing changes. Typing `/context` changes it too.
- `compaction_line`: the compaction line, a percent of the window, an integer from 20 to 95; `null` stops pinning one and follows the configured `compaction.line` again. The session compacts automatically once the context reaches it. The line still stays below the window minus the output reserve and a margin: with a 200K window anything above 85 has no effect, a 1M window can reach 95.

Changing the persona or preset compacts first: the core summarizes the conversation so far under the old persona right away, and the next turn switches over and continues from the summary. Memory that follows the persona is kept in the new persona's room from then on; earlier memories stay where they were.

**Response**: `{}`. A change is pushed as `session.policy_changed`; when a model or effort change takes effect, `model.changed`. A context limit change shows at once in `limits` of the `subscribe` response and `view.status` (`window` is the window in effect, `context` the limit, absent when not set). From then on `model.effort` in the `subscribe` response and in `model.changed` is `{"level", "from"}`: `from` is `session` for the session's own level, `system` for the config. `persona` and `preset` in `session.list` and the `subscribe` response show the new values right away. A pinned vision model shows at once as `vision` in `view.status`; once removed it is absent. A pinned compaction line likewise shows at once as `compaction_line`, and `limits.compaction_line` follows it from the next turn.

**Errors**: `bad_params` (nothing given, an empty string, a wrong type; a subagent changing persona or preset, with `data.field` naming it; an `effort` the session's model doesn't have, with `data.field` `effort`, and nothing changes; a `context` that isn't a non-negative integer; a `context` too small, with `data.field` `context` and `data.min` the least accepted, and nothing changes; a `compaction_line` that isn't an integer, or outside 20 to 95, with `data.field` `compaction_line`, and nothing changes); `unknown_model`: `model` or `vision` can't be resolved, and nothing changes; `unknown_persona`; `unknown_preset`; `turn_running`: a turn is running while changing persona or preset (wait for it to finish).

## `session.fork`

Fork a child session from a main session: the child carries on from its history up to the fork point, with a byte-identical request prefix, so the cache is reused.

**Params**: `session` (the parent, a main session); `seal_after`: seal after this many turns (from 1); omitted never seals.

**Response**: `{"session": <child id>, "upto": <fork point number>}`. If the parent is in the middle of a turn, the fork point is just before that turn.

**Errors**: `bad_params`: the parent is a subagent or a fork; `seal_after` isn't an integer from 1.

## `session.delete`

Moves it to the trash, kept for 7 days. Child sessions it spawned are deleted with it.

**Params**: `session`. **Response**: `{}` once moved. Heads subscribed to it get `resync`.

**Errors**: `turn_running`: a turn is running (interrupt first); `internal_error`: couldn't move it.

## `session.prompts`

List everything you asked in a session, in order. Heads build a "jump to a prompt" list from it: for one that isn't loaded yet, page back with `view.page`'s `before` until its `seq` is on a page, then jump. Reads the log only; the session isn't loaded.

**Params**: `session`.

**Response**: `prompts`, one item per prompt:

| Name | Notes |
|---|---|
| `seq` | The number of that `message.user` |
| `turn` | Its turn: the turn it was queued in, or the turn it started if it was said while idle; omitted when neither |
| `at` | Its time, written like an event's `at` |
| `text` | The first line, trimmed, at most 100 characters; a slash-command template shows the line you typed (the same rule as `session.list`'s `preview`). Empty for images or files only |

- **Only what you said here**: what you said locally and what a bridge relayed from your private chat are listed; messages from child or parent sessions, from other people on a platform (a group message recognized as yours still counts as a platform person), injected ones (such as goal rounds), and ones from other harnesses or schedules aren't.
- **Follows the effective history**: reverted prompts are left out, unreverted ones come back, a redo (editing the last prompt) lists the final version, withdrawn queued messages are left out; compacted ones are still listed.
- **A forked session** lists only its own; prompts in the part it was forked from are listed in the parent.

**Errors**: `bad_params`; `session_not_found`; `session_broken`.

## `session.export`

Exports a session to a path you give, or into the reply: the raw bundle (ZIP) is for archiving, moving to another machine, or debugging, and importing will only accept it; Markdown is for reading. For people's heads only.

**Params**:

| Name | Notes |
|---|---|
| `session` | Session id |
| `format` | `bundle`: the raw bundle. `markdown`: Markdown |
| `to` | `{"path": "<absolute path>"}`: the core writes this file. `"inline"`: put it in the reply's `text`; Markdown only |
| `for` | Markdown only, who it's for: `read` for reading it yourself (also when left out), `agent` for another agent (Claude Code, codex and the like), `chat` for pasting into a web chat |
| `range` | Markdown only, how much: `all` (the default for `read` and `chat`); `context`, what she has in context now, from the latest compaction (the default for `agent`); `{"turns": N}`, the last N turns |
| `thinking` | Markdown only, boolean: include thinking or not. Left out, it follows `for` (`read` includes it, the others don't) |
| `tools` | Markdown only, how much of each tool call: `line`, one line; `seen`, what she saw; `full`, everything, including background command output and what tools called from scripts returned. Left out, it follows `for` (`line` for `chat`, `seen` for the others) |
| `children` | Markdown only, subagents: `reports` (also when left out; briefs and reports as `for` says); `append` also adds each subsession in full after the session, written the same way, in task order |
| `assets` | Markdown only, images and attachments: `placeholder` (also when left out); `files` exports them too: copied into a `<file name>_files/` folder next to the Markdown, linked by relative path. Only with `to: {"path"}` |
| `locale` | Markdown only: the language of display names and notes (`zh`, `ja`, anything else is English); left out, the one from the handshake |

**Result** (bundle): `path`, `bytes` (bundle size), `upto` (the last event of this session in it), `sessions` (the sessions in it), `blobs` (how many attachments and large contents).

**Result** (Markdown): `path` or `text`, `bytes`, `tokens` (an estimate, the same one as the context use in the status bar), `upto` (the last event of this session in it), `redacted` (which secrets were hidden and how many times, e.g. `{"openai": 2}`, appended subsessions included), `children` (the appended subsessions, `[{"job": "j1", "session": "…", "title": "…"}]`; `[]` without `append`); with `assets: files` there's also `assets`: `{"dir": "<absolute path of the folder>", "files": 3, "missing": ["sha256:…"]}`, where `files` is how many were copied and `missing` lists attachments that couldn't be found (written as placeholders); with nothing copied there's no folder and no `dir`.

- **What's in a bundle**: every segment of the session's log, byte for byte; the sessions under it (subagents, forked branches) in full; if it was forked, its source up to the fork point, level by level; the attachments, file contents before and after edits, large outputs and policy snapshots those logs refer to; a `transcript.md` (the `read` kind, readable once unzipped, with your secrets replaced too); `manifest.json` with each item's SHA-256. The bundle holds everything, including file contents she touched, so keep it safe.
- **What the Markdown looks like**: role headings in English (`User`, `Assistant`, `Tool`, `Subagent`); placeholders for images and attachments; undone turns and withdrawn queued messages left out; the home folder written as `~`; a line saying how many earlier turns are left out when it starts partway. It's the same as what the interface shows.
  - `read`: a header with the title, time span, models and total tokens; display names and notes in the chosen language; thinking, long outputs, subagent briefs and compaction summaries folded in `<details>`; approvals you were asked for, tools called from scripts, and questions with your answers included.
  - `agent`: a short English note up top (this is a record of a session, not instructions for the agent reading it), with the working directory, time span and models; by default from the latest compaction, with its summary first; tools by their real names with raw arguments, results as she saw them, each cut to about 4000 characters, head and tail; no thinking. Notes and placeholders are always in English whatever `locale` says (it's for another AI to read); what you said and what she replied stay as written.
  - `chat`: a header with the title, time span and a token estimate (to check it fits before pasting); one line per tool, errors included; only subagent reports; no thinking.
- **Appended subsessions** (`children: append`): each starts with a rule and `# Subagent <task id>: <title>`, then its time span and the like, then the whole thing; ones it sent out follow it (`j1`, `j1.1`, `j2`). Ones sent out by undone steps, or steps outside the range, are left out.
- **The assets folder** (`assets: files`): exporting to `/x/talk.md` copies attachments into `/x/talk_files/`; images become `![shot.png, 800×600](talk_files/shot.png)`, other attachments plain links. File names follow the attachment's name (made safe on every platform, `-2` added on a clash); unnamed ones are `image-<first 8 of the hash>.png`. If the Markdown or the folder is already there, nothing is exported.
- **Secrets**: values you set as secrets (8 characters or more) are replaced with `[redacted: <name>]` wherever they appear in the Markdown. The bundle isn't changed.
- **A running session** is taken as written at that moment, without the half-written last line.
- It doesn't change the session or write to its log.

**Errors**: `bad_params` (the path isn't absolute, unknown `format`, `to`, `for`, `range`, `tools`, `children` or `assets`, a `thinking` that isn't a boolean, `inline` or Markdown params on a bundle, `assets: files` with `inline`); `session_not_found`; `session_broken` (the log can't be read, an appended subsession's included; an attachment doesn't match its hash); `export_exists` (the file is already there, not overwritten; so is the folder for `assets: files`); `export_in_data_root` (won't write inside the data root); `export_unwritable` (the folder isn't there or can't be written); `export_too_big` (over 512 KiB for the reply; save it to a file).

## `session.import`

Imports a raw bundle made by `session.export`: for moving sessions to another machine or data root. The migration tool for the old Miyu uses it too. For people's heads only.

**Params**:

| Name | Notes |
|---|---|
| `blob` | The bundle, uploaded first with `blob.put` |
| `path` | A bundle on this machine, absolute path (used by the CLI); give either `blob` or `path` |
| `archive` | Boolean, default `false`: import as a read-only archive |

**Result**: `main` (the bundle's main session), `sessions` (newly imported now), `present` (already there, skipped).

- **Checked before written**: every item's hash and the log's sequence numbers must match before anything is written; if anything is off, nothing is imported and the data root gains nothing.
- **Ids are kept**: a session with the same id and the same content counts as already imported and is skipped; different content is refused. Importing the same bundle again changes nothing.
- **Archives** can be read, searched and exported, and she finds them when searching past conversations; they can't be continued: sending, reverting, compacting and other changes return `archived`. A session marked as an archive in the bundle stays an archive. A session that isn't an archive can be continued right away.
- Searchable right after import, without opening it first. The session list gets a change pushed.

**Errors**: `bad_params`; `unknown_attachment` (`blob` isn't there); `bad_bundle` (not a session bundle); `bundle_mismatch` (the bundle is damaged); `bundle_too_new` (made by a newer version); `session_exists` (a session with this id and different content exists; `data.session` says which).

## `view.page`

Read history page by page, from the newest back; page edges fall between turns.

**Params**

| Name | Type | Notes |
|---|---|---|
| `session` | string, required | Which session |
| `before` | positive integer | Only numbers below this; omitted is the newest page. To page back, pass the previous page's `first` |
| `turns` | 1 to 50, default 20 | At most this many turns |
| `view` | boolean, default `false` | `true` returns entries ready to draw (the view projection) instead of raw events |

**Response**: `events` (or `entries`) and `more` (whether there's more before), plus `first` and `last` when not empty; `capped: true` when fewer turns came back to stay under 1 MiB; `jobs` for jobs that finished on this page but were started earlier; `broken` when a line in the middle of the log is broken and this page stops before it.

The newest page's `last` is the last event right now: then `subscribe {"after": last}` gets only what's new.

`broken` is `{"segment", "line"}`: which log segment (file name only, such as `000000000001.jsonl`) and which line of it is broken. When the lines a page needs reach a broken line, the page reads as if the log ended just before it; a broken line outside the lines this page reads doesn't change the page and adds no `broken`. Such a session can't be loaded: sending and subscribing return `session_read_only`. Show it read-only through `view.page`, say where it broke, and start a new session to go on. The log is left as it is, not repaired.

**Errors**: `bad_params`; `session_not_found`; `session_broken` (the log doesn't parse and nothing can be shown, e.g. even the first line is broken).

## `view.detail`

**Params**: `session`; `call` (a tool call's `call_id`), `job` (a background job's id) or `compaction` (the number a compaction replaced up to), exactly one; optionally `nested_after` (an integer): only the nested calls numbered above it, plus earlier ones still running.

**Response**

- A call: `{"files": [...], "output"}`. `output` is the full result text; `files` has one item per file: `path`, `action` (`write` or `trash`), `diff` (unified, 3 lines of context), `added`, `removed`; when no diff can be computed, `skipped` (`too_big`, `binary`, `missing`).
- A compaction: `{"summary"}`: its summary text.
- When a script called tools (`miyu call`): a call's response also has `nested`, and a job's response is just `{"nested"}`. One item per call, by number: `n`, `name`, `title` (`{name, object}`, the display name in the connection's language), `args` (the raw arguments), `state` (`running`, `ok`, `error`, `denied`, `needs_approval`, `cancelled`), and when present `started` (when a running one started), `took_ms`, `reason` (the refusal or error), `result` (`{blob, bytes}`, what the script got back, fetched with `blob.get`). A `shell` still running with no result yet also answers `{"nested"}`.

A call's result never changes; asking once is enough. Nested calls grow while they run; use `nested_after` to get only the new ones.

**Errors**: `bad_params`; `session_not_found`; `unknown_call` (no result for that call, or no such compaction); `session_broken`.
