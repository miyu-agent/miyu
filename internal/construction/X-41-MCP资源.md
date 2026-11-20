## 施工单 X-41：MCP 服务的资源

状态：在分支 `step/X-41-MCP资源` 上，主会话审过（2026-10-21：照服务登记一对、同前缀，不加特例）。「MCP 服务挂在哪个包」照现在的写，主会话的 F-17（技能、脚本、MCP 各做成一个内置包）合的时候统一挪。代码等 X-39 补合了再变基开工（都改 `mcp/supervise.rs`）。

### 目的

MCP 服务除了工具、提示词，还能开放「资源」：可以读的资料（文件、文档、表结构……），一份一个地址（URI），有的给的是地址模板（`weather://{city}`）。只开放资源的服务现在对她没用。项目主人 2026-10-21 定：照规范接，照推荐先给她两件工具（列、读），只改核心；人在输入框里 `@` 附资源等头要做的时候再做。

### 蓝图改哪几节

`mcp.md` 新加第七条「资源」，「先不接」那一条去掉资源：

1. **认**：服务的本事里声明了 `resources`（`capabilities.resources` 是对象；新时代看 `server/discover` 的回应，旧时代看 `initialize` 的回应）。适配器多 `offers_resources()`、`resources(cursor)`（`resources/list` 的一页）、`resource_templates(cursor)`（`resources/templates/list` 的一页）、`read_resource(uri)`（`resources/read`），等法同 `prompts`。目录缓存多一格 `resources`（以前的缓存没有的照 `false`），照缓存先登记的那一回也登这两件。
2. **两件工具，跟着这个服务登记**（同一个包、同一个前缀，`spec::register` 那一回一起放进去）：`mcp__<服务名>__list_resources`、`mcp__<服务名>__read_resource`。这样现成的规矩全照用：
   - 预设照功能 `mcp-<服务名>` 开关，场所照服务的 `venues`，先加载照服务的 `need_load`，`miyu call` 照 `clicall`，等多久照 `timeout_ms`；服务停了跟着它的工具一起撤；工具面照「提供的工具跟着登记走」那一条换，不另开特例。
   - 只开放资源、没有工具的服务也上了工具面，它的说明（第三条第 5 条）跟着带。
   - 访问类别是只读（`Read`）：读资料不改东西。
   - 服务自己有一件同名的工具的，服务的赢，这一件不登记，记一行 `WARN mcp resource tool skipped`。
   - 说明文件 `core/mcp/list_resources.json`、`core/mcp/read_resource.json`（同内置工具的写法：`description`、`parameters`），进登记簿。
   - 不另做一对全局的、带服务名参数的工具（Claude Code 是那样）：那要给执行器开一条到 MCP 的端口、工具面另开特例（同知识库那一对），照服务登记全用现成的。
3. **`list_resources {cursor?}`**：一次一页。回一行 JSON：`{"resources": [...], "resourceTemplates": [...], "nextCursor": ...}`。资源每项只留 `uri`、`name`、`title`、`description`、`mimeType`、`size`，模板每项只留 `uriTemplate`、`name`、`title`、`description`、`mimeType`，服务多写的丢掉。模板只在第一页（不带 `cursor` 的那一次）列，没有下一页的不写 `nextCursor`。不另写给她看的话：结构照 MCP 原样，她认得。
4. **`read_resource {uri}`**：`resources/read`，`contents` 每一项照工具结果的规矩换成块（`content::done`）：文字原样；四种图（`image/png`、`jpeg`、`gif`、`webp`）照工具结果的图收，受同样的张数、尺寸上限（主会话定：「不灌原文」管的是二进制原文，图照别的工具交图）；别的二进制写成 `core/mcp/not-shown.txt` 那一行，不灌原文；加起来超过 10 万个字截。服务回错的照工具出错，原话交回。
5. **不做**：订阅（`resources/subscribe`、`notifications/resources/updated`）；资源变了的通知（`notifications/resources/list_changed`：列是现取的，不用记）；参数补全（`completion/complete`）；人在输入框里 `@服务:地址` 附资源。

别的几处（中英各改一遍）：作者文档 `mcp`「服务的说明和提示词」加一条「资源」；协议参考 `packages` 的 `mcp.list`：`tools` 里会有这两件。

### 工具面多多少

每个声明了资源的服务多两件。先加载（默认）时工具面上每件只有名字和说明：每多一个声明了资源的服务，工具面多两件、估 60 token；加载了另加参数格式，估 40 token。都是估的，合时主会话照惯例量。说明照规矩：一句说干什么，每个参数一句。

### 不做什么

- 上面第 5 条。
- 头一样不动。

### 验收

1. 先写会红的测试：
   - 适配器（`miyu-mcp`，它自己的测试服务）：声明了 `resources` 的认、没声明的不认，新旧两个时代都查；列一页、照 `nextCursor` 拿第二页；读回文字、二进制。
   - 端点的测试服务 `miyu-test-mcp` 多一个选项 `resources`：一份文字、一张 PNG、一份 `application/octet-stream`、一个模板，分两页。
   - 真核心：声明了的服务两件工具上工具面，没声明的没有；预设关了这个服务的没有；`list_resources` 第一页带模板和 `nextCursor`，照 `cursor` 拿第二页、不带模板；`read_resource` 文字原样、PNG 成图、octet-stream 一行；只有资源、没有工具的服务说明照带；照缓存先登记的也有这两件；服务自己有同名工具的，服务的赢。
2. 手写变异 15 个左右。
3. `cargo xtask check` 全过；本机跑 Windows 的 clippy；多了起子进程的测试，推 `step/` 过三台 CI。

### 风险

- 工具面变大：只有声明了资源的服务才多这两件，先加载的只占名字和说明。
- 资源很多的服务一页很长：一页多少由服务定，太长的照 10 万个字截。

### 做下来的

- 适配器：`resource.rs` 管一页（`Page`、只留 `RESOURCE_FIELDS`、`TEMPLATE_FIELDS`）和读回的内容；`Client` 多 `offers_resources`、`resources`、`resource_templates`、`read_resource`，等法同 `prompts/get`（不限时，调的一方照 `timeout_ms` 限）。测试服务（`testkit/resources.rs`）带三份资源、一个模板、两页。
- 端点：`McpTool` 多一格 `What`（服务的工具、列资源、读资源），不另写一种工具，暂时不可用、超时、服务回错这几种说法都照原来那一套；`mcp/resources.rs` 照服务登记那两件、列一页、读一份（图的二进制换成图）。`spec::register` 多一个「声明了资源」，看管列完的、照缓存登记的都交；目录缓存多 `resources`。
- 看红：适配器先放空壳，4 条新测试全红，换上实现全绿。端点的实现写在测试前面，看红改成把登记那两件临时关掉：5 条真核心测试红 4 条（预设关了的那条本来就该什么都没有），恢复以后全绿。
- 手写变异 14 个，头一遍漏了「访问类别不是只读」：真核心测试的会话都开成完全放开，联网、只读一样不问。补一条工作区那一级的会话里读资料不问人，重跑抓住，14 个全抓住。
- token 是估的，登记簿那两行写了估多少，合时请量。
- `cargo xtask check` 八项全过（头一回格式没过，`cargo fmt` 以后过）；本机 Windows 的 clippy 过；推 `step/` 过三台 CI（见交接的消息）。
