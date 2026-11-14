## HT-5 供应商侧工具：Anthropic 驱动的服务端工具

状态：施工完，待 core1 审（2026-11-14）。设计 `71-供应商侧工具.md` 第四节 Anthropic 一行、第六节第 3 条、第七节第五行。接在 HT-1、HT-2 后面。
只有技术细节，直接做。线上的形状照 Anthropic 官方文档（网络搜索、抓取网页两页，2026-11-14 读的）。

### 目的

Anthropic 接口那边跑的服务端工具（`web_search`、`web_fetch` 这类），驱动原来整块不理：日志里没有、界面上看不见，下一次
请求也不带。这一步驱动把「调用」和「结果」两块并成一块供应商侧工具交给内核，下一次发给这一家时照原样带回（结果里加密的内容
要原样回传，供应商才接得上）。这一步不管怎么开：Miyu 自己还不往请求里放服务端工具，现在只有中转替人开的才会遇到。

### 蓝图改哪几节

`drivers/anthropic.md`：

- 「解码」`content_block_start` 的表：
  1. `server_tool_use`（带 `id`、`name`）开一块供应商侧工具（`Kind::HostedTool`，名字照 `name` 原样）；它的
     `input_json_delta` 照工具调用交成参数的字。这一块的 `content_block_stop` 不收块：等结果。
  2. 类型以 `_tool_result` 结尾、带 `tool_use_id` 的块（`web_search_tool_result`、`web_fetch_tool_result`，以后别的同形状的
     也算），整块都在 `content_block_start` 里：照 `tool_use_id` 找到这一次响应里开着的那块供应商侧工具，交结果（`Outcome`）、
     私有数据（`Private`）、收块。不占新的块编号。找不到对应的（上一次响应里开的）照旧不理。
  3. 结果的字（给人、给别家模型看的那一份，`outcome.text`）：
     - `web_search_tool_result`：`content` 是数组的，一条结果一行 `<title> - <url>`，没有结果的是空的字；`content` 是出错
       对象的（`web_search_tool_result_error`），`failed` 真，字是 `error_code`。`encrypted_content` 不进字。
     - `web_fetch_tool_result`：成了的写 `<url>`、有标题的下一行 `<title>`、空一行、正文（`content.content.source` 是文本的
       取它的 `data`；是 base64 的不放内容，写一行 `(<media_type>)`）；出错对象的 `failed` 真，字是 `error_code`。
     - 别的 `*_tool_result`：字是 `content` 的 JSON 原文；`content.type` 以 `_error` 结尾的 `failed` 真。
  4. 私有数据：`{"use": <server_tool_use 整块，input 是收全的参数>, "result": <结果块原样>}`，两块都是线上的原文（`use` 的
     `input` 照收到的参数原文拼进去，参数不是合法 JSON 的不交私有数据）。整份超过 1 MiB（`PRIVATE_BYTES`）的不交私有数据：
     日志里不存几兆的网页、PDF，这一块以后照 HT-2 写成字。
  5. 流完了还没等到结果的（被打断、`pause_turn`、和核心的工具同一批并行调用时供应商还没跑）：照常收块，没有结果、没有
     私有数据。
  6. 正文块的 `citations`（`citations_delta`）照旧不理。
- 「编码」assistant 那一条：供应商侧工具带着这一家的私有数据的，照私有数据在原来的位置写回两块原样（`use`、`result`，一个
  字节不改）；别的（别家的、没带私有数据的）照 HT-2 写成一段字。留着编和整份编逐字节相同（V-42）。
- 「收尾」的表：`pause_turn` 照旧是 `other`（续接要内核支持「最后一条是 assistant 的请求」，没做），写进「还没有的」。
- 「还没有的」：`pause_turn` 续接；和核心的工具并行时的那一次搜索（下一次响应里才来的结果块对不上，不理）；引用
  （citations）；`usage.server_tool_use` 的次数不另记（回复里这种块的个数就是）；Miyu 自己开服务端工具的设置。
- 「在哪」「守着它的」跟着加。样本 `docs/designs/samples/drivers/anthropic/` 加一份带服务端工具的流和一份带回的请求。

### 不做什么

- 不往请求的 `tools` 里加服务端工具，不加配置项。
- `pause_turn` 续接、引用、OpenAI Responses、DeepSeek 的服务端搜索。
- 不动内核、视图、扩展那一路。

### 验收

1. `miyu-drivers` 流测试（照 `tests/anthropic_streams.rs` 的写法，流照官方文档「Streaming」一节的样子造）：
   - 正文、`server_tool_use`（参数分两段）、`web_search_tool_result`（两条结果）、正文：交出的增量是 正文块、供应商侧工具块
     （参数、结果两行、私有数据、收）、正文块；块编号 0、1、2，结果块不占号。
   - 搜索出错（`max_uses_exceeded`）：`failed` 真、字是错误码。`web_fetch` 文本的、PDF 的、出错的各一条。不认识的
     `x_tool_result`：字是 `content` 原文。
   - 两次搜索并行（use1、use2、result1、result2）：各对各的。
   - 没等到结果流就完了（`pause_turn`、流断了）：块在、没结果、没私有数据。
   - 对不上 `tool_use_id` 的结果块：不理，别的块照旧。
   - 私有数据超过 1 MiB：不交私有数据，结果照交。参数不是合法 JSON：不交私有数据。
   - 增量交给内核的累积器走一遍不报错（`Accumulator::apply`），拼出的块 `private.driver` 是 `anthropic`。
2. 编码：带私有数据的写回两块，和当初流里的两块逐字节相同（`use` 的 `input` 是参数原文）；前后的正文、思考、工具调用先后
   不变；没带私有数据的、别家的走 HT-2 那段字；备忘录对照逐字节相同；`stable` 打点落在它后面时不乱。
3. 解一遍再编一遍：流 → 增量 → 累积器拼成块 → 放进请求 → 编出的 assistant 内容里那两块和流里的原样一致。
4. 没有服务端工具的流和请求，现有样本、现有测试的期望不变。
5. 手搓变异 10 个左右全红。
6. 门禁全过，分支三平台 CI 全绿。

### 风险

- 没有真端点实测（没有开了网络搜索的 Anthropic key）：带回时不带正文块的 `citations`、请求的 `tools` 里没有声明这件服务端
  工具，这一家收不收，照文档推断是收的，待有 key 的人实测；实测不收的话退路是整对不带回、一律写成字（改一处判断）。
- 同一类驱动、不同账号（两家中转）之间带回加密内容，对方解不开会 400：和思考的签名是同一个老问题，这一步不解。
- 1 MiB 以上的结果以后写成字，等于改了历史：新模型查「历史没被改过」（思考块绑定）时可能拒；抓大 PDF 才会碰到。

### 做下来的

- 代码都在 `crates/miyu-drivers/src/anthropic/hosted.rs`（新）：等结果的块、结果的字、私有数据、能不能写回；`decode.rs`、
  `messages.rs` 各接一处，`wire.rs` 的 `Part` 多一种装原文的。
- 私有数据：`result` 是流里 `content_block` 那一格的原文，逐字节照抄（数字的写法、转义都不变）；`use` 是起始块每一格照原来的
  先后重写、值都是原文，只把 `input` 换成收全的参数原文（起始块里的 `input` 是空对象，参数是从增量拼的，没法逐字节照抄），
  别的格（例如 `caller`）一个不丢。上限照整份数据的字节数算，正好 1 MiB 的照交。
- 原来那份 `server-tool-ignored` 流样本本身就是服务端工具的流（以前整块不理、正文是第 0 块），改名 `server-tool-empty`：现在
  解出一块供应商侧工具（结果是空的字），正文成了第 1 块。别的现有样本、测试的期望一个字节没动。
- `pause_turn`、流断了都是出错，驱动收尾时不交收块（原有规矩），那块供应商侧工具靠内核打断时的拼法留下，没结果、没私有数据。
- 写回的两块照普通的块打缓存点（这一家这两种块收 `cache_control`，照文档推断，没有真端点核实）。
- 测试：`tests/anthropic_hosted.rs` 7 条，流样本新加 9 份（搜索、空结果、搜索出错、抓取三种、不认识的结果、并行、没等到
  结果的三种、对不上编号的），备忘录的随机历史里一半带上能写回的私有数据。手搓变异 15 个全红：结果块占了块编号、`failed`
  不置真、错误码没进字、加密内容进了字、PDF 的 base64 进了字、对不上编号的接到别的块上、超过上限照交、坏参数照交、别家的
  也写回、写回时结果被重排、只写回一块、带私有数据的也写成字、`input` 没换成参数原文、收了的块又收一次、`use` 丢了别的格。
