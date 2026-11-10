## 施工单 YC-2：一时的错不当会话坏了

状态：施工中（2026-11-10）。压测报告 F1 的连带（YC-1 把软上限调高了，这一步管撞上以后说得对、不留后患）。

### 为什么

压测里打开文件数用光的那半小时，核心把一时的错当成了坏了：

- 载入会话时除了「日志不存在」，任何错都回 `session_broken`「会话已损坏、载入不了」（`sessions/found.rs`）。2257 次载入失败（`session log not opened` 1596、`policy snapshot not fetched` 654）全说成损坏，磁盘上其实一个都没坏（801 个会话、44 万条事件逐条核过）。人、im 看到「损坏」会以为对话没了。
- 知识库读不了的当一个库都没有（`Shelves::listed`）：工具面上两件知识库工具跟着没了，回合开始照文件换快照时算「改了」，tools 数组一变缓存全断；下一轮读得了又换回来，再断一次。造会话那一刻读不了的，这个会话一开始就没有知识库工具。

报告里另外几条看过代码，不用改：

- 写日志失败停会话（`actor/store.rs`）：停了再载入是对的恢复办法。写了半截的最后一行载入时自动截掉（报告里实测过），不停的话要自己截回写之前的长度，风险比收益大。回的 `session_stopped` 本来就写明「再发一次会重新载入」。
- 人格、技能读不了（`persona unreadable`、`skills not looked at`）：照旧用上一份，不缺东西。
- 预设读不了：`preset.get` 这类回内部出错，不进请求。
- `session.create` 成了、`session.send` 失败留下的会话：和刚开、还没说话的会话一样，发的人收到了错误，不算丢话。

### 做什么

1. 新拒绝原因 `session_unavailable`「会话暂时无法读取，请稍后重试」（`refusal/unread.rs`）：载入会话（`sessions/found.rs`）照 `Refusal::of_load` 分：日志、策略快照读的时候读写出错的（`OpenError::Io`、`BlobError::Io`）回它；日志真读不懂的（坏行、没有造会话那一条、快照没了、对不上、读不懂、造不出策略、内核过不了账本）照旧 `session_broken`；随核心附带的字读不了的是安装坏了，回内部出错。直接读日志的几处（`view.page`、订阅那一页、`session.prompts`、场所记录）照 `Refusal::of_log` 分。运行日志那一行照旧记原话。给人看的一句（中英）、参考文档 `errors`、`sessions`、`connection` 的表加上。
   - 订阅时补发读不了的照旧 `session_broken`：那边的原话是一句字，分不出两种；载入着的会话日志也可能被弄坏（`replay.rs` 那条测试）。
   - 导出的几处不动：用得少，对不上的 blob 本来就是坏了。
2. 知识库列表读不了不当没有：`Shelves::read_listed` 交回读没读成。回合开始换快照那一处（`actor/persona.rs`）读不了的照快照里的不动（`Seen::Kept`，记一行）。造会话那一刻照旧当没有：第一轮开始换快照时还没发过请求，读得了的在第一个请求之前就换上，不断缓存。
3. 改协议：合了告诉 UI 和 im，`session_unavailable` 照一时的错显示、可以再试。

### 验收

- 单元：载入错误照原因分两种（各种 `LoadError` 一一对上）。
- 集成（`tests/session_unavailable.rs`，只在 Unix 上）：会话目录 `chmod 000` 时 `view.page` 回 `session_unavailable`，权限还回来照常读；日志第一行弄坏的照旧 `session_broken`。先红：分类照旧一律 `BROKEN` 时报红。
- 知识库（`knowledge_tools.rs`，只在 Unix 上）：第一轮以后库的目录 `chmod 000`，第二轮请求的工具面、system 和第一轮逐字节一样，没换快照。先红：照旧当空列表时报红。
- 本机门禁全过。
