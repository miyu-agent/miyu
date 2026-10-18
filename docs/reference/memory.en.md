# Memory and knowledge bases: `memory.*`, `kb.*`

Chinese version: [memory.md](memory.md).

| Method | What it does |
|---|---|
| [`memory.list`, `memory.search`](#memorylist-memorysearch) | List and search memories |
| [`memory.turns`](#memoryturns) | Search past conversations |
| [`memory.remember`, `memory.update`, `memory.forget`, `memory.restore`](#memoryremember-memoryupdate-memoryforget-memoryrestore) | Remember, change, retire or clear, restore |
| [`memory.dream`](#memorydream) | Organize memory now |
| [`kb.list`, `kb.search`, `kb.read`](#kblist-kbsearch-kbread) | List knowledge bases, search, read a page |
| [`kb.create`, `kb.update`, `kb.mount`, `kb.delete`, `kb.add`, `kb.remove`, `kb.reindex`](#managing-knowledge-bases) | Manage knowledge bases |

**Who can call**: people, extensions. People going through the protocol are on the owner's side: everything can be listed, searched and changed.

## Which memory

Memories are kept in separate places: per persona (shared by all sessions with that persona), per session (sessions whose memory scope is `session`), and one more for sessions without a persona. Every `memory.*` method takes one of these to say which:

| Name | Notes |
|---|---|
| `persona` | That persona's memory |
| `session` | The memory that session uses |

With neither, the default persona's. To follow changes, subscribe to the `memory` stream ([Connection](connection.en.md)).

**Common errors**

| `data.reason` | When |
|---|---|
| `memory_not_installed` | The memory package isn't installed (once installed, everything remembered before is there) |
| `memory_unavailable` | No memory here: the session's scope is `off`, or a venue session |
| `unknown_persona`, `session_not_found` | No such persona or session |
| `bad_params` (`-32602`) | Both given |

## A memory

`{"id", "class", "text", "by", "at", "sources", "retired", "used", "used_at", "venue", "keywords"}`:

| Name | Notes |
|---|---|
| `id` | `m<number>` |
| `class` | `user` (about the person), `feedback` (what the person wants from her), `episode` (something that happened), `reference` (where to find something) |
| `text` | The memory, at most 120 characters |
| `by` | `person` (a person remembered it) or `tool` (she did) |
| `at` | When it was remembered |
| `sources` | Where it came from `[{"session", "turn"}]` |
| `retired` | Why it was retired, `null` if it wasn't |
| `used`, `used_at` | How many times it was used, the last time |
| `venue` | Which venue it was said in, `null` for local |
| `keywords` | Words used only to find it (one line, comma-separated, at most 200 characters), `null` if none: searches and recall match them, she never sees this line |

## `memory.list`, `memory.search`

| Method | Params | Response |
|---|---|---|
| `memory.list` | `class`, `from` (a session id: sourced in it), `forgotten` (boolean, include retired), `limit` (1 to 500, default 50) | `{"memories": [...]}`, newest first |
| `memory.search` | `query` (required), `forgotten`, `limit` (1 to 500, default 10) | `{"memories": [...]}`, most relevant first |

## `memory.turns`

Search this memory's past conversations: one entry per turn a person started, matched by keywords and meaning.

**Params**: `query` (required), `limit` (1 to 500, default 10). **Response**: `{"turns": [{"session", "turn", "at", "text"}…]}`: `text` is what the person said, a blank line, and her last answer. Undone turns and deleted sessions aren't included.

## `memory.remember`, `memory.update`, `memory.forget`, `memory.restore`

| Method | Params | Response |
|---|---|---|
| `memory.remember` | `class`, `text` (both required); optional `keywords` | `{"id"}` |
| `memory.update` | `id`, and at least one of `text`, `keywords` | `{"id"}`: with `text`, the new version (the old one is retired; without `keywords` the keywords carry over); with only `keywords`, the same memory, same id; empty `keywords` removes them |
| `memory.forget` | `id` (optionally `why`); or `clear`: `session` (clear what this session remembered; `session` is then required), `me` (clear the whole memory) | `{}` when retiring, `{"cleared": <count>}` when clearing |
| `memory.restore` | `id` | `{}`: a retired memory counts again |

Answered once on disk. The same request `id` sent again counts once, also after a core restart. Cleared memories can't be recovered and never show up again.

**Errors**

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | Empty text; wrong `class`; malformed `id`; not exactly one of `id` and `clear`; `memory.update` with neither `text` nor `keywords` |
| `memory_too_long` | Text over 120 characters or keywords over 200, `data.chars`, `data.limit`, `data.field` is `text` or `keywords` |
| `unknown_memory` | No such memory |
| `memory_not_current` | Already changed, retired or cleared |

## `memory.dream`

Organize now: with `session`, first extract what that session hasn't had extracted yet, then merge the new with the old (duplicates merged, outdated ones retired, a summary of the person written). Answered in the background; takes a few seconds to a dozen or so.

**Params**: only `persona` or `session`. **Response**: `{"given", "retired", "revised", "summary"}`: how many were handed in, retired, revised, and whether the summary changed (boolean). With nothing to organize, all are zero and no model is called.

**Errors**: `memory_busy` (already organizing); `dream_failed` (merging failed, `data.message`).

## `kb.list`, `kb.search`, `kb.read`

Knowledge bases belong to the admin account; each is a folder of documents (Markdown, PDF, web pages…) that the core indexes for search. Whether the knowledge base package is installed doesn't matter here: the files are the person's.

| Method | Params | Response |
|---|---|---|
| `kb.list` | none | `{"bases": [{"id", "name", "description", "documents", "chunks", "auto"}…]}`, by id; `auto` is whether automatic recall is on; unreadable ones have `error`, ones from packages have `package` |
| `kb.search` | `query` (required), `kb` (only this one), `limit` (1 to 50, default 10) | `{"hits": [{"kb", "path", "chunk", "heading", "text", "at"}…]}`, most relevant first; PDF hits also have `page` |
| `kb.read` | `kb`, `path` (required), `page` (from 1, default 1) | `{"text", "page", "pages"}`: 32 KiB per page; PDFs use their own pages |

`heading` is the heading path (`Grammar › te-form`); `at` is when the document changed.

**Errors**: `bad_params` (`page` 0 or past `pages`); `unknown_kb`; `unknown_document`; `internal_error` (a broken PDF, a file over 64 MiB).

## Managing knowledge bases

| Method | Params | Response |
|---|---|---|
| `kb.create` | `kb` (required), `name`, `description`, `auto` | A knowledge base |
| `kb.update` | `kb`; at least one of `name`, `description`, `auto` (empty string or `false` removes) | A knowledge base |
| `kb.mount` | `kb`, `folder` (absolute or `~/`); `name`, `description`, `auto` | A knowledge base: mounts an existing folder |
| `kb.delete` | `kb` | `{"trash"}`: where it went in the system trash; a mounted one only loses its registration, the folder stays |
| `kb.add` | `kb`, `path`, `blob` (from `blob.put`), `replace` (boolean) | `{}`: add a document in any format |
| `kb.remove` | `kb`, `path` | `{"trash"}` |
| `kb.reindex` | `kb` | A knowledge base: the index is deleted and rebuilt from the files |

**Errors**

| `data.reason` | When |
|---|---|
| `kb_exists`, `document_exists` | Already exists (adding without `replace`) |
| `kb_read_only` | Adding to or removing from a mounted one |
| `kb_packaged` | Changing one that came with a package (only `auto` can change) |
| `folder_unavailable` | The folder to mount can't be used, `data.message` |
| `trash_failed` | Couldn't move it to the trash, `data.message` |
| `unknown_attachment` | `blob` doesn't exist |
| `unknown_kb`, `unknown_document`, `bad_params` | As above |
