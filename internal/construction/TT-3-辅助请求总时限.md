## 施工单 TT-3：辅助请求最多等 180 秒，重起标题失败告诉头

状态：做完，等合。

### 目的

起标题、回顾这类辅助请求和对话请求走同一条路由，只有空闲超时（连续 180 秒收不到新的内容，思考程度 high 2 倍、max 4 倍）和它十倍的总时限：模型一直慢慢吐字就一直等。人敲不带字的 `/rename`（`session.retitle`，施工 TT-1）没起成时，头什么都收不到，界面一直没动静。项目主人 2026-11-13 定：

1. 起标题、回顾这类辅助请求总时限 180 秒，从发出去起算，到时没说完当失败，照辅助请求失败的那一条路了结。对话的主请求不变。常量，不进配置。core1 同日定：提前压的摘要不管（大上下文、慢模型常超过 3 分钟，卡掉了回合反而多等），照用途的名单只管 `title`、`recap`，以后新加的辅助请求默认不管，要管的写进名单。
2. 人要的重起标题没起成（超时、出错、取不出标题）告诉头，标题照旧不变。头显示的字放核心的 `human` 资源：中文「更改标题失败」，英文 "Failed to change the title"，日文「タイトルの変更に失敗しました」。自动起的没起成照旧不吵人。
3. 回顾没写成，头那边本来就有反馈（`session.recap` 拒 `recap_failed`），不动。

core1 同日并进来一件：主人本机 `/rename` 起出来的标题是「(The model asked for a tool this session doesn't h…」。起标题的请求里没有工具，模型要了一件，这句占位的字跟着回复交了回来，被当成了标题。核心几个仓库里都没有这句字：是端点（那一次走的网关）把模型要的工具换成的。改成：起标题、回顾的回复里有工具调用的一律算没写成。

### 蓝图改哪几节

- `session/actor.md`「7. 请求模型」第 7 条：多「总时限」一段。`models.md`「怎么走」第十一条第 5 条：补一句辅助请求另有总时限、不跟着档位放大。
- `kernel/session.md`「回顾」第 7 条、「起标题」第 7 条：说完了不取有工具调用的回复的正文；没写成的几种多「到了辅助请求的总时限」「要了工具」。「起标题」第 9 条多最后一款「没起成的告诉头」。「守着它的」`scenario/retitle.rs` 那一行。
- `kernel/events.md`：瞬时事件十一种，多 `session.retitle_failed` 一行。
- `view.md`：只在视图流里有的旁白多一种，`notice` 的 `what` 多 `retitle_failed`。
- `protocol.md` 的 `session.retitle` 第 7 条；「守着它的」多 `retitle_failed.rs` 一行。
- 协议参考 `sessions`、`events`、`view` 中英两份。

### 怎么改的

- 总时限放在会话 actor（`crates/miyu-session/src/actor/model/overdue.rs`），不在路由：哪种端口都管得到（扩展驱动、测试的剧本），档位池里换人、退会话的模型也算在同一个时限里。只管名单 `BOUNDED`（`title`、`recap`）里的用途；名单里的，actor 交给端口时起一个定时的任务，到 `ModelPort::aside_limit()`（出厂 `port::ASIDE_LIMIT`，180 秒；只有测试的剧本 `Script::aside_limit` 设短的）还在路上的，叫停端口、记一行 `… failed`，送进内核 `AsideEnded`，错是 `retryable`、「总超时：180 秒还没有说完」（同路由的总超时）。到时已经说完了、被同一种用途的下一次换掉了的，照交给端口的那一刻认不出来，不理，也不算一次动静。
- 内核（`crates/miyu-kernel/src/session/title.rs`）：起标题说完了没起成、「人要了」还在的，同一批出 `PushTransient`：`session.retitle_failed`，`body` 是 `{error, human}`（`human` 是 `core/sessions/retitle-failed`，同工具结果给人看的那一句的写法，头照 `human.get` 换成字），`by` 是要它的人，不带回合编号、没有 `cause`。要了以后人又改了名的（「人要了」已经放下）不推。
- 内核（`crates/miyu-kernel/src/session/aside.rs`）：回顾、起标题共用的收尾里，回复有工具调用的不取正文，记 `empty_reply`，原话后面接 `, it asked for a tool`。提前压的摘要本来就照「调了工具」另走隔离式（`compaction.md` 第三条第 7 条），不动。
- 视图（`crates/miyu-view`）：`session.retitle_failed` 投成一条只在视图流里有的旁白 `retitle_failed {text, error}`，`text` 照连接的语言；导出不写。
- 字：`resources/core/human/{zh,en,ja}.json` 的 `said` 多 `sessions/retitle-failed`。

### 不做什么

- 不加配置项，不改主请求的超时，不改路由。
- 提前压的摘要不受总时限管（core1 2026-11-13 定），以后新加的辅助请求默认也不管。
- 不做网关占位字的文字识别（core1 2026-11-13 同意）：核心看到的只有字，那句字是网关定的，写死它就是写死别人的字。要了工具的都算没写成，就盖住了这一次。网关只交那句字、不交工具调用的，核心认不出，照旧当标题。
- 回顾没写成的反馈本来就有（`recap_failed`），不另推。

### 验收

1. `cargo test -p miyu-kernel --lib retitle`：出错、空闲超时、空白、要了工具（带着占位字）的重起推一条 `session.retitle_failed`（`by` 是人、分类对得上、`human` 的编号），标题不动；起成了的、自动起的没起成的、要了以后人又改了名的不推；自动起的回复要了工具不当标题、下一轮再起。改之前前两条红（`TransientBody::RetitleFailed` 不存在），要了工具的那两条在只加推送、不改收尾时红（占位字成了标题）。
2. `cargo test -p miyu-kernel --lib a_failed_recap`：回顾的回复要了工具算没写成，拒 `recap_failed`。改收尾之前红。
3. `cargo test -p miyu-kernel --test transient_sample`：`session.retitle_failed` 的样本一字不差。
4. `cargo test -p miyu-endpoint --test all retitle_failed::`：总时限设 300 毫秒，起标题一直不说完的到时叫停端口、推失败（`retryable`、「总超时：0.3 秒还没有说完」），视图流旁白是「更改标题失败」，`model.called` 照出错记，标题不动；出错的推、起成了的不推，过了总时限也不当超时；自动起的超时不推、下一轮照试；回顾到时拒 `recap_failed`；回复要了工具的推失败、标题不动。改之前全红。
5. `cargo test -p miyu-session --test all prepare::`：总时限设 100 毫秒，提前压的摘要 400 毫秒才写完，照常说完、到线换上，不再请求。只按用途名单计时之前红（摘要被当超时扔掉，到线当场压，剧本没排那一次）。
6. 手测（终端，接上以后）：说一句、等她答完，敲不带字的 `/rename`；模型 180 秒内没写完的，头上出「更改标题失败」，标题不变。

### 风险

- 提前压的摘要排除在外（core1 2026-11-13 定）：它照旧只有路由的空闲超时和十倍总时限，慢模型上一直慢慢写也一直等，和改之前一样。
- 到时叫停以后，端口在叫停前一刻交出的一两段增量可能晚到：内核那一次已经了结，不理；同一种用途、同一个名字的下一次紧跟着发出去才会串进去，和原来「换掉」的情况一样。
