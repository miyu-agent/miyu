## 施工单 ACP-1：驱动另报上下文大小

设计 73 第三节。

### 为什么

扩展驱动一次 `driver.call` 里会来回请求几次（`antigravity` 驱动跑 agy 自带的搜索、看文件时有两三次，ACP 的 agent 也一样），每次都带整段上下文。驱动报的用量是几次加起来的，记账照它没错，可内核拿「输入加输出」当上下文大小（`estimate::anchor`），一加就虚高。2026-11-23 晚上和 11-24 13:02 主人的群会话都因此连压几次、停在 too_large，之后不回话。施工 AG-12 在驱动那边改成只报最后一次请求的输入，代价是记账偏少。旧版 0.7 一次调用记两份用量（记账的加总、上下文的最后一次），重制版的协议只留了一份。

### 改了什么

- 内核 `Usage` 多一格 `context: Option<u64>`：没有的不写，旧日志照原样读写；`plus` 不加它，后一份报了的照后一份。
- `estimate::anchor` 的 `reported`：有 `context` 照它，没有照旧是四格加起来。记账、金额照旧只看四格。
- 扩展驱动的回答 `usage` 多一格可选的 `context`（核心读扩展回答的 `usage` 本来就不拒认不出的格，旧核心收到会忽略）。
- 协议参考（扩展的 `driver.call` 回应，中英）、蓝图 `kernel/events-bodies.md` 的 `model.called` 用量那一段；设计 73。

### 怎么验

- `estimate/tests/context.rs` 三条：报了 `context` 的锚照它（四格加起来 106000、`context` 50000，锚是 50000）；带 `context` 的用量写回一字不差、不带的旧日志照原样；`plus` 照后一份的 `context`、四格照旧加。退回 `anchor` 只加四格，第一条红。
- 补齐四十来处构造 `Usage` 的地方（`context: None`），整个 workspace 编得过。
- 门禁。
