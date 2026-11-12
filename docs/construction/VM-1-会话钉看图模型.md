## 施工单 VM-1：会话钉自己的看图模型

状态：做完，等合。

### 目的

替看不了图的模型看图（`models.md`「怎么走」第十三条）只照系统一层的 `models.vision`。接 QQ 的 im 在会话卡片上能按会话钉对话模型（经 `session.configure` 的 `model`），项目主人要卡片上也能按会话设「多模态模型」：看图用的那一个（2026-11-11 项目主人要，技术细节主会话定）。

### 蓝图改哪几节

- `models.md`：「怎么走」第十三条开头一段和第 4 条（发给会话钉的，没钉的照 `models.vision`）、第 7 条（钉的解析不出照没成）；「事件」的 `session.policy_changed` 多 `vision`；「协议」的 `session.create`、`session.configure` 多 `vision`；「在哪」`configure.rs`、`route/sight.rs` 两行；「守着它的」多三行。
- `protocol.md`：方法表的 `session.configure`，`session.create`、`session.configure` 两节的参数表和规矩。
- `kernel/events-bodies.md`：`session.policy_changed` 多 `vision` 一格。
- `kernel/session.md`：`configure.rs` 那一行；「换思考强度」后面多「钉看图模型」一节。
- `view.md`：「会话状态」多 `vision` 一格。
- 参考文档 `docs/reference/` 中英两份：`sessions`（`session.create`、`session.configure`）、`view`（会话状态）、`events`（`session.policy_changed`）。样本 `session.policy_changed.jsonl` 加一条。

### 协议

- `session.configure` 多一格 `vision`：字符串或 `null`，可以不写。写法、解析、校验和 `model` 一模一样：`<供应商>/<模型>` 或 `@<池>`，照这时不算项目配置的最终值解析（`miyu_models::reference::record`）；解析不出 `unknown_model`，空字、不是字符串的 `bad_params`，都什么都不记（同一条里写的别的格也不换）。`null` 是去掉，回到跟着 `models.vision`。六格至少写一格。
- `session.create` 多一格 `vision`：字符串，可以不写，写 `null` 等于没写。先解析（解析不出 `unknown_model`、什么都不造），造好以后记一条 `session.policy_changed`，回应的 `events` 多它（带了 `project` 的排在项目那一条后面）。
- `venue.session` 不收：im 换会话的模型走的是 `session.configure`（`miyu-im` 的 `core/route/policy.rs`），`venue.session` 本来就没有 `model`。
- `view.status` 多一格 `vision`：会话钉着的引用，原样（模型或 `@池`），钉了当场推；没钉的不写。权限同 `model`：谁调得了 `session.configure` 谁就能改（扩展要 `sessions.drive`）。

### 怎么走

1. 存在会话的日志里，同 `model`：`session.policy_changed` 最后多一格 `vision`（字符串），空的是去掉；人换的 `by` 是人，`cause` 是 `<session.configure 那条命令>/vision`（`session.create` 带的同样接 `/vision`）。内核新命令 `SetVision { vision }`，和现在的一样的不记；撤掉的回合里的也算，载入照日志算回来（`configure.rs` 的 `Reference`，`Session::vision()`）。换模型不清它。分叉、子代理不带（同上下文上限）。
2. 下一轮生效，同 `model`：路由造端口时照内核从日志算的（`ForSession.vision`），回合开始换成内核这时的（`ModelPort::turn` 多一个参数），记在 `Pinned` 里。一轮里改了的，这一轮剩下的转述照旧；手动压缩、清空单开的那一轮照上一轮的。
3. 替看图（`route/sight.rs`）：钉着的发给它，没钉的照这一轮的 `models.vision`。钉的解析不出（供应商删了、池删了或者空了）走一次性入口的 `unknown_model`，和 `models.vision` 写了解析不出的一样：这张图这一轮写占位、主请求照发、下一轮再试，记 `INFO image not described`；不退回 `models.vision`。
4. 给头看：`Current` 多 `vision`（内核这一刻的，刚钉了、还没开回合的也是钉的），`view.status` 照它写。`subscribe` 回应、`model.changed` 不变。

### 不做什么

- 不加给模型看的字，请求的字节不变（替看图那一次请求本来就不进会话的前缀）。
- 不改 `models.vision` 的写法、不给池加「看图用」的标记。
- `model.call` 的 `use: "vision"` 照旧照 `models.vision`：它不属于哪个会话。
- `session.list` 不加这一格：im 卡片照 `view.status` 或者自己记的。`subscribe` 回应的是 CT-1补 加的（`CT-1-压缩触发线可配.md`「补」）。
- 钉的解析不出时不退回 `models.vision`、内核也不记退回（`model` 那种 `replaced`）：配置的看图模型和钉的不是一回事，悄悄换成别的看图模型人看不出来。

### 验收

1. `cargo test -p miyu-kernel --lib vision_pin`：两条（钉了记一条只写 `vision`、一样的不记、去掉记成空的、换模型不清它；撤掉的回合里的也算、载入照日志算回来）。`cargo test -p miyu-kernel --lib policy_change_records_the_vision`：事件读写一字不差。
2. `cargo test -p miyu-session --test all route_vision_pin`：两条（钉了的替看图发给它、配置的不问，重启以后照日志算的，去掉了回到配置的；钉的池没了的写占位、主请求照发、不退回配置的）。
3. `cargo test -p miyu-endpoint --test all vision_pin`：三条（没钉的照 `models.vision`、`session.configure {vision}` 以后下一轮发给钉的、`null` 回到配置的，`view.status` 钉了带 `vision`、没钉的不写，日志里记的两条；解析不出的几种 `unknown_model`、空字、数、数组 `bad_params`，和 `model` 一起写的也不换；`session.create` 带 `vision` 的回应 `events` 是 `[1, 2]`、第一轮就照钉的、解析不出的什么都不造）。改之前第一条红：`session.configure {session, vision}` 回 `bad_params`。
4. `cargo xtask prompts` 没有差异。

### 风险

- 钉的模型看不了图（`inputs` 没有 `image`）不在钉的时候拒：和 `models.vision` 一样，发出去由一次性入口照它的资料报错，算没成、写占位。
- 头要显示「这时用哪个看图」得自己合：`view.status` 只交钉的，没钉的照 `model.list` 的 `uses.vision`。
