## 施工单 VW-2：场所会话的「查看会话」

状态：做完，等合（分支 `step/CB-1`）。项目主人 2026-11-17 本机验收第 15、27 条，网页在等。

### 为什么

主人验收 im 卡片的「查看会话」：群聊太简陋，只显示「外部」、看不出谁发的；群聊并行时看不到支线；场所会话的标题 im 已经用 `session.set_meta` 设成群名、对方的名字（日志里有 `session.meta_changed`），可场所会话不在 `session.list` 里，订阅回应、视图流都没有标题，网页只能写「未命名会话」。

### 做什么

1. 视图流 `user` 条目的 `from` 多一格 `name`：那条消息的 `venue.name`，只当显示名、不可信，头照常转义（网页画「昵称（qq:号）」）。没写名字的不带。`from` 照旧是 `by` 的写法；别的 harness 的 `by` 自己就有 `name`，不盖（`miyu-view` 的 `entry/sender.rs`）。
2. 场所会话的并行支线（桥照 `session.fork {seal_after: 1}` 分出去的）像子代理一样，作为主线会话状态 `jobs` 里的一项：`{job, what: "agent", fork: true, session, title, state, started, ended?}`，`job`、`session` 都是支线编号；`state` 是 `running`（在跑、第一轮还没跑完）、`done`、`stopped`（被打断）。网页右侧边栏现成读 `jobs`，不用改。
   - 主线日志里没有分叉的记录，场所会话又不进会话列表的索引：订阅时会话从新到旧读第一条认（父会话是它、`forked` 从它分出来、不是 fork 子代理），比主线早的不看，认出 40 条或读了 2000 个就停；之后会话列表报哪个会话动了，是它的支线才重读那一条（`miyu-endpoint` 的 `view/branches.rs`）。本机会话不量。
   - 留几条：在跑的都列，做完的只留最近的 20 条，同任务表做完的留 20 个（按推荐定）。量过：一条约 250 到 400 字节（标题最长 50 个字），主群一天 46 条全列约 15 KB、整份状态每变一次就推一份；留 20 条约 5 到 8 KB。
3. 标题：订阅回应（`events`、`view` 两种流）和会话状态 `view.status` 带 `title`（起了的才有），照会话列表的算法取；之后照落了盘的 `session.meta_changed` 换，变了推整份状态。
4. 文档：蓝图 `view.md`（条目 `user`、会话状态 `jobs`、`title`）；协议参考 `view`、`connection` 中英。

### 不做什么

- 不给场所会话的支线另起推送、另开方法：头照 `jobs` 画、照 `session` 订阅支线的视图流（主人看场所会话本来就能只读地订阅）。
- 本机会话的 `session.fork` 不列进 `jobs`。
- 头不改（网页的「昵称（qq:号）」由网页会话做）。

### 验收

1. 改前红：`miyu-view` `venue_names::a_venue_message_carries_the_sender_name`（`from` 没有 `name`）；`miyu-endpoint` `venue_view::a_venue_view_shows_names_branches_and_the_title`（订阅回应没有 `title`，`jobs` 里没有支线）。
2. `view::branches::tests` 两条：认支线（别的会话分出来的、fork 子代理不算；`done`、`stopped`、`running`；标题照第一句话的头一行）；46 条里做完的只留最近 20 条、在跑的都在，列出来的不到 8 KB。
3. 门禁十一项、`random::` 长跑、Windows 版 clippy、`cargo xtask prompts`、`cargo xtask logs`。
4. 实测：沙箱数据根装假桥，造一个群会话，旁听一条、两条 fork 支线各跑一轮，`session.set_meta` 设群名；`subscribe {stream: "view"}` 看 `title`、`status.jobs`、`from.name`；网页头开这个会话看右侧边栏和标题。

### 风险

- 订阅一个很老的群主线时，比它新的会话多（别的群的支线夹在中间），最多读 2000 个会话的第一条：一次几十毫秒级，只在订阅时。
- 支线的标题是第一句话的头一行（群消息原文）：im 给支线起了标题的照标题。
