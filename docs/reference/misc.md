# 其他：后台任务、给人看的字、链接卡片、画图

英文版：[misc.en.md](misc.en.md)。

| 方法 | 做什么 |
|---|---|
| [`job.stop`](#jobstop) | 停掉一个后台命令或者子代理 |
| [`job.output`](#joboutput) | 读一条后台命令的输出 |
| [`human.get`](#humanget) | 给人看的字：工具的样子、说法的模板 |
| [`link.preview`](#linkpreview) | 一个链接的卡片 |
| [`mermaid.render`](#mermaidrender) | Mermaid 源码画成 SVG |

**谁能调**：人、扩展。

## `job.stop`

**参数**：`session`（哪个会话派出去的）、`job`（任务编号，`j1` 这样）。

**回应**：`{}`，回报落了盘才回。后台命令整组杀掉；子代理停掉它这一轮连它派的。人停的会叫醒她，她知道是你停的。

**出错**：`unknown_job`（没有这个任务，或者已经结束了）；`bad_params`（编号写法不对）。

## `job.output`

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话派出去的 |
| `job` | 字符串，必写 | 任务编号 |
| `tail` | 1 到 2000，默认 200 | 要最后几行 |
| `after` | 非负整数 | 已经有了前几个字节，只要后面的；和 `tail` 只写一个 |
| `wait` | 布尔，默认 `false` | 带 `after` 的：没有新的、命令还在跑，就等到有新的或者它结束再答（在后台答） |

**回应**

- 不带 `after`：`{"lines", "output", "running", "size", "truncated"}`：一共几行、最后 `tail` 行、还在不在跑、读到的字节数、前面还有没有没交的。
- 带 `after`：`{"more", "output", "running", "size"}`：下一次照 `size` 写 `after`；`more` 是后面还有、这一次的上限截住了。

一次最多交 128 KiB。头一直跟着看的：先不带 `after` 拿一次，之后带 `after` 和 `wait`。

```json
{"id":"o1","jsonrpc":"2.0","result":{"lines":2,"output":"building\nhalf\n","running":true,"size":14,"truncated":false}}
```

**出错**：`unknown_job`；`not_a_command`（是子代理：订阅它的子会话看）；`bad_params`。

## `human.get`

给界面画工具、拼提示用的字，照语言。

**参数**：`language`（不写照这个连接的语言）。

**回应**：`{"language", "said", "tools", "packages"}`：

| 名字 | 说明 |
|---|---|
| `said` | 说法的编号 → 模板原文（`{name}` 这样的字段由头换） |
| `tools` | 工具名 → `{"name", "icon"?, "subject"?}`：给人看的名字、图标、名字后面跟哪个参数的值 |
| `packages` | 工具名 → 它属于哪个包 |

每次现读，开发时改了资源不用重启。装卸包、开关 MCP 服务以后，头收到 `package.changed`、`mcp.changed` 重取一次。

```json
{"id":"h1","jsonrpc":"2.0","result":{"language":"zh","said":{"core/tool-results/unattended":"需要确认，无人确认"},"tools":{"read":{"icon":"→","name":"读取","subject":"file_path"}}}}
```

**出错**：`bad_params`（`language` 写法不对）。

## `link.preview`

编进了联网包（`net`）才有，没有的回 `unknown_method`。在后台答。

**参数**：`url`。

**回应**：`{"card": {"url", "title", "description", "site", "kind", "image", "icon", "author"?, "duration"?}}`：`kind` 是 `video`、`article`、`page`；`image`、`icon` 是 `{"blob", "media_type"}` 或 `null`（用 `blob.get` 读）；`duration` 是秒。做不出卡片的回 `{"card": null, "why"}`：`not_a_url`、`unsupported_scheme`、`no_preview`、`unreachable`，不算出错。

## `mermaid.render`

编进了 Mermaid 包才有，没有的回 `unknown_method`。

**参数**：`source`：Mermaid 源码。

**回应**：`{"svg", "marks": {"text", "line", "label"}}`：SVG 的字，和 SVG 里字、线、连线标签垫底用的三种记号色，头照它们换成自己主题的颜色。

**出错**：`mermaid_too_long`（源码超过 64 KiB）；`mermaid_failed`（画不出，`data.detail` 是原话）。
