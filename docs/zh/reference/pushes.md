# 推送

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

推送是核心主动发出的通知，格式是 `{"jsonrpc": "2.0", "method": <名字>, "params": {…}}`：它没有 `id`，接收方不需要回应。推送只发给订阅了对应流的连接（怎样订阅见 [连接](connection.md)）；扩展另外还会收到几种核心发给它的通知，详细的见 [扩展](extensions.md)。

| 推送 | 哪条流 | `params` |
|---|---|---|
| `event` | `events` | `{"session", "event"}`：会话的一条事件，内容原样转发（见 [事件](events.md)）；补发的事件也使用这个推送 |
| `view.add`、`view.update`、`view.append`、`view.hidden`、`view.remove`、`view.status` | `view` | 详细的见 [视图流](view.md) |
| `sessions.changed` | `sessions` | `{"session", "entry"}`：会话列表里的一项被整个替换（内容和 `session.list` 返回的一项相同）；会话被删除时推送 `{"session", "removed": true}`。子会话发生变化时，父会话和各级祖先的那一项（它们的 `tokens` 连同子孙一起计算）也会跟着一层层推送。人发了话（`last_spoken` 发生变化）时也会推送，回合中途插入的一句话也算 |
| `config.changed` | `config` | `{"layer", "via", "by"?, "version", "keys", "problems"}`：一层配置发生了变化。`keys` 和 `config.set` 的回应相同；`via` 表示变化从哪里来，取值是 `set`、`edit`、`file`（手动修改了文件）、`package`（安装或卸载软件包以后） |
| `models.changed` | `config` | `{"providers"}`：这几家供应商的模型列表发生了变化（核心在后台获取到的列表和原来的不一样）。推送里不带条目，收到以后用你自己的参数重新调用 `model.list`。只有列表真的变了才会推送；因为读得慢而漏掉的推送不会补发，在 `resync` 或者重新连接以后要整份重新读取 |
| `extension.changed` | `extensions` | `{"entry"}`：一个扩展的状态，内容和 `extension.status` 返回的一项相同 |
| `package.changed` | `packages` | `{"package", "entry"}`：软件包列表里的一项；软件包已经不存在时，`entry` 是 `null` |
| `package.progress` | 不需要订阅：只推送给发起 `package.install`（真正安装）的那个连接 | `{"package", "stage", "index", "count", "done"?, "total"?}`：`stage` 表示进行到哪一步，取值是 `download`（带 `done`、`total`，分别是从软件源下载包文件已经下载的字节数和总字节数；开始、结束时各推送一次，中间最多每 200 毫秒推送一次）、`verify`（核对之前）、`install`（安装之前）；`index`、`count` 表示这是第几个、一共有几个。从包文件夹或者包文件安装时，只有 `verify`、`install` 各推送一次。老版本的核心只推送 `{"package", "done", "total"}`，没有写 `stage` 的推送按 `download` 处理 |
| `bus.changed` | `bus` | `{"names": […]}`：总线上的全部名字（和 `bus.list` 的回应相同）。安装或卸载了软件包、扩展启动或停止、登记了方法时都会推送，收到以后用它整个替换原来的名字 |
| `projects.changed` | `projects` | `{"project": <一项>}`：一个项目被整个替换（内容和 `project.list` 返回的一项相同）；项目被删除时推送 `{"project": <编号>, "deleted": true}` |
| `prefs.changed` | `prefs` | `{"package", "values"}`：一个界面包的设置被其他连接修改了，`values` 是修改后的整份设置，收到以后用它整个替换原来的设置（见 [头自己的设置](prefs.md)）。做出修改的那个连接不会收到这个推送，因为它收到的回应里已经是整份设置。订阅掉队时推送 `resync {"stream": "prefs", "package"}` |
| `schedules.changed` | `schedules` | `{"id", "entry": <一项>}`：一个定时任务被整个替换（内容和 `schedule.list` 返回的一项相同，定时任务到点执行以后也会推送）；定时任务被删除，或者一次性的定时任务运行完毕时，推送 `{"id", "removed": true}` |
| `package.event` | `package` | `{"package", "event", "data"}`：软件包通过 `package.emit` 推送给人的内容，格式由软件包自己决定。这个推送不保存到磁盘，没有订阅的连接收不到；订阅掉队时推送 `resync {"stream": "package", "package"}` |
| `notice` | `notices` | `{"kind", "at", …}`：给人的通知，头（连接核心的界面程序，例如终端界面、网页界面）根据 `kind` 弹出系统通知。`schedule.ran` 的内容是 `{"schedule", "session", "ok"}`：每次新开会话的定时任务，在新开的会话的那一轮结束时推送，`ok` 表示这一轮是否答完（出错、被打断时是 `false`）；在已有会话上运行的定时任务不推送。`schedules.reset` 表示定时表损坏了：核心把它改名保留，换上一份空的定时表，所以原来的定时任务都没有了。这个推送不保存到磁盘，没有订阅的连接收不到 |
| `notices.claimed` | `notices` | `{"claimed"}`：是否有连接认领了后台通知的发送（[`notices.claim`](connection.md#noticesclaim)）发生了变化。`true` 表示已经有连接认领，其他头不再为没有人打开的会话弹出系统通知，也不再为 `notice` 弹出系统通知；`false` 表示认领的连接都已经离开，其他头按原来的方式弹出 |
| `kb.indexing` | `kb` | `{"kb", "done", "total"}`：一个知识库正在建立索引，需要重建的 `total` 篇里已经完成了 `done` 篇。第一次推送时是 0，最后一篇完成时一定会推送，中间最多每 0.1 秒推送一次 |
| `kb.indexed` | `kb` | `{"kb"}`：索引已经建好；那个知识库此刻的状态用 `kb.list` 重新读取 |
| `mcp.changed` | `packages` | `{"name", "server"}`：系统配置里的一个 MCP 服务；服务被删除时，`server` 是 `null` |
| `widget.changed` | `widgets` | `{"package", "id", "widget"}`：一块界面块被整个替换；界面块被关掉或者卸载时是 `null` |
| `memory.event` | `memory` | `{"event", "persona"? | "session"?}`：一间记忆的日志里新增的一行 |
| `memory.dreaming` | `memory` | `{"upto", "kind", …, "persona"? | "session"?}`：整理记忆时 AI 刚写完的一条修改或者作废，或者摘要已经写到第几个字；这个推送不保存到磁盘，也不会补发（见 [`memory.dream`](memory.md#memorydream)） |
| `resync` | 每条流都有 | `{"stream", "session"?}`：订阅因为读得太慢而掉队，或者会话已经停止，此时这个订阅已经停止，需要重新订阅 |

**先后**

1. 一个命令引起的推送，一定比这个命令的回应先到达。
2. 推送的都是整项（`entry`），所以重复的、晚到的推送按编号替换即可。
3. 同一块界面块最快每 250 毫秒推送一次，这段时间里只保留最新的内容。

**掉队**：核心不会等待读得慢的头。会话给每个订阅最多积攒 1024 份推送，配置最多 16 份，记忆最多 64 条，再多核心就推送 `resync` 并停止这个订阅。重新订阅时，会话和记忆的订阅带上最后看到的序号（`after`）就能补回漏掉的内容；其他流的订阅回应本来就是整份内容。
