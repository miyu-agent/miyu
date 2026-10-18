# 附件与文件内容：`blob.*`

blob 是核心存的一份文件内容，用内容哈希（`sha256:` 加 64 位小写十六进制）指认：同样的内容永远是同一个哈希，存几次都只有一份。人发消息时附的图片、文件，扩展交回的图，都先存成 blob，再拿哈希去用。英文版：[blob.en.md](blob.en.md)。

| 方法 | 做什么 |
|---|---|
| [`blob.put`](#blobput) | 一次传完一个文件（本机给路径，远程给内容） |
| [`blob.open`、`blob.write`、`blob.close`](#blobopenblobwriteblobclose) | 分块传一个大文件 |
| [`blob.get`](#blobget) | 分块读一个 blob |

一个附件最多 20 MiB（20,971,520 字节）。

## `blob.put`

存一个文件，交回它的哈希和认出来是什么。

**谁能调**：人、扩展。扩展存的，它自己之后不写 `session` 也读得到（见 `blob.get`）。

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `path` | 字符串 | 本机文件的绝对路径，或者 `~`、`~/…` 开头的。核心自己去读 |
| `data` | 字符串 | 文件内容，标准 base64（末尾补 `=`）。远程的头用这个 |
| `name` | 字符串 | 文件名（只是名字，不带目录）。传 `path` 的可以不写，取路径的最后一段；传 `data` 的必写 |
| `media_type` | 字符串 | 媒体类型，例如 `text/markdown`。不写的照内容认 |

`path`、`data` 正好写一个。`data` 放在一行里，一行最长 1 MiB，所以最多七百多 KiB；更大的用分块上传。

**回应**

| 名字 | 说明 |
|---|---|
| `blob` | 内容哈希 |
| `name` | 文件名 |
| `media_type` | 认出来的媒体类型 |
| `kind` | `image` 或 `file` |
| `width`、`height` | 只有图片有：像素 |

```json
→ {"jsonrpc":"2.0","id":"c3","method":"blob.put","params":{"path":"~/Pictures/shot.png"}}
← {"jsonrpc":"2.0","id":"c3","result":{"blob":"sha256:9f86d0…","height":600,"kind":"image","media_type":"image/png","name":"shot.png","width":800}}
```

**怎么认是什么**：只看内容，不看扩展名。

1. PNG、JPEG、GIF、WebP，量得出宽高的：图片，媒体类型照认出的。图片最大 5 MiB、每边最多 8000 像素。
2. 开头是 `%PDF-` 的：`application/pdf`。
3. 别的照 `media_type` 写的；没写的，整份是 UTF-8 文本的是 `text/plain`，否则 `application/octet-stream`。`media_type` 写成 `application/pdf` 或 `image/…`、内容又不是的，不算。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | `path`、`data` 两个都写或都不写；`path` 是相对路径；`data` 不是 base64；传 `data` 没写 `name`；`name`、`media_type` 写法不对 |
| `attachment_in_data_root` | `path` 落在 Miyu 的数据目录里；扩展自己的包目录里的不算（照真实位置，链接指到外面的照旧拒） |
| `attachment_unreadable` | `path` 不存在、不是普通文件、没有权限读 |
| `attachment_too_big` | 超过 20 MiB；图片超过 5 MiB 或某一边超过 8000 像素 |
| `internal_error`（`-32603`） | 核心存不下来 |

没写码的都是 `-32010`。

## `blob.open`、`blob.write`、`blob.close`

把一个文件拆成几块传，最后存成 blob，结果和 `blob.put` 一样。远程的头传大文件用它。

**谁能调**：人、扩展。

| 方法 | 参数 | 回应 |
|---|---|---|
| `blob.open` | `name`（必写）、`media_type`（可不写）、`size`（必写，一共几个字节） | `{"upload": "<上传编号>"}` |
| `blob.write` | `upload`、`offset`（从 0 数）、`data`（base64，一块最多 512 KiB） | `{"received": <一共收到几个字节>}` |
| `blob.close` | `upload` | 同 `blob.put` 的回应 |

```json
→ {"jsonrpc":"2.0","id":"u1","method":"blob.open","params":{"name":"notes.pdf","size":1048576}}
← {"jsonrpc":"2.0","id":"u1","result":{"upload":"5f0c2a9b7e41d3c8"}}
→ {"jsonrpc":"2.0","id":"u2","method":"blob.write","params":{"upload":"5f0c2a9b7e41d3c8","offset":0,"data":"JVBERi0x…"}}
← {"jsonrpc":"2.0","id":"u2","result":{"received":524288}}
→ {"jsonrpc":"2.0","id":"u4","method":"blob.close","params":{"upload":"5f0c2a9b7e41d3c8"}}
← {"jsonrpc":"2.0","id":"u4","result":{"blob":"sha256:…","kind":"file","media_type":"application/pdf","name":"notes.pdf"}}
```

1. 每块的 `offset` 必须正好等于已经收到的字节数；对不上回 `upload_offset`，`data.received` 说收到了多少，从那里接着传。
2. 上传编号只在开它的连接上有效。连接断了、60 秒没有 `blob.write`、`close` 以后，编号作废，暂存的内容删掉。
3. 一个连接同时最多开 4 个上传。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | `name`、`media_type` 写法不对；`data` 不是 base64、超过 512 KiB、加上它超过 `size` |
| `attachment_too_big` | `size` 超过 20 MiB；`close` 时图片超过上限 |
| `too_many_uploads` | 这个连接已经开着 4 个 |
| `upload_offset` | `offset` 不对，另带 `data.received` |
| `upload_incomplete` | `close` 时还没收齐，另带 `data.received` |
| `upload_unknown` | 编号不认识、作废了、是别的连接开的 |
| `internal_error`（`-32603`） | 核心存不下来 |

## `blob.get`

从一个 blob 里读一段。

**谁能调**：人、扩展，读得到的不一样：

- **人**：不写 `session` 的读管理员名下的；写了的照那个会话的属主找（群会话这类归系统账号的也读得到）。
- **扩展，不写 `session`**：只读得到它自己 `blob.put`、分块上传存的，和它调别的包（`package.call`）拿回来的回应里、答的那个包自己读得到的哈希。核心重起以后要再存一次（同样的内容还是那个哈希）。
- **扩展，写了 `session`**：那个会话要是它能看的，blob 也要被那个会话引用过（消息、工具结果里的图片和文件，展示出来的文件，发出去的图）；只在正文里写着哈希的不算。

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `blob` | 字符串，必写 | 内容哈希 |
| `session` | 字符串 | 照这个会话找 |
| `offset` | 整数，默认 0 | 从第几个字节读起 |
| `length` | 整数，默认 524288（512 KiB） | 最多读几个字节，最多 512 KiB；写 0 只问大小 |

**回应**：`data`（这一段，base64）、`size`（整份一共几个字节）。读到结尾就停；`offset` 过了结尾，`data` 是空的。

```json
→ {"jsonrpc":"2.0","id":"g1","method":"blob.get","params":{"blob":"sha256:9f86d0…","offset":0,"length":4}}
← {"jsonrpc":"2.0","id":"g1","result":{"data":"iVBORw==","size":48213}}
```

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | `blob` 不是内容哈希；`session` 写法不对；`length` 超过 512 KiB |
| `unknown_blob` | 没有这个 blob，或者这个连接读不到它。两种不分开说，免得拿它探某个 blob 在不在 |
| `internal_error`（`-32603`） | 核心读不出来 |
