## 施工单 A-F 补：头认得出 fork 子代理

状态：待核心主会话审。经过：项目主人定网页只把 fork 子代理的机器人图标换成分叉图标（时间线那一步、右栏后台任务、任务树）。网页的会话 2026-10-24 说视图流认不出哪个是 fork 的，要两处只加不改的格；核心主会话同一天同意，排在 UC-1 后面。图纸 `agents.md`「一补、fork 子代理」第 10 条。

### 做了什么

- 内核：效果 `job.started` 多 `fork`（布尔，假的不写，以前的日志照读成不是）。`subagent` 写了 `fork` 的报 `true`。会话状态的任务表（订阅回应、视图流的状态）用的就是它，跟着带上。
- 视图：任务行 `JobRow` 多 `fork`，照 `job.started` 抄。
- 会话索引多一列 `forked`（`session.created` 的 `forked` 写了 `within` 的），版本 11 → 12：老的照版本不对删掉、照日志重建。`session.list`、`sessions` 流那一行多 `forked: true`。
- 普通子代理、后台命令、主会话、支线（C-8 的分叉，没有 `within`）两格都不写，字节不变。
- 时间线那一步网页照 `subagent` 调用参数里的 `fork` 认，核心不另加。
- 替身多 `Play::forks_agent`：同 `starts_agent`，派的是 fork 子代理。
- 文档：
  - `kernel/events-bodies.md`（`job.started` 的 `fork`）、`view.md`（任务行）、`protocol.md`（`session.list`）、`store/index.md`（一列、版本）、`agents.md`。
  - 参考文档 `view`、`sessions` 中英两份。

### 验收

1. 内核 `event/effect/tests.rs` 的 `a_started_job_round_trips`：`fork: true` 往返；不写的读成不是。
2. 视图 `tests/status.rs` 的 `a_forked_subagent_row_says_so`：fork 的那一行 `fork` 是真，普通的不写。
3. 会话索引 `index/tests/forked.rs` 的 `only_a_fork_within_a_turn_is_marked_forked`：fork 子代理是，支线（没有 `within`）、普通会话不是；存进表、重开读回来照样。
4. 端点：
   - `tests/fork_subagent.rs`：真核心一步两个 fork 子代理，两个子会话在 `session.list` 里有 `forked: true`，主会话没有；主会话日志里两条 `job.started` 都写 `fork`。
   - `tests/reports.rs`：普通子代理那一行没有 `forked`，任务那一项没有 `fork`。
5. 变异 6 个都报红。头一遍活下来两个，都在索引：支线也算成 fork（测试里没有支线）、存进表写成 0（端点读的不是表里的）；补了第 3 条，原样先过，再跑都红。
6. 本机门禁、Windows 版 clippy；推 `step/` 过三台 CI。合了告诉网页。
