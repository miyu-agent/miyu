# 定时任务：`schedule.*`

到点把一句话交给一个会话跑一轮：每天早上 8 点的早报、每小时看一眼服务器。一张唤醒表，存在 `state/schedules.db`，核心重启不丢；核心停着时错过的不补。设计见 `docs/designs/45-定时和循环.md`，怎么走见 `docs/blueprint/schedules.md`。英文版：[schedules.en.md](schedules.en.md)。

| 方法 | 做什么 |
|---|---|
| [`schedule.add`](#scheduleadd) | 建一个 |
| [`schedule.list`](#schedulelist) | 列出全部 |
| [`schedule.cancel`](#schedulecancel) | 删掉 |
| [`schedule.pause`](#schedulepause) | 停 |
| [`schedule.resume`](#scheduleresume) | 续 |

**谁能调**：人；扩展要批过 `sessions.drive`（施工 J-4），没批的回 `needs_approval`，建的记成包建的，只看得到、删得了、停续得了自己建的：碰别人建的一律 `unknown_schedule`。要一直跟着最新的，订阅 `schedules` 流（[连接](connection.md)），之后推 `schedules.changed`（[推送](pushes.md)）。

## 一项

| 名字 | 说明 |
|---|---|
| `id` | 编号 `t1`、`t2`……，递增，删了的号不再用 |
| `when` | 什么时候：`{"cron": "0 8 * * *", "recurring": true, "zone": "Asia/Tokyo"}`（标准五段，照 `zone` 那个时区读：建的时候交的，没交的记核心所在机器那一刻的；以前建的没有 `zone`，照核心所在机器的）、`{"at": "<时刻>"}`（一次）、`{"every": <秒>}`（每隔一阵，至少 60） |
| `prompt` | 交进会话的那句话，原样 |
| `target` | 跑在哪：`{"session": <编号>}`，或 `{"new": {"persona", "preset", "cwd"}}`（每次照这几样新开一个，`persona` 是 `null` 的是无人格），或 `{"package": {}}`（不进会话，到点推 [`schedule.fired`](#schedulefired) 给建它的扩展） |
| `by` | 谁建的：人 `{"kind": "person"}`、她在会话里建的 `{"kind": "model", "session"}`、包建的 `{"kind": "package", "package"}` |
| `recurring` | 跑完还在不在：`at` 的是 `false`，`every` 的是 `true`，`cron` 的照写的 |
| `paused` | 停着 |
| `loop` | 循环才有：`fixed` 固定间隔、`dynamic` 她自己定节奏 |
| `next` | 下一次的时刻；停着的、她自己定节奏还没定的没有 |
| `last` | 上一次：`{"at", "result", "session"?}`，`result` 是 `ran`、`queued`（会话正在跑一轮，排在后面）、`sent`（推给了建它的扩展）或 `skipped:<原因>`（`session_gone` 会话没了、任务跟着停了，`preset_off` 她建的而会话的预设关了它，`not_opened` 新会话开不成，`package_unavailable` 建它的扩展没在跑、拉不起来） |
| `created` | 建的时刻 |

没有的格不出现。时刻的写法同别处：`2026-10-26T08:00:00.000Z`。

## `schedule.add`

| 参数 | 类型 | 说明 |
|---|---|---|
| `when` | 对象，必写 | 三种写一种，同上；`recurring`、`zone`（IANA 时区名，例如 `Asia/Tokyo`）只跟 `cron` |
| `prompt` | 字符串，必写 | 不能是空的 |
| `target` | 对象，可以不写 | `{"session": <编号>}`、`{"new": {"persona"?, "preset"?, "cwd"?}}` 或 `{"package": {}}`，写一种；不写的是每次新开。`package` 只有扩展能写，人写了回 `bad_params`（不带 `data.problem`）。`persona`、`preset` 不写的照这时的默认定下来，`cwd` 不写是家目录 |

回应 `{"schedule": <一项>}`。写错的什么都不建，回 `bad_params`，`data.problem` 说是哪一样：`when`（没写对一种、时刻写法不对）、`cron`（读不成）、`past`（时刻已经过了）、`short`（隔得少于 60 秒）、`prompt`（空的）、`session`（没有这个会话，或者这个连接碰不到它：说法一样）、`target`（两种都写了）、`zone`（时区不认识）、`cwd`（每次新开的工作目录不在、不是目录，或者落在数据根里工作区以外）。已经有 200 个（人建、她建、包建合计，熔断性质的安全上限）的回 `too_many_schedules`，先删几个；已有的照跑。人格、预设找不到的照开会话回 `unknown_persona`、`unknown_preset`。

## `schedule.list`

参数 `session` 可以不写：写了的只列跑在它上面的。回应 `{"schedules": [<一项>…]}`：有下一次的照时刻在前，停着的、没定下一次的在后。

## `schedule.cancel`

参数 `id`。回应 `{}`。

## `schedule.pause`

参数 `id`。停着的不到点，下一次清掉。回应 `{"schedule": <一项>}`。

## `schedule.resume`

参数 `id`。照现在重算下一次，错过的不补；一次的已经过了的回 `bad_params`（`past`）。回应 `{"schedule": <一项>}`。

## `schedule.fired`

核心 → 扩展，通知（施工 J-4）：扩展建的、`target` 是 `{"package": {}}` 的到点了，推给那个包的进程，不用订阅、不等回应。

| 名字 | 说明 |
|---|---|
| `id` | 任务编号 |
| `at` | 这一次该响的时刻 |
| `prompt` | 建的时候写的，原样交回（写法扩展自己定，例如 JSON） |
| `missed` | 核心停着时错过了几次，最多数到 1000；错过的不补，只报个数。没错过的不写 |

包没连着的不拉起，推不到的记 `skipped:package_unavailable`，一次的照常删、周期的等下一次。
