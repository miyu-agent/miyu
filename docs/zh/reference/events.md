# 事件

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

会话以一份只追加的事件日志为准。订阅了 `events` 流的头（连接核心的界面程序，例如终端界面、网页界面）会逐条收到这些事件（推送 `event`）；`view.page` 不带 `view` 时返回的也是这些事件。如果头只想绘制界面，使用 [视图流](view.md) 就够了，不需要自己根据事件计算。

## 外壳

```json
{"seq": 42, "at": "2026-10-01T06:03:12.345Z", "kind": "message.user", "turn": 41, "by": {"kind": "person"}, "cause": "m1", "body": {…}}
```

| 名字 | 说明 |
|---|---|
| `seq` | 序号，从 1 开始，在一个会话里不重复，也不跳号。 |
| `at` | 事件发生的时刻，使用 UTC，精确到毫秒。 |
| `kind` | 事件的种类，见下表。 |
| `turn` | 事件属于哪一轮，值是那一轮 `turn.started` 的序号；不属于任何一轮的事件不写这一项。 |
| `by` | 谁产生了这条事件：`person`（主人；在平台上识别出是主人的另外带 `via`；在本机的头上说的话、输入的命令另外带 `head`，值是说话的那个连接在握手时报告的 `head.kind`，例如 `tui`、`web`、`cli`，它以小写字母开头，只含小写字母、数字、`-` 和 `_`，最多 32 个，写法不符合的就不带）、`external`（平台上的人，带 `venue`、`id`、`role`，对应表识别出是主人的另外带 `owner: true`）、`model`、`kernel`、`tool`（带 `call_id`）、`module`（带 `id`，例如一个软件包）、`harness`（带 `name`）、`session`（其他会话）、`schedule`（到点的定时任务、循环交进会话的话：带 `schedule`，是任务的编号；循环另带 `loop`，值是 `fixed` 或 `dynamic`；人建的任务带 `person: true`；交进去的正文是出厂的那一句（光打 `/loop` 开的自主循环）时带 `builtin: true`，头根据它画「由 AI 自己决定」，不带的画那句原话）。 |
| `cause` | 哪条命令引起了这条事件，值是那次请求的 `id`。 |
| `body` | 这一种事件自己的字段。 |

每一种事件的完整样子都在仓库的 `internal/designs/samples/events/<种类>.jsonl` 里，一种事件一个文件，文件里是真实的一行事件。

## 落盘的事件

| `kind` | 是什么 |
|---|---|
| `session.created` | 会话创建了，这是会话的第 1 条事件，记录场所、工作目录、人格、预设、模型等。 |
| `session.policy_changed` | 会话的设置变了：可能是权限（`permission: {"level", "read_only"}`，`from` 是更换以前的设置，同样是这两个字段；以前的日志没有 `from`）、模型、思考强度（`effort: {"level"}`，`null` 表示回到跟随配置）、上下文上限（`context: {"tokens"}`，`null` 表示去掉上限）、多模态模型（`vision`，空值表示去掉）、压缩触发线（`compaction_line: {"percent"}`，`null` 表示去掉），也可能是整份策略。 |
| `session.workspace_changed` | 工作目录更换了。 |
| `session.meta_changed` | 标题、置顶或者项目改了（`project` 是项目编号，空值表示移出项目）；没有起名的会话由内核自己起标题时，记录的也是这条事件。 |
| `session.recapped` | 会话的一句回顾，不进入 AI 的上下文。 |
| `session.sealed` | 分叉出来的会话运行满了轮数，已经封存。 |
| `command.ran` | 人使用了一个斜杠命令，这条事件不进入 AI 的上下文。 |
| `turn.started`、`turn.ended` | 一轮开始、一轮结束；`turn.ended` 的 `reason` 是 `completed`、`interrupted`、`error`、`restarted` 等。 |
| `turn.joined` | 已经记下的几条消息并入了正在运行的这一轮。 |
| `turn.reverted`、`turn.unreverted` | 撤回了几轮；恢复了最近一次撤回。 |
| `message.user` | 人（或者其他会话、其他 agent、平台上的人）说的一句话；人用 `!` 输入的命令多带一个 `shell: true`，`blocks` 是那一行命令。由人敲的斜杠命令发出的那一句多带 `command`：`{"name", "text"}`，`name` 是命令名，`text` 是人敲的那一行（去掉了前后空白），例如 `{"name": "loop", "text": "/loop 5m 看看 CI"}`；命令模板、MCP 提示词展开的那一句，人敲 `/loop` 当场开的第一轮（`by` 是 `schedule`），`/goal <目标>` 当场开的第一轮（`by` 是模块 `goal`）都带，之后到点的、续轮不带。头显示 `text`，`blocks` 是发给 AI 的内容；`command` 不进给 AI 的请求。只给出路径的附件（见 `session.send` 的第 3 条）在 `blocks` 里写成 `{"type": "path", "name", "path", "size"?}`：它没有 blob，`size` 是头填写的字节数。 |
| `message.withdrawn` | 正在排队、AI 还没有听到的消息被退回了。 |
| `message.assistant` | 模型一次响应的完整内容：正文、思考、工具调用和供应商侧工具（`hosted_tool`：供应商自己运行的一次工具，例如供应商那边的搜索，核心不执行它，只如实记下；带 `call_id`、`name`、`args`，有结果的另外带 `outcome`：`{"text", "failed"?, "cut"?}`，`cut` 表示结果超过 64 KiB，后面的部分被截掉了）。如果模型在回答后面编出了下一轮（一行开头是一个角色词，后面模仿这一次请求里人这一边的标签或者群消息的那一行），这一段在写入磁盘以前就被截掉了，这里只有截剩下的内容；流式推送的 `model.delta` 是原样的内容，应当以这一条事件为准。 |
| `model.called` | 一次模型请求的记录：用了哪个模型、用量、花费，出错时带 `error`。回复被截过的带 `cut`，每一块文字一项：`[{"block", "at", "bytes", "role"}]`（`block` 是第几块，和 `model.delta` 的 `index` 相同；`at` 是从那一块原文的第几个字节开始截；`bytes` 是截掉了多少字节；`role` 是那个角色词：`user`、`assistant`、`system`、`human`、`tool`）。截掉的原文不记录；没有截过的就不写。 |
| `tool.result` | 一次工具调用的结果：`status` 是 `ok`、`error`、`denied`、`cancelled`、`skipped`。 |
| `tool.nested` | 脚本里的一次 `miyu call` 结束了：带 `id`（前台调用下面的写成 `call_12_1#3`，后台任务下面的写成 `j7#3`）、`name`、`args`、`status`（`ok`、`error`、`denied`、`needs_approval`、`cancelled`），有的另外带 `duration_ms`、`result`（blob）、`bytes`、`reason`。这条事件不进入 AI 的上下文；后台任务下面的不带 `turn`。 |
| `tool.approval_requested`、`tool.approval_decided` | 请人确认一次调用；人做出的决定。如果请求带 `expires_at`（一个时刻），到那一刻还没有人确认就自动拒绝：核心记一条 `decision` 是 `deny`、`by` 是 `kernel` 的决定，AI 拿到「没人确认」的结果后继续工作；头根据它绘制倒计时。配置 `permission.confirm.timeout` 是 `0` 时一直等待，请求不带这一项。扩展驱动在一次调用进行中请求的确认（[`driver.approve`](extensions.md#driverapprove)）多一格 `driver: {seen, index}`：显示在第 `seen` 次请求的第 `index` 块供应商侧工具上；这时 `call_id` 是核心另外分配的 `call_<这一条的序号>_1`，不对应任何一次工具调用，不会有 `tool.result`；调用没有了时，核心记一条 `by` 是 `kernel` 的拒绝。 |
| `question.asked`、`question.answered` | AI 提出了一组问题；人的回答。 |
| `context.injected` | 注入进上下文的一块事实，例如记忆、扩展返回的内容。 |
| `context.compacted` | 压缩的检查点；清空上下文记录的也是这条事件（`trigger` 是 `clear`）。如果摘要请求太长，截掉了最老的几组才写成摘要，事件就带 `truncated`（值是截掉的组数，最早的那一段没有进入摘要）；没有截过的不写。 |
| `context.compaction_paused` | 自动压缩暂停了。 |
| `job.reported` | 一条后台命令结束了；`reason` 是 `too_chatty` 时，表示被监视的命令在一小时里输出了 120 批，被停了下来。 |
| `shell.ran` | 人用 `!` 输入的命令运行完了：带 `message`（那句话的序号）、`exit` 或 `signal`、`duration_ms`、`text`（输出，标准输出和标准错误合在一起，太长的保留开头和结尾，中间用一行写明省略了多少）、`omitted`；异常结束的另外带 `reason`：`timed_out`（运行满了 120 秒）、`interrupted`（人打断了）、`aborted`（核心重启或者崩溃了，不会重新运行）。 |
| `job.lines` | 被监视的后台命令（`shell` 写了 `watch`）输出了一批行：带 `job`、`text`（最多 4000 个字，截过的中间有一行写明省略了多少）、`lines`、`omitted`。这条事件不带 `turn`。 |
| `child.reported` | 子代理发回的回报。 |
| `peer.idle` | 正在等待的那个会话空闲下来了，或者已经等不到了。 |
| `image.described` | 一张图片的转述：看不了图片的模型，由多模态模型替它看过。 |
| `todos.cleared` | 待办清单清空了。 |
| `package.state` | 软件包的程序直接换上了它在这个会话里的一份状态（`{"package", "state"}`，见 [扩展](extensions.md)）。 |
| `goal.set`、`goal.changed`、`goal.paused`、`goal.resumed`、`goal.ended` | 长期目标设下、修改、暂停、继续、结束。 |
| `dream.ended` | 一次记忆整理运行完了：新记了几条、改了几条、作废了几条，有没有写摘要，是不是被叫停的。 |
| `venue.recalled`、`venue.delivered` | 平台上有人撤回了一条消息；AI 的话发送到了平台上。 |
| `ext.<前缀>.<名字>` | 扩展自己记录的事件（通过 `events.append` 记录；前缀是清单里的 `prefix`，没有写的时候是软件包编号）。 |

撤回、压缩、清空都不会删除事件：头需要根据有效的历史自己计算该绘制什么，或者直接使用视图流。

## 瞬时事件

瞬时事件不写入磁盘，没有 `seq`，只推送给当时正在订阅的头，补发时也不会补上（例外：订阅时如果有一次模型请求正在流式输出，核心会把到目前为止的 `model.delta` 补发一遍）。它们的样子在 `internal/designs/samples/transient/` 里。

| `kind` | 是什么 |
|---|---|
| `model.delta` | 模型输出的一段增量：带 `seen`、`index`（第几块），再加上 `start`（块的种类：`text`、`reasoning`、`tool_call`、`hosted_tool`，后两种带 `name`）、`text`、`outcome`（供应商侧工具的结果 `{"text", "failed"?, "cut"?}`，和写进 `message.assistant` 的是同一份）、`end` 中的一个。 |
| `tool.progress` | 工具执行过程中的一段输出：带 `call_id`、`text`。 |
| `tool.nested_started` | 脚本里的一次 `miyu call` 通过了检查，已经派出去运行：带 `id`、`name`；它什么时候结束以 `tool.nested` 为准。 |
| `status` | 请求正在重试，或者换了供应商。 |
| `model.changed` | 会话接下来的请求所用的模型、思考强度、限额变了（更换模型、思考强度时会当场推送一条）：`effort.from` 是 `session`（会话自己记录的）或 `system`（配置）；`why` 是 `failover` 或 `turn`。 |
| `compaction.started`、`compaction.progress`、`compaction.done` | 压缩开始（后台提前压缩的那一种）、摘要写到了哪里、压缩完成；`compaction.done` 同样带 `truncated`，和对应那一条 `context.compacted` 的相同。 |
| `todos.changed` | 当前的整份待办清单。 |
| `package.state_changed` | 一个软件包在这个会话里给头看的状态变了：`{"package", "state": {"widgets"?}}`，状态清掉时 `state` 是 `{}`。 |
| `compactable.changed` | 有没有可以压缩的内容变了：`{"compactable": <布尔>}`，和订阅回应里的 `compactable` 相同，表示 `session.compact` 会不会返回 `nothing_to_compact`。 |
| `goal.changed` | 长期目标现在的整份内容。 |
| `session.retitle_failed` | 人要求的重起标题（[`session.retitle`](sessions.md#sessionretitle)）没有成功，标题没有变：`{"error", "human"}`，`error` 是那一次请求的错误（`class`、`message`，有的话还有 `status`），`human` 是给人看的那一句 `{"key": "core/sessions/retitle-failed"}`，根据 [`human.get`](misc.md#humanget) 的 `said` 换成文字（`更改标题失败`）。`by` 是提出要求的人。核心自己起标题失败时不推送这条事件。 |
