# 定时任务：`schedule.*`

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

定时任务会在到点时把一句话交给一个会话，让它运行一轮：例如每天早上 8 点的早报、每小时看一眼服务器。所有定时任务存在一张定时表 `root/schedules.db` 里，核心重启以后不会丢失；核心停止期间错过的时刻不会补上。

| 方法 | 做什么 |
|---|---|
| [`schedule.add`](#scheduleadd) | 新建一个定时任务 |
| [`schedule.list`](#schedulelist) | 列出全部定时任务 |
| [`schedule.cancel`](#schedulecancel) | 删除一个定时任务 |
| [`schedule.pause`](#schedulepause) | 暂停 |
| [`schedule.resume`](#scheduleresume) | 继续 |
| [`schedule.update`](#scheduleupdate) | 修改时间或者那句话 |

**谁能调用**：人可以调用；扩展需要批准过 `sessions.drive`，没有批准时返回 `needs_approval`。扩展新建的任务记成软件包建的，扩展只看得到、删得了、暂停和继续得了、改得了自己建的任务：碰其他人建的任务一律返回 `unknown_schedule`。想要一直跟着最新的状态，可以订阅 `schedules` 流（[连接](connection.md)），之后核心会推送 `schedules.changed`（[推送](pushes.md)）。

## 一项

| 名字 | 说明 |
|---|---|
| `id` | 编号是 `t1`、`t2`……，依次递增，删除了的编号不会再用 |
| `when` | 什么时候运行：`{"cron": "0 8 * * *", "recurring": true, "zone": "Asia/Tokyo"}`（标准的五段写法，按 `zone` 指定的时区解读：`zone` 是新建时交的，没有交时记下核心所在机器那一刻的时区；以前建的任务没有 `zone`，按核心所在机器的时区解读）、`{"at": "<时刻>"}`（只运行一次）、`{"every": <秒>}`（每隔一段时间运行一次，这个数至少是 60） |
| `prompt` | 交进会话的那句话，原样保存 |
| `target` | 在哪里运行：`{"session": <编号>}`；或者 `{"new": {"persona", "preset", "cwd"}}`（每次按这几项新开一个会话，`persona` 是 `null` 时表示不使用人格）；或者 `{"package": {}}`（不进会话，到点时把 [`schedule.fired`](#schedulefired) 推送给建它的扩展） |
| `by` | 谁建的：人建的是 `{"kind": "person"}`，AI 在会话里建的是 `{"kind": "model", "session"}`，软件包建的是 `{"kind": "package", "package"}` |
| `recurring` | 运行完以后任务是否还保留：`at` 的是 `false`，`every` 的是 `true`，`cron` 的按新建时写的值 |
| `paused` | 任务是否暂停着 |
| `loop` | 只有循环才有这一项：`fixed` 表示固定间隔，`dynamic` 表示由 AI 自己决定节奏 |
| `next` | 下一次运行的时刻；暂停着的任务，以及由 AI 自己决定节奏、还没有定下一次的循环，没有这一项 |
| `last` | 上一次运行的情况：`{"at", "result", "session"?}`，`result` 是 `ran`（开始了一轮）、`queued`（会话正在运行一轮，这一次排在后面）、`held`（那句话记进了会话，但是没有开始一轮，等人下一次开口时一起看到：在 `miyu ask` 开的、这时没有头（连接核心的界面程序，例如终端界面、网页界面）打开着的会话里，AI 自己建的任务和循环是这样；你建的定时任务在那种会话里仍然会开始一轮）、`sent`（推送给了建它的扩展）或 `skipped:<原因>`（`session_gone` 表示会话没有了，任务也跟着停了；`preset_off` 表示任务是 AI 建的，而会话的预设关掉了它；`not_opened` 表示新会话无法打开；`package_unavailable` 表示建它的扩展没有在运行，也无法启动它） |
| `created` | 新建的时刻 |

没有值的字段不会出现。时刻的写法和其他地方相同：`2026-10-26T08:00:00.000Z`。

## `schedule.add`

| 参数 | 类型 | 说明 |
|---|---|---|
| `when` | 对象，必写 | 三种写法写其中一种，和上面相同；`recurring`、`zone`（IANA 时区名，例如 `Asia/Tokyo`）只能和 `cron` 一起写 |
| `prompt` | 字符串，必写 | 不能是空的 |
| `target` | 对象，可以不写 | `{"session": <编号>}`、`{"new": {"persona"?, "preset"?, "cwd"?}}` 或 `{"package": {}}`，写其中一种；不写时每次新开一个会话。只有扩展能写 `package`，人写了会返回 `bad_params`（不带 `data.problem`）。`persona`、`preset` 不写时，按此刻的默认值定下来；`cwd` 不写时是家目录 |

回应是 `{"schedule": <一项>}`。写错时什么都不会新建，返回 `bad_params`，`data.problem` 说明是哪一项错了：`when`（没有正好写对一种，或者时刻的写法不对）、`cron`（无法解析）、`past`（时刻已经过了）、`short`（间隔少于 60 秒）、`prompt`（是空的）、`session`（没有这个会话，或者这个连接碰不到它：两种情况的说法相同）、`target`（两种都写了）、`zone`（时区不能识别）、`cwd`（每次新开会话用的工作目录不存在、不是目录，或者落在数据根里工作区以外的地方）。如果已经有 200 个定时任务（人建的、AI 建的、软件包建的合计，这是熔断性质的安全上限），返回 `too_many_schedules`，需要先删掉几个；已有的任务仍然正常运行。找不到人格、预设时，和开会话一样返回 `unknown_persona`、`unknown_preset`。

## `schedule.list`

参数 `session` 可以不填写：填写了时只列出运行在这个会话上的任务。回应是 `{"schedules": [<一项>…]}`：有下一次的任务排在前面，按时刻先后排列；暂停着的、没有定下一次的任务排在后面。

## `schedule.cancel`

参数是 `id`。回应是 `{}`。

## `schedule.pause`

参数是 `id`。暂停着的任务到点时不会运行，下一次的时刻会被清掉。回应是 `{"schedule": <一项>}`。

## `schedule.resume`

参数是 `id`。核心按现在的时间重新计算下一次，错过的不会补上；只运行一次的任务如果时刻已经过了，返回 `bad_params`（`past`）。回应是 `{"schedule": <一项>}`。

## `schedule.update`

修改一项任务的时间或者那句话。编号不变，`last`、新建的时刻、`target`、`by` 都保留；不填写的项保持原样。

| 参数 | 类型 | 说明 |
|---|---|---|
| `id` | 字符串，必写 | 任务的编号 |
| `when` | 对象，可以不写 | 写法和检查方式都和 `schedule.add` 相同 |
| `prompt` | 字符串，可以不写 | 不能是空的 |

`when`、`prompt` 至少填写一项，两项都不填写时返回 `bad_params`（不带 `data.problem`）。改了 `when` 时，核心按现在的时间重新计算 `next`，错过的不会补上；暂停着的任务改完以后仍然暂停着，`resume` 时按新的时间计算。循环不能更换种类：`loop` 是 `dynamic` 的循环，`when` 只能写 `at`（也就是修改下一轮开始的时刻），`fixed` 的循环只能写 `every`，写其他的返回 `bad_params`（`when`）；两种循环都能修改 `prompt`。写错时什么都不会修改，返回 `bad_params`，`data.problem` 和 `schedule.add` 相同（`when`、`cron`、`past`、`short`、`prompt`、`zone`）。回应是 `{"schedule": <一项>}`，订阅了 `schedules` 的连接仍然会收到 `schedules.changed`。

```json
{"jsonrpc": "2.0", "id": "u1", "method": "schedule.update", "params": {"id": "t1", "when": {"every": 1800}}}
{"jsonrpc": "2.0", "id": "u1", "result": {"schedule": {"id": "t1", "when": {"every": 1800}, "prompt": "看看 CI", "target": {"session": "<编号>"}, "by": {"kind": "person"}, "recurring": true, "paused": false, "created": "2026-10-26T00:00:00.000Z", "next": "2026-10-26T00:50:00.000Z", "last": {"at": "2026-10-26T00:10:00.000Z", "result": "ran"}}}}
```

## `schedule.fired`

这是核心发给扩展的通知：扩展建的、`target` 是 `{"package": {}}` 的任务到点时，核心把它推送给那个软件包的进程，不需要订阅，也不等待回应。

| 名字 | 说明 |
|---|---|
| `id` | 任务的编号 |
| `at` | 这一次应该触发的时刻 |
| `prompt` | 新建任务时写的内容，原样返回（写法由扩展自己决定，例如 JSON） |
| `missed` | 核心停止期间错过了几次，最多数到 1000；错过的不会补上，只报告次数。没有错过时不写这一项 |

如果软件包没有连接，核心不会启动它；推送不到时记成 `skipped:package_unavailable`，只运行一次的任务和平常一样被删除，周期性的任务等待下一次。
