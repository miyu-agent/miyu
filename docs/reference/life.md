# 人生会话：`life.*`

给一个人格开一条一直跑循环的「人生会话」：她定下次几点醒，醒来过一段自己的日子，记下的进人格记忆。一个人格一条。人生会话就是一个普通会话，用出厂的预设 `life`（推演人生）开；醒来靠定时表里人生那一行（`loop` 是 `life`），这一行谁都碰不到（不在 `schedule.list` 里，见[定时任务](schedules.md)），只有这一组方法动得了。设计见 `docs/designs/70-推演人生.md`，怎么走见 `docs/blueprint/life.md`。英文版：[life.en.md](life.en.md)。

| 方法 | 做什么 |
|---|---|
| [`life.start`](#lifestart) | 给一个人格开人生会话，马上醒第一次 |
| [`life.status`](#lifestatus) | 问这个人格的人生会话在哪、在不在跑 |
| [`life.pause`](#lifepause) | 暂停：到点不醒 |
| [`life.resume`](#liferesume) | 继续：马上醒一次，接着照她定的醒 |
| [`life.kill`](#lifekill) | 结束这段人生：不再醒、归档只读，这段人生的记忆清掉 |
| [`life.new`](#lifenew) | 有就先结束，再开一条新的 |

**谁能调**：只给人（本机、远程的头都行）；核心拉起的扩展调了回 `local_only`。

这几个方法的参数都只有一个：`persona`（字符串，必写），人格的编号。没有这个人格的回 `unknown_persona`。

**认得出人生会话**：会话日志里记着一条 [`life.started`](events.md)；会话列表（`session.list`、`sessions` 流）里那一项多一格 `life: true`，别的会话不写这一格。结束了的另记一条 [`life.ended`](events.md)，列表那一项再多一格 `life_ended: true`。

## 状态

`life.status`、`life.pause`、`life.resume` 都回这样一个对象：

| 名字 | 说明 |
|---|---|
| `state` | `none`（这个人格没有人生会话）、`running`（在跑）、`paused`（暂停着）、`ended`（最近的那一条已经结束、还没开新的） |
| `session` | 人生会话的编号；`none` 的不写 |
| `next` | 下一次醒的时刻；只有 `running` 的写 |
| `doing` | 她在人生会话里最近一次定下次醒来时写的「现在在干嘛」：`{"text", "at"}`，`text` 是那一句（换行换成了空格，最多 200 个字），`at` 是写下的时刻；她没写过、最近一次没写的不带。暂停着的照样带 |

定时表那一行不在了（例如表坏了、换成了空的）的，算 `paused`：`life.resume` 照会话日志把它重建出来。

她写的「现在在干嘛」也会在同一个人格别的会话（本机的、群和私聊、分叉出来的支线；派出去的子代理不算）的下一轮开头交给她，人生在跑才交，同一句不重复交。

## `life.start`

照这一刻的默认模型、预设 `life`、这个人格开一个新会话（标题照常自动起），上下文上限设成 32000 token（这一刻的模型最少要的比它大的，照最少要的设；以后照常能经 `session.configure` 的 `context` 改），在日志里记下身份，再建人生那一行，第一次是现在，于是马上醒第一次。回应 `{"session": <编号>}`。

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

暂停：到点不醒，下一次清掉。暂停着的再暂停照旧。回应见上面「状态」。没有人生会话的回 `no_life`，已经结束了的回 `life_ended`。

## `life.resume`

继续：下一次写成现在，马上醒一次，之后照她定的醒；她留给下次自己的话照旧留着。在跑的不变；定时表那一行不在的照日志重建。回应见上面「状态」。没有人生会话的回 `no_life`，已经结束了的回 `life_ended`。

## `life.kill`

结束这段人生，做完才回：日志里记一条 `life.ended`（人结束的，没有遗言），正在跑的那一轮打断；会话归档只读（发话回 `session_sealed`），照常能读、能翻历史；人生那一行删掉，往后不再醒；人格记忆里出处全在这个会话里的条目清掉（清掉的恢复不了），以前对话的索引里拿掉它；给订着 `notices` 流的头推一条 `life.ended`（见[推送](pushes.md)）。没有接上的入口。回应 `{"state": "ended", "session": <编号>}`。

- 没有人生会话的回 `no_life`。
- 已经结束了的照样成功：清到一半没清完的（例如核心中途停了）接着清完，不再推通知。

## `life.new`

有在跑、暂停着的人生先照 `life.kill` 结束，再照 `life.start` 开一条新的。回应 `{"session": <新的编号>, "ended": <结束了的那一条>}`，没有要结束的不写 `ended`。新的一条想不起旧的那一段（记忆已经清了）。

## 出错

| `reason` | 什么时候 | `data` |
|---|---|---|
| `unknown_persona` | 没有这个人格 | |
| `life_exists` | `life.start`：这个人格已经有人生会话 | `session` |
| `no_life` | `life.pause`、`life.resume`、`life.kill`：这个人格没有人生会话 | |
| `life_ended` | `life.pause`、`life.resume`：这个人格最近的那一段人生已经结束了 | `session` |
| `too_many_schedules` | `life.start`、`life.resume`：定时任务已经有 200 个，人生那一行建不上 | `life.start` 的带 `session` |
