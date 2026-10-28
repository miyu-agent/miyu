## 施工单 PT-1（下）：MCP 工具的进度、新版服务的订阅、SSE 边读边交

状态：写好了（2026-10-28）。主会话 2026-10-28 派：PT-1 差距报告（MCP 规范 2026-07-28 对照 `miyu-mcp`）的 P2 前半；`miyu-endpoint` 只动 `src/mcp/tool.rs`、测试用的 `src/bin/miyu-test-mcp.rs` 和一个新测试文件（开工前报过主会话）。图纸 `docs/blueprint/mcp.md`「怎么走」第三条第 2 条、第四条第 6 条、第五条第 4 条。

### 目的

1. 长时间的 MCP 工具在界面上看不到进度：客户端从不带进度令牌，服务推的进度只记日志。
2. 新版服务的「工具、提示词变了」只经 `subscriptions/listen` 推：我们不开，换了工具要等 `ttlMs` 过期或重连才看得到。
3. 网址的服务回 SSE 时我们整段读完才往回交，过程中的进度、订阅那条一直开着的流都收不到。

### 做了什么

1. `miyu-mcp`：
   - `Client::call_reporting`：调工具在 `_meta` 带进度令牌，服务推的 `notifications/progress` 一条条交给调的一方（`Progress`：做到多少、总数、一句话，`said()` 说成 `3/10 Copying`）；回应到时没交的先交完。`link` 照令牌分发。新版请求的 `_meta` 改成合并：调的一方写了的留着。
   - 订阅：新版服务在 `server/discover` 的本事里声明了工具、提示词 `listChanged` 的，连上以后后台开一条 `subscriptions/listen`，只订声明的那几种；推来的「变了」照旧数进 `changes()`，核心看管那一头照旧重列。订阅结束、出错退避重开（1 秒起翻倍到 60 秒）；回「没有这个方法」、连接断了的不再开；丢下客户端取消。
   - HTTP 转接：SSE 回应边读边拆（`http/answer.rs` 的 `Stream`），凑齐一个事件就写回；单个事件的上限同原来整段的 64 MiB。
2. `miyu-endpoint`：`mcp/tool.rs` 调工具改用带进度的调用，进度照脚本的进度行（`miyu-progress: <那一句>`，X-45）推给头，视图流照旧认。
3. 测具：stdio 假服务的剧本多 `listen`（关、开、拒）、一件推进度的 `steps`，只有开了订阅的剧本声明 `listChanged`（别的剧本新版客户端不开订阅，原来的测试不受影响）；网址的假服务 `steps` 分两次写（先进度、隔一秒再答）；`miyu-test-mcp` 多参数 `steps`。
4. `client.rs`、`http.rs`、`testkit.rs` 过了 500 行，拆出 `client/era.rs`（认时代）、`client/listen.rs`（订阅）、`testkit/calls.rs`（假服务答调用），查工具标记的两个方法挪进 `http/params.rs`，只挪不改。
5. 图纸 `mcp.md`。

### 不在这一步

- 进度给超时续命（规范许，但总时限照旧）。
- 资源更新的订阅（`resourceSubscriptions`）、资源列表变了：图纸七.5 定了不做。
- elicitation 等补东西的多轮请求、预注册 `client_id`、CIMD：等定。

### 验收

1. `miyu-mcp`：`tests/listen.rs`（新旧两个时代的进度、不要进度的不带令牌、版本和令牌并在一起；开了订阅的收到变了、只订声明的、丢下客户端取消；没声明的不开；不认的只试一次）；`src/http/tests.rs`（SSE 拆事件的边界；隔一秒才答的进度先到）；`src/progress.rs`。
2. `miyu-endpoint`：`tests/mcp_progress.rs`（真核心真起 `miyu-test-mcp steps`，视图流那一步带过 `1/2 half`，结果到了不带）；原有的 MCP 测试照旧全过。
3. 手写变异 12 个：头一轮「接着找空行的起点往后挪」没抓住（原测试里后一块碰巧还有别的空行），补了「空行只在跨块处」一条以后全抓住。
4. `cargo xtask check`、Windows 版 clippy；三平台 CI。
