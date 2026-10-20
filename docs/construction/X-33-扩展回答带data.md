## 施工单 X-33：扩展工具的回答带 `data`

状态：主会话审过（2026-10-20）；碰 clicall（设计 39）和嵌套调用，等 M-3 合进 main 再开工。

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
