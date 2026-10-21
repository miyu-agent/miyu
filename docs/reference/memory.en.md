# Memory and knowledge bases: `memory.*`, `kb.*`

Chinese version: [memory.md](memory.md).

| Method | What it does |
|---|---|
| [`memory.list`, `memory.search`](#memorylist-memorysearch) | List and search memories |
| [`memory.turns`](#memoryturns) | Search past conversations |
| [`memory.remember`, `memory.update`, `memory.forget`, `memory.restore`](#memoryremember-memoryupdate-memoryforget-memoryrestore) | Remember, change, retire or clear, restore |
| [`memory.dream`](#memorydream) | Organize memory now |
| [`memory.dream_stop`](#memorydream_stop) | Stop the organizing in progress |
| [`kb.list`, `kb.search`, `kb.read`, `kb.documents`](#kblist-kbsearch-kbread-kbdocuments) | List knowledge bases, search, read a page, list the documents in one |
| [`kb.create`, `kb.update`, `kb.mount`, `kb.delete`, `kb.add`, `kb.remove`, `kb.reindex`](#managing-knowledge-bases) | Manage knowledge bases |

**Who can call**: people, extensions; extensions writing (remembering, changing, forgetting, restoring, consolidating; adding, changing, mounting, removing and reindexing knowledge bases) need the `memory.write` capability. People going through the protocol are on the owner's side: everything can be listed, searched and changed.

## Which memory

Memories are kept in separate places: per persona (shared by all sessions with that persona), per session (sessions whose memory scope is `session`), and one more for sessions without a persona. Every `memory.*` method takes one of these to say which:

| Name | Notes |
|---|---|
| `persona` | That persona's memory; `_none` is the one for sessions without a persona (not in the persona list; list it yourself) |
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

`{"id", "class", "text", "by", "at", "sources", "retired", "used", "used_at", "venue", "keywords", "who"}`:

| Name | Notes |
|---|---|
| `id` | `m<number>` |
| `class` | `user` (about the owner), `feedback` (what the person wants from her), `episode` (something that happened), `reference` (where to find something), `person` (someone else she knows, shown as "people you know") |
| `text` | The memory, at most 120 characters |
| `by` | `person` (a person remembered it) or `tool` (she did) |
| `at` | When it was remembered |
| `sources` | Where it came from `[{"session", "turn"}]` |
| `retired` | Why it was retired, `null` if it wasn't |
| `used`, `used_at` | How many times it was used, the last time |
| `venue` | Which venue it was said in, `null` for local |
| `keywords` | Words used only to find it (one line, comma-separated, at most 200 characters), `null` if none: searches and recall match them, she never sees this line |
| `who` | Who a `person` memory is about: a platform id (`qq:10086`) or a name; `null` for other classes |

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
| `memory.remember` | `class`, `text` (both required); optional `keywords`; `who`, required when `class` is `person` and ignored otherwise | `{"id"}` |
| `memory.update` | `id`, and at least one of `text`, `keywords`, `who` | `{"id"}`: with `text`, the new version (the old one is retired; without `keywords` or `who` they carry over); with only `keywords`, the same memory, same id; empty `keywords` removes them; with only `who`, a new version with the same text |
| `memory.forget` | `id` (optionally `why`); or `clear`: `session` (clear what this session remembered; `session` is then required), `me` (clear the whole memory) | `{}` when retiring, `{"cleared": <count>}` when clearing |
| `memory.restore` | `id` | `{}`: a retired memory counts again |

Answered once on disk. The same request `id` sent again counts once, also after a core restart. Cleared memories can't be recovered and never show up again.

**Errors**

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | Empty text; wrong `class`; `person` without `who`; malformed `id`; not exactly one of `id` and `clear`; `memory.update` with none of `text`, `keywords`, `who` |
| `memory_too_long` | Text over 120 characters or keywords over 200, `data.chars`, `data.limit`, `data.field` is `text` or `keywords` |
| `unknown_memory` | No such memory |
| `memory_not_current` | Already changed, retired or cleared |

## `memory.dream`

Organize now: with `session`, first extract what that session hasn't had extracted yet, then merge the new with the old (duplicates merged, outdated ones retired, a summary of the person written). Answered in the background; takes a few seconds to a dozen or so.

**Params**: only `persona` or `session`. **Response**: `{"given", "retired", "revised", "summary"}`: how many were handed in, retired, revised, and whether the summary changed (boolean). With no memories at all, all are zero and no model is called; with nothing new it still looks over the older memories whose turn it is (R-19). If it's stopped ([`memory.dream_stop`](#memorydream_stop), or for one asked from a session that session's `/dream stop` or `/stop`), it still answers, with the counts merged before the stop and `"stopped": true` (R-22, R-23): the batch being merged is dropped and merged next time; finished batches stay.

**Errors**: `memory_busy` (already organizing); `dream_failed` (merging failed, `data.message`).

**Progress and what it did**: read it from the memory log (subscribe to the `memory` stream). Organizing merges batch by batch; both a requested one and one in the background are written:

| `kind` | `body` | What it is |
|---|---|---|
| `ext.memory.merging` | `upto`, `fresh`, `left`, `manual`, `given` | A batch starts: how many new memories in it, how many are left including it, whether a person asked |
| `ext.memory.saved` | with `replaces` | A memory revised: the new text; the old one is the `replaces` memory |
| `ext.memory.retired` | `id`, `why`, `kind`, `kept` | A memory removed: `kind` is `outdated` (superseded by a later one), `repeat` (a duplicate) or `worthless` (not needed later); `kept` is the one kept instead (none for `worthless`). A person's removal has no `kind` or `kept` |
| `ext.memory.summary` | `text` | The new summary about the person |
| `ext.memory.tagged` | `id`, `keywords` | Keywords filled in: after the batch's `merged` |
| `ext.memory.merged` | `upto`, `given`, `revised`, `retired`, `failed`? | The batch is done (`failed`: three failures in a row, skipped) |
| `ext.memory.merge_failed` | `upto`, `why`, `tries`? | The batch failed this time; `why` `interrupted` means the core stopped midway, `stopped` that the person stopped it (R-22); neither has `tries` |
| `ext.memory.swept` | `upto`, `count` | How many older memories were looked over again, and up to which one (R-19): after the batch's `merged` |

- A `merging` and the `merged` or `merge_failed` that ends it carry the same `upto`; the revisions, removals and summary between them whose `by` is the memory module are what the batch did (another session may be extracting or a person editing at the same time; `by` tells them apart). One not yet ended is in progress.
- Progress: the first batch's `left` is the total; the `fresh` of the ended batches adds up to what is done. There is no finer progress within a batch.
- To show what she is writing during a requested organize (R-21): `merging`'s `given` lists the memories handed in for the batch, so they can drift by while waiting; and a push that isn't logged, `memory.dreaming`: one each time she finishes a revision (`kind: revised`, `id`, `text`) or a removal (`kind: retired`, `id`, `retire`, `kept`, `why`), and for the summary every 20 more characters and when done (`kind: summary`, `chars`, `done`), all with the batch's `upto`. It's what she wrote at that moment; what lands is checked as a whole and may have fewer, so trust the log. Missed ones aren't replayed.
- Each run, the first batch with room also looks over up to 20 older memories in turn (by id, starting over after the last), and revisions or removals of them count as the batch's. A requested run with nothing new still looks them over: that batch's `fresh` and `left` are 0.
- When a session asks, what is extracted first (`ext.memory.saved` by the memory module, with sources) comes before the first `merging`, followed by `ext.memory.extracted`.

## `memory.dream_stop`

Stop the organizing a person asked for in this room (R-23), however it was asked (`memory.dream`, or `/dream` in a session); background organizing isn't stopped. Finished batches stay; the batch being merged is dropped and merged next time, and the memory log gets an `ext.memory.merge_failed` with `why: stopped`. The stopped `memory.dream` still answers, with `"stopped": true`. `memory.dream` is answered in the background, so send this on the same connection.

**Params**: only `persona` or `session`, as for `memory.dream`. **Response**: `{"stopped": true}`.

**Errors**: `not_dreaming` (nothing a person asked for is organizing in this room).

## `kb.list`, `kb.search`, `kb.read`, `kb.documents`

Knowledge bases belong to the admin account; each is a folder of documents (Markdown, PDF, web pages…) that the core indexes for search. Whether the knowledge base package is installed doesn't matter here: the files are the person's.

| Method | Params | Response |
|---|---|---|
| `kb.list` | none | `{"bases": [{"id", "name", "description", "documents", "chunks", "auto"}…]}`, by id; `auto` is whether automatic recall is on; unreadable ones have `error`, ones from packages have `package`, mounted ones have `mount` (the folder as written); `indexing` is whether it is being built right now (one being built elsewhere is not waited for; it is listed as the index stands), and one built before has `indexed_at` (when it last finished); subscribe to the `kb` stream for build progress |
| `kb.search` | `query` (required), `kb` (only this one), `limit` (1 to 50, default 10) | `{"hits": [{"kb", "path", "chunk", "heading", "text", "at"}…]}`, most relevant first; PDF hits also have `page` |
| `kb.read` | `kb`, `path` (required), `page` (from 1, default 1) | `{"text", "page", "pages"}`: 32 KiB per page; PDFs use their own pages |
| `kb.documents` | `kb` (required) | `{"documents": [{"path", "kind", "size", "at", "chunks"}…], "folders": [{"path", "reason", "why"}…]}`, by path: every document in the base; ones that couldn't be read this time also have `reason` and `why` |

`heading` is the heading path (`Grammar › te-form`); `at` is when the document changed.

In `kb.documents`, `kind` is `markdown`, `text`, `pdf` or `html`; `size` is in bytes; `chunks` is how many pieces are in the index (an unreadable document that was indexed before keeps its old ones). `reason` is one of `too_large`, `not_utf8`, `no_text` (a PDF with no text: scanned or only pictures), `no_extractor` (PDFs and web pages need `miyu-extract`), `extract_failed`, `unreadable`; show your own words for it, `why` is an English sentence for logs. `folders` are subfolders that couldn't be read (common in mounted ones). Files that aren't documents aren't listed. A mounted folder that's gone: `folder_unavailable`.

**Errors**: `bad_params` (`page` 0 or past `pages`); `unknown_kb`; `unknown_document`; `internal_error` (a broken PDF, a file over 64 MiB).

## Managing knowledge bases

| Method | Params | Response |
|---|---|---|
| `kb.create` | `kb` (required), `name`, `description`, `auto` | A knowledge base |
| `kb.update` | `kb`; at least one of `name`, `description`, `auto` (empty string or `false` removes) | A knowledge base |
| `kb.mount` | `kb`, `folder` (absolute or `~/`); `name`, `description`, `auto` | A knowledge base: mounts an existing folder |
| `kb.delete` | `kb` | `{"trash"}`: where it went in the system trash; a mounted one only loses its registration, the folder stays |
| `kb.add` | `kb`, `path`, `blob` (from `blob.put`), `replace` (boolean) | `{}`: add a document in any format |
| `kb.remove` | `kb`, and `path` (one) or `paths` (several): a document, or a folder (a path inside the base; the whole folder goes) | With `path`, `{"trash"}`; with `paths`, `{"trash": [...]}` in order. If any one is missing, nothing is removed; both or neither given, repeats, or one inside another: `bad_params` |
| `kb.reindex` | `kb` | A knowledge base: the index is deleted and rebuilt from the files |

**Errors**

| `data.reason` | When |
|---|---|
| `kb_exists`, `document_exists` | Already exists (adding without `replace`) |
| `kb_read_only` | Adding to or removing from a mounted one |
| `kb_packaged` | Changing one that came with a package (only `auto` can change) |
| `folder_unavailable` | The folder to mount can't be used, or for `kb.documents` a mounted folder is gone, `data.message` |
| `trash_failed` | Couldn't move it to the trash, `data.message` |
| `unknown_attachment` | `blob` doesn't exist |
| `unknown_kb`, `unknown_document`, `bad_params` | As above |
