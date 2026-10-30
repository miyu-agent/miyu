## SM 会话各记模型和思考强度

状态：施工中（2026-10-30 开工）。照设计 51。样子项目主人 10-30 定了（在会话 A 换了模型或 `/effort`，切到会话 B，B 也跟着变了，要的是各管各的；`/effort` 管当前会话、设置页管默认，加一项「跟着默认」），拆法、协议是技术细节：CI 全绿、测试全过就合。

### 分步

| 步 | 做什么 |
|---|---|
| SM-1 | 核心：没写模型的会话照造它那一刻的 `models.chat` 原样记（池记池），以后改默认它照旧，以前记成没有的照旧跟着默认；`session.configure` 多 `effort`（字或 `null`），照会话这时的模型的档位查，记进 `session.policy_changed` 的 `effort`，下一个回合起压着配置，这个模型没有那一档的照配置；`subscribe`、`model.changed` 的 `effort.from` 多 `session`。协议、蓝图、参考文档跟着改 |

### 照的图纸

设计 51 第一、二节；蓝图 `models.md`「事件」「协议」「怎么走」第六条、第十一条，`protocol.md` 的 `session.create`、`session.configure`、`subscribe`，`kernel/events-bodies.md` 的 `session.policy_changed`，`kernel/session.md`「换思考强度」，`session/actor.md` 第 8 条；参考 `reference/sessions*.md`、`reference/events*.md`、`reference/connection.en.md`。历史：施工 8-18（会话给每个模型记一格）、8-18（补）（去掉会话那一层）。

### 不做什么

- 头的改动（设计 51 第三节）：`/effort` 改调 `session.configure`、选择框多「跟着默认」、设置页说明写成「新会话的默认」、底栏给 `from: session` 标记号，终端、网页各自做。
- 子代理、一次性会话（`miyu ask`）记模型的规矩不变；以前记成没有的会话不回头改日志。
- 8-18 那时日志里带 `model` 的那一格不当成会话的一档（8-18（补）以后一直没人理，现在认会让旧会话突然换强度）。
- 不恢复 8-18 的 `unknown_effort` 原因码：不在档位里的回 `bad_params`，`data.field` 是 `effort`。
- 会话刚换的一档不当场推 `model.changed`：同换模型，下一个回合开始才推。

### 验收

- 模型：开会话 → 改 `models.chat` → 这个会话下一轮还是原来的模型；以前记成没有的会话照旧跟着这时的 `models.chat`（`crates/miyu-session/tests/route_pools.rs`）。
- 思考强度（`crates/miyu-session/tests/route_effort_session.rs`、`crates/miyu-endpoint/tests/models_effort.rs`、`crates/miyu-kernel/src/session/tests/effort.rs`）：A、B 两个会话，A 设了 B 不变；A 设了以后改配置，A 照旧、B 跟着；`null` 回到默认；不在档位里的被拒、什么都不记、一起写的模型也不换；换到没有那一档的模型退回配置、`from` 照实写，换回来又用上；重启以后从日志读回来。
- 变异：不记 `models.chat`、会话那一档不压配置、`null` 不回默认、`from` 不写 `session`，各退回去一处都报红。
- 拿不准、等主会话审的：事件照 8-18 的对象形状去掉 `model`；池的档位取成员的合起来；`session.configure` 查档位照 `subscribe` 的 `model.ref`（跟着默认的会话、默认改了还没开回合的，照的是上一轮的那个）；限额里给模型看的那一块事实（`Identity.effort`）跟着会话的一档改。
