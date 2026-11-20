## 施工单 NS-1：网络搜索——Exa、AnySearch 能选能配 key，DeepSeek 的 key 显示「已继承」，设置带英文名

状态：做完，等验收（分支 `step/OA-1`）。项目主人 2026-11-17 本机验收第 10、13 条；英文名是网页 W-1206 要的（同第 10 条），主会话派进这一步。

### 目的

1. 包名「联网搜索」改「网络搜索」（项目主人原话那个词；中文只改这一处和字表里的包名，「联网服务」这一项的名字不动，英文、日文本来就是 Web search、ウェブ検索）。
2. DeepSeek 的 key 两处都留（项目主人定）：网络搜索里填了的先用；没填、模型那边有官方的 DeepSeek 供应商的借它的 key，设置页显示「已继承」。借的认法原来就有（`miyu_models::provider_key::serving`，PK-1补 的官方地址判法），这一步只让头画得出来：`config.get` 这一项的来源写 `{"layer": "inherited", "from": "providers.<编号>.key"}`，值照旧 `null`、不交 key。借哪几项登记在 `provider_key::INHERITED` 一处，联网包借、`config.get` 写来源都照它。
3. 「联网服务」的选项加上 Exa、AnySearch（项目主人：有免费额度不代表不用它的 key）：选了它们的先用它们，配了 key 的用自己的额度，没配的照免费额度。Exa 的 key 原来就有；AnySearch 新加 `net.search.anysearch.key`，配了的头里带 `Authorization: Bearer`（自动的顺序里问它也带）。
4. 英文名：`config.schema` 每一项、每一组，`package.list`、`package.info` 每个包，连接不是英文的多一格 `name_en`，头拿它和 `name` 一起搜（中文界面搜 web 搜得到「网络搜索」）。

### AnySearch 官方文档（2026-11-17 查）

- 鉴权 https://www.anysearch.com/docs/auth ：地址 `https://api.anysearch.com`；`Authorization: Bearer <key>`；不带的照每天的免费额度、按 IP 限速，带了的照 key 的额度、并发更高。
- 搜索 https://www.anysearch.com/docs/api-endpoints/v1-search ：`POST /v1/search`，`Content-Type: application/json`，`{query, max_results(1–10)}`，回 `{code, message, request_id, data: {results: [{title, url, snippet?, content?}]}}`；401 key 不对、402 额度用完、429 限速。
- 读网页 https://www.anysearch.com/docs/api-endpoints/v1-extract ：`POST /v1/extract {url}`，回 `data.title`、`data.content`（至多 5 万字）。这一步不接（见「不做什么」）。
- 价格 https://www.anysearch.com/pricing ：免费每天 1000 次、10 QPS，UTC 零点重置；付费 1000 次 3 美元起。

### 蓝图改哪几节

- `config.md`：`net.search.source`、新加 `net.search.anysearch.key`、`net.search.deepseek.key` 三行；来源表加 `inherited`、`from`；`config.schema` 加英文名一条、嵌的样本；名字表（AnySearch 那一项，DeepSeek 说明，组 `base-net` 的中文名）。
- `tools/web_search.md` 第 3 条、AnySearch 的请求；`packages.md`「格式」后第 1 条（`name_en`）；`cli/pkg.md` 例子里的包名。
- 协议参考 `config.md`、`packages.md`（中英）。

### 验收

1. 改前红：`miyu-net` 的 `anysearch_picked_uses_its_key_or_the_free_quota`（新；选了 AnySearch、配了 key 的头里 `Bearer as-key`，没配的不带，免费的几家不问）、`settings/tests.rs` 的选项对照（下拉里多了能配 key 的两家）；`miyu-endpoint` 的 `config_inherited.rs`（新：官方供应商的写 `inherited`、中转站的照 `default`、自己写了的照写的；中文连接的 `config.schema` 有 `name_en`、英文的没有）、`config/inherited/tests.rs`（新）；`config.rs` 那条一字不差的样本加了 `name_en`。在 `origin/main` 上都红（新的编不过，样本不一样）。
2. 门禁十一项、长跑随机、Windows 目标的 clippy、`cargo xtask prompts`、`cargo xtask logs`。

### 不做什么

- 不给 AnySearch 接读网页（`/v1/extract`）：选了它的 `web_fetch` 照旧先自己抓、再照自动的名单。要接另开一步。
- 选了 Exa 的照 NF-1 的规矩，`web_fetch` 自己抓之前先交 Exa 代抓（Exa 能抓页面）。
- 「联网服务」这一项的名字不改（项目主人只说了「联网搜索」）。
