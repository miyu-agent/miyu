## 施工 NF-1：联网服务切供应商

状态：已完成（2026-11-14）。项目主人 2026-11-14 定：「搜索来源」改成选「联网服务」的供应商，选了 DeepSeek 的搜索、代抓都走它，调不通的退回免费的几家；看过施工后又定了两条：自动不花钱（DeepSeek 不进自动的顺序），选了带代抓的服务的 `web_fetch` 先交它、不先自己抓。

### 目的

设置页原来的「搜索服务供应商」能挑出厂的每一家，挑的只管搜索；`web_fetch` 抓不下来的一直交 Exa、Firecrawl、Parallel。改成选供应商：自动（免费额度）照旧，选了 DeepSeek 的搜索走 DeepSeek Search、代抓走 DeepSeek Fetch（新加），key 和模型那边的 DeepSeek 供应商共用。

### 蓝图改哪几节

- `config.md`：`net.search.source` 那一行（选项只剩 `auto`、`deepseek`、`tavily`、`brave`、`searxng`，扩展的来源照旧接在后面）、几家 key 那几行、给人看的字那张表、嵌的样本块。键不改名：头、扩展清单（`SEARCH_SOURCE`）都认这个键，改名只换字。
- `tools/web_search.md`「怎么走」第 3 条：选了的那一家用不了（没配 key、限额、余额不足、服务忙、超时）照自动的顺序接着试；5xx 回 JSON 的、DeepSeek 回 `error` 的也当出错。
- `tools/web_fetch.md`「怎么走」新加第 4.1 条：选了带代抓的服务的，自己抓之前先交它（交出去以前照自己抓的那几道闸先查），它没成再自己抓，再照第 12 条的免费名单；第 12 条写 DeepSeek Fetch 怎么问、怎么读。
- `net.md` 的「在哪」；协议参考 `extensions.md`、`extensions.en.md` 和写扩展的 `manifest.md`、`manifest.en.md` 里下拉的名字。

### 怎么做的

1. `search.json` 的出厂顺序 `order` 拿掉 DeepSeek（自动不花钱，项目主人 2026-11-14 看过施工后定）：有 key 也不进自动，只在选了它时用，搜索、代抓都是。DeepSeek 多一格 `fetch_url`（只有 DeepSeek 认，不能是空的）；`Source::fetches` 认它。自动的代抓名单 `fetch.order` 不动，DeepSeek 不进去。
2. `web_fetch` 的先后：选了的那一家能抓页面、用得上（要 key 的配了，同搜索的 `usable`）的，自己抓之前先交它（`Search::fetch_by_provider`）；交出去以前 `Pages::before_others` 照自己抓的那几道先查（不是网址、不是 http、内部域名、地址闸，本机解析不出来又像内网名字的也不交），记着的照记着的给。它没成的自己抓，再照 `fetch.order`（`Search::fetch`，问过的那一家不再问）。自动、选了只管搜索的照旧：先自己抓，抓不下来照 `fetch.order`。DeepSeek Fetch 是 REST：`POST {"url"}`、`Authorization: Bearer`；读 `content`、`title`（空串当没有），回 `error` 的照原话认；不是 http、https，超过 8192 个字，带用户名密码的不去问。冷却和搜索共用同一个编号 `deepseek`。
3. 搜索的先后原来就是「选了的最前、用不了的往下试」，不改。顺手修了两处会让 DeepSeek 失败被当成「没搜到」的：REST 的 5xx 不管回的是什么都当出错（原来回 JSON 的照样读，DeepSeek 503 读成零条）；DeepSeek 搜索回 `{"error": …}`（key 不对这些）照原话认。
4. 三语的字：名字「联网服务」「Web provider」「ウェブプロバイダー」，自动写「自动（免费额度）」；几家 key 的说明跟着改；扩展能力的摘要里的下拉名跟着改。配置样本重新生成。
5. 给她看的字不加：结果里是哪一家照旧写在给人看的那一句的 `via`。

### 不做什么

- 不给 Tavily、Brave、SearXNG 加代抓：选了它们的，代抓照自动的名单。
- 不改扩展的来源：照旧接在下拉后面，选了的只管搜索。

### 验收

1. `cargo test -p miyu-net --test all net_service`：选了 DeepSeek 的搜索、代抓都打到 DeepSeek，key 是模型那边 DeepSeek 供应商的，代抓的正文是 `{"url"}`；DeepSeek 回 429、402、503 的搜、抓都退回 Exa；搜索报了限额以后抓也不问它；自动的免费几家都不成、模型那边配了 DeepSeek 也一次不问它；没 key 的跳过 DeepSeek；选了 DeepSeek 时第一次请求就交 DeepSeek Fetch、本机直抓没被调用，它失败了才自己抓、自己抓成了不找免费的几家。改前红：退回修复前的 `pages.rs`、`rest.rs`、`sources.rs`，`deepseek_searches_and_fetches_with_the_model_provider_key`、`deepseek_failing_falls_back_to_the_free_ones` 两条红（代抓去了 Exa；503 时搜索交回「DeepSeek，零条」）。`auto_never_asks_deepseek` 在 DeepSeek 还在出厂顺序里时红（免费的都不成时自动问了 DeepSeek）；`a_picked_provider_fetches_before_her_own_try`、`her_own_try_comes_after_the_provider_fails` 在先自己抓的时候红（本机直抓先成了，DeepSeek Fetch 一次没问）。
2. `search/pages/tests.rs`：DeepSeek Fetch 的请求、读法、不去问的几种网址；`search/rules/tests.rs`：`fetch_url` 写给别家、写空的读不了。
3. `settings/tests.rs`：下拉里是自动和 `search.json` 里要 key、要地址的几家。`miyu-core` 的 `kit/tests.rs`：DeepSeek 限额以后下拉写冷却到几点。
4. 整个工作区的测试、`random::` 长跑、clippy（含 Windows 目标）、fmt、doc、`cargo xtask prompts`、`cargo xtask logs`。

### 风险

1. 选了 DeepSeek、它抓回来的正文太短（登录墙）的，照别的失败一样再自己抓、再走自动的名单；DeepSeek 照官方只在拿到内容时收钱。
2. 选了 DeepSeek 以后，自己抓得下来的页面也先交 DeepSeek，每页按次收费；网址照自己抓的那几道闸先查，内网、内部域名的不会交出去。
