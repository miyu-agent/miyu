## 施工单 ACP-8：扩展驱动在一次调用进行中请核心问人

设计 73 第四节第 7 条（项目主人 2026-11-26 定：用 Miyu 自己的确认框，没人看着的会话一律拒绝）。

### 为什么

`acp` 包把 ACP agent 当模型驱动。设置选了「也放行对方的工具」时，agent 要跑命令、改文件会发 `session/request_permission`，要问主人。
扩展发不起核心的确认框（`session.answer` 只给人、`tool.guard` 只管核心执行的工具），ACP-4 只好交回核心一次 `ask_user` 来问，时间线上
多一次「AI 提问」。主人看了 Zed 的做法（工具卡片上「允许这一次 / 拒绝这一次」）以后定用 Miyu 自己的确认框，头不用改。

### 改了什么

- 协议：扩展连接上多一个方法 `driver.approve {call, index, access, title, command?, text?, options}`（要 `model.driver`），只认这个连接上
  正在进行的那一次 `driver.call`；回 `{decision: once|deny|cancelled, option?, reason?, auto?}`。别的 `call` 回 `unknown_call`，参数不对、
  那一块不是这一次已经开始的供应商侧工具回 `bad_params`（`data.why`）。连接在后台等，不挡别的请求。
- 事件：`tool.approval_requested` 多一格可选的 `driver {seen, index}`：画在第 `seen` 次请求的第 `index` 块上；这时 `call_id` 是内核另给的
  `call_<这一条的序号>_1`，不对着调用。账本照它不查「在等结果」，只查编号是这么写的。
- 内核（`session/driver_ask.rs`）：`Input::DriverAsked`、`Action::AnswerDriver`。问不了的当场回（不在路上的那一次 `Cancelled`、不是供应商侧
  工具 `NotHosted`、摘要请求 `Aside`、`attended` 是假 `Unattended`、只读时要写或者执行 `ReadOnly`）；问得了的记请求（`by` 是包，没有规则，
  带截止的时刻）。人答经 `session.answer`（没有规则：`session` 回 `no_rule`），决定落了盘再回；到时没人确认内核替人拒、落盘再回；这一次
  请求有了结局（`call/settle.rs` 的 `settle`：说完了、出错、打断、撤掉重发、重启）时还在等的各记一条内核的拒绝，排在回复前面，当场回
  `Cancelled`。
- 执行器（`miyu-session`）：`DriverPort::call` 多交一头收 `driver.approve`；`route/extension/approve.rs` 先收下在它之前到的增量，主请求的
  交给 actor（`Back::DriverAsk`，`Reports::asker`），辅助请求的当场回 `auto: aside`；问着的时候空闲时限、总时限都停表。参数怎么读、
  `detail` 怎么写、选项怎么挑在 `driver_ask.rs`：有命令的照跑命令的确认写 `{driver, tool, title, command}`，别的照包的守卫问人写
  `{driver, guard, tool, reason}`，头照已有的两种样子画；允许照对方的「允许这一次」，没有才「总是允许」，拒绝同理。
- 视图：确认画在那一块供应商侧工具上（`projector/hosted.rs`），那一步的 `call` 先写成这一问的编号（头照它回答），回复落了盘换成那件工具
  自己的；读回日志时请求排在回复前面，先记在手里、开了条目再写上。
- 端点：`models/approve.rs`、`reverse.rs` 的 `Peer::approve`、权限表一行；测试扩展多 `driver:approve`、`driver:approve-other`。
- 文档：协议参考 `extensions.md`（方法、时限、供应商侧工具那一段）、`events.md`、`sessions.md`（中英）；作者文档 `dev/extensions.md`（中英）；
  蓝图 `kernel/asking.md`「驱动问的确认」、`kernel/events-bodies.md`、`extensions.md`；方法快照；设计 73 第四、七、八节。

### 头不用改

两个头都照推来的 `tool.approval_requested` 开抽屉、照 `tool.approval_decided`（或者 `tool.result`）收、用 `session.answer {call, decision}`
答；`detail` 认 `command`/`title`（跑命令）和 `guard`/`tool`/`reason`（包问人），多的格不拒认。决定只有 `once`、`deny`，同两个头现有的
两个按钮。驱动问的这一问有 `tool.approval_decided` 了结（打断、到时、调用没了都记），抽屉照常收。网页的历史照视图里那一步的
`approval` 画，视图照写。

### 怎么验

- 内核 `session/tests/driver_ask.rs` 九条：记请求、画在哪一块、编号；允许落了盘才回；拒绝带理由、`session` 回 `no_rule`；没人能确认当场拒；
  只读时执行拒、读照问；问的不是这一次、不是供应商侧工具；打断先了结再写回复、回 `Cancelled`；说完了在回合里了结；到时没人确认。
- `miyu-session` 的 `driver_ask/tests.rs`：两种 `detail`、截字、参数写错的原因、选项怎么挑。
- 端点 `models/drivers/tests.rs`：只交给这个连接上在进行的那一次。
- 真核心、真扩展进程 `tests/extension_driver_approve.rs` 五条：问到（`by`、`driver`、`detail`、编号、视图里那一步带着）再允许；拒绝带理由、
  记住的被拒；握手不能输入的连接开的会话当场 `unattended`、不记请求；打断回 `cancelled`、内核记拒绝、之后答 `not_asking`；别的 `call`
  回 `unknown_call`、没开始的块 `bad_params`、这一轮照常说完。
- 门禁。
