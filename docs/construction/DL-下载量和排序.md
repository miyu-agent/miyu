# DL：下载量和排序

设计 `54-下载量和排序.md`（2026-10-31 项目主人定：最新、下载量、趋势三种排序都做）。三步：DL-1 核心读、交；DL-2 `miyu-index` 生成；DL-3 软件源仓库 `miyu-agent/packages` 的 Actions 带上 `--github`。头（网页、终端）照 `package.search` 的三格排。

## DL-1：核心认三格、`package.search` 交出去

### 目的

- 索引的每一条多 `published`（这一版第一次进这个软件源的时刻）、`downloads`（各版本、各系统一共被下载了几次）、`downloads_week`（近 7 天）。
- 以前生成的索引没有；写法不对的（不是 RFC 3339 的时刻、不是非负整数）读成没有，那一条照收，不让整份索引读不成。
- `package.search` 每一项有值的才写这三格。

### 怎么做

1. `miyu-source` 的 `index::Entry` 多三格，读的时候各用一个宽松的读法（`time_or_none`、`count_or_none`），没有的不写回去。
2. 收录（`collect.rs`）造条目时三格先空着，DL-2 照上一份索引、GitHub 填。
3. 端点 `packages/source/search.rs` 照索引写进每一行。

### 验收

1. `miyu-source` 的 `index::tests::published_and_download_counts_are_read_leniently`：写对的读成，写错的（「yesterday」、负数、小数）读成没有、那一条照收，以前生成的没有；没有的不写回去。
2. 端点 `packages_search::published_and_downloads_are_handed_over_when_the_index_has_them`：有的交出去，写法不对的、没有的不写；修前红（三格都是 `null`），修后绿。
3. 手写变异、本地门禁、三平台 CI。

## DL-2：`miyu-index` 生成三格

### 目的

- 写 `index.json` 之前读输出目录里的上一份：编号、版本没变、上一份有 `published` 的照抄，新包、升了版的、上一份没有这一格的写这一次生成的时刻。`build`、`folder`（自建的软件源）都这样；上一份读不成的从这一次算起，记一笔。
- `build --github`：照每个包的来源仓库（`origin`，`github.com/<主>/<仓库>`）列它全部的发布，文件名是 `<编号>-<数字开头的版本>[-<系统>].miyu` 的 `download_count` 加起来（旧版不在索引里，照发布页上的全部算；`memes-extra-1.0.miyu` 不算 `memes` 的）。和设计 54 字面「照每个文件的地址认」不同：收录的文件本来就来自那个仓库的发布，结果一样（core1 同意）。
- `stats.json` 和 `index.json` 放在一处：`{"version": 1, "days": [{"date": "2026-11-02", "downloads": {"<编号>": 42}}]}`，日期照 UTC，只留最近 8 天，同一天的换掉；`downloads_week` 是今天的减 7 天前那一天的（那天没记、没有这个包的不写，少了的是 0）。读不成、版本不认得的从头攒。不签名。
- 令牌照环境变量 `GITHUB_TOKEN`（只发给 API 的 `/repos/` 下面，没有的照匿名，一小时 60 次），不进输出、不进 `stats.json`。问不成的（限流、网络错、仓库没了）那个包照上一份索引同编号的 `downloads`、`downloads_week` 沿用（不写成 0），不记进今天的 `stats.json`；上一份也没有的不写；一个都问不成也照样生成。

### 怎么做

1. `miyu-source` 新模块 `stats`：`carry_published`、`downloads`（翻页，一页 100 个，最多 10 页）、`Stats`（`parse`、`record`、`week`）、`count`（把上面连起来写进索引）。
2. `miyu-index`：`build` 多 `--github`；`build`、`folder` 写之前读上一份（`previous`），`write_out` 里沿用 `published`；`--github` 的数完写 `stats.json`。

### 验收

1. `stats::tests`：沿用 `published`（版本没变照抄，升了版、新包、上一份没有的写这次的时刻，没有上一份的）；下载次数各版本各系统加起来、别的文件不算、满一页翻下一页、不在 GitHub 上的和拿不到的报错；`stats.json` 只留 8 天、去掉比今天晚的和写错的、近 7 天照 7 天前那天减、少了是 0、版本不认的不读；`count` 问成的照这次、问不成的照上一份沿用且不记进今天、上一份也没有的不写。
2. `miyu_index_build::build_with_github_carries_published_over_and_counts_downloads`：真跑 `build --github`（假服务器冒充 GitHub），沿用上一份的 `published`、下载次数 42、近 7 天 30、`stats.json` 两天、列发布带令牌、令牌不进输出和 `stats.json`；去掉列发布的路由再跑，照上一份沿用、退出码 0；不带 `--github` 的到空目录只有 `published`（这次的时刻），不问发布、不写 `stats.json`。修前红（`--github` 是参数不对，退出码 2）。
3. 手写变异 11 个全抓住；本地门禁、三平台 CI。
