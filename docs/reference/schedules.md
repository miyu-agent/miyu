# 定时任务：`schedule.*`

到点把一句话交给一个会话跑一轮：每天早上 8 点的早报、每小时看一眼服务器。一个账号一张唤醒表，存在 `home/<账号>/state/schedules.db`，核心重启不丢；核心停着时错过的不补。设计见 `docs/designs/45-定时和循环.md`，怎么走见 `docs/blueprint/schedules.md`。英文版：[schedules.en.md](schedules.en.md)。

| 方法 | 做什么 |
|---|---|
| [`schedule.add`](#scheduleadd) | 建一个 |
| [`schedule.list`](#schedulelist) | 列出这个账号的 |
| [`schedule.cancel`](#schedulecancel) | 删掉 |
| [`schedule.pause`](#schedulepause) | 停 |
| [`schedule.resume`](#scheduleresume) | 续 |

**谁能调**：只给人，成员也能（碰的是他自己的表）；扩展调了回 `local_only`。都照这个连接替谁干活的账号：别人的编号一律 `unknown_schedule`。要一直跟着最新的，订阅 `schedules` 流（[连接](connection.md)），之后推 `schedules.changed`（[推送](pushes.md)）。

## 一项

| 名字 | 说明 |
|---|---|
| `id` | 编号 `t1`、`t2`……，账号里递增，删了的号不再用 |
| `when` | 什么时候：`{"cron": "0 8 * * *", "recurring": true}`（标准五段，照核心所在机器的时区）、`{"at": "<时刻>"}`（一次）、`{"every": <秒>}`（每隔一阵，至少 60） |
| `prompt` | 交进会话的那句话，原样 |
| `target` | 跑在哪：`{"session": <编号>}`，或 `{"new": {"persona", "preset", "cwd"}}`（每次照这几样新开一个，`persona` 是 `null` 的是无人格） |
| `by` | 谁建的：`{"kind": "person", "account"}`、她在会话里建的 `{"kind": "model", "session"}`、包建的 `{"kind": "package", "package"}` |
| `recurring` | 跑完还在不在：`at` 的是 `false`，`every` 的是 `true`，`cron` 的照写的 |
| `paused` | 停着 |
| `loop` | 循环才有：`fixed` 固定间隔、`dynamic` 她自己定节奏 |
| `next` | 下一次的时刻；停着的、她自己定节奏还没定的没有 |
| `last` | 上一次：`{"at", "result", "session"?}`，`result` 是 `ran`、`queued`（会话正在跑一轮，排在后面）或 `skipped:<原因>`（`session_gone` 会话没了、任务跟着停了，`preset_off` 她建的而会话的预设关了它，`not_opened` 新会话开不成） |
| `created` | 建的时刻 |

没有的格不出现。时刻的写法同别处：`2026-10-26T08:00:00.000Z`。

## `schedule.add`

| 参数 | 类型 | 说明 |
|---|---|---|
| `when` | 对象，必写 | 三种写一种，同上；`recurring` 只跟 `cron` |
| `prompt` | 字符串，必写 | 不能是空的 |
| `target` | 对象，可以不写 | `{"session": <编号>}` 或 `{"new": {"persona"?, "preset"?, "cwd"?}}`，两种写一种；不写的是每次新开。`persona`、`preset` 不写的照这时的默认定下来，`cwd` 不写是家目录 |

回应 `{"schedule": <一项>}`。写错的什么都不建，回 `bad_params`，`data.problem` 说是哪一样：`when`（没写对一种、时刻写法不对）、`cron`（读不成）、`past`（时刻已经过了）、`short`（隔得少于 60 秒）、`prompt`（空的）、`session`（没有、不是你的）、`target`（两种都写了）。人格、预设找不到的照开会话回 `unknown_persona`、`unknown_preset`。

## `schedule.list`

参数 `session` 可以不写：写了的只列跑在它上面的。回应 `{"schedules": [<一项>…]}`：有下一次的照时刻在前，停着的、没定下一次的在后。

## `schedule.cancel`

参数 `id`。回应 `{}`。

## `schedule.pause`

参数 `id`。停着的不到点，下一次清掉。回应 `{"schedule": <一项>}`。

## `schedule.resume`

参数 `id`。照现在重算下一次，错过的不补；一次的已经过了的回 `bad_params`（`past`）。回应 `{"schedule": <一项>}`。
