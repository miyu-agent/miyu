## 施工单 LF-6 修：推演人生的三条测试不再偶尔红

状态：已完成（2026-11-16）。三条都是测试等错了东西，核心的行为没改。

### 红在哪

| 测试 | 哪台 | 哪一趟 | 报的 |
|---|---|---|---|
| `miyu-session` 的 `memory_fresh::prefix::the_next_turn_starts_with_the_request_that_carried_the_block_byte_for_byte` | Windows | run 43728705001（DOC-2，只动文档） | 叫醒的那一次请求里没有醒来时交新记的那一块 |
| `miyu-session` 的 `memory_fresh::what_the_summary_cut_is_not_brought_on_waking` | Windows | WN-1 分支 c09779d2c 那一趟（没碰这些代码） | 同上：`fresh` 一块都没有 |
| `miyu-endpoint` 的 `life_command::bare_life_with_one_tells_its_state_and_carries_it` | Linux | 同上那一趟 | 回的是「人生会话进行中，这会儿醒着。」，期望「……下次醒来：……」 |

### 根因

**头两条**：回合开始问记忆各限时 300 毫秒（`actor/model/hooks.rs` 的 `HOOKS`），过了当这一轮没交、下一次醒来再交。这是有意的（人在等这一轮开始，读盘不能拖住它，`memory.md` 第三条第 4 款）。测试断言的却是「这一轮一定带」，等于拿真的磁盘去赛 300 毫秒，慢的机器上（Windows 的 CI）就输。同一个文件夹里 `the_first_wake_and_after_compaction…` 输过一回（施工 LF-7 里先把回合库开好绕过去），知识库那边也绕过一回（`knowledge_recall.rs` 的 `world`）。

**第三条**：`until_turns` 看的是日志文件里有没有回合结束那一行。那一行写进文件以后，写的线程回了话，会话才算空（内核的 `idle()`：结束了、`turn.ended` 还没落盘的不算空），才写「没在跑」。这中间打 `/life`，她照规矩还算醒着，回「这会儿醒着」。核心没错，测试等早了。

### 怎么证的（都在另一台测试机上，插延迟放大）

| 实验 | 没修的 | 修好的 |
|---|---|---|
| 回合开始读记忆每次先慢 400 毫秒，跑 `prefix` 那一条 | 红，报的和 CI 上逐字一样（只剩第一轮的那一块） | — |
| 同样慢 400 毫秒，跑 `what_the_summary_cut…` | 红：`left: []`、`right: [["m61"]]` | 绿 |
| 日志写完、回话以前慢 300 毫秒，跑 `/life` 那一条 | 红，报的和 CI 上逐字一样 | 绿 |

平常这一次读只要 0.07 毫秒，Linux 上从不红。Windows 上具体慢在哪一步没有数据（没有那边的计时），只证了「慢过时限就是这个样子」。头一回把延迟插在写「没在跑」那一步，没放大出来：那一步和回话在同一趟里，空当在它前面。

### 改了什么

1. **回合开始等记忆多久能在测试里改**（只有 `testkit` 才编进去）：`Memory::wait_reads(时限)`，记在这个核心的 `Reads` 里；`hooks.rs` 问记忆的四件（摘要、人卡、醒来时新记的、联想）照它，没改过的照出厂的 300 毫秒。出厂的行为一个字没变。做法同施工 X-16 修（回合开始问扩展等多久）。
2. **`miyu-session` 的测试场地**（`tests/support` 的 `Home::new`）把它换成 60 秒：断言「这一轮带了那一块」的测试不再赛真的磁盘。
3. **测「读得慢的不挡回合」的两条**（`memory_summary::a_slow_read_does_not_hold_the_turn`、`memory_fresh::slow::a_slow_read_does_not_hold_the_wake`）自己换成 50 毫秒：它们握着记忆日志的锁，测的就是过了时限。那一轮过去、放开锁以后换回 60 秒再测「下一轮交了」：头一版忘了换回去，Windows 的 CI 上下一轮输给了这 50 毫秒（run 43752924857），是这一步自己带出来的。
4. **去掉先前的绕法**：`the_first_wake_and_after_compaction…` 里先把回合库开好的那一行。
5. **守门的一条** `memory_fresh::slow::a_read_slower_than_the_shipped_limit_still_arrives_in_tests`：读慢 400 毫秒，这一轮照样带。有人把场地的时限改回去，它每次都红，不是偶尔红。
6. `memory_fresh.rs` 到了行数上限：读得慢的两条挪进 `memory_fresh/slow.rs`。
7. **`/life` 那一条**多一个等法 `asleep`：等到会话列表里这个人生会话那一项没有 `busy`，再打 `/life`。断言这句回话的只有这一条测试。

### 验收

- 上面三组放大实验：没修的红、修好的绿。
- 变异：把场地的 60 秒拿掉，守门的那一条红。
- 整个 `miyu-session` 的测试程序在两种时限下用时一样（13.5 秒上下）：没有哪一条原来靠 300 毫秒收场、现在要白等 60 秒。
- 门禁；三台 CI。

### 没管的

- 知识库那个挂接点还照出厂的 300 毫秒、用它自己的绕法（先补好索引）：它的时限不在记忆这边，没红过第二回，这一步不动。
- 用真核心的测试（`miyu-endpoint`、`miyu-core`）照出厂的 300 毫秒：断言第一轮带了常驻那一块的那些理论上有同样的毛病，没红过；红了照同样的办法给 `Core` 的测试场地换时限。
