# 人生会话：`life.*`

给一个人格开一条一直跑循环的「人生会话」：她定下次几点继续，每一轮过一段自己的日子，记下的进人格记忆。一个人格一条。人生会话就是一个普通会话，用出厂的预设 `life`（推演人生）开；到点继续靠定时表里人生那一行（`loop` 是 `life`），这一行在 `schedule.list` 和 `schedules` 流里看得到（见[定时任务](schedules.md)），但只有这一组方法动得了：通用的定时方法改它、删它回 `life_schedule`。英文版：[life.en.md](life.en.md)。

| 方法 | 做什么 |
|---|---|
| [`life.start`](#lifestart) | 给一个人格开人生会话，第一轮马上开始 |
| [`life.status`](#lifestatus) | 问这个人格的人生会话在哪、在不在跑 |
| [`life.pause`](#lifepause) | 暂停：到点不继续 |
| [`life.resume`](#liferesume) | 继续：马上开始一轮，接着照她定的时间继续 |
| [`life.kill`](#lifekill) | 结束这段人生：不再继续、归档只读，这段人生的记忆清掉 |
| [`life.new`](#lifenew) | 有就先结束，再开一条新的 |
| [`life.list`](#lifelist) | 列出所有人格没结束的人生会话，和结束了、还没看过的 |
| [`life.seen`](#lifeseen) | 说一段结束了的人生看过了 |

**谁能调**：只给人（本机、远程的头都行）；核心拉起的扩展调了回 `local_only`。

除了 `life.list`（不带参数）、`life.seen`（参数是 `session`），这几个方法的参数都只有一个：`persona`（字符串，必写），人格的编号。没有这个人格的回 `unknown_persona`。

**认得出人生会话**：会话日志里记着一条 [`life.started`](events.md)；会话列表（`session.list`、`sessions` 流）里那一项多一格 `life: true`，别的会话不写这一格。结束了的另记一条 [`life.ended`](events.md)，列表那一项再多一格 `life_ended: true`。

## 状态

`life.status`、`life.pause`、`life.resume` 都回这样一个对象：

| 名字 | 说明 |
|---|---|
| `state` | `none`（这个人格没有人生会话）、`running`（在跑）、`paused`（暂停着）、`ended`（最近的那一条已经结束、还没开新的） |
| `session` | 人生会话的编号；`none` 的不写 |
| `next` | 下一轮开始的时刻；只有 `running` 的写 |
| `rounds` | 推演到第几轮：每次到点开始的算一轮，开出来马上开始的那一轮是第 1 轮；暂停着不加。只是记着，头不显示。`none`、`ended` 的不写 |
| `doing` | 她在人生会话里最近一次定下次继续时写的「现在在干嘛」：`{"text", "at"}`，`text` 是那一句（换行换成了空格，最多 200 个字），`at` 是写下的时刻；她没写过、最近一次没写的不带。暂停着的照样带 |

定时表那一行不在了（例如表坏了、换成了空的）的，算 `paused`：`life.resume` 照会话日志把它重建出来。

她写的「现在在干嘛」也会在同一个人格别的会话（本机的、群和私聊、分叉出来的支线；派出去的子代理不算）的下一轮开头交给她，人生在跑才交，同一句不重复交。

## `life.start`

照这一刻的默认模型、预设 `life`、这个人格开一个新会话（标题照常自动起），上下文上限照人格写的 `life.context` 设，没写的 100000 token（这一刻的模型最少要的比它大的，照最少要的设）。以后在人格里经 `persona.set` 改的当场跟着换；在人生会话里经 `session.configure` 的 `context` 改的也一直管用，到了下一轮不会改回去，两处后改的算数，在日志里记下身份，再建人生那一行，第一次是现在，于是第一轮马上开始。回应 `{"session": <编号>}`。

- 这个人格已经有人生会话的回 `life_exists`，`data.session` 是那一个。最近的那一条已经结束了的不算有：开一条新的（旧的留着当档案，它的记忆已经清了）。两个连接同时开同一个人格的，只开出一条，另一个回 `life_exists`。
- 会话开不成的照 `session.create` 的原因回，什么都没留下。设上下文上限、记身份没成的，删掉刚开的会话，照原因回。
- 身份记上了、定时表那一行没建成的（例如定时任务已经有 200 个，回 `too_many_schedules`），拒绝的 `data.session` 写着这个会话：它已经是这个人格的人生会话，`life.status` 说 `paused`，腾出地方以后 `life.resume` 接上。

```json
{"jsonrpc": "2.0", "id": "l1", "method": "life.start", "params": {"persona": "miyu"}}
{"jsonrpc": "2.0", "id": "l1", "result": {"session": "<编号>"}}
```

## `life.status`

回应见上面「状态」。

```json
{"jsonrpc": "2.0", "id": "s1", "method": "life.status", "params": {"persona": "miyu"}}
{"jsonrpc": "2.0", "id": "s1", "result": {"state": "running", "session": "<编号>", "next": "2026-11-14T10:00:00.000Z"}}
```

## `life.pause`

暂停：到点不继续，下一次清掉。暂停着的再暂停照旧。回应见上面「状态」。没有人生会话的回 `no_life`，已经结束了的回 `life_ended`。

## `life.resume`

继续：下一次写成现在，马上开始一轮，之后照她定的时间继续；她留给下次自己的话照旧留着。在跑的不变；定时表那一行不在的照日志重建。回应见上面「状态」。没有人生会话的回 `no_life`，已经结束了的回 `life_ended`。

## `life.kill`

结束这段人生，做完才回：日志里记一条 `life.ended`（人结束的，没有遗言），正在跑的那一轮打断；会话归档只读（发话回 `session_sealed`），照常能读、能翻历史；人生那一行删掉，往后不再继续；人格记忆里出处全在这个会话里的条目清掉（清掉的恢复不了），以前对话的索引里拿掉它；给订着 `notices` 流的头推一条 `life.ended`（见[推送](pushes.md)）。没有接上的入口。回应 `{"state": "ended", "session": <编号>}`。

- 没有人生会话的回 `no_life`。
- 已经结束了的照样成功：清到一半没清完的（例如核心中途停了）接着清完，不再推通知。

## 她自己结束

人生会话的工具面上有一件 `end_life`（出厂的包「结束人生」，`base-life`）：她可以自己结束这段人生，必须写遗言。这件工具只在没结束的人生会话里有，拿预设 `life` 开的普通会话没有。她调了以后，这一轮照常说完，之后和 `life.kill` 一样：会话归档只读、不再继续、这段人生的记忆清掉，日志里那条 `life.ended` 的 `by` 是她那次工具调用、带遗言；推给头的 `life.ended` 通知 `by` 是 `self`、带 `last_words`（见[推送](pushes.md)）。之后 `life.status` 说 `ended`。

## 她记下自己变了什么

同一个包里另有一件 `note_change`（功能「后天」，`change`，预设里能单独关），也只在没结束的人生会话里有：她在人生里觉得自己真的变了（习惯、喜好、说话的样子、对人对事的态度），记下一条，最多留 10 条，可以改一条、去掉一条。记下的是记忆里类为 `change` 的一条（见[记忆](memory.md)）：这个人格每个会话的常驻记忆里都带着，别的会话、协议都改不了，人只能用 `memory.list`（`class` 写 `change`）看、`memory.forget` 作废、`memory.restore` 恢复，`memory.update` 改它回 `change_note`。人生结束时随这段人生的记忆一起清掉，`life.new` 开的新一段从零条开始。

## `life.new`

有在跑、暂停着的人生先照 `life.kill` 结束，再照 `life.start` 开一条新的。回应 `{"session": <新的编号>, "ended": <结束了的那一条>}`，没有要结束的不写 `ended`。新的一条想不起旧的那一段（记忆已经清了）。

## `life.list`

列出所有人格没结束的人生会话（在跑的、暂停着的），一个人格至多一条，照人格的编号排。不带参数，只读，不在哪个会话里留痕；头的「推演人生」页、快速跳转照它列。

```json
{"jsonrpc": "2.0", "id": "q1", "method": "life.list", "params": {}}
{"jsonrpc": "2.0", "id": "q1", "result": {"lives": [
  {"state": "running", "session": "<编号>", "next": "2026-11-14T10:00:00.000Z", "persona": "miyu", "name": "Miyu"}
]}}
```

`lives` 的一项就是 [`life.status`](#状态) 对那个人格的回应（`state` 只会是 `running`、`paused`），多带三格：

| 名字 | 说明 |
|---|---|
| `persona` | 人格的编号 |
| `name` | 人格给人看的名字，照连接的语言；没起名字的是编号 |
| `busy` | 她这会儿正在跑一轮的写 `true`，别的不写 |

一条都没有的 `lives` 是空的。命令 `/life`（光打）的回应里带同一份 `lives`，这个会话的人格的那一条排最前（见[软件包和命令](packages.md)）。

回应里另有一格 `ended_unseen`：她自己结束了、还没人看过的那几段人生，结束得早的在前，没有的是空的。一项 `{"session", "persona", "name", "at"}`：那个人生会话、人格的编号和名字（会话那时不带人格的不写）、结束的时刻。头照它在顶上出一条提示、在推演人生的入口上画记号。主人结束的不进这里；她结束的那一刻有头正开着那个会话的也不进。有头打开那个会话（`session.viewing`）、调了 [`life.seen`](#lifeseen)、会话删了就清掉，清掉时推一条 `life.seen`（见[推送](pushes.md)）。

## `life.seen`

说会话 `session` 上那段结束了的人生看过了（人收起了提示）：从 `ended_unseen` 里拿掉，推一条 `life.seen`。本来就不在里面的照样回 `{}`。

```json
{"jsonrpc": "2.0", "id": "s2", "method": "life.seen", "params": {"session": "<编号>"}}
{"jsonrpc": "2.0", "id": "s2", "result": {}}
```

## 出错

| `reason` | 什么时候 | `data` |
|---|---|---|
| `unknown_persona` | 没有这个人格 | |
| `life_exists` | `life.start`：这个人格已经有人生会话 | `session` |
| `no_life` | `life.pause`、`life.resume`、`life.kill`：这个人格没有人生会话 | |
| `life_ended` | `life.pause`、`life.resume`：这个人格最近的那一段人生已经结束了 | `session` |
| `too_many_schedules` | `life.start`、`life.resume`：定时任务已经有 200 个，人生那一行建不上 | `life.start` 的带 `session` |
