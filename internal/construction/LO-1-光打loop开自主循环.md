## 施工 LO-1：光打 `/loop` 开自主循环

状态：已完成（2026-11-14）。项目主人 2026-11-14 定：照 Claude Code，不带话的 `/loop` 开一个她自己定节奏的「自主」循环。编号原定 LP-1，和已有的 LP-1（扩展替本机页面换票）撞了，改叫 LO-1。

### 目的

以前光打 `/loop` 回 `bad_params`（「参数错误。」），也不说怎么写。现在光打 `/loop` 和 `/loop <话>` 一样建一行她自己定节奏的循环、马上跑一次，之后她每轮调 `schedule_wakeup` 定下一次，不调就停；不同的只是交进去的那句不是人写的，是一句固定的英文。参数写错了的，报错带上用法。

### 蓝图改哪几节

- `schedules.md` 第五节加第 9 条：`/loop` 的几种写法（以前只在施工单 J-2 里），光打的那一种、那句话从哪来、存在哪；写错了带用法。第八节代码、测试各加一行。
- `protocol.md`「`command.run`」加第 16 条 `/loop`，`bad_params` 那一行加上「`/loop` 写错了，`data.why` 是用法」。协议参考 `packages.md`（中英）「核心认的命令」加 `/loop` 一行，`bad_params` 写上 `data.why`。
- 给模型看的字一份：`resources/software/loop/autonomous.txt`，登记进 `26-提示词.md` 第十节，`cargo xtask prompts` 重新生成。原文（19 token，o200k）：

  ```text
  No new instructions. Continue on your own judgment, then schedule the next wakeup or stop.
  ```

  怎么续由收尾那一句（`loop/common/message-close.txt`「Call schedule_wakeup to continue the loop.」）和工具说明讲，这里不再写工具名。

### 「自主」怎么存

存那句话的原文，不另加标记。敲命令那一刻读出厂的文件、去掉行尾空白，原样写进定时表那一行的 `prompt`，`loop` 照旧是 `dynamic`。

- 改动最小：表的形状、到点交话（`schedules/fire.rs`）、组装（`miyu-assemble` 的 `schedule.rs`）、`schedule_wakeup` 一个都不用改。存标记的话，到点要换成那句、工具说明叫她「原样传回」的 `prompt` 也要认标记，几处都得改。
- 字节稳定：交进会话的就是存着的原文，记进 `message.user`，组装照日志渲染，同一份日志每次一样。以后改了这份文件，已经开着的循环照旧用它存的那一句，前缀不变；只有新开的用新的。
- 她照工具说明原样传回 `prompt` 的，传回的也是这一句；她自己换了一句的照她的（同 `/loop <话>`）。
- 头的定时列表里这一行的 `prompt` 显示这句英文：给人看的是真的交进去的话。

### 用法从哪来

照这个命令的 `Slash::usage()`：加一条 `("", "commands/usage/loop/auto")`（光打 `/loop`），命令菜单（`command.catalog` 的 `usage`）和报错共用 `commands/usage.rs` 的 `forms`，不另写一套。`data.why` 第一行「参数不对。用法：」（`commands/bad-usage`），下面一行一种写法（`commands/usage-line`：写法、空一格、做什么那一句）。终端照拒绝的 `data.why` 显示（`miyu-tui` 的 `take.rs`），不用改头。`commands/argument/loop` 改成 `[[<间隔>] <要说的话> | off]`。三种语言都加。

参数写错了现在只有一种：写了间隔、没写话（`/loop 5m`、`/loop every 5m`）。间隔本身写错的（`/loop 5x 看看`）照旧当成她自己定节奏的一句话（`looping/tests.rs` 守着，「every day is a gift」也是一句话），不改成报错。

### 不做什么

- 不分场所：群里的 `/loop` 由 im 自己截下，不到核心；私聊和终端、网页一样走这一路。
- 不改 `schedule_wakeup` 的工具说明和收尾那一句。
- 不拿真模型 A/B 这句话：登记照现在的原文，以后有实测再改（改了只影响新开的循环）。

### 验收

1. `miyu-endpoint` 单元测试：光打的读成自主，只写了间隔的照旧 `bad_params`。
2. `miyu-endpoint/tests/loop_command.rs`（真核心、手拨的钟）：
   - `a_bare_loop_runs_now_on_the_fixed_sentence`：光打 `/loop` 建一行 `dynamic`、人建的，马上交进一轮，存的和交进请求的都是出厂的那句，交过以后停着等她定；回执「已开始循环：由 AI 自己定做什么、下一次什么时候」。
   - `a_bare_loop_s_second_wakeup_keeps_the_prefix`：她调 `schedule_wakeup` 定 120 秒、没传 `prompt`，到点再交一次；第二轮请求的工具面、system 和第一轮一样，第一轮的消息原样在前。
   - `a_wrong_loop_answers_with_the_usage`：`/loop every 5m` 回 `bad_params`，`data.why` 是四行用法，什么都不建。
3. 改前红：前两条和单元测试那条改前是 `bad_params`；报错那条改前没有 `data.why`。
4. 原有的 `/loop`、定时、命令菜单测试一条不改、全过（菜单的用法照样，只多光打那一行）。
5. 门禁全过；Windows 版 clippy 过。

### 风险

- 出厂文件读不出：回 `internal_error`、记一行运行日志，原来的循环不动（先读再删）。
- 用法的字要连读几次人的字表：只在参数写错时走，和命令菜单一样。
