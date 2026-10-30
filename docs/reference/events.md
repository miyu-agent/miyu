# 事件

会话的真相是一份只追加的事件日志。订阅 `events` 流的头逐条收到它们（推送 `event`）；`view.page` 不带 `view` 时交的也是它们。只想画界面的头用 [视图流](view.md) 就够，不用照事件自己算。英文版：[events.en.md](events.en.md)。

## 外壳

```json
{"seq": 42, "at": "2026-10-01T06:03:12.345Z", "kind": "message.user", "turn": 41, "by": {"kind": "person"}, "cause": "m1", "body": {…}}
```

| 名字 | 说明 |
|---|---|
| `seq` | 序号，从 1 起，一个会话里不重不跳 |
| `at` | 时刻，UTC，毫秒 |
| `kind` | 种类，见下表 |
| `turn` | 属于哪一轮（那一轮 `turn.started` 的序号）；不属于哪一轮的不写 |
| `by` | 谁：`person`（主人；平台上认出的另带 `via`）、`external`（平台上的人：`venue`、`id`、`role`，对应表认出是主人的带 `owner: true`）、`model`、`kernel`、`tool`（`call_id`）、`module`（`id`，例如一个包）、`harness`（`name`）、`session`（别的会话） |
| `cause` | 哪条命令引起的：请求的 `id` |
| `body` | 这一种自己的格 |

每一种的完整样子在仓库的 `docs/designs/samples/events/<种类>.jsonl`，一种一个文件，就是真实的一行。

## 落盘的事件

| `kind` | 是什么 |
|---|---|
| `session.created` | 会话创建（第 1 条）：场所、工作目录、人格、预设、模型…… |
| `session.policy_changed` | 换了权限、模型、思考强度（`effort: {"level"}`，`null` 是回到跟着配置），或者整份策略 |
| `session.workspace_changed` | 换了工作目录 |
| `session.meta_changed` | 改了标题、置顶、项目（`project`：编号，空的是移出来）；没起名的会话由内核自己起标题，也是它 |
| `session.recapped` | 一句回顾（不进她的上下文） |
| `session.sealed` | 分叉出来的会话跑满了轮数，封存了 |
| `command.ran` | 人用了一个斜杠命令（不进她的上下文） |
| `turn.started`、`turn.ended` | 一轮开始、结束；`turn.ended` 的 `reason` 是 `completed`、`interrupted`、`error`、`restarted` 等 |
| `turn.joined` | 照记下的几条并进了正在跑的这一轮 |
| `turn.reverted`、`turn.unreverted` | 撤销了几轮、恢复了最近一次撤销 |
| `message.user` | 人（或别的会话、别的 agent、平台上的人）说的一句 |
| `message.withdrawn` | 排着队、她还没听到的话被退回了 |
| `message.assistant` | 模型一次响应的完整内容：正文、思考、工具调用 |
| `model.called` | 一次模型请求的记录：用了谁、用量、花了多少、出错的 `error` |
| `tool.result` | 一次工具调用的结果：`status` 是 `ok`、`error`、`denied`、`cancelled`、`skipped` |
| `tool.nested` | 脚本里的一次 `miyu call` 了结了：`id`（前台调用下面的写成 `call_12_1#3`，后台任务下面的写成 `j7#3`）、`name`、`args`、`status`（`ok`、`error`、`denied`、`needs_approval`、`cancelled`），有的另带 `duration_ms`、`result`（blob）、`bytes`、`reason`。不进她的上下文；任务下面的不带 `turn` |
| `tool.approval_requested`、`tool.approval_decided` | 请人确认一次调用、人的决定 |
| `question.asked`、`question.answered` | 她问了一组题、人的回答 |
| `context.injected` | 注入进上下文的一块事实（记忆、扩展交的……） |
| `context.compacted` | 压缩的检查点；清空上下文也是它（`trigger` 是 `clear`） |
| `context.compaction_paused` | 暂停了自动压缩 |
| `job.reported` | 一条后台命令结束了 |
| `child.reported` | 子代理的回报 |
| `peer.idle` | 等的那个会话空下来了，或者等不到了 |
| `image.described` | 一张图的转述（看不了图的模型由看图的模型替它看过） |
| `todos.cleared` | 待办清空了 |
| `package.state` | 包的程序直接换上了它在这个会话里的一份状态（`{"package", "state"}`，见 [扩展](extensions.md)） |
| `goal.set`、`goal.changed`、`goal.paused`、`goal.resumed`、`goal.ended` | 长期目标设下、改了、停了、继续、结束 |
| `dream.ended` | 一趟整理记忆跑完了：新记、改了、作废了几条，有没有写摘要，是不是叫停的 |
| `venue.recalled`、`venue.delivered` | 平台上有人撤回了一条；她的话发到了平台上 |
| `ext.<前缀>.<名字>` | 扩展自己记的（`events.append`；前缀是清单的 `prefix`，没写的是包编号） |

撤销、压缩、清空都不删事件：头照有效历史自己算该画什么（或者直接用视图流）。

## 瞬时事件

不落盘、没有 `seq`，只推给当时订阅着的头，补发不补（例外：订阅时正在流的那一次模型请求，会补一遍到目前为止的 `model.delta`）。样子在 `docs/designs/samples/transient/`。

| `kind` | 是什么 |
|---|---|
| `model.delta` | 模型输出的一段增量：`seen`、`index`（第几块），再加 `start`（`name` 是块的种类）、`text`、`end` 之一 |
| `tool.progress` | 工具执行中的一段输出：`call_id`、`text` |
| `tool.nested_started` | 脚本里的一次 `miyu call` 过了检查、派去跑了：`id`、`name`；了结以 `tool.nested` 为准 |
| `status` | 请求在重试、换了供应商 |
| `model.changed` | 会话接下来请求的模型、思考强度、限额变了：`effort.from` 是 `session`（会话记的）或 `system`（配置）；`why` 是 `failover` 或 `turn` |
| `compaction.started`、`compaction.progress`、`compaction.done` | 压缩开始（后台提前压的）、写到哪了、压好了 |
| `todos.changed` | 当前的待办整份 |
| `package.state_changed` | 一个包在这个会话里给头看的状态变了：`{"package", "state": {"widgets"?}}`，清掉了的 `state` 是 `{}` |
| `compactable.changed` | 有没有能压的变了：`{"compactable": <布尔>}`，同订阅回应的 `compactable`（`session.compact` 会不会回 `nothing_to_compact`） |
| `goal.changed` | 长期目标现在的整份 |
