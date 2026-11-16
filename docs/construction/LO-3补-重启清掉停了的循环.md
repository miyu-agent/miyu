## 施工 LO-3补：核心停过以后的循环

状态：已完成（2026-11-16）。LO-3 交回时留的一件（核心停过以后，「动态、没定下一次」的那一行还留着），核心主会话照推荐定「删」；施工中实测查出 **LO-3 带进来的一个问题**，核心主会话同日定了改法（A），一起修。

### 查到的

- **留着的那一行**：LO-3 只在内存里盯循环那一轮。核心被强杀、断电的，那一轮的结束没人看见；核心停着时错过的，起来照「错过的不补」清掉下一次。这两种都剩一行「动态、没定下一次」：再也不会到点，头上一直画「◷」。
- **LO-3 带进来的问题**：核心有计划地停下（`SIGTERM`、`stop_sessions`）时，正在跑的那一轮照「重启了」结束（`turn.ended` 的 `reason` 是 `restarted`），再载入时内核接着开一轮把它干完。LO-3 的盯梢把「重启了」也当成那一轮结束了，当场删掉那一行。接着的那一轮里她调 `schedule_wakeup`，报找不到循环，循环断了。实测（沙箱、真模型）：停核心那一刻日志就有 `loop ended without a next wakeup schedule=t3`，起来以后续上的那一轮里 `schedule_wakeup` 是 `error=true`。
- 接着干的那一轮由「重启了」那一条触发，`trigger` 不是交进去的那一条，按 `trigger` 认不出。它的命令编号（`cause`）照旧是 `schedule/<任务>/<编号>/say`，按这个能认。
- 被强杀的那一轮没有「重启了」那一条，载入时不接着干（实测：强杀以后起来，会话载入了，没有新的请求）。

### 怎么改（A，核心主会话 2026-11-16 定）

1. **「重启了」结束的那一轮不删**（`schedules/ended.rs`）：盯着的那一轮照 `restarted` 结束的，不查表、不删，那一行留着、下一次还空着。别的结束照 LO-3。
2. **核心起来时接着盯，或者删掉**（`ended.rs` 的 `boot`，`start_schedules` 在跳过错过的以后调）：「动态、没定下一次、没停着」的每一行，起一个任务载入它跑在的会话，先订阅再看忙不忙。
   - **忙**：在跑的是接着干的那一轮，照命令编号前缀 `schedule/<任务>/` 认。看到的头一个 `turn.ended` 是它的，就照第 1 条办；不是它的，说明循环那一轮没接着干，删掉。
   - **空着**（被强杀、错过的），或者会话没了：当场照 `end_unpaced` 删掉，推 `removed`。
   - 人停着的那一行留着，给人续。
3. 盯梢的判断抽成一个小的 `Seen::judge`：交进去的那一轮认 `trigger`，接着干的那一轮认 `cause` 前缀，两种共用同一个删法。

不选「起来时一律删」（B）：接着干的那一轮里她照样定不了下一次，只是删的时刻往后挪了。

### 蓝图改哪几节

`schedules.md`：状态行；第二节第 4 条（错过的 `dynamic`）；第三节第 10 条，加第 11 条「核心停过以后的循环」；第八节代码、测试。

### 验收

1. 改前红（`ended.rs`、`fire.rs`、`run.rs` 换回 main 那份跑过，四条都红）：
   - `tests/loop_ends.rs` 的 `a_planned_stop_lets_the_resumed_turn_set_the_next`：循环那一轮停在请求上时 `stop_sessions`，那一行留着、下一次空着；再起来那一轮接着干，她调 `schedule_wakeup` 成功，那一行留着、下一次是她定的。
   - `a_resumed_turn_that_sets_no_next_ends_the_loop`：同上，接着的那一轮里她没定，那一轮结束时删掉。
   - `a_loop_left_without_a_next_ends_on_restart`：模拟强杀留下的那一行（交过、没定），起来时删掉；人停着的、定了下一次的留着。
   - `tests/schedule_life.rs` 的 `a_missed_life_loop_wakes_once_on_restart_and_a_dynamic_one_ends`（原来守「照旧停着等」，改成删掉、没交）。
2. 循环、定时、人生相关的测试全过。
3. 门禁全过；`cargo test -p miyu-kernel random:: -- --include-ignored`；整个工作区 Windows 版 clippy；`cargo xtask prompts`、`cargo xtask logs` 没有要改的。
4. 实测（2026-11-16 至 17，沙箱数据根、真模型 marvis/deepseek-v4-1-flash-261017、终端界面在 niri 代理窗口里操作），两次都敲 `/loop 用一句话报一下现在几点，然后定 2 分钟后再报`，看到 `schedule fired` 马上按 pid 停核心：
   - **强杀**（`SIGKILL`）：`23:03:18.590 schedule fired schedule=t4`，`23:03:20` 杀掉。起来时 `23:03:24.339` 会话载入（空着），`23:03:24.348 loop ended without a next wakeup schedule=t4`，表空了，头上的图标没了。
   - **正常停**（`SIGTERM`）：`23:03:42.063 schedule fired schedule=t5`，`23:03:44.064 stopped`，停的时候不删，表里 `t5|dynamic|`（下一次空着）。起来时 `23:03:47.713` 会话载入，那一轮接着干：`23:03:51.666 tool=schedule_wakeup`，成功，表里下一次 23:05:51，图标「◷ 23:05」。`23:05:51.736 schedule fired schedule=t5` 到点再跑，她又定了下一次，图标「◷ 23:07」。

### 风险

- 起来时要载入挂着这种行的会话，每个会话一个任务、一次载入。这种行只在核心停过、错过时才有，很少。
- 起来时会话忙的，可能是接着干的别的轮（例如人那一轮被重启打断了）或者刚到点的人生那一轮：循环那一轮没接着干，那一轮结束时删掉。这一行本来就不会再到点了。
