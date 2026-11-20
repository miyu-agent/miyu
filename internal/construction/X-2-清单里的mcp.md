## 施工单 X-2：清单里的 MCP

状态：合了（2026-10-11，5688d0dd，run 1779；变基到 T-13 以后本地八项重跑过）。图纸 `mcp.md`（项目主人 2026-10-11 照推荐定了四条）、`packages.md`「MCP 服务」。

### 这一步做什么

1. 清单多一种表 `[[mcp]]`（`miyu-config` 的 `package/mcp.rs`）：`name`、`command`、`env`（字，或者 `{ secret = … }`、`{ env = … }`）、`venues`（不写只给本机）、`timeout_ms`（1000 到 3600000，不写 600000）。每一种写错报新代码 `bad_mcp`，`detail` 是哪一格，报在那一段；三种语言的字。
2. 不带程序、带了 `[[mcp]]` 的包是新种类 `bundle`：能写 `[package]`、`[[mcp]]`、`[mascot]`，不说协议。扩展包也能带 `[[mcp]]`；界面、小程序、内置包不能。
3. 一个服务一个功能：`Manifest::features_of` 接在包自己的功能后面，编号 `mcp-<服务名>`、名字是服务名，记着是哪个服务（`Feature::service`）；`Features::of_tool` 把 `mcp__<服务名>__…` 的工具归它，包自己的工具照旧只看包自己的功能。两个包同名的服务照功能编号撞了报。
4. 列出来：`package.list` 多 `mcp: [{name, command, venues}]`（不列环境变量）、`kind: "bundle"`；看一眼多 `mcp`（服务名），`bundle` 没有 `program`；`miyu pkg install` 照它印「MCP 服务 <名字>」。
5. 这一步只认清单：起服务、登记工具、`bundle` 的开关都是 X-3；这一步 `bundle` 的状态是 `ready`、没有开关。
6. 图纸 `packages.md`（格式、哪种能写哪几张表、出错、列表、守着它的、起草时定的、新的一节「MCP 服务」）、`mcp.md` 状态、`protocol.md`（`package.list`、看一眼）、`cli/pkg.md`。

### 定了的（主会话，技术题照推荐）

- 新种类叫 `bundle`：一组东西、不带程序，以后技能、脚本也算进来；种类只给程序用，界面不印，不另造给人看的叫法。
- `bundle` 不说协议：带的服务说的是 MCP，同吉祥物包。
- 服务的工具照名字的前缀归功能，归法在这一步定好，X-3 登记工具时预设不用再动。
- `package.list` 列服务的命令：装了以后会在这台机器上跑它，人该看得到；环境变量可能引用密钥，不列。
- 改了协议（`kind` 多一个值、多 `mcp`）：合了以后告诉终端、网页两个会话。

### 验收

- 单测：`[[mcp]]` 每一格、默认值、每一种写错；`bundle`、哪种包能写；一个服务一个功能；工具照前缀归；同名服务撞了；命令行那一份。
- 契约测试：列表里的 `bundle`、`mcp`、`features`；看一眼的 `mcp`。
- 变异 6 个（不认 `bundle`、前缀不查 `__`、包自己的功能把服务的也算进去、`bundle` 照说协议、看一眼给 `bundle` 写程序、`venues` 不查重），都红。
- `cargo xtask check` 八项；三台 CI。
