# Attachments and file contents: `blob.*`

A blob is a piece of file content the core stores, named by its content hash (`sha256:` plus 64 lowercase hex digits): the same content always has the same hash and is stored once. Images and files people attach, and images extensions hand back, are stored as blobs first and then used by hash. Chinese version: [blob.md](blob.md).

| Method | What it does |
|---|---|
| [`blob.put`](#blobput) | Upload a file in one go (a path for local heads, the content for remote ones) |
| [`blob.open`, `blob.write`, `blob.close`](#blobopen-blobwrite-blobclose) | Upload a large file in chunks |
| [`blob.get`](#blobget) | Read a blob in chunks |

An attachment is at most 20 MiB (20,971,520 bytes).

## `blob.put`

Store a file; get back its hash and what it was recognized as.

**Who can call**: people, extensions. A blob an extension stores is readable by that extension later without a `session` (see `blob.get`).

**Params**

| Name | Type | Notes |
|---|---|---|
| `path` | string | An absolute path to a local file, or one starting with `~`, `~/…`. The core reads it |
| `data` | string | The file content, standard base64 (padded with `=`). For remote heads |
| `name` | string | The file name (just a name, no directories). Optional with `path` (the last segment is used); required with `data` |
| `media_type` | string | Media type, e.g. `text/markdown`. Recognized from the content when omitted |

Give exactly one of `path` and `data`. `data` sits on one line and a line is at most 1 MiB, so it fits a bit over 700 KiB; use a chunked upload for anything bigger.

**Response**

| Name | Notes |
|---|---|
| `blob` | Content hash |
| `name` | File name |
| `media_type` | The recognized media type |
| `kind` | `image` or `file` |
| `width`, `height` | Images only: pixels |

```json
→ {"jsonrpc":"2.0","id":"c3","method":"blob.put","params":{"path":"~/Pictures/shot.png"}}
← {"jsonrpc":"2.0","id":"c3","result":{"blob":"sha256:9f86d0…","height":600,"kind":"image","media_type":"image/png","name":"shot.png","width":800}}
```

**How the type is recognized**: from the content only, never the extension.

1. PNG, JPEG, GIF and WebP whose size can be read are images, with the recognized media type. Images are at most 5 MiB and 8000 pixels on each side.
2. Content starting with `%PDF-` is `application/pdf`.
3. Anything else takes the given `media_type`; without one, valid UTF-8 text is `text/plain` and the rest `application/octet-stream`. A given `application/pdf` or `image/…` that the content doesn't match is ignored.

**Errors**

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | Both or neither of `path` and `data`; a relative `path`; `data` not base64; `data` without `name`; a malformed `name` or `media_type` |
| `attachment_in_data_root` | `path` is inside Miyu's data directory; an extension's own package folder doesn't count (by real path, so a link pointing outside is still refused) |
| `attachment_unreadable` | `path` doesn't exist, isn't a regular file, or can't be read |
| `attachment_too_big` | Over 20 MiB; an image over 5 MiB or 8000 pixels on a side |
| `internal_error` (`-32603`) | The core couldn't store it |

Reasons without a code are `-32010`.

## `blob.open`, `blob.write`, `blob.close`

Upload a file in several chunks and store it as a blob; the result is the same as `blob.put`. Remote heads use it for big files.

**Who can call**: people, extensions.

| Method | Params | Response |
|---|---|---|
| `blob.open` | `name` (required), `media_type` (optional), `size` (required, total bytes) | `{"upload": "<upload id>"}` |
| `blob.write` | `upload`, `offset` (from 0), `data` (base64, at most 512 KiB per chunk) | `{"received": <bytes received so far>}` |
| `blob.close` | `upload` | Same as `blob.put`'s response |

```json
→ {"jsonrpc":"2.0","id":"u1","method":"blob.open","params":{"name":"notes.pdf","size":1048576}}
← {"jsonrpc":"2.0","id":"u1","result":{"upload":"5f0c2a9b7e41d3c8"}}
→ {"jsonrpc":"2.0","id":"u2","method":"blob.write","params":{"upload":"5f0c2a9b7e41d3c8","offset":0,"data":"JVBERi0x…"}}
← {"jsonrpc":"2.0","id":"u2","result":{"received":524288}}
→ {"jsonrpc":"2.0","id":"u4","method":"blob.close","params":{"upload":"5f0c2a9b7e41d3c8"}}
← {"jsonrpc":"2.0","id":"u4","result":{"blob":"sha256:…","kind":"file","media_type":"application/pdf","name":"notes.pdf"}}
```

1. Each chunk's `offset` must equal the bytes received so far; otherwise you get `upload_offset` with `data.received`, and you continue from there.
2. An upload id only works on the connection that opened it. It expires when the connection closes, after 60 seconds without a `blob.write`, and after `close`; the staged content is deleted.
3. A connection can have at most 4 uploads open at once.

**Errors**

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | Malformed `name` or `media_type`; `data` not base64, over 512 KiB, or past `size` |
| `attachment_too_big` | `size` over 20 MiB; at `close`, an image over the limits |
| `too_many_uploads` | This connection already has 4 open |
| `upload_offset` | Wrong `offset`, with `data.received` |
| `upload_incomplete` | `close` before everything arrived, with `data.received` |
| `upload_unknown` | Unknown or expired id, or opened by another connection |
| `internal_error` (`-32603`) | The core couldn't store it |

## `blob.get`

Read part of a blob.

**Who can call**: people and extensions, with different reach:

- **People**: without `session`, any blob; with it, the session must be local or your own private chat (confined group chats and strangers' private chats are out of reach, as the sessions themselves are).
- **Extensions, without `session`**: only blobs it stored itself (`blob.put`, chunked upload), and hashes in responses it got back from calling another package (`package.call`) that the answering package can read itself. After the core restarts, store it again (the same content keeps the same hash).
- **Extensions, with `session`**: the session must be one it can see, and the blob must be referenced in that session (images and files in messages and tool results, shown files, images she sent); a hash merely written in some text doesn't count.

**Params**

| Name | Type | Notes |
|---|---|---|
| `blob` | string, required | Content hash |
| `session` | string | Look it up through this session |
| `offset` | integer, default 0 | Byte to start at |
| `length` | integer, default 524288 (512 KiB) | At most this many bytes, at most 512 KiB; 0 asks for the size only |

**Response**: `data` (this part, base64) and `size` (total bytes). Reading stops at the end; an `offset` past the end gives empty `data`.

```json
→ {"jsonrpc":"2.0","id":"g1","method":"blob.get","params":{"blob":"sha256:9f86d0…","offset":0,"length":4}}
← {"jsonrpc":"2.0","id":"g1","result":{"data":"iVBORw==","size":48213}}
```

**Errors**

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | `blob` isn't a content hash; malformed `session`; `length` over 512 KiB |
| `unknown_blob` | No such blob, or this connection can't read it. The two aren't told apart, so it can't be used to probe whether a blob exists |
| `internal_error` (`-32603`) | The core couldn't read it |
