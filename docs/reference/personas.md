# 人格与预设：`persona.*`、`preset.*`

人格是「她是谁」：人设、示范对话、角色扮演提示、头像、主题色。预设是「她能用什么」：开哪些功能、关哪几件工具。两样都是文件，三层叠起来：出厂的、系统区的、管理员家目录里的；改只写家目录那一层，同名的盖住下面的。英文版：[personas.en.md](personas.en.md)。

| 方法 | 做什么 |
|---|---|
| [`persona.list`、`persona.get`](#personalistpersonaget) | 列出人格、读一个人格叠好的样子 |
| [`persona.read`](#personaread) | 读一份提示词的原文和版本 |
| [`persona.set`](#personaset) | 新建、改一个人格 |
| [`persona.delete`](#personadelete) | 删掉你那一层 |
| [`persona.avatar`、`persona.background`](#personaavatarpersonabackground) | 读头像、背景图 |
| [`preset.list`、`preset.get`](#presetlistpresetget) | 列出预设、读一个预设叠好的样子 |
| [`preset.set`](#presetset) | 新建、改一个预设 |
| [`preset.delete`](#presetdelete) | 删掉你那一层 |

**谁能调**：读的人、扩展都能调；`persona.set`、`persona.delete`、`preset.set`、`preset.delete` 只给人，扩展调回 `local_only`。

**编号**：小写字母开头，只有小写字母、数字、`-`、`_`，最长 64 个字符；不合的回 `bad_params`。

**名字和说明**（`name`、`summary`）照这个连接的语言挑一句。

## `persona.list`、`persona.get`

**`persona.list`**：没有参数。回应 `{"personas": [...]}`，照编号排。一项：

| 名字 | 说明 |
|---|---|
| `persona` | 编号 |
| `name`、`summary` | 名字、一句说明 |
| `avatar`、`background` | 头像、背景图的版本，没有的 `null` |
| `seed` | 主题色，小写的 `#rrggbb`，没有的 `null` |
| `seeds` | 从头像里挑的候选主色，最多 4 个，没有头像的 `null` |
| `background_opacity`、`background_focus` | 背景图露出多少（0 到 100）、焦点 `[x, y]`，没有的 `null` |
| `background_blur` | 背景图模糊多少（1 到 100），没有的 `null`，不模糊 |

文件写错了的那一个只有 `persona`、`problem`（照连接的语言说哪里错了），知道第几行的另带 `line`。

**`persona.get`**：参数 `persona`。回应：上面那些，另有：

| 名字 | 说明 |
|---|---|
| `prompts` | `{"persona", "reminders"}`：人设、角色扮演提示有没有字（布尔）；原文用 `persona.read` 读 |
| `examples` | 示范对话几轮 |
| `subagent` | 派子代理时能不能挑它 |
| `remove` | 删了会怎样：`restore`（回到出厂的样子）、`delete`（就没了）、`null`（你那一层没有，没什么可删） |

**出错**：`unknown_persona`；`persona_invalid`（文件写错了：`data.problem`、`data.message`、`data.line`）。

## `persona.read`

**参数**：`persona`；`prompt`：`persona`（人设）、`examples`（示范对话）、`reminders`（角色扮演提示）。

**回应**：`{"text", "version"}`：叠好的那一份的原文、你家目录那一层这一份的版本（没有的都是 `null`）。示范对话另带 `pairs: [{"user", "assistant"}…]`。

编辑器照它在原文上改，存的时候把 `version` 当 `expect` 带回 `persona.set`。

## `persona.set`

**参数**（`persona` 以外至少写一样）

| 名字 | 类型 | 说明 |
|---|---|---|
| `persona` | 字符串 | 编号；不写是新建，核心起一个 `persona-<n>` |
| `changes` | 数组 | 改 `persona.toml`，写法同 `config.set` 的 `changes`；键是 `persona.name`、`persona.summary`、`persona.subagent`、`memory.scope`、`appearance.seed` |
| `prompts` | 对象 | 提示词名（`persona`、`examples`、`reminders`）→ `{"text": <整份>}` 或 `{"unset": true}`；示范对话另可写 `{"pairs": [{"user", "assistant"}…]}`；都可带 `"expect": <版本>` 或 `null`（还没有） |
| `avatar`、`background` | 对象 | `{"blob": <哈希>}` 换成传上来的那一张，`{"unset": true}` 删掉你那一层的；可带 `expect` |
| `seed` | 字符串或 `{"unset": true}` | 主题色 `#rrggbb` |
| `background_opacity` | 0 到 100 或 `{"unset": true}` | 背景图露出多少 |
| `background_focus` | `[x, y]` 或 `{"unset": true}` | 背景图的焦点，各 0 到 100 |
| `background_blur` | 1 到 100 或 `{"unset": true}` | 背景图模糊多少，删掉就是不模糊 |

**回应**：同 `persona.get`，改完叠好的样子。几样一起查，有错什么都不写。和你那一层现在一样的不写。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 编号写法不对；一样都没写；提示词名不认识；一份里 `text`、`unset`、`pairs` 不是正好一个；示范对话有一句是空的 |
| `persona_invalid` | 改完的文件写错了：`data.problem`、`data.message`、`data.line` |
| `persona_conflict` | `expect` 对不上（`data.current` 是现在的样子或版本）；写到一半撞上有人手改（不带 `data.current`，重读再来） |
| `avatar_not_image`、`background_not_image` | 不是 PNG、JPEG、WebP |
| `avatar_too_big`、`background_too_big` | 头像超过 1 MiB 或 1024 像素，背景图超过 5 MiB 或 4096 像素；`data` 带量到的和上限 |
| `unknown_attachment` | `blob` 不在 |
| `local_only` | 扩展调了 |

## `persona.delete`

**参数**：`persona`。**回应**：`{"remains"}`：`true` 下面还有出厂、系统区的，回到它们的样子（界面写「恢复出厂」）；`false` 这个人格就没了。你家目录里的那一份挪进回收处，留 7 天。

**出错**：`nothing_to_delete`（你那一层本来就没有）；`local_only`。

## `persona.avatar`、`persona.background`

**参数**：`persona`。**回应**：`{"avatar" 或 "background", "media_type", "data"}`：版本、媒体类型、图片的 base64；没有的都是 `null`。

## `preset.list`、`preset.get`

**`preset.list`**：没有参数。回应 `{"presets": [...]}`，照编号排：`preset`、`icon`（Lucide 图标名，没有的 `null`）、`name`、`summary`。写错的只有 `preset`、`problem`、`line`。

**`preset.get`**：参数 `preset`。回应：

| 名字 | 说明 |
|---|---|
| `preset`、`name`、`summary`、`icon` | 同上 |
| `unlisted` | 叠好以后的 `on` 或 `off`：没列出来的功能默认开不开 |
| `features` | 一个个功能：`{"id", "name", "summary", "on", "by", "installed", "tools"}`；`by` 是开不开谁定的：`preset`（预设里写了）、`global`（预设没写，照技能、脚本、MCP 服务的全局开关，见 [`feature.set`](packages.md#toolset_loadingfeatureset)；只在 `unlisted` 是 `on` 的预设里有）、`default`（照 `unlisted`：`unlisted` 是 `off` 的预设没写的都是它，全局开关打开的也不开）；`tools` 是归它的工具 `{"name", "label", "on"}`，`label` 是给人看的名字（MCP 服务给了 `title` 的照它，没给的是去掉 `mcp__<服务名>__` 的工具名）。写了、没装的接在后面，`installed: false` |
| `remove` | 同 `persona.get` |

`features[].id` 是 `preset.set` 写 `features.<id>` 用的，`tools[].name` 是写 `tools.<name>` 用的，别露在界面上。

**出错**：`unknown_preset`；`preset_invalid`（`data.message`、`data.line`）。

## `preset.set`

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `preset` | 字符串 | 编号；不写是新建，核心起一个 `preset-<n>` |
| `changes` | 数组，必写、不能是空的 | 每项 `{"key", "value" | "unset": true, "expect"?}`；键是 `preset.name`、`preset.summary`、`preset.unlisted`、`features.<功能>`、`tools.<工具>` |

**回应**：同 `preset.get`。说明写空的字就是没有说明（盖住出厂的那句），`unset` 才回到出厂的。

**出错**：`bad_params`；`preset_invalid`；`preset_conflict`（`expect` 对不上，`data.current`）；`local_only`。

## `preset.delete`

**参数**：`preset`。**回应**：`{"remains"}`，同 `persona.delete`。开着的会话照旧用它开会话时的那一份。

**出错**：`nothing_to_delete`；`local_only`。
