## 施工单 CX-3：累计用量按上下文的块分开

状态：合了（2026-10-23）。项目主人看了网页「本会话累计」浮窗，说太简单，要「本会话累计应当包含上下文 token 明细中所有项目」；网页提口径，主会话定 CX-3、口径和下面「定的」。

### 目的

头查得到一个会话（可连子代理）累计用掉的输入照上下文的块分开：system（拆到人设、核心说明、技能……）、工具（内置、每个包、每个 MCP 服务……）、示范对话、检查点摘要、对话（人说的、她说的、工具结果、回报、记忆……），外加一块 AI 输出。按用途、请求数、命中、花费照旧用 `usage.query`。

### 定的（2026-10-23，主会话）

1. 只在被问时从日志走一遍算，不碰热路径。
2. 每次主请求的输入照那一刻各块的估值比例摊开再累加；输出单成一块。
3. 压缩、子代理、回顾照用途交给 `usage.query`，不进块；`tree` 把子代理的会话各走一遍相加。
4. v1 命中、没命中不分开摊，只摊输入总数；施工单写明偏差。
5. 三处总数核得上（`session.context`、`usage.query`、新方法），测试里核一遍。
6. 形状我定：新方法 `session.usage_parts {session, tree?}`，不给 `session.context` 加一格——两件事的口径（这一刻 / 累计）、参数（新会话预览 / `tree`）都不一样，并在一起头要分辨回应是哪一种。

### 蓝图改哪几节

- `protocol.md`：方法表、`session.usage_parts` 一节。
- `docs/reference/sessions.md`、`sessions.en.md`。

### 做了什么

- 内核：`Frame`（照一份策略组装一段空的历史，估 system、每件工具、示范对话）和 `breakdown_in(frame, history, price)`（照一段有效历史出明细）；会话自己的 `breakdown()` 改走同一套，口径一处。
- 会话：`Handle::usage_parts` 交 `Spent`（主请求几次、输出、每次报了用量的主请求那一刻的上下文和输入）。actor 在阻塞线程里读自己的日志回放：主请求在它看到的那一条记下那一刻；快照照造会话的那份，换了策略的到下一个回合开始才换；一份快照只备一次，system 照 CX-1 下的拆法。`handle.rs` 满 500 行，两样走同一条 `Context` 消息（`Asked::Now` / `Asked::Spent`）。
- 用量库：`UsageIndex::tree`，和 `usage.query` 的 `tree` 同一张父子表。
- 端点：`session.usage_parts`；块树从 `shape` 里分出来（`tree`），每次请求照它的输入摊（同 CX-1 下的摊法），照编号并起来，最后接 `output`；工具归哪块问过的记下（几百次请求不重问）。三种语言的「AI 输出」。

### 偏差（写清楚）

- 命中、没命中不分开：命中多半是前缀（system、工具、老对话），照比例摊会把一部分命中算到新内容头上——最近的对话、工具结果偏大，system、工具、老对话偏小。总数不变。
- 估算器的偏差照 `session.context` 带着：中文、代码、代理接回请求而日志里看不到的思考（CX-2 查过）都按比例平均摊到各块。

### 不做什么

- 不分开摊命中、没命中；不按模型调估算器。
- 不记进用量库、不在发请求时算。

### 验收

- `miyu-kernel` 的 `session/tests/limits.rs` 的 `a_frame_and_a_history_give_the_same_parts_as_the_session`：照 `Frame` 和有效历史算的和会话自己的明细一样。
- `miyu-session` 的 `actor/persona/spent/tests.rs` 的 `main_requests_are_charged_with_the_context_they_saw`：辅助、摘要请求不算，没报用量的算次数不摊；每次照它看到的那一刻（第一次没有回复、第二次连第二句）；换的策略下一个回合才生效；一份快照只备一次。
- `miyu-endpoint` 的 `tests/session_context.rs` 的 `spent_parts_add_up_with_usage_and_context`：真核心说一轮，`requests`、`input`、`output` 和 `usage.query` 的 `main` 一样，块加起来是输入加输出，`input + output` 就是 `session.context` 的锚；分叉出来的子会话照 `tree` 加进来，和 `usage.query` 的 `tree` 一样（底子不重算），编号不重复；不写 `tree` 的只算它自己。
- 手写变异 9 个都红（两个先活下来，补测试才红：测试日志里回复排在 `model.called` 后面，不像真日志，改成回复在前；查编号重复只去了相邻的，改成排序后去重）；`cargo xtask check`、Windows 版 clippy；推 step 分支过三台。

### 风险

- 很长的会话每次问都整份读一遍日志回放：只在头点开时算，在阻塞线程里，不挡会话。要是慢，以后照用量库的补法记下回放到哪。
