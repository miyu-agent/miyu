## 施工单 CX-1（上）：上下文明细和用量按用途

状态：合了（2026-10-23）。项目主人 2026-10-23 经终端定「两样都加，要详细数据」，主会话排成 CX-1，形状和主会话对过（下面「定的」），拆上下两步。

### 目的

头能画「上下文明细」：一个会话这时的上下文里 system、工具说明、示范对话、检查点摘要、对话各占多少，往下拆到技能、每个包和 MCP 服务的工具、常驻和召回的记忆；累计消耗能按「这个会话（连子代理）× 用途」拆。

### 定的（2026-10-23，主会话审过）

1. 新请求 `session.context`，参数同 `session.tools`，没有 `model`；回 `window`、`line`、`used`、`basis`（`measured`/`estimated`）、`anchor`、`estimated_total`、`parts`。`part` 编号稳定，`label` 照连接的语言（`core/human/<语言>.json` 的 `context/part/<编号>`），包、服务照原名；是 0 的不交。
2. 常驻记忆不在 system 里，是每回合注进对话的事实：拆成 `messages.memory.resident`、`messages.memory.recall`，知识库想起的单列 `messages.knowledge`，别的事实 `messages.facts`。没有 `system.memory`、`system.preset`。
3. system：快照里单存了原文的几块（技能、知识库、关于你、项目说明、扩展追加的）在拼好的 system 里找得到的各成一块，其余 `system.other`。
4. 只在被问时算：会话 actor 加一条只看不动的消息。
5. 各块交本地估的原值和 `estimated_total`，头照 `used / estimated_total` 摊。
6. `usage.query` 加分组 `use`：`main`、`compaction`、`subagent`、`recap`、`title`、`memory`，认不出的 `other`，不开新方法。

### 蓝图改哪几节

- `protocol.md`：方法表、`session.context` 一节、`usage.query` 的 `group`。
- `models.md`「协议」`usage.query`：`use` 的归法和例子。

### 不做什么

- 新会话预览（不写 `session`）、system 拆全（人设、场所说明、核心几行、风格锁各成一块）：要动造会话的 `open.rs` 和 `compose`，在 CX-1 下。
- 不改给模型的字，不碰回合的热路径。

### 验收

- `miyu-kernel` 的 `session/tests/limits.rs` `the_breakdown_splits_the_context_by_who_put_it_there`：system、工具每件、对话照谁的、事实照模块和类别；答过以后锚是供应商报的，换了模型锚不算。
- `miyu-session` 的 `actor/persona/context_tests.rs`：找得到的几块各算、找不到的和空的不认、加起来等于 system。
- `miyu-endpoint`：`session_context/tests.rs`（块名、工具照来处、待加载的、记忆和知识库单列、是 0 的不交、basis）；`tests/session_context.rs`（真核心造会话、说一轮，名字照语言；参数不对、没有的会话拒）。
- `miyu-store` 的 `tests/usage.rs` `use_sorts_rows_into_main_compaction_subagent_and_helpers`。
- 手写变异 16 个都红；`cargo xtask check`、Windows 版 clippy。

### 风险

- 本地估算器（字节 /4）偏差：压缩图纸 6-3 记的中文多估约两成半、英文少估约一成半，`estimated_total` 和 `used` 能差 15% 以上。头照比例摊，摊完的总数对得上 `used`，各块之间的比例仍带着估算器的偏差（中文多的块显得偏大）。要不要改成核心摊，报主会话定。
- 已有会话的快照里单存的原文和拼好的 system 对不上（以前的快照、扩展改了还没换）：不认，算进其余的，总数不变。
