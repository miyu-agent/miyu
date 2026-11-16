## 施工 MC-1：换了模型当场报新的

状态：待验收（2026-11-16）。项目主人 2026-11-16 定「当场报新的」，主会话转来。

### 现状

`session.configure` 换了模型，照 8-10「下一轮生效」：`subscribe` 回应的 `model`、`model.changed` 推送要到下一个回合开始（`Actor::begin` → `ModelPort::turn` → `hand()`，`crates/miyu-session/src/actor/model.rs`）才变。两个头选完当场照选的显示，但选完不说话、切到别的会话再切回来（或者重连）时重新 `subscribe` 拿到的是旧模型，画回旧的，到下一轮才又变新的。

QU-2修（main 6b79cd048）在 `crates/miyu-endpoint/src/quota.rs` 另算了一份「接下来请求的那一家」，`subscribe` 没跟着；同一个想法写了两处。

### 定了的

1. 显示当场变，请求下一轮变：configure 换了模型、思考强度以后，`subscribe` 回应、`view.status` 的 `model` 和 `model.changed` 当场照会话记着的报；真正发请求照旧下一个回合开始（路由的重新解析、池的指针、钉着的成员不提前动）。头不用改。
2. 算法收到一处：`miyu-session` 的 `route/upcoming.rs`（路由预演下一个回合开始会怎么解析），actor 落盘前问它、写进给头看的那一份（`Shown::foreseen`）；`subscribe`、`view.status`、`model.changed`、`session.quota` 都读 `Handle::next`，不另算。QU-2修 在 `quota.rs` 的 `upcoming` 删掉。
3. 解析同回合开始的那一份（`route/turn.rs` 拆出 `settle`，回合开始和预演共用）：改过名的照新编号，解析不出的退回 `models.chat`。只查，不记、不改钉着的。
4. 换成模型的带 `endpoint`、`model`；`effort` 照这一刻的配置和会话记的一档算（同 `Route::shown_effort`，SM-1 的口径），算不出的不带。换成池的只有 `ref`（照「轮换的池、解析不出的没有」）。和钉着的一样的照钉着的（钉住的池照钉着的成员）。
5. 推送时机：带 `model` 或 `effort` 的 `session.policy_changed` 落盘以前（同换上下文上限，回了的命令、之后的 `subscribe` 读得到新的），给头看的变了推一条，`why` 是 `turn`（协议形状不变，同 CT-1 换快照时推的）。下一个回合开始真解析前清掉预演的，和刚推的一样不再推；不一样的（池钉了成员、窗口变了、解析不出退回了）照旧推。
6. 限额（窗口、压缩线）不预演，下一个回合开始照旧换：那是内核判到线用的那一条。
7. 预演着的时候这一轮里出错换了成员，不推 `failover`：头看的是下一轮的。

### 技术上定的（按推荐、留痕）

- `why` 用 `turn`，不加新值：协议形状不变，头不用改；同 CT-1 换快照时的先例。
- 思考强度只换（没换模型）也当场报：同一个 `model.changed`，同一类毛病（选完切走再切回来画回旧的一档），不另立规矩。原来 `route_effort_session.rs` 断言「下一个回合起生效」的那一句改成当场。
- 人格、预设现在不带来模型变化（预设只在限额的事实里写编号），不另接；以后带了，照同一处（带 `model` 的 `session.policy_changed`）。
- 配置改了 `models.chat`、跟着它的会话：不在这一步，照旧下一个回合开始才换（8-10）。
- 预演照这一刻的配置：落盘前另取一份（`Turning::peek`，照会话目录带项目配置读一次），不换这一轮冻结的那一份。刚在设置页加了一家供应商、马上换过去的也认得。
- `port.rs` 到了 500 行，`Cancel` 挪进 `port/cancel.rs`。

### 验收

1. 测试（改前红）：
   - `miyu-session` 的 `tests/route_turn.rs` 新两条：`a_new_model_is_shown_at_once_and_sent_from_the_next_turn`（开会话用 `a/m`、换 `b/n` 不说话：`Handle::next` 是 `b/n`、推一条 `model.changed` 写 `b/n`；说一轮请求发给 `b`、不再推；换回去当场照钉着的）、`a_pinned_pool_is_shown_at_once_and_its_member_after_the_next_turn`（换成钉住的池：当场只有 `ref`、没有 `endpoint`/`model`；说一轮照钉的成员推一条）。
   - `miyu-endpoint` 的 `tests/models_pools.rs` 的 `subscribe_says_which_model_the_session_asks_next` 多一段：换了模型先推一条 `model.changed`，再订阅的 `model` 是换成的。
   - 跟着改的：`route_turn.rs` 的 `a_new_model_is_used_from_the_next_turn_and_announced_once`（原来断言下一个回合开始才换）、`route_effort.rs` 的 `a_request_takes_the_configured_default_or_nothing`（原来等回合开始那一条推送）、`route_effort_session.rs` 的 `one_session_s_level_leaves_the_other_and_the_config_alone`（原来断言下一个回合起才显示）。
   - QU-2修 的两条（`session_balance.rs`、`session_quota.rs` 的 `switching_…_before_the_next_turn`）照旧过。
2. 实测：沙箱数据根起核心，真模型两个，开会话、换、重新订阅记 `model` 原文，说一轮看请求发给谁、`model.changed` 推了几次；终端头上换模型、切到别的会话再切回来看底栏。

### 施工结果

1. **测试**：退回改前的源码（测试留着），新两条、`models_pools` 那一段、跟着改的三条都红（`Handle::next` 还是 `a/m`、一条 `model.changed` 都没推）；改后 `miyu-session` 的 `route_` 91 条、`miyu-endpoint` 的 `models_`、`session_balance`、`session_quota` 40 条都过，QU-2修 两条照旧过。
2. **实测**（沙箱数据根，magpie 网关同一个模型配成 `dev`、`dev2` 两家，另配钉住的池 `@both` = `dev2`、`dev`；协议原文）：
   - 开会话用 `dev/…`，说一轮：请求 `endpoint=dev`。
   - `session.configure` 换 `dev2/…`、不说话：回应前推来 1 条 `model.changed` `{"ref":"dev2/marvis/deepseek-v4-1-flash-261017","endpoint":"dev2","model":"marvis/deepseek-v4-1-flash-261017","limits":{…},"why":"turn"}`；另开一条连接重新订阅，`model` 是 `{"endpoint":"dev2","model":"marvis/deepseek-v4-1-flash-261017","ref":"dev2/…"}`。说一轮：请求 `endpoint=dev2`，这一轮推来 0 条。
   - 换 `@both`：当场 1 条，只有 `ref: "@both"`、没有 `endpoint`/`model`；重新订阅的 `model` 是 `{"ref":"@both"}`。说一轮：请求 `endpoint=dev2`，这一轮推来 1 条（`endpoint: "dev2"`，钉的成员）。
   - 终端头（miyu-tui main 65d0ab79 编的，连沙箱核心）：说一轮、`/model` 换到 `marvis/claude-sonnet-4-6`、不说话，`/sessions` 切到另一个会话再切回来，底栏是 `marvis/claude-sonnet-4-6 dev`（改前切回来画回旧的）。
