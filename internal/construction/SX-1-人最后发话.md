## 施工单 SX-1：会话列表记下人最后一次发话的时刻

状态：做完，待合（2026-11-16，分支 `step/SX-1`）。

### 目的

项目主人 2026-11-16 定（经界面会话转）：会话列表（终端右栏「最近会话」、网页左栏）只切进去不挪，人在那个会话里亲手发了一句话才挪到顶上；循环、定时任务、人生自己醒、回报叫醒、续跑、子代理回报开的那几轮都不算。原来列表项只有 `last_active`（最后一条事件），分不出是不是人发的。这一步给列表项多一格 `last_spoken`，头照它排。

### 蓝图改哪几节

- `store/index.md`「一行记什么」：多一列 `last_spoken`（毫秒，空的是还没人说过话），照最后一条 `by` 是人（`By::is_person`）的 `message.user` 派生；结构版本 15 → 16，老的照版本不对删掉重建、照日志补。「怎么走」第 3 条：补好的那一行换整行时每一格照列的先后写（顺手修：原来的语句手写到 `context_model`，`dream_result`、`life` 补上了没写回去）。「守着它的」加两行。
- `protocol.md`「`session.list`」：多 `last_spoken`，写法同 `last_active`，没有的不写；第 4 条写怎么算。「会话列表的推送」第 3、7 条：人发了话也推，回合中途插的一句（没有 `turn.started`）也推。
- `session/actor.md`「推送和订阅」第 7 条：`by` 是人的 `message.user` 记成列表那一项变了。
- 协议参考 `sessions.md`、`pushes.md` 中英两份。

### 不做什么

- 头的排序：终端、网页各自的会话照它改。
- 她的 `sessions` 工具：给模型的字不加这一格（非必要不加），它照旧排 `last_active`。
- `venue.sessions`：只交编号和场所，不带这一格。

### 验收

1. `crates/miyu-store/src/index/tests/spoken.rs`：只认人说的；模型回复、定时任务、三种循环、内核、别的会话发来的都不动它，`last_active` 照样走；群里的人说的也算；写进表里读得回来，照日志补的那一行写回去带着它。`life.rs` 的 `a_caught_up_row_writes_its_life_back`：补的那一行写回去，后加的几格都在。
2. `crates/miyu-endpoint/tests/last_spoken.rs`：人说一句有 `last_spoken`，之后循环两次、定时任务一次只动 `last_active`；没人说过话的不带；删了索引的行照日志补回来一样；回合中途插的一句推 `sessions.changed`、带新的那一格；场所会话里旁听的一句写进那一行。
3. 先红：去掉 `Row::see` 那一段，上面三条端点测试、`only_what_a_person_says_moves_it` 红；换整行的语句退回只写 20 格，`a_caught_up_row_writes_its_life_back` 红；去掉推送那一行，`a_word_said_mid_turn_is_pushed` 等不到推送、红。
4. 真模型实测（沙箱数据根，`relay/deepseek-v4-1-flash-261017`）：开两个会话，一个人说一句，一个 `/loop 1m` 让她自己醒几次；`session.list` 里人说过话的那个 `last_spoken` 是那一句的时刻，之后不动；开循环的那个没有这一格，`last_active` 跟着每次醒往后走。
5. 本机门禁全过；`random::` 长跑、Windows 版 clippy、`cargo xtask prompts`、`cargo xtask logs` 过。

### 风险

- 索引升了版本，升级后第一次起来删掉重建、照日志补一遍：会话多的第一次列表慢一点（量出来的：250 个会话、13 MB 日志 59 毫秒，release）。
- 场所里旁听的话也算人说的，群里一直有人说话的场所会话 `last_spoken` 一直往前走：场所会话不进本机的列表，不碍事。
