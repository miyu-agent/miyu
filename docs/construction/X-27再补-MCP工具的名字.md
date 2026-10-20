## 施工单 X-27 再补：MCP 工具给人看的名字

状态：已合（2026-10-20，4353bc22）。2026-10-20，网页的会话实测 X-27 时报的：`preset.get` 里 MCP 服务的工具 `label` 是空的（照工具名），网页画成 `mcp__fake__echo`。

### 做了什么

`preset.get` 每件工具的 `label` 照这个先后：给人看的字里有的；工具自己登记的显示名（MCP 服务给的 `title`、扩展、脚本写的），照连接的语言挑；都没有的，MCP 工具去掉 `mcp__<服务名>__` 只留工具自己的名字，别的照工具名。

### 验收

- `crates/miyu-endpoint/tests/mcp_status.rs` 的 `a_preset_turns_a_service_off_whole_or_one_tool_at_a_time`：给了 `title` 的 `echo` 是 `Echo back`，没给的 `env` 是 `env`。
- `cargo xtask check` 八项。
