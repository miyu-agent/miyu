## 施工单 9-8 七补：human.get 带上工具的显示名

状态：已合（2026-10-14，9f7e87ef，CI run 38469035843）。终端的会话问的（9-8 六补以后）：终端不吃视图流，照原始事件画时间线，工具的显示名只照 `human.get` 的 `tools` 查，六补只改了视图条目的 `title.name`，终端看不到。图纸 `docs/blueprint/protocol.md`「`human.get`」第 4 条。

### 做了什么

1. `human.get` 的 `tools` 并进工具目录里这一刻写了显示名的工具（脚本的「显示名称」、MCP 服务给的 `title`、扩展登记时写的 `display_name`）：照要的语言挑（这种语言没写的照英文、中文、日文的先后），`{"name": …}`，没有 `icon`、`subject`；核心和出厂包的字里已经有的照那一份。
2. 这是「这一刻」的：调用那一刻的显示名在 `tool.result` 的 `display_name`（六补），终端画跑完了的那一步照记下的，这一份给还在跑的、内核替它写的结果兜底。工具目录变了，头收到 `package.changed`、`mcp.changed` 重取。

### 验收

- `crates/miyu-endpoint/tests/view_tool_names.rs`：真 MCP 服务给了 `title` 的在 `human.get` 里、没给的不在；真扩展登记的 `display_name` 照要的语言挑（中文「打招呼」、英文「Greet」）。
- 手写变异 2 个都被逮住：不并进、不照语言挑。

### 留下的

- 家目录 `scripts/` 里放、删脚本不推事件：只影响「还在跑」那一下的名字，跑完照记下的。
