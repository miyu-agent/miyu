## 施工单 SIDE-2：看图模型写了空的固定池时，同样按没有写处理

状态：做完了，等合并（分支 `step/SIDE-2`，从 main 开）。施工 SIDE-1 的收尾，设计 43 第三节第 3 条。

### 为什么

施工 SIDE-1 定了「明写的固定池是空的时按没有写模型处理」，当时没有包括会话的看图模型：`session.create`、`session.configure` 的 `vision` 写了空的固定池时仍然返回 `unknown_model`。主会话 2026-11-20 定：看图模型用同一个规矩，保持一致。

### 改成什么样

- `session.configure` 的 `vision` 写了空的固定池：和写 `null` 相同，去掉固定的看图模型，回到跟随 `models.vision`。
- `session.create` 的 `vision` 写了空的固定池：按没有写处理，会话创建成功，不记 `session.policy_changed`。
- 运行日志同样记一行 `INFO empty pool fallback pool=<池名>`。
- 人自己建的池是空的、池不存在、引用写错，仍然是 `unknown_model`。

代码上，`crates/miyu-endpoint/src/models.rs` 的 `record` 并进 `record_or_none`（`model` 和 `vision` 都用它，原来的 `record` 已经没有其他调用）；`models/vision.rs` 多一个 `created`，给 `session.create` 用。

蓝图 `models.md`、设计 43、协议参考 `sessions.md` 中英文一起修改。

### 验收

- `crates/miyu-endpoint/tests/vision_pin.rs` 的 `an_empty_fixed_pool_as_the_vision_model_counts_as_not_written` 在修改之前是红的。
- 门禁、Windows 版 clippy、三个平台的 CI 全部通过。

### 做下来的

- `models/vision.rs` 的 `command` 改用 `record_or_none`，多一个 `created`；`methods/session.rs` 建会话时调用 `created`。
- `models.rs` 的 `record` 并进 `record_or_none`，两处说明里的旧名字跟着修改。
- 新测试在修改之前是红的：换看图模型写 `@lite` 时得到 `unknown_model`。修改之后 `vision_pin` 的 5 条都通过。

### 怎么实测验收

2026-11-20 在沙箱里用编出来的核心实测（`MIYU_HOME` 和 `HOME` 都指向临时目录；这一步不发模型请求）。配置里 `[pools.lite] models = []`。

1. `session.create` 带 `vision = "@lite"`：会话创建成功，回应的 `events` 是 `[1]`，会话日志里没有 `session.policy_changed`。
2. `session.configure` 带 `vision = "@lite"`：回应 `{}`。
3. `session.configure` 带 `vision = "@nope"`：仍然是 `unknown_model`，「模型或模型池不存在。」
4. 运行日志里有两行 `INFO empty pool fallback pool=lite`。
