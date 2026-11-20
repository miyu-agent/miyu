## 施工单 CT-1：压缩触发线可配

状态：做完，等合。

### 目的

自动压缩的线写死在窗口的 85%（施工 6-11 再补）。项目主人要能改：系统配置一项，会话上也能单独钉一个数（2026-11-12 项目主人定，todolist 33）。

定了的：

- 写百分比：窗口的百分之几，整数，20 到 95，出厂 85。
- 两层：系统配置一项 `compaction.line`（不进项目配置）；会话上能钉一个数，不钉的跟着系统配置。
- 安全那条减法照留：压缩线 = min(窗口的 N%, 窗口 − 输出预留 − 余量)。20 万窗口调到 85 以上不起作用，百万窗口才到得了 95；这一句写进配置项的说明。

### 别家怎么做

- Claude Code：一个百分比的环境变量，只能往下调。
- Codex：`model_auto_compact_token_limit` 写 token 数，封顶在窗口的 90%。
- opencode：`buffer` 写给回答留的 token 数。

写百分比是因为窗口不同的模型共用一份配置；写 token 数换了模型就不对。没选 token 数写法（项目主人定）。

### 蓝图改哪几节

- `compaction.md` 第二条第 2 条：公式里的 85% 写成 N%，加「N 怎么定」「什么时候生效」两款。
- `policy.md`：快照的 `compaction` 那一行（`line_percent` 造会话照配置、回合开始照钉的和配置换），加「压缩触发线」一节。
- `session/actor.md`「换快照」第 5 条。
- `kernel/events-bodies.md`：`session.policy_changed` 多 `compaction_line` 一格，样本加 137 号。
- `kernel/session.md`：`configure.rs` 那一行，加「钉压缩触发线」一节。
- `protocol.md`：方法表的 `session.configure`，`session.create`、`session.configure` 的参数表。
- `view.md`：「会话状态」多 `compaction_line` 一格。
- `config.md`：嵌着的参考文件样本（生成的）。
- 参考文档 `docs/reference/` 中英两份：`sessions`、`view`、`events`。

### 改了什么

1. 配置项 `compaction.line`（`miyu-session` 的 `settings.rs`）：整数 `[20, 95]`，出厂 85，只在系统层，「高级」页「压缩」组，下一轮生效。中、英、日三份名字和说明。
2. 协议（字段名用 `compaction_line`，不用 `line`：太含糊，2026-11-12 主会话定；`view.status` 顶层的 `compaction_line` 是钉的百分数，`context.compaction_line` 照旧是生效的压缩线 token 数）：`session.configure` 多 `compaction_line`（整数或 `null`，`null` 是去掉），`session.create` 也收（造好以后记一条，回应的 `events` 多它，排在 `vision` 后面）。范围照配置项声明的查（`CompactionSettings::line_range`），不在范围里、不是整数的 `bad_params`，`data.field` 是 `compaction_line`。`view.status` 钉了带 `compaction_line`。
3. 内核：`session.policy_changed` 最后多一格 `compaction_line: {"percent": 整数或 null}`，新命令 `SetCompactionLine`，和现在一样的不记、撤掉的回合里的也算、换模型不清它、载入照日志算回来（`Session::pinned_line`）。内核判到线只照策略里的 `line_percent`，不直接读钉的数。
4. 快照里的数怎么变：造会话时照这时的配置写进快照。回合开始（`actor/model/line.rs`，人格那一步以后）照会话钉的、没钉的照这一轮的配置定 N，和快照里的比，不一样的只换这一个数（`Snapshot::with_line`），存成新快照，记一条换快照的 `session.policy_changed`（`policy`）。人格那一步重拼的照旧快照的数。这个数不进请求，前缀一个字节不变。重放、载入照日志里记的快照；以前造的快照没有这一格的读成 85。fork 子代理冻住的快照不换。
5. 换上新快照以后给头看的限额重写（`store.rs` 的 `follow_context`），压缩线变了推 `model.changed`（`why` 是 `turn`）。
6. 挪了两处放不下的：`miyu-endpoint` 的 `session.configure` 挪到 `methods/session/configure.rs`；`session.create` 带的几样钉的造好以后发，从 `models/vision.rs` 挪到共用的 `models/pin.rs`。

### 不做什么

- 不改输出预留、余量、尾巴这些别的数，不加 token 数写法。
- 不改给模型看的字：`cargo xtask prompts` 无差异。
- `session.list` 不加这一格：生效的线看 `limits.compaction_line`，钉的看 `view.status`、`subscribe` 的回应（`subscribe` 的是下面「补」加的）。

### 验收

1. `cargo test -p miyu-kernel --lib line_pin`：压缩线照 20、85、95 三档在 20 万、百万、小窗口上算，减法封顶；钉了记一条只写 `compaction_line`、一样的不记、`null` 去掉、换模型不清它；撤掉的回合里的也算、载入照日志算回来。`policy_change_records_the_line_pin`：事件读写一字不差。
2. `cargo test -p miyu-policy --lib snapshot::tests::line`：只动那一个数，换回来和原来的一样，没有压缩的数的照旧。
3. `cargo test -p miyu-session --lib settings`：出厂 85、20 到 95、19 和 96 超范围、只在系统层、下一轮生效。
4. `cargo test -p miyu-session --test all line_pin`：系统配置改成 60 以后没钉的会话下一轮照 60 压；钉 50 的下一轮照 50 压，重启以后照日志，写 `null` 回到配置的 85；只在换的那一轮记换快照。改之前红：读配置那一处还原成 85，两条都红。
5. `cargo test -p miyu-endpoint --test all line_pin`：`session.configure {compaction_line}` 下一轮换上、`null` 回到配置、`view.status` 带 `compaction_line`；19、96、0、字符串、小数、负数 `bad_params`，什么都不记；`session.create` 带 `compaction_line` 的回应 `events` 是 `[1, 2]`、第一轮就照钉的。
6. `cargo xtask prompts` 没有差异。

### 风险

- 钉的数在协议那一头照这时的范围查过；以后改范围，日志里旧的数照记的用，不再查。
- 回合开始多比一个数；不一样时多存一份快照（只差一个数）。
- 换快照那一条挡住锚（`estimate::anchor` 不跨带 `policy` 的 `session.policy_changed`）：换线的那一轮用量照整份本地估算（中文多估约两成半、英文少估约一成半，`compaction.md` 第一条第 2 款），下一次请求起照锚。只在改线的那一轮，少估的落在余量里，真超了有被动压缩兜底。没为它改锚的规矩：内核分不出哪次换快照没动请求的字节。

### 补：订阅会话的回应带上钉的数

状态：做完，等合。

为什么：终端做 `/compact-line` 时报的（2026-11-12）。上面「不做什么」原来写着 `subscribe` 回应不加钉的数，可终端照 `subscribe` 的回应画会话设置：钉的数只能从最近一页事件里找 `session.policy_changed`，很早以前钉的翻不到，就显示成「跟着设置」。顺手查了 VM-1 钉的看图模型 `vision`，缺口一样，一起补。

蓝图改哪几节：`protocol.md` 的 `subscribe`（回应那一段、`compaction_line` 和 `vision` 两段、例子），`session.configure` 参数表里 `vision`、`compaction_line` 两行，代码表 `connection.rs` 那一行。参考文档 `connection` 中英两份的 `subscribe`。

改了什么：`subscribe` 事件流（带不带 `after` 都一样，视图流的回应照旧带「上面几格」）的回应顶层多两格，照会话 actor 同一步里拿的「当前的」（`miyu_session::Current`）写，同 `view.status`：`compaction_line` 是钉的百分数，`vision` 是钉的看图模型的引用，没钉的都不写。`limits.compaction_line` 名字一样、层级不同：那是生效的压缩线，token 数，钉了以后下一个回合开始才换。只加格，以前没钉的会话回应字节不变。

验收：`cargo test -p miyu-endpoint --test all the_events_subscription_carries`：钉了 50 的会话回应顶层 `compaction_line` 是 50、`limits.compaction_line` 照旧 850000，核心重启照日志载入以后、带 `after: 0` 的也是 50，没钉的没有这一格；看图模型同样（`c/w`）。改之前两条都红。

风险：没有。头不认得的格照不理。
