## 施工单 VP-1：场所消息「戳一戳」

状态：做完，等合（分支 `step/CB-1`）。im I-248 在等；项目主人 2026-11-17 定写法。

### 为什么

QQ 的拍一拍改名「戳一戳」（英文 poke：双击对方头像）。im 想把它交给她：当成戳的人发来的一条空消息。核心原来收到 `venue.poke` 回 `bad_params`（`venue` 不认识的格），内容块是空的又回 `empty_message`。主人定：被戳一般都有提示，写成「谁戳了你」，加一句它是什么，免得她理解错。

### 做什么

1. `session.send` 的 `venue` 多两格（`venues.md`「场所的格」）：`poke` 只收 `true`（写 `false` 的 `bad_params`）；`poke_text` 可选，只跟着 `poke` 来，同 `name` 的规矩（去掉前后空白不空、最多 64 个字符、没有控制字符），不可信。原样记进 `message.user` 的 `venue`；内核认 `poke` 当内容，带它的空消息照收（`session/send.rs`）。
2. 她看到的那一句：资源 `core/venues/poke.txt`，`{name} poked you (a {platform} nudge: they double-clicked your avatar)`，造会话时冻进快照（群会话的 `group.poke`、私聊的 `core.private_media.poke`，同 O-33 的 `voice`），组装时照它写（`miyu-assemble` 的 `poke.rs`）：
   - `name` 照 `venue.name`，没有的写平台身份；`platform` 照戳的人的平台身份头一截，认识的写惯用的大小写（`QQ`、`Telegram`……），别的原样；两格照模板的规矩转义。`poke_text` 转义、空一格接在后面。
   - 群里那一行的内容最前面写它，群聊近况、判官看的那一行（`venue.records`）同一份；私聊那一条正文前面单独一块。
   - VP-1 以前造的会话快照里没有那一句，只写记号 `[poke]`（代码写的格式，同 `[image]`）：私聊里一条什么块都没有的 user，供应商回 400。
3. 别的规矩（限流、判官、并行支线、重发不判）不动：桥照「冲她来」交，群里不写 `mentions_me`。
4. 给模型的字登记进登记簿（`26-提示词.md` 第十节）、量 token，`cargo xtask prompts` 重写 `prompts.md`。
5. 文档：蓝图 `venues.md`「场所的格」、`kernel/request.md`「群里的一行」和快照的表；协议参考 `venues` 中英。

### 不做什么

- 不为 Telegram 这些没有戳一戳的平台另写字：平台名照场所的平台写，桥不交就不会出现。
- 以前造的会话不补冻那一句。

### 验收

1. 改前红：`miyu-endpoint` `venue_poke` 两条（main 上 `venue.poke` 回 `bad_params`）；`miyu-assemble` `poke::tests` 三条（main 上没有这一句）；`miyu-kernel` `session::tests::venue::a_poke_without_blocks_is_kept`（main 上没有 `poke`，空消息回 `empty_message`）。沙箱里 main 的核心收到 im 的形状回 `bad_params`。
2. 跟着改的：`miyu-policy` 群会话、私聊记号的测试（新的一格、写错造不出策略、以前造的字节不变），`miyu-store` 资源的测试。
3. 门禁十一项、`random::` 长跑、Windows 版 clippy、`cargo xtask prompts`、`cargo xtask logs`。
4. 实测：沙箱数据根装一个假桥（扩展，`[connection] platform = "qq"`），经它发 im 的形状；请求经一个记请求的转发到真模型（中转站）：群里一行、判官看的那一行、私聊那一条照上面的写法；真模型私聊里被戳回「诶——被戳了！（捂住小脑袋）主人有什么事吗？」。

### 风险

- 平台名的表写在代码里（同 `miyu-tool` 记忆那几行的一份），加平台要两处一起改。
