## 施工单 V-D：视图的类型给头读回来

状态：已合（2026-10-23）。项目主人 2026-10-22 定终端也换成照视图流画；终端要照门面把条目、推送、会话状态读回来，核心主会话 10-23 派给 clicall 会话。图纸 `view.md`「头读回来」、`client.md`「门面上有什么」。

### 做什么

1. `miyu-view` 里头要读的类型都加 `Deserialize`：`Entry`、`EntryId`、`Body` 和各种条目、`Notice` 和各种旁白、`Change`、`Status` 和它的几格、`NestedItem`。
   - 可以没有的格（空的列表、`false`、0）读的时候缺省；`Option` 的本来就缺省。
   - 不认识的格不管（serde 的默认）：核心加格子，旧的头照样读得懂。
   - 原来写成 `&'static str` 的三格（附件的 `kind`、换模型的 `why`、任务的 `why`）改成 `String`，写出去的字节不变。
2. 内核的 `By`、`RawJson` 照原文读，在 `flatten`、照 `kind` 分派的条目里读不成：那几格经通用的 JSON 转一道（`relay.rs`）。不动内核：内核读日志靠原文那条快路，有性能闸门。
3. 门面加 feature `view`（默认不开，终端打开），`miyu_client::view` 转出这些类型和 `VERSION`。层序：门面在第 3 层，`miyu-view` 在第 2 层，能引。

### 验收

- `miyu-view` 的测试帮手 `same` 把投影交出的每一条条目、每一个推送写成 JSON 再读回来，和原来的一样；会话状态同样。所有测试、三百份随机剧本都走一遍。
- 门面 `tests.rs`：转出的每一样钉住；一段真推送读回来再写一字不差；带不认识的格、缺着可以没有的格的条目、状态读得懂。
- `cargo xtask check` 全过；合了告诉终端提交编号。

### 修（2026-10-23，终端报的）

- `Change::Update` 的 `after` 是两层的 `Option`：`null`（挪到最前）读回来成了「没挪」。serde 读两层的 `Option` 把 `null` 也当没有；那一格改照 `relay::present` 读（没有是 `None`、`null` 是 `Some(None)`）。三百份随机剧本里没出过挪到最前，往返测试没抓到。
- 线上的推送是 `view.<op>` 加参数、不带 `op`，终端原来自己换；加 `Change::read(method, params)` 照线上的写法读回来。
- 测试：`same` 里每个推送另照线上的写法写一遍、用 `Change::read` 读回来比相等；`tests/web.rs` 一条专测挪到最前和没挪。
