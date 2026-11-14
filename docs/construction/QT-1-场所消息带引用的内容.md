## 施工 QT-1：场所消息带被引用的那一条的内容

状态：已完成（2026-11-14）。im 要的：群消息引用了一条不在她会话里的消息时，把那一条的内容带给她。

### 目的

以前她只看得到 `reply-to: msg=X`，不知道 X 说了什么；判官那份另外照日志带了内容，日志里没有的也只有编号。现在桥在场所消息里多带一格 `quoted`（被引用的那一条），她的日志里这一条以前没有 X 的，就在 `reply-to` 那一行下面写出来。

### 蓝图改哪几节

- `venues.md`「场所的格」加一行 `quoted`：形状和 `forwards` 里的一条完全一样 `{name, text, media?, forwards?}`，查法同那一条；里面的转发从第一层数、最多 3 层；它和它里面的转发算进整条消息的 100 条、16 KiB，超了照 `forwards` 拒（`bad_params`，`data.limit` 是 `forward_nodes`、`forward_bytes`）。要带着 `reply_to`，不带的 `bad_params`。判官那一节第 4 条加上 `quoted` 的写法。测试表加一行。
- `kernel/request.md`「群里的一行」加一条：什么时候写、写在哪、怎么缩进，带示例。
- `kernel/events-bodies.md` 的 `venue` 加 `quoted`。
- 协议参考 `venues.md`（中英）：「`venue`」表加 `quoted`，`venue.records` 的回应写上。

### 渲染的规矩

- 只看这一条以前的日志（有效历史里序号比它小的）里有没有一条场所的 `message.user`、`venue.delivered` 的编号是 `reply_to`（同判官认引用的 `quoted_as`，睡着时收到的也算；补拉回来的不算：它不进群聊近况，她那一轮看不到，引用的正好是补拉进来的那一条时照样写出 `quoted`，主会话 2026-11-14 定）。有的照现在的样子，只写编号；没有的在 `reply-to` 那一行下面写，比那一行多缩进 2 格：`名字: 正文 [记号]`，里面的转发接在下面、再缩进一层，写法同合并转发里的一条（`group/forward.rs` 的 `quoted`，和转发共用一段 `node`）。
- 只看「这一条以前」：被引用的那一条后来才记下（补拉回来的），已经写出去的这一行不改，同一份日志每次渲染一字不差。
- 快照里没有转发那几句的（O-34 以前造的会话）不写，同转发；没带 `quoted` 的一字不变。私聊没有 `reply-to` 那一行，不写。
- 不加给模型看的字：写法是代码写的格式，同转发里的一条。
- 群聊近况、回顾、起标题照同一个函数（`group.rs` 的 `unseen`）。
- 判官（`venue.records`）：要判的那一条照旧先找日志里它以前最后一条那个编号的，有的写在编号后面（施工 O-41）；下面写不写 `quoted` 照同她看到的一样的规矩（补拉回来的不算），引用的只有补拉回来的那一条时两样都写。之前的记录里的行同她看到的一行，照那一行以前的日志判（`Picked::unseen`）。有效历史那一路（`records_in`）一条条比。边读边记（`pass.rs`）、记录窗（`recent.rs`）照先后收每一条的时候，手里的「编号的哈希 → 最后一条的序号」正好是它以前的，当场判下来记着（`venues/records/unseen.rs`）：哈希记着的是有；没记着、从没挤掉过的是没有；挤掉过的、记在补拉回来的那几条的哈希里的说不准，问到这一行的照整份读（记录窗记的补拉回来的哈希多过 3584 个的作废）。不照序号读回那一条对编号：先试过「问的时候往回找」，引用的那一条早就挤出留着的几条、编号又有重的时候一问就要读日志，记录窗「第二问不读日志」守不住；64 位的哈希撞上只是少写一段引用的内容（同 QT-1 以前），不值得。

### 不做什么

- 不在记下时替桥判「她那里有没有」、不改记下的：内核原样记平台的格，判在渲染。
- 私聊不渲染（没有 `reply-to` 那一行）。
- 不改判官找引用的那一条的规矩（最后一条那个编号的）。

### 验收

1. `miyu-assemble`（`group/forward/tests/quoted.rs`）：
   - `a_quote_already_in_the_log_is_not_written_again`：之前的日志里有，不写；
   - `a_quote_missing_from_the_log_goes_under_reply_to_with_its_forwards`：没有，写在 `reply-to` 下面，里面的转发展开；
   - `without_a_quote_or_the_forward_texts_nothing_changes`：没带的、O-34 以前造的会话一字不变；
   - `a_quote_recorded_later_does_not_change_the_line`：后来才记下被引用的那一条，这一行不变；同一份日志渲染两次字节相同；
   - `judged_records_follow_the_same_rule`：判官的 `current`、之前的记录照同一个规矩，日志里有的照旧写在编号后面；
   - `a_quote_only_backfilled_is_written_anyway`：被引用的那一条只以补拉回来的样子在日志里，她看到的照样写出引用的内容，判官的 `current` 编号后面写它、下面也写。
2. `miyu-endpoint`（`tests/venue_quoted.rs`）：原样记下；写错的、没写 `reply_to` 的、套 4 层的 `bad_params`；算进整条消息的上限，超了 `forward_nodes`、`forward_bytes`。判官的三条路（有效历史、边读边记、记录窗）答出来一样（`venues/records/tests`）。
3. 改前红：渲染的几条改前没有 `quoted` 这一格（日志读得进、不写）；端点的几条改前 `quoted` 是不认的格（`deny_unknown_fields`，全拒）。
4. 原有的群里一行、转发、判官、记录窗测试一条不改、全过。
5. 门禁全过；Windows 版 clippy 过。

### 风险

- 场所的格多了一格，内核的 `Command::Send` 比别的种类大出一截（clippy `large_enum_variant`）：`venue` 装箱（`Option<Box<VenueMessage>>`），`quoted` 自己也装箱；JSON 的写法不变。

- 每条带 `quoted` 的要往前找一遍日志：只在带了的才找，带了的本来就少。
- 判官的快路说不准时退回整份读：只在记录窗挤掉过编号、留着的几条里又有带 `quoted` 的时候。
- 判官的快路信编号的哈希：撞上的那一行少写一段引用的内容，和照整份读的答案差这一段。
