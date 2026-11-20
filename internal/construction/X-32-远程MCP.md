## 施工单 X-32：远程 MCP 服务

状态：主会话审过（2026-10-20）；项目主人定做远程 MCP，OAuth 推迟到有一个真要它的服务。

### 目的

接给一个网址的 MCP 服务（Streamable HTTP）：GitHub 这类官方只给网址、认请求头里的令牌。现在只接本机拉起的（`mcp.md`「还没有的」）。

### 蓝图改哪几节

- `mcp.md`「对外的样子」：`[[mcp]]`、个人设置的 `[mcp.<名字>]` 写 `command` 或 `url`，二选一。写 `url` 的：
  - `url`：`https://` 开头；`http://` 只认本机地址（`localhost`、`127.0.0.1`、`[::1]`）。
  - `bearer`：可以不写，写法同 `env` 的值（字、`{ secret = … }`、`{ env = … }`），发成 `Authorization: Bearer <值>`。
  - `headers`：可以不写，别的请求头，值的写法同上（`X-API-Key` 这类）。
  - `venues`、`timeout_ms`、`need_load`、`clicall` 照旧；`env` 不认（报 `bad_mcp`）。
- `mcp.md`「怎么走」第二条「进程」多一种：写 `url` 的没有进程，「起」就是认时代、列工具；连不上、401、403、5xx 都算一次失败，照进程的退避、连续 5 次停下；状态那一格照旧（没有 `pid`、`stderr`，停下的 `reason` 写 HTTP 状态和原话的头 200 个字）。
- `mcp.md`「怎么走」新一条「网址的服务怎么说话」：转接把 `miyu-mcp` 的一行一条接到 HTTP 上，客户端不改。转接放在 `miyu-net`（第 3 层，已经有和 `miyu-http` 同一套的 `reqwest`），`miyu-mcp` 不加依赖；开工前照门禁的「分层」一项核对。每条请求 POST 到 `url`，`Accept: application/json, text/event-stream`，带上服务给过的 `Mcp-Session-Id`；回来的是 JSON 照写回去，是 SSE 的照 `data:` 一条一条写回去；会话过期（404）的从认时代重来一次。服务主动推的（`list_changed`）只在 SSE 回应里收得到，不另开 GET 那条长连接：工具变了靠新时代的 `ttlMs`、重连时重列。
- `mcp.md`「目录」：缓存照 `url` 和请求头的名字认，不认值（换了令牌不作废）。
- `net.md`：搜索来源里走 MCP 的几家改用同一个转接，`miyu-net/src/search/mcp.rs` 只留认限额那几行。
- 作者文档 `mcp`（中英）、协议参考 `packages`（`mcp.set` 收 `url`、`bearer`、`headers`；`mcp.list` 带回 `url`、请求头的名字，不带值）。

- 写法 `url` 加 `bearer` 加 `headers`：2026-10-20 项目主人照推荐定（`bearer` 是最常见的那一种，写成一格省得人把 `Bearer ` 连着令牌存进密钥）。

### 不做什么

- OAuth（浏览器授权、刷新令牌）：推迟到有一个真要它的服务（2026-10-20 项目主人定）。
- GET 的长连接、断点续传（`Last-Event-ID`）、旧的 HTTP+SSE 传输（2024-11-05 那一版的两个地址）。
- 远程服务不分账号：同本机的，一个核心一份（等多用户）。

### 验收

1. 先写会红的测试：测试里起一个本机 HTTP 的假服务（`miyu-test-mcp` 多一个 `--http`，照两个时代、JSON 和 SSE 两种回法）：写 `url` 的服务起来、列出工具、调得到；`bearer`、`headers` 发出去是对的，运行日志、`mcp.list` 里没有值；404 以后重认一次；401 退避着停下、`reason` 带状态；`http://` 非本机的报 `bad_mcp`。
2. 搜索来源的测试照旧全过（换了转接）。
3. 真服务走一遍：一个公开的、认令牌的远程 MCP 服务，真模型调一次（记进施工单）。
4. `cargo xtask check` 全过。

### 风险

- 令牌进日志：请求头的值只在转接里用，`Debug` 不打印值；测试断言日志里没有。
- Windows 上证书：照 `miyu-http` 那一套 rustls、系统证书加 webpki 两份，不另起依赖。
