## 施工单 X-33：扩展工具的回答带 `data`

状态：合了（2026-10-21，a5dd9c24）。扩展线的会话做，主会话审过、合进 main。

### 目的

扩展 `provide` 时写了 `"clicall": true` 的工具，脚本里经 `miyu call` 调到，现在只拿得到文字：`tool.call` 的回答没有 `data` 这一格。出厂的四件、MCP 的 `structuredContent` 都交结构化结果（施工 M-2），扩展的也该一样（`39-脚本调工具.md` 第六节第 3 条、MC7）。

### 蓝图改哪几节

- `providers.md`（扩展提供的工具）「回答」：多一格 `data`，任意 JSON，可以不写；照收、交进 `Done.data`：只给脚本，不进上下文、不进 `tool.result`，出错的（`error: true`、JSON-RPC 的错）不交，同 MCP。不另截断（MC7），整条回答照旧受管道一行的上限。
- 协议参考 `extensions`（中英）`tool.call` 的回应表、作者文档 `extensions`（中英）「被调」：加 `data` 一条；`clicall` 那一格补一句「有结构化结果的写进 `data`，脚本拿到一行 JSON」。

### 不做什么

- 不拿 `detail` 顶 `data`：`detail` 是给头画卡片的形状，扩展要给脚本什么自己写（出厂四件照 `detail` 填是它们自己的事）。
- `--schema` 的 `Result:` 一行照旧（M-3 定了先不印）。

### 验收

1. 先写会红的测试：`miyu-test-extension` 回带 `data` 的结果，嵌套调用交给脚本的是这份 JSON；`error: true` 的不交；直接调用的 `tool.result` 里没有它。
2. `cargo xtask check` 全过。

### 风险

- 和 M-3 撞 `miyu-tool`、嵌套调用那几处：等 M-3 合进 main 再开工。

### 怎么做的

- `provide/remote.rs` 的回答多读一格 `data`（`#[serde(default)]`），`error` 是假的照收进 `Done.data`；嵌套调用、`tool.result` 那两头 M-2、M-3 已经接好，不动。
- 测试扩展 `miyu-test-extension` 的 `serve` 多两件：`data` 回带 `data` 的结果，`data_error` 回 `error: true` 也带 `data`。
- 契约测试 `tests/provide_data.rs`：假的 `shell` 经嵌套端口调这两件，再直接调一次 `data`。先红（脚本拿不到 `data`）后绿。
- 契约测试看不出源头漏没漏出错时的 `data`（嵌套调用那头也拦），`provide/tests/remote_tests.rs` 另补一个：照收、原样交，出错的不交。
- 手写变异 8 个全抓住：这一步只有两行代码，围着 `data` 走的这条路（读、出错不交、不进 `tool.result`、嵌套调用交给脚本）能改坏的就这么多，不硬凑 15 个。原来还有一个「去掉 `#[serde(default)]`」，`Option` 的格子缺了 serde 本来就当空的，等价，拿掉了。
