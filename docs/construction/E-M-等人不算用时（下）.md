## 施工单 E-M 下：等人不算用时，她查得到干了多久

状态：合了（2026-10-23）。上半（她知道自己是什么模型）见 `E-M-她知道自己是什么模型（上）.md`。项目主人 2026-10-22 要 `session_usage` 多给已用时间和缓存命中率（花费照旧只给算得准的）；2026-10-23 定等人批准、等人回答的时间不算用时，「统一」：视图的一步、一段、一轮（clicall 的 9-8 十四补）和给她看的已用时间一个口径。图纸 `docs/blueprint/tools/session_usage.md`。

### 定的（技术题，主会话定）

1. **一份算法放内核**：`miyu_kernel::waits`，纯逻辑、只认日志里的事件不看钟。起：`tool.approval_requested`、`question.asked`；止：`tool.approval_decided`、`question.answered`、`tool.result`；几件一起等的第一件起、最后一件了结才止，算一段；回合开始、结束清掉。视图（`miyu-view` 的 `projector/status.rs`）以后换成调它、删掉自己那份（clicall 接着做）。
2. **她干活的时间**：每一轮从开始到结束，刨掉等人的那几段，加起来；轮和轮中间闲着的不算；撤销、压缩不减（干过就算）；在跑的这一轮算到派出去那一刻。会话每追加一条、载入时过账本每一条都喂（`Busy`，同 `rounds` 那几样），载入以后照日志算出一样的数。
3. **给她的字两句**：用量后面 `cache.txt`「Cache hit rate: N%.」（有输入才写，命中的除以输入取整）；最后 `time.txt`「Working time so far: …, not counting waits for the user.」（时分秒，零的那一截不写）。后半句写明，免得她把挂着等人的几分钟当成自己慢。都进登记簿。
4. **端口**：`UsagePort::busy_ms` 默认没有（测试里的假端口），执行器派 `session_usage` 时照 `Session::busy_ms(此刻)` 抄一份。

### 做了什么

- 内核：`waits.rs`（`Waits`、`Busy`），会话多 `busy`，提交、载入两处喂；`Session::busy_ms`；替身 `Stage::busy_ms`。
- 工具：`UsagePort::busy_ms`；会话层 `usage::asked` 多派出去的时刻、`Asked.busy_ms`。
- 基础系统：`session_usage` 多两句，`hms` 写时分秒。
- 测试：`waits` 单元 5 条（一件、几件一起、还在等的算到哪、回合边界清掉、结果到了不再等、跨轮累加）；场景 1 条（等人批准的 10 分钟、轮间闲着的 30 分钟都不算，重启载入以后一样）；时分秒一条；`session_usage` 两处输出期望。变异 5 个全抓住（等人那一段不止、回合结束不累加、提交不喂、载入不喂、不写命中率）。
