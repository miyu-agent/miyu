# Files: `fs.*`

Local file operations for heads: path completion, finding files, resolving real paths, reading a file in chunks. Chinese version: [fs.md](fs.md).

| Method | What it does |
|---|---|
| [`fs.list`](#fslist) | List one directory level (path completion in the input box) |
| [`fs.find`](#fsfind) | Fuzzy-find files in a directory |
| [`fs.realpath`](#fsrealpath) | Resolve a path to its real location |
| [`fs.read`](#fsread) | Read a local file in chunks |

**Who can call**: people; extensions need the `fs.read` capability.

**Boundary**: inside Miyu's data directory (by default `~/.miyu`), only the workspace folder `workspace/` can be listed, searched and read; anything else gets `path_forbidden`. `fs.realpath` only reports a location without reading, so it doesn't check the boundary.

**Paths**: absolute, or starting with `~`, `~/…` (joined to the system home).

## `fs.list`

**Params**

| Name | Type | Notes |
|---|---|---|
| `cwd` | string, required | Relative paths are joined to it |
| `dir` | string, default `""` | The directory part typed so far: `~` follows home, absolute stays, anything else follows `cwd` |
| `prefix` | string, default `""` | Start of the name |

**Response** `{"items": [...], "partial"}`. An item:

| Name | Notes |
|---|---|
| `path` | How the list shows it; directories end in `/` |
| `full` | Absolute path, in the platform's form |
| `dir` | Whether it's a directory |
| `size` | A file's bytes; none for directories |
| `marks` | Positions (character indexes) in `path` that matched `prefix` |

Names match `prefix` at the start, ignoring case; dotfiles are listed only when `prefix` starts with `.`. Directories first, then files, each by name. At most 50; when cut, `partial` is `true`.

```json
→ {"jsonrpc":"2.0","id":"f1","method":"fs.list","params":{"cwd":"~/src/miyu","dir":"src/","prefix":"ma"}}
← {"id":"f1","jsonrpc":"2.0","result":{"items":[{"dir":false,"full":"/home/me/src/miyu/src/main.rs","marks":[4,5],"path":"src/main.rs","size":2048}],"partial":false}}
```

**Errors**: `path_unreadable` (doesn't exist, not a directory, no permission); `path_forbidden`.

## `fs.find`

**Params**

| Name | Type | Notes |
|---|---|---|
| `cwd` | string, required | Directory to search in |
| `query` | string, default `""` | What was typed |
| `fresh` | boolean, default `false` | Pass `true` when opening the list: rebuild an index older than 10 seconds |
| `wait` | boolean, default `false` | If the index is still being built, answer when it's done (answered in the background) |

**Response**: as for `fs.list`, plus `building`: the index is still being built and this answer covers the part done so far.

1. The core builds a file index of `cwd`: honors `.gitignore`, skips hidden directories, `node_modules`, `target`, doesn't follow links; at most 8 levels and 20000 entries (`partial` is `true` when full). `path` is relative to `cwd`, joined with `/`.
2. A path matches when every typed character appears in it in order (ignoring case); matches in the file name, at the start of a segment, and consecutive ones rank higher; a file name equal to the query comes first. At most 50.

**Errors**: as for `fs.list`.

## `fs.realpath`

**Params**: `path` (required); `cwd`: only needed when `path` is relative.

**Response**: `{"path": <real location>}`. Links are followed to the target; parts that don't exist yet are appended to the last existing level as written.

**Errors**: `bad_params` (relative without `cwd`); `path_unreadable` (no level exists, a link points nowhere, no home directory).

## `fs.read`

**Params**: `path` (required, not relative); `offset` (default 0); `length` (default 512 KiB, at most 512 KiB, 0 asks for the size only).

**Response**: `{"data", "size"}`, as for [`blob.get`](blob.en.md#blobget). No link anywhere on the path is followed.

**Errors**: `bad_params` (relative path; `length` over 512 KiB); `path_unreadable` (doesn't exist, not a regular file, no permission); `path_forbidden`.
