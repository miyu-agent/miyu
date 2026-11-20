## 施工 LF-1：定时表多一种「人生」循环

状态：已完成（2026-11-14）。设计 `70-推演人生.md` 第三节第 1、2 条。

### 目的

定时表的 `loop` 多一种 `life`。它和她自己定节奏的循环走同一段代码，只有三处不同：某一轮没定下次也照默认间隔再醒、核心起来时错过的马上补醒（停了多久都只补一次）、`stop` 不收。通用的定时入口（协议 `schedule.*`、`schedules` 流，她的 `cron_list`、`cron_delete`）碰不到这一行，只有以后的 `life.*` 动得了：不然删一行定时就绕开了「只有有意的结束」（核心主会话 2026-11-14 审定）。这一步只有这一种循环本身；谁来建它（`/life`）、醒来那段话、进记忆、结束都是后面的步子。

### 蓝图改哪几节

- `schedules.md` 第一节的表：`loop` 一格多 `life`。
- 第二节第 4 条（错过的不补）：`life` 的起来时把下一次写成现在，马上到点一次；停了多久（例如三天）都只补这一次，交过以后下一次照「现在加默认间隔」。
- 第三节第 2 条（到点）：`life` 的先把下一次写成「现在加默认间隔」再交进会话；她在那一轮里定了的照她的（交过以后只记上一次那几格，不带着旧的下一次写回去）。默认间隔 1 小时，数放在哪照最短 60 秒那个数的放法。
- 第三节第 4 条：交进会话的 `by.loop` 多 `life`（`kernel/ids.md`「谁」跟着改）。
- 第四节 `update`：`life` 同 `dynamic`，`when` 只收 `at`（核心里的 `cancel`、`pause`、`resume`、`set_next`、`update` 不拦 `life`，以后 `life.*` 用）。
- 第五节第 2、3、3a、5、6 条：`schedule.list`、`schedules` 流的回应和推送不列 `life` 行；`schedule.cancel`、`pause`、`resume`、`update` 落在 `life` 行上照没有这个编号回 `unknown_schedule`（同扩展只看得到自己建的那套）。
- 第六节：`life` 的这一步先用循环那一对标签（LF-2 换成它自己的）。
- `tools/` 里 `schedule_wakeup` 那一页：跑在这个会话上的 `life` 那一行同样改下一次；`stop` 在 `life` 上不收，答一句（字登记进提示词登记簿）。`cron_list` 不列 `life` 行，`cron_delete` 删不了它（照没有回）。
- 协议参考 `docs/reference/schedules.md`、`schedules.en.md`：`loop` 的取值多 `life`。

### 不做什么

- 不做 `/life` 命令、协议 `life.*`、会话日志里的 `life.started`（LF-5、LF-4）。这一步的 `life` 行只有测试经 `Schedules::add` 建。
- 不动醒来那句话的写法（LF-2）、回合索引和抽取（LF-3）。
- 不加配置项。
- 不另加「你是照默认醒的」这类给模型看的话。

### 验收

1. `cargo test -p miyu-store schedules`：`life` 一行写进去读得回原样。
2. `cargo test -p miyu-endpoint --test all schedules`（手拨的钟）：
   - `life` 的到点交进会话以后，表里的下一次是到点那一刻加默认间隔，不是空的；
   - 她在那一轮里调了 `schedule_wakeup` 的，下一次照她定的；
   - 核心起来时已经过了的 `life`：起来马上交一次；同样情形的 `dynamic` 照旧停着等；
   - 核心停了三天再起来：`life` 只交一次，交过以后下一次是现在加默认间隔；
   - 她在被叫醒的那一轮里定了 10 分钟：回合结束、上一次也记完以后，表里的下一次是 10 分钟那个，不是默认的；
   - 在 `life` 上 `stop`：不收，那一行还在、下一次没动；`cron_list` 不列它，`cron_delete` 删不了它；
   - `schedule.*` 的列表（`schedule.list`、`schedules` 流的回应和推送）不列 `life` 行，`schedule.cancel`、`pause`、`resume`、`update` 落在它上面回 `unknown_schedule`，那一行不动。
3. `dynamic`、`fixed`、定时任务原有的测试一条不改、全过。
4. 交进会话那句话的 `by` 写出来是 `{"kind":"schedule","schedule":"t1","loop":"life"}`，读得回。
5. 门禁（`cargo xtask check`）全过；Windows 版 clippy 过。

### 风险

- `Loop`、`ScheduleLoop` 多一种以后，别处的 `match` 要补全：照编译器报的一处处补，每一处想清楚 `life` 该跟 `dynamic` 还是另算，不写 `_ =>` 兜底。
- 老程序读到 `loop` 是 `life` 的行：照现有的「一行读不懂的跳过」，不用另管。
