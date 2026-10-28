## 施工单 PT-1（中）：取消真断开、多轮请求照规范重发、参数请求头

状态：写好了（2026-10-28）。主会话 2026-10-28 派：PT-1 差距报告（MCP 规范 2026-07-28 对照 `miyu-mcp`）的 P1，只动 `miyu-mcp`。图纸 `docs/blueprint/mcp.md`「怎么走」第二条（两个时代）、第五条第 7、8 条。

### 目的

新版服务「正常用」就会碰到的三处：

1. 网址的服务上，客户端取消一次请求，那个 POST 原来照旧在路上，服务照跑到完；新版规范里断开回应流才算取消。
2. 新版服务回 `input_required`、只带 `requestState` 的（规范的例子是服务忙时分流），原来一律当出错；规范许客户端直接重发。
3. 工具参数标了 `x-mcp-header` 的，Streamable HTTP 的客户端必须把值镜像成 `Mcp-Param-<名>`，标得不合规矩的工具必须从列表里剔掉；原来都没做，用了这个标记的新版服务会拒掉那几件工具的调用。

### 做了什么

1. 取消（`src/http.rs`）：转接照编号记着每个还在路上的 POST，客户端发 `notifications/cancelled` 时停掉它，连接跟着断。握过手的老服务另把通知照旧发过去（老规范里断开不算取消），新版的不发。
2. 多轮请求（`src/client.rs`）：`tools/call`、`prompts/get`、`resources/read` 回 `input_required`、`inputRequests` 是空的、带 `requestState` 的，换个编号把原请求连同原样的 `requestState` 再发，至多 8 回，还叫重发的当写法不对；要补东西的照旧报不支持；别的方法这样回当写法不对。
3. 参数请求头（新的 `src/http/params.rs`）：列工具的回应经过转接时照规范查标记（名字、HTTP 头名许的字、不分大小写不重名、只标在 `string`、`integer`、`boolean` 上、只经一串 `properties` 到得了），不合的整件剔掉、服务日志写一行 `WARN`；合的记下，调这件工具时照路径取值、照 `Mcp-Name` 的编码带 `Mcp-Param-<名>`。
4. 测具：stdio 的假服务多三件不列出来的工具（`busy` 叫重发一次、`nagging` 一直叫重发、`elicit` 要补东西）；网址的假服务多两个开关：列两件带标记的工具（`region` 合规矩、调它要带对 `Mcp-Param-Region`；`broken` 的标记写在 `items` 里），慢调用中途客户端断开的记一次。
5. `http.rs`、`testkit/http.rs` 过了 500 行，拆出 `http/headers.rs`（标准请求头和编码）、`http/answer.rs`（读回应、认 JSON-RPC 错误）、`testkit/http/modern.rs`（新版假服务的答法），只挪不改。
6. 图纸 `mcp.md`：第二条两个时代那一段、第五条第 7、8 条、代码表、测试表（顺带补上 PT-1 上漏记的那几条测试）。

### 不在这一步

- 规范说服务因为 `Mcp-Param-*` 回 `-32020` 时客户端「应」重列再试一次：没做，重连、`ttlMs` 过期都会重列。
- SSE 边读边交、进度、`subscriptions/listen`：PT-1 下。
- 要补东西的多轮请求（elicitation）：等定。

### 验收

1. `miyu-mcp`：
   - `tests/calls.rs`：只带 `requestState` 的重发一次就答完，两次编号不同、第二次原样带回 `round-2`、参数照旧；`inputRequests` 是空表的同样重发；一直叫重发的发了 9 次（头一回加 8 回）停下、说「kept asking」；带 `inputRequests` 的照旧不支持。
   - `src/http/tests.rs`：带标记的工具，不合规矩的剔掉、日志有 `WARN tool broken left out`，合的调用带 `Mcp-Param-Region`（非 ASCII 的照规范编码）、假服务查对得上才答，JSON、SSE 两种都通；丢掉的慢调用，新版、老版服务都看到断开，新版不发取消通知、老版照发。
   - `src/http/params/tests.rs`：合规矩的标记和路径（嵌套的、`default` 里的键、叫 `x-mcp-header` 的参数都不误判）；十二种不合规矩的（含头名里带冒号的）；取值（字、`42.0` 当整数、真假，没有值、`null`、带小数的不带）。
2. 手写变异 13 个：头一轮「空的 `inputRequests` 当要了东西」「头名许所有可见字」两个没抓住，补了空表重发、冒号两条以后全抓住。
3. `cargo xtask check`、Windows 版 clippy；三平台 CI。
