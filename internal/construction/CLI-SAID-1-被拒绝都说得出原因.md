## 施工单 CLI-SAID-1：被拒绝时都说得出原因

状态：已完成（2026-11-17）。

### 为什么

LF-13 实测时看到：`miyu schedule rm t1`（删人生那一行）、`miyu schedule pause t1`、`miyu schedule rm t99`（没有的编号）被拒，命令行都只印「已拒绝」，不说为什么。核心主会话 2026-11-17 定：排一步，所有原因码都说整句。

根因：拒绝时给人看的话是核心照原因码查表给的（`refusal/message.rs`），命令行、头原样印。表里没有的原因码落到兜底的「已拒绝。」。查下来有 15 个原因码没有话：

- 定时任务的三个：`unknown_schedule`、`life_schedule`（LF-13 新加的）、`too_many_schedules`。
- 语义模型的两个：`embedding_off`、`embedding_failed`。
- 软件包的三个：`not_exported`、`unknown_widget`、`bad_widget_content`。
- 内核会拒的七个：`unknown_level`、`unknown_decision`、`nothing_to_title`、`too_many_messages`、`duplicate_message`、`inbox_full`、`unknown_watch`。

以前也漏过（知识库的五个，施工 K-7 下补的），每次都是新加原因码时忘了写话。

### 改成什么样

1. 这 15 个原因码各补一句中文、一句英文。
2. 一条测试守着：协议参考 `docs/reference/errors.md` 里列着的每一个原因码、内核 `Reason::code` 的每一个，中英文都不是兜底的那一句。以后新加原因码、照规矩写进协议参考了却没写话的，这条测试红。

蓝图不用改：`protocol.md` 本来就写着「每个原因码一句给人看的话」，这一步是补齐。

### 不做

- 命令行自己不另写一套话：话只在核心一处。
- 带着 `data` 里的数说的（哪一个编号、哪一块）：这一步只保证每个都有整句，不逐个加细节。

### 验收

- 改前红：新测试在没补话的核心上红，报出这 15 个。
- 补完绿；门禁。
- 实测：沙箱里 `miyu schedule rm`、`pause` 对人生那一行、对没有的编号，印出整句。

### 做下来的

- 改前红：新测试在没补话的核心上报出 15 个原因码（上面那 15 个）。
- 实测（沙箱数据根，核心和命令行都是这一步编出来的，不用模型）：定时表里有一行人生循环 `t1`。
  - `miyu schedule rm t1`、`miyu schedule pause t1` → `miyu: 这是人生会话的循环，不能当定时任务改：暂停、继续、结束请用 /life`，那一行没动。
  - `miyu schedule rm t99` → `miyu: 没有这个定时任务`。
  - 英文环境 `miyu schedule rm t1` → `miyu: this is a life session's loop and can't be changed as a schedule. Use /life to pause, resume or end it`。
  - 改前这三条都只印 `miyu: 已拒绝`。
- `shutting_down` 不在这一步里：它是软件包端口回给工具的原因，不是协议的拒绝，不经这张表。
