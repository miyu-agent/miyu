## 软件包和自己放的斜杠命令

### 是什么

- **命令模板**：一个 Markdown 文件，人敲 `/<名字> 参数` 时核心照它展开成一段话，当这个人说的一句发给 AI（照 Claude Code 的 custom slash commands：`.claude/commands/<名字>.md`，文件名就是命令名，`$ARGUMENTS` 换成参数）。软件包带在包文件夹的 `commands/<名字>.md`，本机自己放的在 `home/<账号>/commands/<名字>.md`。
- **扩展的命令**（施工 X-14 下）：清单里声明、交给包的程序办，回一段文字（同核心自己的命令那样回执）。

已经定了的（2026-10-15 项目主人，不在这里重定）：

1. 包的斜杠命令两种都要：交给包的程序办的、展开成一段话发给 AI 的。
2. 交给程序办的回一段文字（可以多行）。
3. 谁能用：和内置命令同一关（`protocol.md` 的 `command.run` 第 2 条），且这个会话的预设开着它。
4. 核心升级后内置命令和装着的包的命令撞名：内置的赢，`miyu check` 报。
5. 装包时查冲突（`packages.md`「冲突」）：斜杠命令名也算一样，撞了不装。
6. 和自己放的同名：照装，自己的盖住包里的，`miyu check` 报（同技能、脚本，设计 31 第五节第 3 条）。
7. 模板命令时间线上显示人敲的那一行，展开的那段只给 AI，记在事件里。

状态：施工 X-14 上做完（2026-10-15）：模板的找、读、展开、发出去，`command.catalog`、`preset.get` 列出来，预设开关。装包时查命令名、`miyu check` 报跳过的和被盖住的在 X-14 上补；扩展的命令在 X-14 下。

### 代码（施工 X-14 上）

| 文件 | 管什么 |
|---|---|
| `crates/miyu-policy/src/slash.rs` | 命令名的写法、功能编号 `command-<名字>`；读模板（开头可有可无，`description`、`argument-hint`、`metadata` 的 `summary_zh`、`summary_ja`）；照参数展开。纯逻辑 |
| `crates/miyu-store/src/slash.rs` | 找：家目录 `home/<账号>/commands/` 先，再找装了的（清单读成了的）包的 `commands/`，出厂的先于家目录、同一层照编号；同名的先找到的赢，后来的、名字不合写法的、开头写错的跳过并说为什么；子文件夹、点开头的、不是 `.md` 的不算；照名字排。另有包文件夹带了哪几条（`carried`，装包时查用） |
| `crates/miyu-kernel/src/event/message.rs`、`session/input/command.rs`、`session/send.rs`、`session/redo.rs` | `message.user` 的 `command`（`TypedCommand`：名字、人敲的那一行）；内核命令 `Typed`：照 `session.send` 发、不急着插话；重做照原样的带着它，换了字的不带 |
| `crates/miyu-endpoint/src/commands/templates.rs` | `command.run` 认不出的名字照这一条：找会话、认说话的人、找模板、判身份、看预设、展开、交给内核；`command.catalog` 的一项 |
| `crates/miyu-endpoint/src/commands.rs`、`commands/catalog.rs` | 登记表里有的照旧，没有的转给模板；菜单里模板照名字排进去 |
| `crates/miyu-endpoint/src/presets.rs`、`presets/features.rs` | `preset.get` 列出每条模板命令一个功能 |
| `crates/miyu-view/src/projector/users.rs`、`crates/miyu-endpoint/src/undo.rs`、`crates/miyu-store/src/index/row.rs` | 给人看的照人敲的那一行：时间线那一条、撤销的回执、会话列表的预览 |

### 模板

1. **写法**：开头可以有一段 `---` 包起来的 YAML，照 Claude Code 认 `description`（一句说明，命令菜单里给人看的，算英文那一份，也是别的语言没写时退回的那一份）、`argument-hint`（名字后面跟什么的提示，例如 `<路径>`，照原样）；中文、日文的说明另写在 `metadata` 的 `summary_zh`、`summary_ja`（同技能，`skills.md`「技能」第 1 条）。Claude Code 的别的键（`allowed-tools`、`model`、`disable-model-invocation`）认、不用。没有开头那一段的（第一行不是 `---`、或者没有收尾的 `---`）整份都是正文。开头不是一张表、YAML 读不懂的算写错了。
2. **名字**：文件名去掉 `.md`，写法同 `[command]` 的子命令名：小写字母开头，只用小写字母、数字、`-`，最多 32 个。不合写法的跳过、说为什么。
3. **展开**：正文里的 `$ARGUMENTS[<第几个>]` 换成名字后面跟的字照空白切开的第几个（从 0 数，没有的换成空的），`$ARGUMENTS` 换成整段字（去掉前后空白）；参数里带的记号不再展开。正文里一处都没写、人又跟了字的，末尾空一行接上那段字（Claude Code 另加一个 `ARGUMENTS:` 标签，这里不加：给模型的字非必要不加）。Claude Code 的 `$1` 这些简写各版本数法不一，不认，照原样留着；`` !`命令` ``、`@文件` 也不认：展开不跑命令、不读文件。展开完去掉前后空白，只剩空的照内核拒 `empty_message`。
4. **发出去**：展开的那段成一块文字，当这个人说的一句进会话，和 `session.send` 一样（闲着开一轮，正忙的排进这一轮，不急着插话；`by` 是说命令的人）。`message.user` 多一格 `command`：`{"name", "text"}`，`text` 是人敲的那一行原文（去掉前后空白）；`blocks` 是展开的那段，模型照它看。展开的那段记在事件里，以后改了、删了模板，历史照旧。
5. **给人看的照那一行**：视图里这一条的 `text`（`view.md` 的 `user`，撤回放回输入框的也是它）、撤销的回执 `said`（`protocol/undo.md`）、会话列表的预览（`protocol.md` 的 `session.list`）。
6. **回应**：`command.run` 回 `{"command": "<名字>", "events": [<这一句 message.user 的序号>], "said": ""}`：时间线上已经有人敲的那一行，没有回执；不另记 `command.ran`。同一个编号再发照内核只算一次。
7. **重做**：`session.redo` 不写 `text` 的照原样重发（展开的那段和 `command` 都照旧，不重读模板）；写了 `text` 的照写的字发，不再是命令。只换附件的照旧是命令。

### 找、盖住、开关

1. **从哪找**：这个会话的属主（`Handle::owner`，同找技能）家目录的 `commands/` 先，再找装了的包的 `commands/`，出厂的先于家目录、同一层照编号（同技能，`packages.md`「冲突」第 5 条）。同名的先找到的赢，后来的跳过、记被谁盖住。`command.run`、`command.catalog`、`preset.get` 每次照这一刻找：文件少、只读开头，改了模板不用重启。不写会话的 `command.catalog`、`preset.get` 照管理员找。
2. **内置的赢**：名字和核心登记的（名字、别名）撞了的，模板不算：`command.run` 照内置的办，菜单、预设里不列它。
3. **开关**：每条模板命令算一个功能 `command-<名字>`，挂在它的包下（自己放的挂在 `~home`），预设里能逐个开关；`preset.get` 列在技能后面：`{"id": "command-<名字>", "name": "/<名字>", "summary"?, "on", "installed": true, "tools": []}`，说明照模板写的、照连接的语言挑，没写的不带。`command.run` 照会话现在的预设判（会话表里记的编号，照这一刻的文件；没有预设的、读不出的当开着），关着的回 `command_not_allowed`，`data.why` 说「预设「<名字>」未启用 /<命令>。」（`commands/unavailable/command-preset-off`）；`command.catalog` 写了会话的不列。
4. **谁能用**：和内置命令同一关（`may_run`：本机的会话是本人；场所会话照 `as`，主人、对应表里有的、管理的人），别人回 `command_not_allowed`，场所会话不带 `as` 的 `venue_session`。先找会话、认说话的人，再找模板：认不出的 `unknown_command`，谁打的都是（场所里的桥照它把话当平常说的记下，`onebot.md`），认出了再判身份。
5. **菜单里的样子**：`command.catalog` 一项 `{"name", "aliases": [], "summary", "argument"?}`，`summary` 照连接的语言挑，没写说明的照正文第一行不是空白的那一行（同 Claude Code）；写了 `argument-hint` 的带 `argument`。和内置的照名字排在一起。

### 装包时查（施工 X-14 上补）

`packages.md`「冲突」多一样 `slash`：包的 `commands/` 里的命令名，和装着的别的包的、核心内置的（名字和别名）比；内置的那几条记在拥有它的包上（`/clear`、`/stop`、`/workspace` 是 `basesystem`，`/remember`、`/dream`、`/clear-memory` 是 `memory`，`/goal` 是 `goal`）。命令行写「斜杠命令」、英文 `slash command`。

### `miyu check`（施工 X-14 上补）

报跳过的模板（开头写错、名字不合写法）、被盖住的（`command_covered`：被自己放的、被内置的、被先读到的包），各一条警告，同 `check/skipped.rs`。

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-policy/src/slash/tests.rs` | 名字的写法、功能编号；开头给人看的几格、Claude Code 别的键不碍事；没有开头、只有开头一行、空开头；开头写错、不是表；`$ARGUMENTS`、`$ARGUMENTS[N]`、不是数字的方括号、参数里带记号、没跟字；没写记号的接在末尾 |
| `crates/miyu-store/src/slash/tests.rs` | 两处找、照名字排、自己放的盖住包里的、记被谁盖住；出厂的先于家目录；读坏了的清单不算；点开头、不是 `.md`、子文件夹不算；名字不合写法、开头写错的跳过说为什么；包文件夹带了哪几条 |
| `crates/miyu-kernel/src/session/tests/typed.rs` | 闲着照 `session.send` 开一轮、`message.user` 带上那一行；正忙的排进这一轮、不插话；重做照原样的带着它、不重读，换了字的不带 |
| `crates/miyu-view/tests/lines.rs` 的 `a_typed_command_shows_the_line_that_was_typed` | 时间线那一条是人敲的那一行 |
| `crates/miyu-endpoint/tests/slash_templates.rs` | 真核心：展开发出去、日志里两样、模型看到展开的那段、没有回执、不记 `command.ran`；自己放的盖住包里的、`$ARGUMENTS[0]`、内置的赢；认不出的、展开是空的拒、什么都不说；预设关了的拒、说清哪个预设、菜单不列、`preset.get` 列成关着；菜单照名字排、说明照语言、没写的照正文第一行、和内置同名的不列；撤销的回执、会话列表的预览是那一行；场所会话照内置的那一关，认不出的照旧 `unknown_command`（`crates/miyu-onebot/tests/group.rs` 的 `slash_commands_in_a_group_answer_in_the_group` 照它把 `/xxx` 记成旁听） |

### 施工拆分

| 步 | 做什么 |
|---|---|
| X-14 上 | 模板：找、读开头、展开、发出去（`message.user` 的 `command`、给人看的照那一行）、`command.catalog` 列出来、每条一个功能、预设开关：做完了（2026-10-15） |
| X-14 上补 | 装包时查斜杠命令名（包之间、和内置的）；`miyu check` 报跳过的、被盖住的 |
| X-14 下 | 扩展的命令：清单声明、能力、核心反向调用、回一段文字 |
