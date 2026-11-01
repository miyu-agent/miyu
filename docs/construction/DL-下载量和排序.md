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
