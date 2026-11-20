## 施工单 X-48：能力和 MCP 的格对文档

状态：做完，本机门禁过（2026-10-23），交主会话审。照 X-47 的路子对扩展、MCP 两份作者文档（主会话同一天点头）。

### 目的

清单格式（Y-5 末上）、脚本的环境变量（X-47）已经由测试拿作者文档对代码。还剩两处只靠手写：扩展能要的 17 项能力（代码里的 `Capability::ALL`，作者文档 `manifest` 的能力表），一个 MCP 服务的十格（`fields::MCP`，作者文档 `mcp` 的表）。对了一遍现在都写全了，可加一项能力、一格的时候没有东西提醒写文档。

### 改了什么

- `miyu-config` 的 `package/fields/tests.rs` 加两条：`every_capability_is_in_the_authors_docs`（每项能力在中英 `manifest` 里写着）、`every_mcp_field_is_in_the_mcp_docs`（每一格在中英 `mcp` 的表里有一行）。文档本来就写全了，不用改。

### 验收

1. 文档已经写全，测试一上来就绿；变异 2 个报红（英文 `manifest` 的能力表少一行、中文 `mcp` 的表少一行）。
2. `cargo xtask check` 九项；Windows 版 clippy；只加测试，本机门禁过了就交主会话合。
