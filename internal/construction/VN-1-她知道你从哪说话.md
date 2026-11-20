## 施工单 VN-1：她知道你是从哪说话的

状态：做完，等验收（分支 `step/VS-batch`）。项目主人 2026-11-17 本机验收第 16 条。

### 目的

主人在网页里跟她说话，叫她念一句，她用 `say` 合成了 mp3、再用 `qq_send_file` 发进 QQ 群，而不是 `show` 给他；问她「你知道我在哪和你说话吗」，她照记忆猜成群里。本机会话里她不知道这一轮的话是在终端界面、网页、命令行（`miyu ask`）、终端集成上说的，还是到点叫醒、没人在场。

本机会话每轮开始照开这一轮的那句话告诉她从哪来；换了地方才说。场所会话（群、私聊）不归这一步：桥已经在 system 尾部写了 `<venue>…</venue>`，核心不另写。

### 蓝图改哪几节

- `kernel/ids.md`「`by`」：`person` 多一格 `head`（握手报的 `head.kind`，写法同模块编号，写法不对的不写）；第 21 条。
- `kernel/request.md`「事实」第 2 条多 `channel`；「样子：给模型看的字」表多一行。
- `policy.md`「`CoreTexts`」的 `facts` 多 `channel`；`store/resources.md` 两处。
- 登记簿（设计 `26-提示词.md` 第十节）六行，`prompts.md` 重新生成。
- 协议参考：`connection.md` 的 `head`、`events.md` 的 `by`（中英）。

### 做法

1. `miyu-kernel`：`HeadKind`（`id.rs`）；`Person::head`。`facts/channel.rs`：`Channels`（头的种类 → 那一句、`other`、`nobody`），`FactTemplates::channel` 照 `by` 挑：本机的头上的人照 `head`（没有这一种的、没报的用 `other`），`schedule` 用 `nobody`，别的不挑；和有效历史里内核记的同类最近一块比原文，一样的不注入。`open_turn_on` 在边界事实（环境、权限、会话编号、模型）后面、角色扮演提示前面注入。
2. `miyu-policy`：`FactTexts::channel`（`ChannelTexts`，没有的不写，旧快照字节不变）。`miyu-store`：`core/facts/channel/head/` 列目录读，一种头一份；`other.txt`、`nobody.txt`。
3. `miyu-session`：`Parts::local`，场所会话拼快照时去掉这几句；换快照照旧快照有没有。
4. `miyu-endpoint`：握手记下 `head.kind`（`Shaken::head`、`Caller::head`），`session.send` 人说的话、`command.run` 开轮的命令（模板、MCP 提示词）记成 `{"kind":"person","head":…}`（`sessions::person_on`、`commands::spoken`）；重做照原来的 `by`。

给模型的字（英文、一句、只说在哪，不写该怎么做；2026-11-17 照 `o200k_base` 量，带行尾换行）：

| 文件 | 原文 | token |
|---|---|---|
| `head/tui.txt` | `<channel>The user is talking to you in the Miyu terminal UI.</channel>` | 17 |
| `head/web.txt` | `<channel>The user is talking to you in the Miyu web UI.</channel>` | 17 |
| `head/cli.txt` | `<channel>The user sent this from the command line (miyu ask).</channel>` | 18 |
| `head/shellhook.txt` | `<channel>The user is talking to you from their shell prompt (Miyu shell integration).</channel>` | 21 |
| `other.txt` | `<channel>The user is talking to you in a Miyu app.</channel>` | 16 |
| `nobody.txt` | `<channel>No user is present. A schedule started this turn.</channel>` | 15 |

### 给头的

- 终端无缝集成现在握手报的也是 `tui`（和终端界面同一个连接代码），她看到的是「终端界面」那一句。要分开，终端头在 shellhook 的连接上报 `shellhook`；核心这边已经备好那一句。
- 头的种类不认识的（托盘、第三方头）看到 `other` 那一句；要有自己的一句，加一份 `core/facts/channel/head/<种类>.txt` 并登记。

### 验收

1. 改前红：`miyu-kernel` 的 `session::tests::channel`（三条）、`facts::tests::channel`（七条）；`miyu-session` 的 `snapshot::tests`（两条）；`miyu-endpoint` 的 `channel`（两条：网页、终端各说、同一个头接着说不再加；写法不对的种类不记、用 `other`）、`venues::a_venue_voice_limit_is_recorded_and_no_channel_is_told`（场所会话不说）。
2. 实测：沙箱核心、真模型，网页头和 `miyu ask` 各问一次「你知道我是从哪里和你说话的吗？不要用工具」，记回答原文（见提交说明）。
3. 门禁十一项、长跑随机、Windows 目标的 clippy、`cargo xtask prompts`、`cargo xtask logs`。

### 不做什么

- 场所会话不写。
- 回合中途别的头插话不换这一块（照开这一轮的那句）。
