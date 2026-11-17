## 施工单 SW-1：分节跟着运行时给选项的项

状态：做完，等合（分支 `step/CB-1`）。2026-11-17 语音会话要的，核心主会话派。

### 为什么

语音会话（miyu-voice VO-32）要把朗读的引擎做成一张列表：内置的 MiniMax、MiMo、OpenAI 兼容，后面接着装着的 `tts.engine` 包。装了哪些包只有运行时才知道，所以 `say.engine` 改成 `text` 加 `choices_from = "engines"`（施工 SG-2）；下面三节（MiniMax、MiMo、OpenAI 兼容）照它的值 `shown_when`（施工 SG-1）。核心原来两条规矩互斥：`shown_when` 只能指 `option`、值要在它写死的选项里（`bad_shown_when`）；`choices_from` 只能写在 `text` 上。这样的清单装不上。

### 做什么

1. `shown_when` 也可以指带 `choices_from` 的 `text`：只按值比，值不查选项（同那一项自己写什么存什么）。指 `option` 的照旧查值；不带 `choices_from` 的 `text` 和别的类型照旧 `bad_shown_when`（人手填的字比不出节来）。
2. 两个头本来就只按值比（终端 `src/settings/pages/sections.rs` 的 `hidden()`、网页 `packages/settings/package-sections.js` 的 `shown()`），不改。`config.schema` 交的 `sections` 形状不变。
3. 文档：蓝图 `packages.md`「配置项」第 1 条、出错码表、测试表；作者 `manifest` 中英；协议参考 `config` 中英。
4. 顺手：协议参考 `bus` 中英，`stt.transcribe`、`stt.record` 的回应加 `speech`（只写 `false`：整段没听到说话、没送去认）、`device`（`stt.record` 才有，`speech` 是 `false` 时开的那个麦克风给人看的名字）两格（miyu-voice VO-33 已合）。

### 不做什么

- 不查值在不在包运行时给的列表里：核心不调那个方法（SG-2 定的）。
- 头不改。

### 验收

1. 改前红：`miyu-config` `package::settings::tests::section::a_group_may_follow_a_text_with_choices_from`（main 上报 `engine is not an option`）。
2. 门禁十一项、`random::` 长跑、Windows 版 clippy、`cargo xtask prompts`、`cargo xtask logs`。
3. 实测：沙箱数据根，miyu-voice `step/VO-32-engine-list`（`543e3df`）的 say 包连程序装进去，`config.schema` 里 `say.engine` 是 `text`、带 `choices_from`，say 那一组的三节 `shown_when` 指 `say.engine`；`config.set` 换引擎写得进，节跟着值显示、隐藏（网页头）。

### 风险

- 指文字项的 `shown_when` 值写错了（拼错引擎名）不再在装的时候报，那一节永远不画。作者自己测得出来。
