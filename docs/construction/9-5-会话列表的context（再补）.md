## 施工单 9-5 再补：会话列表的上下文用量

状态：在做（2026-10-19 主会话）。网页转来项目主人要的：全部会话页每一行写「上下文消耗 xx/xxx · 总计消耗 xk」；总计照 `tokens`，上下文以前只有读进来的会话能照会话状态算。

### 这一步做什么

1. 会话列表的索引多两列（版本 8 → 9，对不上的删掉重建）：`context` 是最近一次带用量的主请求（不是压缩、起标题这些）的输入（没命中、缓存读、缓存写），压缩以后清空；`context_model` 是那一次的 `<供应商>/<模型>`。
2. `Models` 多一个查窗口的口子（`window`）：照这时的配置、模型资料查，会话没载入也查得到；测试的端口没有。
3. `session.list` 每一项、`sessions` 流的每一项多 `context: {used, window?}`：同一个模型只查一次窗口，查不到的不写 `window`。

### 验收

- 索引 `the_context_follows_the_last_main_request`；`route_facts.rs` 的 `a_window_is_looked_up_without_a_session`；端点 `list/context/tests.rs`、`list_tokens.rs` 的 `the_context_used_by_the_last_request_is_listed`；`cargo xtask check` 八项；合了告诉网页、终端。
