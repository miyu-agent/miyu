# 文件：`fs.*`

给头用的本机文件操作：补全路径、找文件、换成真实的位置、分块读一份文件。英文版：[fs.en.md](fs.en.md)。

| 方法 | 做什么 |
|---|---|
| [`fs.list`](#fslist) | 列一层目录（输入框里补全路径） |
| [`fs.find`](#fsfind) | 在一个目录里模糊找文件 |
| [`fs.realpath`](#fsrealpath) | 一个路径换成真实的位置 |
| [`fs.read`](#fsread) | 分块读一份本机文件 |

**谁能调**：人；扩展要能力 `fs.read`。

**边界**：Miyu 的数据目录（默认 `~/.miyu`）里只有这个账号自己的默认工作区能列、能找、能读，别处回 `path_forbidden`。`fs.realpath` 只说位置、不读内容，不查边界。

**路径的写法**：绝对路径，或者 `~`、`~/…`（照系统的家目录接）。

## `fs.list`

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `cwd` | 字符串，必写 | 相对的路径照它接 |
| `dir` | 字符串，默认 `""` | 已经打的那一截目录：`~` 开头的照家目录，绝对的照原样，别的照 `cwd` |
| `prefix` | 字符串，默认 `""` | 名字的开头 |

**回应** `{"items": [...], "partial"}`。一项：

| 名字 | 说明 |
|---|---|
| `path` | 列表上显示的写法，目录后面带 `/` |
| `full` | 绝对路径，照平台的写法 |
| `dir` | 是不是目录 |
| `size` | 文件的字节数，目录没有 |
| `marks` | `path` 里对上 `prefix` 的那几个字的位置（第几个字） |

名字照开头对 `prefix`，大小写不论；点开头的只在 `prefix` 以 `.` 开头时列。目录在前、文件在后，各照名字排。最多 50 条，多了截掉、`partial` 是 `true`。

```json
→ {"jsonrpc":"2.0","id":"f1","method":"fs.list","params":{"cwd":"~/src/miyu","dir":"src/","prefix":"ma"}}
← {"id":"f1","jsonrpc":"2.0","result":{"items":[{"dir":false,"full":"/home/me/src/miyu/src/main.rs","marks":[4,5],"path":"src/main.rs","size":2048}],"partial":false}}
```

**出错**：`path_unreadable`（不在、不是目录、没有权限）；`path_forbidden`。

## `fs.find`

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `cwd` | 字符串，必写 | 在哪个目录里找 |
| `query` | 字符串，默认 `""` | 打的字 |
| `fresh` | 布尔，默认 `false` | 打开列表时写 `true`：清单建好 10 秒以上的重建 |
| `wait` | 布尔，默认 `false` | 清单还在建的，建完再答（在后台答，不挡别的请求） |

**回应**：同 `fs.list`，另有 `building`：清单还在建，这一次只照建好的那一部分答。

1. 核心在 `cwd` 里建一份文件清单：认 `.gitignore`，跳过隐藏目录、`node_modules`、`target`，不跟链接；最深 8 层，最多 20000 个，收满了 `partial` 是 `true`。`path` 相对 `cwd`，用 `/` 连。
2. 打的字照先后都在 `path` 里（大小写不论）才列；落在文件名里的、在一段开头的、连着的排前面，文件名正好是打的字的最前。最多 50 条。

**出错**：同 `fs.list`。

## `fs.realpath`

**参数**：`path`（必写）；`cwd`：`path` 是相对的才要。

**回应**：`{"path": <真实的位置>}`。链接顺着找到本体；还没建出来的那几段原样接在最后一层在的后面。

**出错**：`bad_params`（相对的又没给 `cwd`）；`path_unreadable`（一层都不在、链接指向不存在的地方、没有家目录）。

## `fs.read`

**参数**：`path`（必写，不能是相对的）；`offset`（默认 0）；`length`（默认 512 KiB，最多 512 KiB，写 0 只问大小）。

**回应**：`{"data", "size"}`，同 [`blob.get`](blob.md#blobget)。路上一层链接都不跟。

**出错**：`bad_params`（相对路径；`length` 超过 512 KiB）；`path_unreadable`（不在、不是普通文件、没有权限）；`path_forbidden`。
