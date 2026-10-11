## 施工单 X-4：MCP 的功能和状态

状态：施工中（2026-10-11 主会话）。图纸 `mcp.md`「对外的样子」「怎么走」、`package-pages.md`「开关」、`protocol.md`。

### 这一步做什么

1. 软件包列表上一个服务一格带自己的状态（`package.list`、`package.info`、推送的 `package.changed` 都是那一项）：`mcp` 的每一格同 `extension.status` 的一项多 `state`（`off`、`starting`、`running`、`waiting`、`stopped`）、`failures`，在跑的带 `pid`，在等的带 `retry_in`，停下的带 `reason` 和标准错误（`state/logs/mcp-<服务名>.stderr`）的最后几行 `stderr`。服务的状态变了照扩展那样报一声，订阅着软件包列表的连接收到这个包的新一项（X-3 起就报，这一步那一项里多了每个服务的）。
2. 新方法 `package.restart {package}`：扩展同 `extension.restart`（要的能力还有没批的照它拒），再重启它带的 MCP 服务；`bundle` 重启它带的服务。请它们退出、等它们退出，再起，连续失败从零数；工具留在目录里，起好、列完照新的换。关着的（卸掉了的出厂包也算）`package_off`；内置包、界面、小程序、吉祥物包没有能重启的，`not_restartable`。扩展进程调不了（同 `package.enable`）。回应同 `package.list` 的一项。`extension.restart` 照旧只重启扩展的进程。
3. 预设里一个服务一个功能（X-2 起就是）：契约测试证明整个关掉（`[features] mcp-<服务名> = false`）她的工具面里一件都没有、system 里「装了没开」那一行写着它，单件关掉（`[tools] mcp__<服务名>__<工具名> = false`）只少那一件，`preset.get` 列出这个功能和它的每一件工具、各自开没开。
4. 图纸更正：预设里关掉只管这个会话的工具面，不停进程（服务一个核心一份、所有会话共用，别的会话的预设可能开着它）；进程跟着包的开关、装卸走。原来写的「预设里关……进程停掉」改掉。
5. 工具加载一组（`25-工具加载.md` W3：一个 MCP 服务一组）随存根、按组加载那一步做，这一步不做，写进 `mcp.md`「还没有的」。
6. 图纸 `mcp.md`、`package-pages.md`（开关那张表多一列重启）、`protocol.md`（方法表、`package.list` 的 `mcp`、`package.restart`、两个原因码和它们的字）。

### 定了的（主会话，技术题照推荐）

- 状态放在软件包列表那一项里，不另开方法、不另开推送：头的软件包页本来就照它画，`bundle` 没有进程、不在 `extension.status` 里。
- 一格的写法照 `extension.status` 的一项：头画扩展状态的那一套直接能用。
- 重启另起 `package.restart`，不让 `extension.restart` 管服务：软件包页的开关已经是 `package.enable`、`package.disable`，重启放在一处；`bundle` 不是扩展，`extension.*` 碰不到它。
- 关着的另起 `package_off`，不借 `extension_off`：`bundle` 不是扩展，那句字说不通。

### 验收

- 契约测试 `miyu-endpoint` 的 `tests/mcp_status.rs` 四条：起不来的服务停下，那一格带原因、失败 5 次、标准错误，推送里也是；挪开挡着的再 `package.restart`，在跑、带进程号、失败从零数；在跑的重启换了进程，工具照常调得到；关着的、内置包、没有的拒；预设整个关掉、单件关掉照常换工具面、`preset.get` 列出工具。`tests/packages.rs` 的 `bundle` 那一条多两格。
- 变异 6 个（状态写死、不带细节、重启不先停、关着的也重启、`bundle` 不能重启、服务状态一律关着），都报红。
- `cargo xtask check` 八项；三台 CI。
- 合了告诉终端、网页两个会话：`mcp` 每一格多的几格、`package.restart`、两个原因码。
