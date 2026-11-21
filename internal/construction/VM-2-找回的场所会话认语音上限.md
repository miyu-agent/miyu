## 施工单 VM-2：找回的场所会话也认语音上限

状态：做完，等验收（分支 `step/CB-2`）。im 在等；SD-1 合了以后主人机器上的主群、私聊仍旧没有上限。

### 目的

SD-1 让 `venue.session` 收 `voice_max_ms`，可核心只在**造**场所会话时记进 `session.created`。主人机器上的场所会话早就造好了，找回时核心不看，这些会话一直没有上限，`say` 不知道 QQ 只到 60 秒。改成：找回的会话照这次交的认，和记过的不一样（含没记过的）追加一条事件记下，之后的 `tool.call` 带最新的；没交的照记过的。

### 蓝图改哪几节

- `venues.md`「`venue.session`」`voice_max_ms` 那一行：「找回的不看」改成找回的也看、怎么记；「守着它的」加一行测试。
- `kernel/events-bodies.md`：`session.policy_changed` 多 `voice_max_ms` 一格（写在 `from` 后面），说明里加「桥报了新的语音上限时只写它」；`session.created` 的 `voice_max_ms` 改「之后不变」。
- `kernel/session.md`：多「换场所的语音上限」一节、`voice.rs` 一行。
- `providers.md`：`tool.call` 的 `voice_max_ms` 照最新的。
- 协议参考 `venues.md`、`extensions.md`（中英）：找回的也看。

### 做法

1. 记在哪：照现有「会话设置变了」的写法，`session.policy_changed` 末尾多一格 `voice_max_ms`（只设不去，没有去掉的写法），不另加事件种类。以前的日志没有这一格，读写一字不差。
2. `miyu-kernel`：命令 `SetVoiceLimit { ms }`（`session/voice.rs`），一样的接受、什么都不记，不一样的记一条，`by` 是报的桥；`Reference` 从 `session.created` 和后来的 `policy_changed` 算（撤掉的回合里的也算），`Session::voice_max_ms()`；`ForkPlan::voice_max_ms` 照分叉点那一刻的。封存了的拒（同换模型）。
3. `miyu-session`：落了盘的一批里有它的当场换 `Callers` 的（`actor/store.rs` 的 `follow_context`）；载入照内核算的（`open/load.rs`），分叉照 `ForkPlan`。
4. `miyu-endpoint`：`venue.session` 找回的、写了 `voice_max_ms` 的，发 `SetVoiceLimit`，命令编号 `<请求>/voice`，`by` 照 `events.append` 的记法（桥的包）。记不成（只读、会话停了）照样回找到的会话，记 `WARN venue voice limit not recorded`：桥接着说话时照那一处的原因办。

### 给 im 的

不用改：每次 `venue.session` 照旧交 `voice_max_ms`。主群、私聊下一次找回就记上，之后 `tool.call` 带它；日志里多一条 `session.policy_changed {"voice_max_ms": 60000}`，`by` 是 im 的包。

### 验收

1. 改前红：`miyu-endpoint` 的 `venue_voice_limit::a_found_venue_session_takes_the_voice_limit_reported_now`（找回时报 60000，`tool.call` 没带，红在 `voice_max_ms` 是 `null`）。另加：`miyu-kernel` 的 `session::tests::voice_limit` 三条、`event::session::tests::a_policy_change_records_the_venue_voice_limit`，`miyu-session` 的 `caller::a_reported_voice_limit_reaches_later_calls_and_survives_a_reload`（拿掉 `follow_context` 那一段红在「开着的会话当场换」，载入改回 `created` 的红在「载入照日志算回来」）。
2. 实测（沙箱数据根，真模型是中转站的 `opencode-go/deepseek-v4.1-flash`）：假桥先不带、再带 60000 两次找回同一个群会话，真模型调了 `note`，`tool.call` 带 `voice_max_ms: 60000`，日志里只一条 `policy_changed`、`by` 是 `bridge`；核心停了重起、找回时不带，`tool.call` 照旧 60000。
3. 门禁十一项、长跑随机、Windows 目标的 clippy、`cargo xtask prompts`、`cargo xtask logs`。

### 不做什么

- 不加去掉上限的写法：平台的上限不会没了，没交的照记过的。
- 不改给模型看的字。
