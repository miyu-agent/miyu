## 施工单 YC-6：日志中间坏一行，坏行以前的照样看得到

状态：施工中（2026-11-10）。压测报告 F6。

### 为什么

会话日志中间坏了一行（中间一行换成了别的、一行写到一半、序号重了一条、末尾多了一整行别的），载入、翻历史一律回 `session_broken`「会话已损坏，无法载入」：坏行以前好好的对话也看不到，人以为整个会话没了。最后一行写到一半的照旧在打开时截掉（它从未被确认过），不在这一步里。

项目主人 2026-11-10 定：只读打开，不给修。坏行以前的照样能看，告诉人坏在哪；不提供修的命令，要接着聊新开一个会话；日志文件一个字节都不动。

### 做什么

1. 存储（`miyu-store` 的 `log/open.rs`）：报坏了的错说得出坏在哪，`OpenError::broken_at` 交回段的文件名、行号，和照段名加行号算的这一行该是第几条。
2. `view.page`、视图流订阅时的最新一页（`miyu-endpoint` 的 `view/broken.rs`）：要读的行里碰到坏行的，拿坏行的序号当 `before` 再读一遍，又碰到更早的再往前收；回应多一格 `broken: {"segment", "line"}`。坏在第一条的、算不出更早的照旧 `session_broken`。没坏的、坏行不在这一页要读的行里的，回应一个字节都不变。
3. 载入（`refusal/unread.rs` 的 `Refusal::of_load`）：日志坏了一行、坏行以前还有的，回新的拒绝 `session_read_only`（-32010），`data.segment` 是段的文件名、`data.line` 是第几行；给人看的话带上行号：「会话日志第 N 行坏了，只能看到这一行以前的内容，不能接着聊。要继续请新开一个会话。」/「The session log is broken at line N; only the part before it can be viewed and the session can't continue. Start a new session to go on.」（`Refusal::message` 照 `data` 填数，`refusal/message.rs` 的 `filled`）。坏在第一条的、没有造会话那一条的照旧 `session_broken`。说话、订阅、改配置这些要载入会话的都走这一条。
4. 不把坏行以前的载入成封存的会话：载入会收尾崩了的那一轮、接着干被打断的那一轮，会往日志里写；封存了的会话改标题、回顾照样写。只读的载入要内核、actor、日志都另开一种不写的样子，不值。头照 `view.page` 只读地显示就够了。
5. `session.list` 不动：坏了的会话本来就照样列，只是标题、动静照坏的那一段以前的算（`protocol.md` 的 `session.list` 第 3、4 条），这一步不改。
6. 蓝图 `protocol.md`（`view.page` 第 6、8 条和回应、「会话表」第 2 条、出错表、给人看的字、守着它的）、`store.md` 第六节第 7 条、`session/actor.md` 的载入错误表；参考文档 `errors`、`sessions`、`connection` 中英两份。

### 不做什么

- 不修日志，不截坏行，不出修的命令；只截最后一段末尾写到一半的那半行（原来就这样）。
- 段名接不上（有人改了段的文件名）这一类：载入照段名算出的那一条说 `session_read_only`，`view.page` 算不出更早的上界，照旧 `session_broken`。只有改文件名才碰得到，不另做。
- 坏行以后的轮次：最新一页没碰到坏行的照样看得到（照没坏的交，原来就这样），往前翻碰上坏行的那一页起只给坏行以前的。整份自检每一页要多解一遍全部的行，不做。

### 验收

- 单元：`miyu-store` 的 `a_broken_log_says_which_event_broke`（段名、行号算出序号，段名不是序号的、行号是 0 的、不是坏了的没有）；`refusal/unread/tests.rs`（坏了一行的 `session_read_only` 带 `segment`、`line`，坏在第一行的照旧 `session_broken`）。
- 集成（`tests/session_read_only.rs`）：说完三轮、停下会话，照四种坏法弄坏中间一行或者末尾多一行，换一份核心：`view.page`（事件、条目）交坏行以前的、带 `broken`，不碰坏行的一页不带；说话、订阅拒 `session_read_only`，带段名、行号，给人看的话说第几行；`session.list` 照样列；前后比日志的字节一个不差。坏在第一行的照旧 `session_broken`。先红：退回原来的 `view.rs`、`refusal` 时报红（`view.page` 回 `session_broken`）。
- 最后一行写到一半的照旧截掉（`miyu-store` 原有的测试）。
- 改协议（新拒绝、`view.page` 多一格），合了告诉 UI 和 im：头遇到 `session_read_only` 改用 `view.page` 只读地显示，照 `broken`、`data.line` 说坏在哪，提示新开会话。
