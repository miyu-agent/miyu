## 施工 LO-3：循环的两处收尾

状态：已完成（2026-11-16）。LO-2 交回时留了两件待拍板，项目主人 2026-11-16 定：

1. 她自己定节奏的循环没定下一次就结束（照 Claude Code「不调就停」）：不带间隔的 `/loop`（`dynamic`）交进去开的那一轮里她没调 `schedule_wakeup`，那一轮结束时删掉那一行、推 `removed`，头上的「◷」跟着没了。推翻 J-2 的「停着等她定」。只管 `dynamic`；固定间隔的、人生、定时任务不动。人插的、别的会话发来的一轮不算。
2. `/loop clear`、`/loop stop` 也算关：同 `/loop off`。只认单独一个词，大小写不论、前后空白不算；`/loop clear the cache` 照旧是要说的话。

### 查到的

- 到点时（`run.rs`）她自己定节奏的那一行先把下一次清掉再交，她在那一轮里调 `schedule_wakeup` 写回去（`set_next`）。没调的，那一行一直留着、`next` 是空的：再也不会到点，头照定时表画「◷」。LO-2 的主人数据根里就是这样：图标在、循环不在。
- 每次新开的定时已经有「交之前先订阅、等交进去的那一条开的一轮结束」的现成写法（`schedules/ran.rs`，认 `turn.started` 的 `trigger`），照它盯就分得清是不是循环自己那一轮。
- `/loop off` 走 `Schedules::cancel`，删完经 `notify` 推 `removed`。照同一条路推就行，头不用改。
- im 群里的 `/loop` 由 im 自己截下、自己解析（`miyu-im` 的 `core/route/command/looping.rs`，只认光打和 `off`），不到核心，不共用这处解析；私聊和本机的头走核心这一路，`clear`、`stop` 在私聊里跟着生效。这一步不动 im。

### 怎么改

1. **盯循环自己那一轮**（`schedules/ended.rs`，新文件）：`fire.rs` 交 `dynamic` 那一行之前先订阅会话（同每次新开的那种），交的那一刻会话空着的才盯（正忙的排进了别人的一轮，LO-2 以后只在极少的竞争里有）。认 `turn.started` 的 `trigger` 是交进去的那一条的那一轮，它结束时（怎么结束的都算：打断、出错的留着那一行也不会再到点）查那一行。交进去的那一条并进了别人开的一轮的，那一轮结束就不再盯。
2. **查和删一条语句**（`miyu_store::schedules::end_unpaced`）：`DELETE … WHERE id = ? AND loop = 'dynamic' AND next_at IS NULL AND paused = 0`。她这一轮里定了下一次的、人这时停了它的、再敲 `/loop` 换掉了的都不删，不和她的 `schedule_wakeup` 抢。删了的记 `INFO loop ended without a next wakeup`，叫醒睡着的那个、`notify` 推 `removed`。
3. **`clear`、`stop` 同 `off`**（`commands/looping.rs` 的 `parse`）：先去掉前后空白，单独一个 `off`、`clear`、`stop`（大小写不论）是关。`/loop OFF` 以前开一个说「OFF」的循环，现在同样是关。
4. **用法三语**：`commands/usage/loop/off` 补「也可以写 clear、stop」。这是给人看的字（`resources/core/human/`），`/loop` 的命令说明、用法不进模型面，不登记；`cargo xtask prompts` 没有变化。

### 蓝图改哪几节

- `schedules.md`：状态行；第一节 `next_at`；第三节第 2 条、加第 10 条「她不定下一次循环就结束」；第四节 `set_next`；第五节第 9 条（`clear`、`stop`、不调就停删掉那一行）；第八节代码、测试。
- `protocol.md`「`command.run`」第 16 条；设计 45 第三节动态那一条；协议参考 `packages.md`（中英）`/loop` 一行。

### 不做什么

- 核心重启：盯只在内存里。核心在循环那一轮里停了的不删；起来时错过的 `dynamic` 照第二节第 4 条清掉下一次、停着（`schedule_life.rs` 那条照旧）。这种行同样再也不会到点、图标留着，要不要在起来时一并删，待项目主人拍板（见交回）。后来定了删，施工中还查出正常重启会误删那一行，一起在 LO-3补 修了（`LO-3补-重启清掉停了的循环.md`）。
- 不动 im 群里的 `/loop`；不动固定间隔的、人生、定时任务。

### 验收

1. 改前红（改了的源文件换回 main 那份跑）：
   - `tests/loop_ends.rs` 的 `a_dynamic_loop_ends_when_its_turn_sets_no_next`（她没调：那一行删了、推了 `removed`）、`clear_and_stop_alone_turn_the_loop_off`（`/loop clear`、`/loop STOP`、`/loop stop ` 关掉，`/loop clear the cache` 开循环）；
   - `tests/schedules.rs` 的 `a_dynamic_loop_ends_after_a_turn_that_sets_no_next`；
   - `commands/looping/tests.rs` 的 `clear_and_stop_alone_stop_the_loops_too`。
2. 守住的（改前改后都绿）：`a_dynamic_loop_whose_turn_sets_the_next_stays`（调了的留着、下一次是她定的、到点再跑）、`a_turn_a_person_starts_while_the_loop_waits_does_not_end_it`（循环等着时人插的一轮没调，不删）；存储那一层 `an_unpaced_dynamic_loop_ends_and_nothing_else_does`。
3. J-2、LO-1、LO-2 那几条照新规矩改：`loop_command.rs` 三条（动态的、光打的、重发的）让她在那一轮里定下一次，断言照她定的；`loop_busy.rs` 第一条断言那一行删了；`schedules.rs` 的「停着等她定」那条换成第 1 条那一条；命令菜单、写错带用法两条照新的用法字。
4. 门禁全过；`cargo test -p miyu-kernel random:: -- --include-ignored`；整个工作区 Windows 版 clippy；`cargo xtask prompts`、`cargo xtask logs` 没有要改的。
5. 实测（2026-11-16，沙箱数据根、真模型 marvis/deepseek-v4-1-flash-261017、终端界面在 niri 代理窗口里操作）：
   - `/loop 用一句话报一下现在几点，不要定下一次`：她答「现在是 2026年11月16日 22:30（JST，UTC+9）。下一次唤醒不安排，循环到此结束。」，答完图标「◷」没了；日志 `22:30:11.234 schedule fired schedule=t1 result=ran`、`22:30:16.997 loop ended without a next wakeup schedule=t1`。
   - `/loop 报时，然后定 1 分钟后再报`：她调 `schedule_wakeup`，图标「◷ 22:31」留着；22:31:48 到点再跑（`schedule fired schedule=t2 result=ran`），又定了一次，图标「◷ 22:32」。
   - `/loop clear`：日志 `22:32:08.267 schedule cancelled schedule=t2`，图标当场没了，没有新建的一行，22:33 以后也没再到点。

### 风险

- 她这一轮里先调 `schedule_wakeup` 再调 `stop: true` 的：照旧删掉（`stop` 那一路），这里查不到那一行，什么都不做。
- 订阅只为盯这一轮：那一轮结束、并进别人的一轮结束、会话没了，都放掉。
