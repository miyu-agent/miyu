# 附件与文件内容：`blob.*`

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

blob 是核心存储的一份文件内容，用内容哈希（`sha256:` 加 64 位小写十六进制）来指认：同样的内容永远是同一个哈希，无论存几次都只有一份。人发送消息时附带的图片、文件，以及扩展返回的图片，都要先存成 blob，再通过哈希来使用。

| 方法 | 做什么 |
|---|---|
| [`blob.put`](#blobput) | 一次上传完一个文件（本机的头给出路径，远程的头给出内容；头是连接核心的界面程序，例如终端界面、网页界面） |
| [`blob.open`、`blob.write`、`blob.close`](#blobopenblobwriteblobclose) | 分块上传一个大文件 |
| [`blob.get`](#blobget) | 分块读取一个 blob |
| [`session.shown`](#sessionshown) | 分块读取 AI 展示过的一个文件 |

一个附件最多 20 MiB（20,971,520 字节）。更大的文件不上传内容，而是在 [`session.send`](sessions.md#sessionsend) 的 `attachments` 里只给出它在核心所在机器上的路径（`{"path", "name", "size"?}`，见那一节第 3 条）。

## `blob.put`

这个方法存储一个文件，返回它的哈希和识别出的类型。

**谁能调用**：人、扩展。扩展存储的 blob，它自己之后不填写 `session` 也能读取到（见 `blob.get`）。

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `path` | 字符串 | 本机文件的绝对路径，或者以 `~`、`~/…` 开头的路径。核心会自己去读取这个文件 |
| `data` | 字符串 | 文件内容，使用标准 base64（末尾补 `=`）。远程的头使用这个参数 |
| `name` | 字符串 | 文件名（只是名字，不带目录）。传 `path` 时可以不填写，此时取路径的最后一段；传 `data` 时必须填写 |
| `media_type` | 字符串 | 媒体类型，例如 `text/markdown`。不填写时根据内容识别 |

`path` 和 `data` 必须正好填写一个。由于 `data` 放在一行里，而一行最长 1 MiB，所以最多只能放七百多 KiB；更大的文件要使用分块上传。

**回应**

| 名字 | 说明 |
|---|---|
| `blob` | 内容哈希 |
| `name` | 文件名 |
| `media_type` | 识别出的媒体类型 |
| `kind` | `image` 或 `file` |
| `width`、`height` | 只有图片才有：宽和高的像素数 |

```json
→ {"jsonrpc":"2.0","id":"c3","method":"blob.put","params":{"path":"~/Pictures/shot.png"}}
← {"jsonrpc":"2.0","id":"c3","result":{"blob":"sha256:9f86d0…","height":600,"kind":"image","media_type":"image/png","name":"shot.png","width":800}}
```

**怎么识别类型**：只看内容，不看扩展名。

1. 能量出宽高的 PNG、JPEG、GIF、WebP 识别为图片，媒体类型使用识别出的类型。图片最大 5 MiB，每边最多 8000 像素。
2. 开头是 `%PDF-` 的内容识别为 `application/pdf`。
3. 其他内容使用 `media_type` 填写的类型；没有填写时，整份是 UTF-8 文本的识别为 `text/plain`，否则识别为 `application/octet-stream`。如果 `media_type` 填写成 `application/pdf` 或 `image/…`，内容却不是这种类型，填写的类型不算数。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | `path` 和 `data` 两个都填写了或者都没有填写；`path` 是相对路径；`data` 不是 base64；传了 `data` 却没有填写 `name`；`name` 或 `media_type` 的写法不对 |
| `attachment_in_data_root` | `path` 位于 Miyu 的数据目录里；扩展自己的包目录里的文件不算（按照真实位置判断，所以链接指到外面的仍然会被拒绝） |
| `attachment_unreadable` | `path` 不存在、不是普通文件，或者没有权限读取 |
| `attachment_too_big` | 超过 20 MiB；或者图片超过 5 MiB、某一边超过 8000 像素 |
| `internal_error`（`-32603`） | 核心无法存储这个文件 |

表里没有写错误码的都是 `-32010`。

## `blob.open`、`blob.write`、`blob.close`

这组方法把一个文件拆成几块上传，最后存成 blob，结果和 `blob.put` 相同。远程的头上传大文件时使用它们。

**谁能调用**：人、扩展。

| 方法 | 参数 | 回应 |
|---|---|---|
| `blob.open` | `name`（必须填写）、`media_type`（可以不填写）、`size`（必须填写，一共几个字节） | `{"upload": "<上传编号>"}` |
| `blob.write` | `upload`、`offset`（从 0 开始数）、`data`（base64，一块最多 512 KiB） | `{"received": <一共收到几个字节>}` |
| `blob.close` | `upload` | 和 `blob.put` 的回应相同 |

```json
→ {"jsonrpc":"2.0","id":"u1","method":"blob.open","params":{"name":"notes.pdf","size":1048576}}
← {"jsonrpc":"2.0","id":"u1","result":{"upload":"5f0c2a9b7e41d3c8"}}
→ {"jsonrpc":"2.0","id":"u2","method":"blob.write","params":{"upload":"5f0c2a9b7e41d3c8","offset":0,"data":"JVBERi0x…"}}
← {"jsonrpc":"2.0","id":"u2","result":{"received":524288}}
→ {"jsonrpc":"2.0","id":"u4","method":"blob.close","params":{"upload":"5f0c2a9b7e41d3c8"}}
← {"jsonrpc":"2.0","id":"u4","result":{"blob":"sha256:…","kind":"file","media_type":"application/pdf","name":"notes.pdf"}}
```

1. 每一块的 `offset` 必须正好等于已经收到的字节数；对不上时返回 `upload_offset`，`data.received` 说明收到了多少，从那里接着上传即可。
2. 上传编号只在打开它的连接上有效。连接断开、60 秒没有 `blob.write`，或者 `close` 以后，编号作废，暂存的内容会被删掉。
3. 一个连接同时最多打开 4 个上传。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | `name` 或 `media_type` 的写法不对；`data` 不是 base64、超过 512 KiB，或者加上它以后超过 `size` |
| `attachment_too_big` | `size` 超过 20 MiB；或者 `close` 时图片超过上限 |
| `too_many_uploads` | 这个连接已经打开了 4 个上传 |
| `upload_offset` | `offset` 不对，另带 `data.received` |
| `upload_incomplete` | `close` 时还没有收齐，另带 `data.received` |
| `upload_unknown` | 上传编号无法识别、已经作废，或者是其他连接打开的 |
| `internal_error`（`-32603`） | 核心无法存储这个文件 |

## `blob.get`

这个方法从一个 blob 里读取一段。

**谁能调用**：人、扩展都能调用，但是能读取到的范围不一样：

- **人**：不填写 `session` 时可以读取任何 blob；填写了 `session` 时，那个会话必须是本机的会话、你自己的私聊，或者关在沙盒里的群、陌生人的私聊（这些会话只能以只读方式查看，和会话本身相同，见[场所](venues.md)「主人看场所会话」）。
- **扩展，不填写 `session`**：只能读取到它自己用 `blob.put`、分块上传存储的 blob，以及它调用其他软件包（`bus.call`）拿回来的回应里、回答的那个软件包自己能读取到的哈希。自己存储的 blob 在核心重启以后仍然能读取到；从其他软件包拿回来的只在核心重启之前有效，重启以后要再拿一次。软件包卸载以后重新安装，就从头开始计算。
- **扩展，填写了 `session`**：那个会话必须是它能看到的，并且这个 blob 必须被那个会话引用过（消息、工具结果里的图片和文件，以及发出去的图片）；只在正文里写着哈希的不算。

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `blob` | 字符串，必写 | 内容哈希 |
| `session` | 字符串 | 通过这个会话查找 |
| `offset` | 整数，默认 0 | 从第几个字节开始读取 |
| `length` | 整数，默认 524288（512 KiB） | 最多读取几个字节，最多 512 KiB；填写 0 时只查询大小 |

**回应**：`data`（这一段内容，base64）、`size`（整份一共几个字节）。读到结尾就停止；如果 `offset` 超过了结尾，`data` 是空的。

```json
→ {"jsonrpc":"2.0","id":"g1","method":"blob.get","params":{"blob":"sha256:9f86d0…","offset":0,"length":4}}
← {"jsonrpc":"2.0","id":"g1","result":{"data":"iVBORw==","size":48213}}
```

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | `blob` 不是内容哈希；`session` 的写法不对；`length` 超过 512 KiB |
| `unknown_blob` | 没有这个 blob，或者这个连接读取不到它。这两种情况不分开说明，以免有人用它来探测某个 blob 是否存在 |
| `internal_error`（`-32603`） | 核心无法读取 |

## `session.shown`

AI 用 `show` 展示的文件不会存成 blob：核心只记下它在核心所在机器上的真实路径、大小和修改时间（视图工具卡的 `shown`，见 [视图](view.md)）。

- **本机的头**（握手回应带 `host` 的连接）：直接按照 `shown.path` 读取硬盘，不需要使用这个方法。
- **远程的头**（在局域网里通过网页软件连接进来的）：无法读取那台电脑的硬盘，所以使用这个方法，按照「会话 + 这一步的调用编号」分段读取。视频、音频可以边传边播放：拖动进度时，按照需要的那一段计算 `offset`、`length`（类似 HTTP Range）。很大的文本只读取开头作为预览。核心不会替你截取，读取多少由你决定。

**谁能调用**：人、扩展，能读取到的范围和填写了 `session` 的 `blob.get` 相同：人能读取本机会话、你自己的私聊、关在沙盒里的场所会话里展示的文件；扩展要求那个会话是它能看到的。

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 会话编号 |
| `call` | 字符串，必写 | 展示文件的那一步的调用编号，也就是视图工具卡的 `call` |
| `offset` | 整数，默认 0 | 从第几个字节开始读取 |
| `length` | 整数，默认 524288（512 KiB） | 最多读取几个字节，最多 512 KiB；填写 0 时只查询大小 |

这个方法不接受路径：它只读取这一步展示时记下的那个文件。填写了其他参数时返回 `bad_params`。

**回应**：和 `blob.get` 相同，`data`（这一段内容，base64）、`size`（整份一共几个字节）。读到结尾就停止；如果 `offset` 超过了结尾，`data` 是空的。

```json
→ {"jsonrpc":"2.0","id":"s1","method":"session.shown","params":{"session":"0193…","call":"call_12_1","offset":1048576,"length":524288}}
← {"jsonrpc":"2.0","id":"s1","result":{"data":"AAAAIGZ0eXBpc29t…","size":734003200}}
```

每次读取之前，核心都会核对文件的大小和修改时间：如果和展示时记下的不一样（文件被修改过、被替换过），就拒绝读取；文件不在了也会拒绝。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | `session` 或 `call` 的写法不对；`length` 超过 512 KiB；填写了其他参数 |
| `unknown_shown` | 没有这一步的展示：会话不存在、这个连接读取不到，或者这一步不是展示。这几种情况不分开说明 |
| `file_changed` | 文件的大小或修改时间变了。头会显示「文件已变」 |
| `file_gone` | 文件不在了，或者已经不是普通文件。头会显示「文件已不在」 |
| `internal_error`（`-32603`） | 核心无法读取 |

表里没有写错误码的都是 `-32010`。
