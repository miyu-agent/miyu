# 其他：后台任务、给人看的字、链接卡片、画图

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

| 方法 | 做什么 |
|---|---|
| [`job.stop`](#jobstop) | 停止一个后台命令或者子代理 |
| [`job.output`](#joboutput) | 读取一条后台命令的输出 |
| [`human.get`](#humanget) | 获取给人看的文字：工具的样子、说法的模板 |
| [`link.preview`](#linkpreview) | 获取一个链接的卡片 |
| [`mermaid.render`](#mermaidrender) | 把 Mermaid 源码画成 SVG |

**谁能调用**：人和扩展都能调用；扩展调用 `job.output` 需要能力 `events.read`，调用 `link.preview` 需要能力 `network`。

## `job.stop`

**参数**：`session`（任务是哪个会话派出去的）、`job`（任务编号，例如 `j1`）。

**回应**：`{}`，核心在任务的回报落盘以后才返回。后台命令会整组被杀掉；子代理会停掉它正在进行的这一轮，连同它派出去的任务一起停掉。如果是人停掉的，核心会给 AI 开一轮，让 AI 知道是你停掉的。

**出错**：`unknown_job`（没有这个任务，或者任务已经结束了）；`bad_params`（编号的写法不对）。

## `job.output`

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 任务是哪个会话派出去的 |
| `job` | 字符串，必写 | 任务编号 |
| `tail` | 1 到 2000，默认 200 | 要读取最后几行 |
| `after` | 非负整数 | 已经拿到了前面多少个字节，只要后面的部分；和 `tail` 只能写一个 |
| `wait` | 布尔，默认 `false` | 带 `after` 时：如果没有新的输出、命令还在运行，就等到有新的输出或者命令结束再应答（在后台应答） |

**回应**

- 不带 `after` 时：`{"lines", "output", "running", "size", "truncated"}`：分别是一共有几行、最后 `tail` 行的内容、命令是否还在运行、读到的字节数、前面是否还有没交出的输出。
- 带 `after` 时：`{"more", "output", "running", "size"}`：下一次把 `size` 作为 `after` 填写；`more` 表示后面还有内容，这一次被上限截住了。

一次最多交出 128 KiB。如果头（连接核心的界面程序，例如终端界面、网页界面）要一直跟着看输出，就先不带 `after` 获取一次，之后带上 `after` 和 `wait` 获取。

```json
{"id":"o1","jsonrpc":"2.0","result":{"lines":2,"output":"building\nhalf\n","running":true,"size":14,"truncated":false}}
```

**出错**：`unknown_job`；`not_a_command`（这个任务是子代理：需要订阅它的子会话来查看）；`bad_params`。

## `human.get`

这个方法返回界面画工具、拼提示时用的文字，按语言区分。

**参数**：`language`（不填写时使用这个连接的语言）。

**回应**：`{"language", "said", "tools", "packages"}`，各字段如下：

| 名字 | 说明 |
|---|---|
| `said` | 从说法的编号映射到模板原文（`{name}` 这样的字段由头替换） |
| `tools` | 从工具名映射到 `{"name", "icon"?, "subject"?}`：分别是给人看的名字、图标，以及名字后面跟哪个参数的值 |
| `packages` | 从工具名映射到它所属的软件包 |

核心每次都现读资源，所以开发时改了资源不需要重启。安装、卸载软件包，或者打开、关闭 MCP 服务以后，头会收到 `package.changed`、`mcp.changed`，这时重新获取一次即可。

```json
{"id":"h1","jsonrpc":"2.0","result":{"language":"zh","said":{"core/tool-results/unattended":"需要确认，无人确认"},"tools":{"read":{"icon":"→","name":"读取","subject":"file_path"}}}}
```

**出错**：`bad_params`（`language` 的写法不对）。

## `link.preview`

只有编译进了联网包（`base-net`）时才有这个方法，没有时返回 `unknown_method`。这个方法在后台应答。

**参数**：`url`。

**回应**：`{"card": {"url", "title", "description", "site", "kind", "image", "icon", "author"?, "duration"?}}`：`kind` 是 `video`、`article`、`page`；`image`、`icon` 是 `{"blob", "media_type"}` 或 `null`（用 `blob.get` 读取）；`duration` 的单位是秒。做不出卡片时返回 `{"card": null, "why"}`，`why` 是 `not_a_url`、`unsupported_scheme`、`no_preview`、`unreachable` 之一，这不算出错。

## `mermaid.render`

只有编译进了 Mermaid 包时才有这个方法，没有时返回 `unknown_method`。

**参数**：`source`：Mermaid 源码。

**回应**：`{"svg", "marks": {"text", "line", "label"}}`：分别是 SVG 的文字，以及 SVG 里文字、线、连线标签底色所用的三种记号色，头把它们换成自己主题的颜色。

**出错**：`mermaid_too_long`（源码超过 64 KiB）；`mermaid_failed`（无法画出，`data.detail` 是原话）。
