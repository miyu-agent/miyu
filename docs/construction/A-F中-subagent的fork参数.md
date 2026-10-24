## 施工单 A-F 中：`subagent` 的 `fork` 参数

状态：待核心主会话审。经过：A-F 上合了（内核能从一轮当中分叉）；这一步把它接到她手上。核心主会话 2026-10-24 定的三条照做：派它那一刻那条回复一定先落盘、重筛不许换 fork 子会话的快照、记忆照「是 fork 子代理」在运行时关。图纸 `agents.md`「一补、fork 子代理」、`tools/subagent.md`。

### 做了什么

- `subagent` 多一个参数 `fork`（布尔）。说明第二句前面加 `Without fork`：不 fork 的才看不到这边的对话。
  - 写了 `pool`、`persona` 的报参数不对：模型、人格只能照父会话的。
  - fork 出来的再写 `fork` 的，回 `agent/no-fork.txt`：它的工具面照父会话的，`subagent` 还在，只能在调用时拒。
- 端口：
  - `Order` 多 `fork`。
  - `AgentPort::forked()`（默认否）。
  - `SessionPort::fork(ForkChild)`，默认造不了。`ForkChild` 带父会话和层数、命令编号、属主、派它的那次调用、占位那一句。
- 执行器（`agents/spawner.rs`）：
  - fork 的读外壳（`core/jobs/fork-directive.txt`），去掉末尾空白、空一行接她写的交代。
  - 读占位那一句（`core/tool-results/forked.txt`），经 `SessionPort::fork` 分叉，其余照子代理：交代作为父会话发来的话送进去、派到一半被掐掉的停掉。
  - 派的时候照深度挡：fork 子代理到了上限也有 `subagent`。
  - `Agents::for_call` 带上这次调用的编号。
- 会话层的分叉（`open/fork.rs`）多 `within`：照派它的那次调用分叉（`fork_plan_at`），子会话的 `depth` 照交的，走 `Session::fork_within`。普通子代理也能派 fork，fork 子代理、支线不能。
- 端点（`sessions/forked.rs`）：拿着会话表的锁，父会话不在表里的不再造；写好头几条、放了锁再载入，会话列表报一声。
- 载入认得 fork 子代理（`memory::fork_child`）：
  - 照子代理回报、不问人，`memory::subagent` 也认它，所以不召回、不记。
  - `Refresh::frozen`：回合开始不重筛，快照一直是父会话那一份。
- 两句新字进登记簿：外壳 41 token，`no-fork` 12 token；`subagent` 的说明多 32 token。给人看的三种语言：分叉出来的不能再分叉、A fork cannot fork、フォークからはフォークできません。参数的错照 `serde` 的写法拼，不另登记（同 `pool` 写错的那一句）。
- 读两句的函数放进 `miyu-store` 的 `resources/fork.rs`：`resources.rs` 已经 497 行。

不在这一步：端点、真模型量两轮请求的缓存（A-F 下），通知两个头。

### 验收

1. `crates/miyu-session/tests/fork_subagent.rs` 五条：
   - 父会话真调 `subagent` 写 `fork`，假的会话表真去分叉：
     - 会话表收到的是派它的那次调用、第 1 层、`<父会话>/j1`、占位那一句。
     - 交代前面接上外壳，作为父会话发来的话送进子会话。
     - 子会话载入、收到交代说一轮：快照一直是父会话那一份；第一次请求的 system、工具面和父会话的一样，人那一句、派它的那条回复一样，后面有占位、外壳。
     - 照常向上回报。
   - fork 写了人格的报参数不对，不分叉。
   - fork 出来的再 fork 回 `no-fork`，不再分叉。
   - 工具面上照父会话有 `remember`，fork 子代理调了记不下。
   - 第 1 层的子代理 fork 出来的是第 2 层：它派普通子代理派不了，会话表一个都没造；会话层的分叉不许从它再分（`NotMain`）。
2. `crates/miyu-basesystem/tests/subagent.rs` 的 `it_declares_a_title_the_task_and_a_pool`：参数多 `fork`（布尔、不必填），说明里那一句改成 `Without fork it sees nothing…`。别的子代理、C-8 分叉、记忆支线的测试照旧过。记忆、深度上限那两条拆在 `fork_subagent/limits.rs`（守 500 行）。
3. 变异 11 个，第一遍都报红：写了人格的拒、再 fork 的拒、深度挡、走会话表的 fork、外壳后面的空行、fork 子代理向上回报、快照冻住（两处）、记忆照子代理、会话层不许从 fork 子代理再分、子会话的层数。
4. 本机门禁、Windows 版 clippy；推 `step/` 过三台 CI。
