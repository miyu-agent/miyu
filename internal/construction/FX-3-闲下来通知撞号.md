## 施工单 FX-3：会话时钟停住时，第二次「空了告诉我」不再被当成重发吞掉

状态：做完，等合（分支 `step/FX-3` 上）。来源：LiteMiyu 会话 2026-11-13 转来（LiteMiyu 上证实过、修在它的 `4cf6aef`），先在 Miyu 上证实再修。FX-2 施工单第二节留着「另行再看」的就是这一处。

### 目的

等的一边会话时钟停住（日志里的时刻在未来、系统时间往回拨了）时，先后两次订同一个会话，两次都等得到「空了」的通知、都叫醒她。

### 蓝图改哪几节

- `cross-session.md` 第六条第 3、6、9 款（订带着哪一次订、通知的编号），「起草时定的」第 53 条（改写编号，理由不变，补上时钟停住的那一种），新加第 67 条记这一步，「守着它的」两行。
- `kernel/history.md`「在等的通知」第 1 条（一次订多记那条结果的序号，`watching()` 交回序号和起算时刻）。
- `kernel/events-bodies.md` `peer.idle` 的 `cause`。
- `session/tools.md`「1c2. 订、计时、再订」第 2 条。
- `session/actor.md`「被等的名单」第 1、2、4 条。

协议参考（`docs/reference/`）没写这个编号的形状，不用改。

### 证实

通知的命令编号是 `<被等的会话>/idle/<等的会话>/<起算毫秒>`，起算毫秒是订的那条工具结果的 `at`，也就是等的一边会话时钟的读数。会话时钟不往回走：载入时日志最后一条比系统时间晚，它就一直停在那一毫秒。这时先后两次订同一个会话，两次的起算时刻一样，第二次的通知和第一次同一个编号，等的一边的内核照编号只生效一次，照上一次回应（`Recapped`），不记 `peer.idle`、不叫醒，她一直等到 12 小时作废。执行器那一头也照起算时刻认「新多出来的」，时钟停住时在等的过程中又订一次也认不出来。

新测试 `miyu-endpoint` 的 `watch::she_hears_back_each_time_while_her_clock_stands_still`：A 的日志挪到 2100 年再载入（照 FX-2 `goal_rounds` 的做法），A 先后两次订 B、B 各做一轮。改前红：第一次照常叫醒，第二次 B 做完以后六十秒内没等到「A 被第 2 次的通知叫醒」（本机跑了两遍，都红在第 2 次）。

### 修法

一次订的身份换成记下它的那条工具结果的序号。序号由内核追加时分配，一个会话里不重复、不跳号（`02-内核.md` 不变量 1），本来就在日志里：载入、恢复撤销以后照日志再订，还是同一个序号。

- 账本（`ledger/peers.rs`）每次订多记那条结果的 `seq`；`watching()` 交回 `PeerWatching { seq, since }`。
- 执行器（`miyu-session/src/peers.rs`）照整个 `PeerWatching` 比，序号变了就是新多出来的；经 `SessionPort::watch(被等的, 等的, PeerWatching)`、`Handle::watch(等的, PeerWatching)` 交给被等的那一边的 actor。
- 被等的那一边（`actor/watchers.rs`）名单上记着 `PeerWatching`，上膛照旧看起算时刻，通知的编号是 `<被等的会话>/idle/<等的会话>/<订的那条结果的序号>`。

同一次订再交一遍（载入、恢复撤销以后再订、核心重启）编号不变，那边照旧认得出是重的，第 53 条的用意不变。类型叫 `PeerWatching`，不叫 `Watching`：`miyu-session` 里头订阅会话的那一样已经叫 `Watching`。编号最长 36 + 6 + 36 + 1 + 20 = 99 字节，在命令编号的 128 字节以内。

**没选**：照施工时的建议每次订造一个 UUIDv7（`miyu_session::new_id`）存进订的记录。订的记录就是效果 `peer.watch`，要给它加一格：事件格式、样本、推给头的效果都跟着变，内核又不造随机数，得在执行器把效果换成内核的那一步盖上。序号不用改任何格式，同样落盘、同样唯一、同样在重发时不变。

### 不做什么

- 不改事件、效果、协议的格式。
- 不写旧版兼容：没发版。改前在路上的通知（编号带毫秒）重启以后不会再交，等的一边载入时照新编号再订。

### 验收

1. `cargo test -p miyu-endpoint --test all watch::`：新测试改前红、改后绿；旧的四条照过。
2. `cargo test -p miyu-session --test all watch::`：载入以后再订的是同一次（序号、时刻都不变）；通知的编号是订的那条结果的序号；新测试 `the_notice_id_follows_the_watch_not_the_moment`：起算时刻一样的三次订（序号 5、5、9），同一次订再交一遍是同一个编号，又订了一次是新的。
3. `cargo test -p miyu-kernel --lib ledger::tests::peers`：`watching()` 交回订的那条结果的序号，又订一次换成新的序号，撤掉又订回到前一次的序号；新测试 `a_watch_at_the_same_moment_is_told_apart_by_its_seq`：等到过以后在同一个时刻又订，序号不一样。
4. `miyu-kernel`、`miyu-session`、`miyu-endpoint`、`miyu` 全套测试，`miyu-kernel` 的 `random::` 连 ignore 的一起；clippy（含 Windows 目标）、`fmt --check`、`doc -D warnings`、`xtask prompts`。

### 风险

- `SessionPort::watch`、`Handle::watch` 改了参数：只有核心里的会话表和测试在用，头不碰。
