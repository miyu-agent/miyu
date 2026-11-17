## 施工 VP-1补：视图流带上戳一戳

状态：做完，等合（分支 `step/UX-1修`）。主会话 2026-11-17 转来（网页在等）。

### 为什么

VP-1 让场所消息带上 `venue.poke`、`venue.poke_text`，模型那边写成「谁戳了你」，可视图流的 `user` 条目只照 `venue.name` 带了 `from.name`，没交这两格：网页上戳一戳只画出名字、下面空着。

### 改了什么

1. `miyu-view` 的 `User` 条目多两格：`poke`（那条消息的 `venue.poke`，只写 `true`）、`poke_text`（`venue.poke_text`，不可信，原样交，头照常转义；没写的不带）。没戳的两格都不写，以前的条目字节不变；读回来照 `default`。
2. 蓝图 `view.md`「条目」的 `user` 和守着它的表；协议参考 `view` 中英。

### 验收

1. 改前红：`miyu-view` `venue_names::a_poke_carries_its_mark_and_text`（造一条带 `venue.poke`、`poke_text` 的 `message.user`，main 上 `user` 条目没有这两格）。
2. 门禁十一项、`random::` 长跑、Windows 版 clippy、`cargo xtask logs`、`cargo xtask prompts`（给模型的字不变）。

### 风险

- 头要照 `poke`、`poke_text` 画（网页、终端），没画的照旧只出名字。
