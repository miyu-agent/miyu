## 施工单 PT-1（上）：网址的 MCP 服务带标准请求头

状态：写好了（2026-10-28）。主会话 2026-10-28 派：PT-1 差距报告（MCP 规范 2026-07-28 对照 `miyu-mcp`）的 P0，技术题主会话定了。图纸 `docs/blueprint/mcp.md`「怎么走」第五条第 1、2 条。

### 目的

规范 2026-07-28 要求 Streamable HTTP 的每个请求带 `Mcp-Method`，`tools/call`、`resources/read`、`prompts/get` 另带 `Mcp-Name`；缺了的，新版服务必须回 `400` 和 `-32020`。我们原来不带，又把 HTTP 出错时服务回的 JSON-RPC 错误一律换成自己的 `-32000`，探时代的认不出这是新版服务的错，退回老握手。结果是只认新版、照规范查请求头的网址服务整个连不上（新旧都认的服务照旧走老版能用）。

### 做了什么

1. `miyu-mcp` 的转接（`src/http.rs`）：
   - 每个请求带 `Mcp-Method`，那三个方法另带 `Mcp-Name`（`params.name` 或 `params.uri`）。照规范编码：不是可见 ASCII、首尾有空白、本身像 `=?base64?…?=` 的编成 `=?base64?<base64>?=`。不分时代都带，老服务不认、照样无害。
   - 回 4xx、5xx 时，回应体本身是一条 JSON-RPC 错误的原样交回、编号换成这次请求的；回应头写 SSE 的先照事件拆，拆不出的照整段读。不是的照旧合成 `-32000`。
   - 探时代那一头不用改：`-32022` 照服务列的版本重试，`-32099..-32020` 当新版的错不退回，别的错退回握手，现在 HTTP 上也认得出了。
2. `miyu-config`：`Mcp-Method`、`Mcp-Name` 和 `Mcp-Session-Id` 这些一样留给核心，清单、个人设置的 `headers` 里不许写。
3. 测具（`miyu-mcp` 的 `testkit::http`）：新版的假服务照规范「Server Validation」查全：`MCP-Protocol-Version` 和 `_meta` 对不上、`Mcp-Method` 缺了或对不上、`Mcp-Name` 缺了或解开以后对不上的回 `400` 和 `-32020`；版本不认的回 `400` 和 `-32022`、列出认的几版；不认的方法回 `404` 和 `-32601`。另加两个开关：只认别的版本（`support`）、老服务对 `server/discover` 回 `400` 和一条没有编号的 `-32000`（官方 TypeScript SDK 的老服务没会话就这样）。
4. 图纸 `mcp.md`：第五条第 1、2 条；顺手改了差距报告里列的五处和代码对不上的（开头那句「只提供工具」、服务名的写法、握手时 `401` 那句、待定第 3 题的预先注册和回调端口、定了的第 3 条 `instructions`）。协议参考 `packages` 中英：`headers` 那一格。

### 不在这一步

- HTTP 上真断开被取消的请求、只带 `requestState` 的多轮请求重发、`x-mcp-header`：PT-1 中。
- SSE 边读边交、进度、`subscriptions/listen`：PT-1 下。

### 验收

1. `miyu-mcp` 的 `http::tests`：
   - 照规范查请求头的新版服务（JSON、SSE 两种）认得成新时代、列得出、调得到，每个请求的 `Mcp-Method` 和方法名一样，调工具带 `Mcp-Name: echo`。
   - 只认别的版本的新版服务：连不上、说没有共同的版本、一次 `initialize` 都没发，日志里是 `HTTP 400: -32022`。
   - 老服务对 `server/discover` 回 `400`：退回握手照常用，日志里是原样的 `-32000` 那句。
   - 编码照规范的五个例子；标准请求头只给有方法名的、三个方法带 `Mcp-Name`；只有 JSON-RPC 错误的回应体原样交回，通知不写回。
2. `miyu-config`：`headers` 写 `Mcp-Method`、`mcp-name` 的报错。
3. 手写变异 13 个全抓住（不带标准请求头、`prompts/get` 不带名字、`resources/read` 取错格、编码的三个条件各去掉一个、错误不原样交回、不换编号、没有码也当错误、SSE 拆不出就丢、通知也写回、两个保留名各去掉一个）。
4. `cargo xtask check`、Windows 版 clippy、三平台 CI。
