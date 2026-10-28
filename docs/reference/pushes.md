# 推送

推送是核心主动发的通知：`{"jsonrpc": "2.0", "method": <名字>, "params": {…}}`，没有 `id`，不用回应。只发给订阅了的连接（订阅见 [连接](connection.md)）；扩展另收几种核心发给它的通知，见 [扩展](extensions.md)。英文版：[pushes.en.md](pushes.en.md)。

| 推送 | 哪条流 | `params` |
|---|---|---|
| `event` | `events` | `{"session", "event"}`：会话的一条事件，原样（见 [事件](events.md)）；补发的也是它 |
| `view.add`、`view.update`、`view.append`、`view.hidden`、`view.remove`、`view.status` | `view` | 见 [视图流](view.md) |
| `sessions.changed` | `sessions` | `{"session", "entry"}`：会话列表的一项整个换掉（同 `session.list` 的一项）；删了的 `{"session", "removed": true}`。子会话变了，父会话、祖先的那一项（`tokens` 连同子孙算）跟着一层层推 |
| `config.changed` | `config` | `{"layer", "via", "by"?, "version", "keys", "problems"}`：一层配置变了，`keys` 同 `config.set` 的回应，`via` 是 `set`、`edit`、`file`（手改的）、`package`（装卸包以后） |
| `extension.changed` | `extensions` | `{"entry"}`：一个扩展的状态，同 `extension.status` 的一项 |
| `package.changed` | `packages` | `{"package", "entry"}`：软件包列表的一项，没有了的 `entry` 是 `null` |
| `package.progress` | 不用订阅：只推给发起 `package.install {"source"}` 的那个连接 | `{"package", "done", "total"}`：从软件源下包文件下了多少字节、一共多少（开始、结束各一次，中间最多 200 毫秒一次） |
| `projects.changed` | `projects` | `{"project": <一项>}`：这个账号的一个项目整个换掉（同 `project.list` 的一项）；删了的 `{"project": <编号>, "deleted": true}` |
| `schedules.changed` | `schedules` | `{"id", "entry": <一项>}`：这个账号的一个定时任务整个换掉（同 `schedule.list` 的一项，到点交过的也推）；删了的、一次的跑完了的 `{"id", "removed": true}`（施工 J-1 下） |
| `package.event` | `package` | `{"package", "event", "data"}`：包经 `package.emit` 推给这个账号的（施工 IM-1 中下二），写法由包自己定。不存盘，没订着的收不到；掉了队推 `resync {"stream": "package", "package"}` |
| `notice` | `notices` | `{"kind", "at", …}`：给人的通知（施工 J-3 中），头照 `kind` 弹系统通知。`schedule.ran`：`{"schedule", "session", "ok"}`，每次新开会话的定时，开出来的那一轮结束了，`ok` 是答完了（出错、被打断的是 `false`），跑在已有会话上的不推。`schedules.reset`：这个账号的唤醒表坏了，改名留着、换了一份空的，原来的定时都没了。不存盘，没订着的收不到 |
| `kb.indexing` | `kb` | `{"kb", "done", "total"}`：这个账号的一个知识库在建索引，要重建的 `total` 篇里做完了 `done` 篇；开头是 0，最后一篇照推，中间至多 0.1 秒一次（施工 W-15） |
| `kb.indexed` | `kb` | `{"kb"}`：建好了；那个库这一刻的样子照 `kb.list` 重读 |
| `mcp.changed` | `packages` | `{"name", "server"}`：系统配置里的一个 MCP 服务，删了的 `server` 是 `null` |
| `widget.changed` | `widgets` | `{"package", "id", "widget"}`：一块界面块整个换掉，关掉、卸掉的是 `null` |
| `memory.event` | `memory` | `{"event", "persona"? | "session"?}`：一间记忆的日志里新的一行 |
| `memory.dreaming` | `memory` | `{"upto", "kind", …, "persona"? | "session"?}`：整理时她刚写完的一条改的、作废的，或者摘要写到第几个字；不落盘、不补（见 [`memory.dream`](memory.md#memorydream)） |
| `resync` | 都有 | `{"stream", "session"?}`：读得太慢掉了队，或者会话停了：这个订阅停了，重新订阅 |

**先后**

1. 一个命令引起的推送，一定比它的回应先到。
2. 推的都是整项（`entry`）：重复的、晚到的照编号替换就对。
3. 同一块界面块最快 250 毫秒推一次，中间的只留最新的。

**掉队**：核心不等读得慢的头。会话给一个订阅最多攒 1024 份、配置 16 份、记忆 64 条，再多推 `resync`、停掉这个订阅。重新订阅：会话、记忆带上最后看到的序号（`after`）就能补回来；别的流的订阅回应本来就是整份。
