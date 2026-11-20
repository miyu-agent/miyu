## 施工单 SD-1：她说的话合成以后交给人

状态：做完，等验收（分支 `step/VS-batch`）。项目主人 2026-11-17 本机验收第 16、26 条；语音会话的「say 交付方案」（2026-11-17，主人定过两条）里「核心」那一节。

### 目的

`say` 只合成一个文件，她要让人听见只能发文件：网页里叫她念一句，她发进 QQ 群；QQ 里她永远在发文件、发不了语音消息。照方案：`say` 默认合成以后把音频放进工具结果的 `sent`（「她发出去的」），标成语音，谁收到谁照自己的地方交。核心这一半：

1. `sent` 收语音一项 `{"blob", "media_type", "voice": true, "duration_ms"?, "name"?}`，至多一段，blob 至多 10 MiB。
2. 推给头的视图流里分得清这一轮是谁开的：本头发话的、别的头发话的、没人开口的（定时、循环、人生）。和 VN-1 是同一份信息（开这一轮的那句话的 `by`），投影照它带。
3. 本机会话里没人开口的一轮出了语音、这一刻没有一个头开着这个会话（`session.viewing`）：核心经总线调 `tts.play {"audio": <blob>}`，从这台电脑的音箱放。有头开着的交给头放。
4. 场所会话的语音上限：`venue.session` 多 `voice_max_ms`（同 `images`），记进 `session.created`，这个会话的 `tool.call` 原样带上。

`say` 的参数（`file_only`）、超上限回报错让她重说、合成后交给人，是语音会话那一半；头怎么放是头那一半；QQ 发 record 段是 im 那一半。

### 蓝图改哪几节

- `kernel/events-bodies.md`：`tool.result` 的 `sent`、`session.created` 的 `voice_max_ms`。
- `providers.md`：`tool.call` 多 `persona`、`permission`（TC-1 漏写的）、`voice_max_ms` 两行；回应的 `sent` 那一段。
- `view.md`「条目」多 `voice`。
- `venues.md`「`venue.session`」多 `voice_max_ms`。
- `store/resources.md`、登记簿一行（`core/tool-results/voice-too-big.txt`）。
- 协议参考：`extensions.md`（`tool.call` 的 `voice_max_ms`、回应的 `sent`）、`view.md`、`venues.md`、`bus.md`（`tts.play` 核心也调），中英。

### 做法

1. `miyu-kernel`：`event::Sent` 改成两种（`event/tool/sent.rs`）：`Voice`（写了 `"voice": true` 的，只认 `true`）、`SentImage`（原来的图）。JSON 不加种类的格，以前的日志读进来再写出去一字不差。`SessionCreated::voice_max_ms`。
2. `miyu-endpoint`：`provide/voice.rs` 查写法（图至多 4 张照旧、语音至多一段、`audio/` 开头）、查大小（10 MiB，大了回 `voice-too-big.txt`）、兜底放（这次调用的 `by` 是 `schedule` 或没有、本机会话、成功的、没有头开着这个会话，后台调 `backstage::bus::call_by_core`，不带 `from`、`language`）。`provide/remote.rs` 的 `tool.call` 带 `voice_max_ms`；`Cancel` 挪进 `provide/remote/cancel.rs`（行数）。`venue.session` 收 `voice_max_ms`（`0` 是 `bad_params`）。
3. `miyu-session`：`Create`、`ToolKit`、`Callers` 带 `voice_max_ms`，造会话记进 `session.created`、载入照它读；分叉照父会话的。`miyu-tool` 的 `Caller::voice_max_ms`。
4. `miyu-view`：投影记下每条消息的 `by`，开轮时照 `trigger` 找，连同 `turn.started` 的 `cause` 带在 `voice` 条目上；Markdown 导出照文件写一条链接。

大小上限定 10 MiB（方案的例子，技术细节按推荐定）：mp3 128 kbps 十来分钟，16 位单声道 24 kHz 的 wav 三四分钟，说一段话够用；blob 上传上限是 20 MiB。

### 给头、im、语音的

- 头：视图流 `voice` 条目 `{blob, media_type, name?, duration_ms?, by?, cause?}`。`cause` 是自己发的 `session.send` 的编号 → 本头说的，当场放；`by.kind` 是 `schedule`、没有 `by` → 没人开口，开着的都放；别的 → 只画播放条。
- im：事件 `tool.result.body.sent` 里写了 `"voice": true` 的是语音（照 `media_type` 判格式）；没有的照旧是图。`venue.session` 带 `voice_max_ms`。
- 语音（say）：`tool.call` 的 `voice_max_ms`；回应 `sent: [{"blob", "media_type", "voice": true, "duration_ms"}]`，至多一段、≤ 10 MiB；`tts.play {"audio"}` 核心也会调（不带 `language`）。

### 验收

1. 改前红：`miyu-kernel` 的 `event::tool::tests::sent_images_and_a_voice_round_trip`、`a_voice_that_is_not_true_is_not_read`；`miyu-view` 的 `media::a_spoken_voice_carries_who_opened_the_turn`；`miyu-endpoint` 的 `provide::voice::tests`（三条）、`provide::tests::remote_tests::a_call_carries_the_permission_and_persona`（多查 `voice_max_ms`）、`venues::a_venue_voice_limit_is_recorded_and_no_channel_is_told`；`miyu-session` 的 `caller::a_venue_voice_limit_is_recorded_and_handed_to_every_call`。
2. 实测：沙箱核心，假扩展工具在 `sent` 里交一段音频，看视图流的 `voice` 条目、没头开着时核心调了假的 `tts.play`（见提交说明）。
3. 门禁十一项、长跑随机、Windows 目标的 clippy、`cargo xtask prompts`、`cargo xtask logs`。

### 不做什么

- 不替头放、不替 im 发；`miyu ask` 发话的一轮出了语音，核心不放（`ask` 是开着话的头，语音照样在视图流里，要放由命令行头自己调 `tts.play`）。
- 回合中途别的头插话，`voice` 条目照开这一轮的那句话算。
