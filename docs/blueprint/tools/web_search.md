## `web_search`

### 是什么

联网包（`net`）的工具（施工 N-2 上，设计 `36-联网搜索.md` 第一节第 2 条、第二节）：搜网页，交回几条标题、网址、日期、摘录。没配置时照出厂的顺序一家家试几家免 key 的来源，被限额的进冷却、到点回来。要 key 的几家、网页抓取兜底在 N-3，「搜索来源」设置在 N-2 下，扩展登记来源在 N-5。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-net/src/web_search.rs` | 参数、写给她的几条、给人的一句、给头的 `links` |
| `crates/miyu-net/src/search.rs` | `Search`：懒读规矩、照顺序试、冷却、筛域名、去重、截摘录 |
| `crates/miyu-net/src/search/rules.rs` | 读 `search.json` |
| `crates/miyu-net/src/search/cooldown.rs` | 限额冷却的台阶（纯逻辑） |
| `crates/miyu-net/src/search/mcp.rs` | MCP over HTTP（Streamable HTTP）的最小客户端：握手、会话编号、JSON 或 SSE 的回应 |
| `crates/miyu-net/src/search/sources.rs` | 各家的参数怎么写、回应怎么读 |
| `resources/software/net/search.json` | 顺序、条数、摘录长度、时限、冷却台阶、各家的地址 |
| `resources/software/net/tools/web_search.json` | 说明和参数格式 |
| `resources/software/net/web_search/*.txt` | 输出里给她看的几句 |
| `resources/software/net/human/{zh,en,ja}.json` | 显示名「网络搜索」、结果那一句 |
| `resources/packages/net/package.toml` | 功能 `web-search` |

### 对外的样子

访问类别 `read`，哪里都不问人（设计 36 第一节第 4 条），场所、子会话的工具面上都有。

样本 `resources/software/net/tools/web_search.json`：

```json
{
  "description": "Search the web and get titles, URLs and excerpts of matching pages. Read a page in full with web_fetch.",
  "parameters": {"type":"object","properties":{"query":{"type":"string"},"allowed_domains":{"type":"array","items":{"type":"string"}},"blocked_domains":{"type":"array","items":{"type":"string"}}},"required":["query"]}
}
```

| 参数 | 必填 | 怎么认 |
|---|---|---|
| `query` | 是 | 去掉前后空白，空的算参数不对 |
| `allowed_domains` | 否 | 只要这几个域名（含子域名）的结果；写成 `https://www.example.com/x` 也认，收成 `example.com` |
| `blocked_domains` | 否 | 不要这几个域名（含子域名）的结果 |

交回的字（成了的），条和条之间空一行，结果里不写是哪一家：

```text
[1] select in tokio - Rust
https://docs.rs/tokio/latest/tokio/macro.select.html (2026-09-01)
Waits on multiple concurrent branches, ……
```

给人看的那一句：`web_search/found`「{hits} 条（{source}）」（一条的发 `found/one`）、`web_search/nothing`「没搜到（{source}）」，`source` 是那一家给人看的名字（设计 36 第三节第 2 条）。给头的 `detail` 是通用形状 `links`（施工 W-11 补）：`{"kind": "links", "links": [{"title", "url", "snippet"?, "date"?}]}`，和给她的几条一一对应；没搜到的、出错的不带。

### 怎么走

1. 读参数：读不懂、`query` 是空的：参数不对。
2. 第一次搜读 `search.json`，读不懂的记一条 `WARN web search not ready`，这个核心的生命周期里不再试，每次回 `not-ready`。
3. 照 `order` 一家家试（出厂 Exa → Parallel → Firecrawl → AnySearch → You.com，设计 36 第二节第 2 条）：
   - 冷却着的跳过。
   - 成了的（哪怕一条都没有）清零冷却，就用它，后面的不问。
   - 报限额的（HTTP 429；或者原话里有 rate limit、too many requests、quota、credit、usage limit、limit reached、HTTP 429）进冷却：台阶 30 分钟、1 小时、2 小时，最多 6 小时（`cooldown_minutes`），记 `WARN web search source limited source=… minutes=…`，换下一家。
   - 别的出错（超时、连不上、5xx、读不懂）记 `WARN web search source failed source=… why=…`，这一次换下一家，不冷却。
   - 都试过、都不成：`failed`，原因是最后一家的（「{名字}: {原因}」）。一家都没试（都在冷却）：`cooling`。
4. 问一家：
   - 要几条照 `results`（8）；要筛域名的要两倍，筛完还够数。
   - 认 `site:` 的几家（`site_operator`：Parallel、Firecrawl、You.com）把只要的域名写进查询，`site:a OR site:b`。
   - MCP 的四家照 `sources.rs` 写参数（Exa `web_search_exa {query, numResults, objective}`，Parallel `web_search {objective, search_queries, session_id}`，Firecrawl `firecrawl_search {query, limit}`，You.com `you-search {query, count}`）；Parallel 的 `session_id` 是这个核心的一个固定编号（它照它算免费额度）。AnySearch 是 REST，`{query, max_results}`，至多 10。
   - 时限每次请求 `timeout_seconds`（20）。回应至多读 4 MiB。
   - MCP：第一次握手（`initialize`、`notifications/initialized`），对方给了 `Mcp-Session-Id` 的记下、以后带上，会话过期（404）的重新握手一次。回应可以是 JSON 也可以是 SSE，取最后一条带 `result`、`error` 的；`isError`、JSON-RPC 出错的当这一家出错。
5. 收拾：没有网址的、同一个网址第二次出现的去掉；照 `allowed_domains`、`blocked_domains` 筛主机名；至多 `results` 条；摘录截到 `excerpt_chars`（1200）个字，截了的接一个省略号；日期只留前十个字。
6. 请求固定的几家公开地址，不过地址闸；代理照环境变量，和抓网页同一套（`net.md`「怎么走」第 5 条）。
7. 冷却、MCP 的会话记在这一份 `Search` 里：核心重启、装卸软件包（核心重新要内置工具）以后从头算。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-net/src/search/cooldown/tests.rs` | 台阶一级级往上、最多 6 小时；到点回来；各家各算；成功清零 |
| `crates/miyu-net/src/search/rules/tests.rs` | 出厂的顺序、台阶、地址；是 0 的、台阶空的、顺序里写了没列的、种类不认得的、多一格的读不了 |
| `crates/miyu-net/src/search/mcp/tests.rs` | JSON、SSE 里最后一条的读法；工具结果的文字；`isError` 说限额的、别的 |
| `crates/miyu-net/src/search/sources/tests.rs` | 五家的回应读成一样的几条（样本照实测删短）；拒绝和乱码；没有结果不算出错 |
| `crates/miyu-net/src/search/tests.rs` | 去重、筛域名（含子域名、写法不同）、截摘录、至多几条；`site:` 只写给认的几家；认限额的几种原话 |
| `crates/miyu-net/tests/search.rs` | 本机假服务器扮五家：访问类别、参数；第一家成了不问后面的、写给她的样子、单数、给头的 `links`；429、SSE 里说限额的冷却，下一次跳过，500 的不冷却照样再试；成功一次清零（台阶换成 0 秒、1 小时，假服务器同一个地址先回 429 再回成功）；都不成照最后一家的原因说；都冷却着一家都不问；没搜到不算出错 |
| `crates/miyu-net/tests/live.rs` 的 `every_keyless_source_answers_web_search`（`#[ignore]`，还要设 `MIYU_NET_LIVE=1`） | 真网络：五家各搜两条（中、英），都搜得到对口的结果 |
| `crates/miyu-net/tests/budget.rs` | 两件工具的说明合计不超过 663 字节 |

### 出处

- 设计 `36-联网搜索.md` 第一节、第二节（项目主人 2026-10-18 定）；顺序照同一天六家免 key 的实测。
- Claude Code 的 `WebSearch`（名字、`allowed_domains`、`blocked_domains`）；opencode 的 `websearch` 照 Exa、Parallel 的 MCP 接。

### 还没有的

- 要 key 的几家、网页抓取兜底（N-3）；配了来源的先用它、设置页「搜索来源」下拉、选项后面写冷却到几点（N-2 下）；扩展登记来源（N-5）。
