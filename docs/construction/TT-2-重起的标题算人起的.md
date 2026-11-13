## 施工单 TT-2：重起的标题算人起的

状态：做完，等合。

### 目的

终端的会话报：不带字的 `/rename`（`session.retitle`，施工 TT-1）重起的标题，核心记成内核起的（`session.meta_changed` 的 `by` 是内核）；终端只把人起的标题画在输入框右上方的框线上，重开会话就不画了。core1 2026-11-13 定：这个标题是人亲手敲 `/rename` 要的，算人起的，和 `session.set_meta` 写的一样：`session.list`、会话列表的推送、视图都照人起的给，以后自动起标题也不会盖掉它。不另加格。

### 蓝图改哪几节

- `kernel/session.md`「起标题」第 9 条：记的两条不再写「`by` 内核」；多一条「起好的标题算要它的人起的」：`session.meta_changed` 的 `by` 是 `Retitle` 命令的 `by`，`model.called` 照旧是内核的，并进自动起的那一次的、只有续轮拿目标原文的也是；「守着它的」那一行。
- `kernel/events-bodies.md` 的 `session.meta_changed`「谁改的看 `by`」：人要求重起的 `by` 是要它的人。
- `protocol.md`：`session.list` 的 `named` 那一句、`session.retitle` 第 2 条、「守着它的」`retitle.rs` 那一行。`store/index.md` 的 `named` 那一格补一句。`cli/rename.md`「守着它的」。
- `docs/reference/sessions.md`、`sessions.en.md` 的 `session.retitle` 回应那一段。`docs/designs/04-核心协议.md` 第九节「自动起标题」那一条补一句。

### 怎么改的

- 「谁起的」只看 `session.meta_changed` 的 `by`：会话列表的索引照最后一条写了标题的那一条的 `by` 是不是人算 `named`（`miyu-store` 的 `index/row.rs`，施工 3-8 八补），列表、推送、重启以后都读它。所以只改内核记的那一条，别处不动。
- 内核（`crates/miyu-kernel/src/session/title.rs`）：起标题的账里「人要了」从布尔换成记着要它的人（`Option<By>`，照旧只在内存里），`Retitle` 命令带上发命令的 `by`；起好了照它记 `session.meta_changed`（不带回合编号、没有 `cause`，`record_outside`），没有的照旧是内核。只有续轮拿目标原文当场记的那一条也一样。`model.called` 不动。
- 不带 `cause`：TT-1 定的回应不附事件、命令收下就回；带了 `cause`，载入时照 `cause` 认回命令编号会附上这一条，活着时却是空的，两边对不上。
- 以后自动起的不盖：「标题动过」本来就照任何一条写了标题的 `session.meta_changed` 算，载入时照日志算回来，不用改。

### 不做什么

- 不加格，不改 `model.called` 的 `by`，不改起标题的请求（一个字节不变）。
- 以前记成内核起的老标题不补改：日志只追加。

### 验收

1. `cargo test -p miyu-kernel --lib retitle`：原有六条改成标题那条是人的，加两条：还没自动起过就要了的算人起的，以后答完的轮、重启以后都不再自动起；并进在路上的自动起的那一次，起好的也算人起的。`cargo test -p miyu-kernel --lib retitling_a_goal`：只有续轮的会话要了，目标原文那一条 `by` 是人。改之前这六条红、`asked_during…` 那条红，没有标题的两条照过。
2. `cargo test -p miyu-endpoint --test all retitle::`：推来的 `session.meta_changed` 的 `by` 是 `{"kind": "person"}`，`session.list` 带 `named: true`；换一份核心（同一个数据根，像重启过）以后照样，再答完一轮不再起标题；人起过名的换掉以后照样 `named`。改之前头两条红。
3. `cargo test -p miyu-cli rename`：不带标题的那一条，日志里那条 `by` 是人。改之前红。
4. 手测（终端）：说一句、等她答完，敲不带字的 `/rename`，标题到了以后切到别的会话再切回来，输入框右上方的框线上照样画着这个标题；停掉核心再起来、重开这个会话，照样画着。

### 风险

- 扩展经 `sessions.drive` 要的重起，`by` 是那个扩展（`harness`），不算人起的：照 `session.set_meta` 一样，可以接受。
