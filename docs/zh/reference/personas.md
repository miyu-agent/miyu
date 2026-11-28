# 人格与预设：`persona.*`、`preset.*`

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

人格说的是「AI 是谁」：人设、示范对话、角色扮演提示、头像、主题色。预设说的是「AI 能用什么」：打开哪些功能、关掉哪几件工具。两样都是文件，分几层叠在一起：出厂的、软件包带的、你自己的（数据根下的 `root/personas/`、`root/presets/`）；修改时只写入你自己的那一层，同名的文件会盖住下面几层的。

| 方法 | 做什么 |
|---|---|
| [`persona.list`、`persona.get`](#personalistpersonaget) | 列出人格；读取一个人格叠好以后的样子 |
| [`persona.read`](#personaread) | 读取一份提示词的原文和版本 |
| [`persona.set`](#personaset) | 新建或修改一个人格 |
| [`persona.delete`](#personadelete) | 删除你自己的那一层 |
| [`persona.avatar`、`persona.background`](#personaavatarpersonabackground) | 读取头像、背景图 |
| [`preset.list`、`preset.get`](#presetlistpresetget) | 列出预设；读取一个预设叠好以后的样子 |
| [`preset.set`](#presetset) | 新建或修改一个预设 |
| [`preset.delete`](#presetdelete) | 删除你自己的那一层 |

**谁能调用**：读取类的方法人和扩展都能调用；`persona.set`、`persona.delete`、`preset.set`、`preset.delete` 只给人调用，扩展调用时返回 `local_only`。

**编号**：编号以小写字母开头，只能包含小写字母、数字、`-`、`_`，最长 64 个字符；不符合这些要求时返回 `bad_params`。

**名字和说明**（`name`、`summary`）按这个连接的语言挑选一句。

## `persona.list`、`persona.get`

**`persona.list`**：没有参数。回应是 `{"personas": [...]}`，按编号排序。每一项的字段如下：

| 名字 | 说明 |
|---|---|
| `persona` | 人格的编号 |
| `name`、`summary` | 名字和一句话的说明 |
| `avatar`、`background` | 头像、背景图的版本，没有时是 `null` |
| `seed` | 主题色，写法是小写的 `#rrggbb`，没有时是 `null` |
| `seeds` | 从头像里挑出的候选主色，最多 4 个，没有头像时是 `null` |
| `background_opacity`、`background_focus` | 背景图露出多少（0 到 100）、背景图的焦点 `[x, y]`，没有时是 `null` |
| `background_blur` | 背景图模糊多少（1 到 100），没有时是 `null`，表示不模糊 |

文件写错了的人格只有 `persona`、`problem`（按连接的语言说明哪里写错了）两项，知道错在第几行时另外带上 `line`。

**`persona.get`**：参数是 `persona`。回应包含上面那些字段，另外还有：

| 名字 | 说明 |
|---|---|
| `prompts` | `{"persona", "reminders"}`：人设、角色扮演提示里有没有文字（布尔值）；原文需要用 `persona.read` 读取 |
| `examples` | 示范对话有几轮 |
| `subagent` | 派子代理时能不能选择这个人格 |
| `remove` | 删除以后会怎样：`restore`（回到出厂的样子）、`delete`（删除以后就没有了；出厂带的软件工程师这类会连同软件包一起卸载的人格也是这个值）、`null`（没有可以删除的内容） |

**出错**：`unknown_persona`；`persona_invalid`（文件写错了，附带 `data.problem`、`data.message`、`data.line`）。

## `persona.read`

**参数**：`persona`；`prompt` 是要读取的那一份：`persona`（人设）、`examples`（示范对话）、`reminders`（角色扮演提示）。

**回应**：`{"text", "version"}`：分别是叠好的那一份的原文，和你家目录那一层里这一份的版本（没有时都是 `null`）。示范对话另外带 `pairs: [{"user", "assistant"}…]`。

编辑器根据它在原文上修改，保存时把 `version` 作为 `expect` 带回 `persona.set`。

## `persona.set`

**参数**（除了 `persona` 以外至少填写一项）

| 名字 | 类型 | 说明 |
|---|---|---|
| `persona` | 字符串 | 人格的编号；不填写时新建一个人格，核心为它起一个 `persona-<n>` 的编号 |
| `new` | 布尔 | 和 `preset.set` 的这一项相同：编号已经存在时拒绝，返回 `persona_exists` |
| `changes` | 数组 | 修改 `persona.toml`，写法和 `config.set` 的 `changes` 相同；键是 `persona.name`、`persona.summary`、`persona.subagent`、`appearance.seed` |
| `prompts` | 对象 | 从提示词名（`persona`、`examples`、`reminders`）映射到 `{"text": <整份>}` 或 `{"unset": true}`；示范对话还可以写成 `{"pairs": [{"user", "assistant"}…]}`；每一份都可以带 `"expect": <版本>` 或 `null`（表示还没有这一份） |
| `avatar`、`background` | 对象 | `{"blob": <哈希>}` 表示换成上传的那一张图片，`{"unset": true}` 表示删除你那一层的图片；可以带 `expect` |
| `seed` | 字符串或 `{"unset": true}` | 主题色，写法是 `#rrggbb` |
| `background_opacity` | 0 到 100 或 `{"unset": true}` | 背景图露出多少 |
| `background_focus` | `[x, y]` 或 `{"unset": true}` | 背景图的焦点，两个值各是 0 到 100 |
| `background_blur` | 1 到 100 或 `{"unset": true}` | 背景图模糊多少，删除这一项就是不模糊 |

**回应**：和 `persona.get` 相同，内容是修改以后叠好的样子。几项修改会一起检查，只要有一项出错，就什么都不写入。和你那一层现在的内容相同的项不会写入。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 编号的写法不对；一项都没有填写；提示词名不能识别；一份提示词里 `text`、`unset`、`pairs` 不是正好有一个；示范对话里有一句是空的 |
| `persona_invalid` | 修改以后的文件有错误：附带 `data.problem`、`data.message`、`data.line` |
| `persona_conflict` | `expect` 对不上（`data.current` 是现在的样子或版本）；或者写到一半时碰上有人手动修改了文件（这时不带 `data.current`，需要重新读取再试一次） |
| `avatar_not_image`、`background_not_image` | 图片不是 PNG、JPEG、WebP 格式 |
| `avatar_too_big`、`background_too_big` | 头像超过 1 MiB 或 1024 像素，背景图超过 5 MiB 或 4096 像素；`data` 里带有量到的大小和上限 |
| `unknown_attachment` | `blob` 不存在 |
| `local_only` | 调用的一方是扩展 |

## `persona.delete`

**参数**：`persona`。**回应**：`{"remains"}`：`true` 表示下面还有出厂的、软件包带的层，人格回到它们的样子（界面上写「恢复出厂」）；`false` 表示这个人格就没有了。你的那一份会挪进回收处，保留 7 天。出厂带的软件工程师这类只存在于一个出厂包里的人格，在你那一层没有它的情况下，删除它就是卸载那个软件包（和 `package.remove` 相同），返回 `false`；可以用 `package.install {"package": "<包编号>"}` 重新安装回来。

**出错**：`nothing_to_delete`（你那一层本来就没有这个人格，它也不是能连同软件包一起卸载的人格）；`package_depended`（连同软件包卸载时，那个软件包正被其他软件包依赖着）；`local_only`。

## `persona.avatar`、`persona.background`

**参数**：`persona`。**回应**：`{"avatar" 或 "background", "media_type", "data"}`：分别是版本、媒体类型、图片的 base64；没有图片时都是 `null`。

## `preset.list`、`preset.get`

**`preset.list`**：没有参数。回应是 `{"presets": [...]}`。出厂的「全部功能」（`full`）和「无功能」（`none`）排在最前面，先是全部功能，再是无功能；其余的按编号排序。每一项有 `preset`、`icon`（Lucide 图标名，没有时是 `null`）、`name`、`summary`、`tokens`（用这个预设开新会话时，不算人格，system 和工具说明占多少 token，这个数在本地估算；算不出来时没有这一项）。写错了的预设只有 `preset`、`problem`、`line`。全部功能和无功能是不能修改的预设，这两项多一个 `fixed: true`，其他预设没有这一项；界面看到 `fixed: true` 时，不应该提供修改和删除。无功能里菜单能开关的功能全都关着。「无工具」（`preset: null`）不在列表里；开会话、换预设时仍然可以交 `preset: null`。

**`preset.get`**：参数是 `preset`。回应的字段如下：

| 名字 | 说明 |
|---|---|
| `preset`、`name`、`summary`、`icon`、`fixed` | 和上面相同 |
| `unlisted` | 叠好以后的 `on` 或 `off`：表示没有列出来的功能默认是否打开 |
| `unattended_steps` | 叠好以后 `[limits]` 里写的数：没有人看着时，一个回合最多请求几次模型；几层都没有写时是 `null`，按出厂的 500 计算 |
| `features` | 每一个功能是 `{"id", "name", "summary", "package", "on", "by", "installed", "tools"}`；`package` 是这个功能归属的软件包，用来按软件包分组，软件包的名字用 [`package.list`](packages.md) 查询（你自己放的脚本、MCP 服务、模板命令是 `~home`，不归属软件包的技能和没有安装的功能没有这一项）；`by` 表示功能是否打开是由谁决定的：`preset`（预设里写了）、`global`（预设没有写，按技能、脚本、MCP 服务的全局开关决定，见 [`feature.set`](packages.md#featureset)；只出现在 `unlisted` 是 `on` 的预设里）、`default`（按 `unlisted` 决定：`unlisted` 是 `off` 的预设里没有写的功能都是这个值，即使全局开关打开了也不打开）；`tools` 是归属这个功能的工具 `{"name", "label", "on"}`，`label` 是给人看的名字（MCP 服务给了 `title` 时使用它，没有给时是去掉 `mcp__<服务名>__` 的工具名）。预设里写了、但是没有安装的功能接在后面，带 `installed: false`。接入平台的软件包（清单里写了 `[connection]`）只列出本机会话拿得到的内容：一件 `venues` 里带 `local` 的工具都没有的功能不会列出，列出来的功能也只列出带 `local` 的工具；`sections` 也是这样 |
| `sections` | 把同样这些功能分好组、排好先后，界面按它画出来即可，不需要自己翻译、排序、识别编号。每组是 `{"kind", "id", "name", "on", "total", "items"}`：`kind` 是 `group`（出厂的组：基础系统、任务和定时、注入环境信息、人格、查资料，`id` 是组名）、`package`（安装的扩展带了几个功能时，各自成为一组，`id` 是包编号）、`extensions`（安装的扩展只带一个功能时合进这一组，一个扩展占一行，名字是包名；没有这样的扩展时不列出）、`skills`、`scripts`、`mcp`（没有内容时也会列出，`total` 是 0），按这个先后排列；`on`、`total` 分别是开着的功能和全部功能的个数。`items` 的每一项是 `{"kind": "feature", "id", "name", "on", "by"}`，同一组里重名的项会带上包名；`by` 多出一种取值 `group`（表示预设里写的是功能所在的软件包或组，而 `preset` 表示预设里写的是功能本身）；带几件工具的功能另外带 `tools: [{"id", "name", "on"}]`，只带一件工具的不带 |
| `remove` | 和 `persona.get` 相同 |

`features[].id` 用于在 `preset.set` 里写 `features.<id>`，`tools[].name` 用于写 `tools.<name>`；不要把它们显示在界面上。

**出错**：`unknown_preset`；`preset_invalid`（附带 `data.message`、`data.line`）。

## `preset.set`

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `preset` | 字符串 | 预设的编号；不填写时新建一个预设，核心为它起一个 `preset-<n>` 的编号 |
| `new` | 布尔 | 新建一个预设并自己写编号（需要和 `preset` 一起填写）：如果这个编号已经存在（出厂的、软件包带的、你那一层的），就拒绝并返回 `preset_exists`，不会去修改那一份 |
| `changes` | 数组，必写、不能是空的 | 每一项是 `{"key", "value" | "unset": true, "expect"?}`；键是 `preset.name`、`preset.summary`、`preset.unlisted`、`features.<功能>`、`tools.<工具>`、`limits.unattended_steps`（正整数） |

**回应**：和 `preset.get` 相同。说明写成空字符串就表示没有说明（会盖住出厂的那一句），只有 `unset` 才会回到出厂的说明。

**出错**：`bad_params`；`preset_invalid`；`preset_conflict`（`expect` 对不上，附带 `data.current`）；`preset_fixed`（要修改的是带 `fixed: true` 的预设，什么都不会写入）；`local_only`。

## `preset.delete`

**参数**：`preset`。**回应**：`{"remains"}`，和 `persona.delete` 相同。已经开着的会话仍然使用开会话时的那一份预设。

**出错**：`nothing_to_delete`；`preset_fixed`（要删除的是带 `fixed: true` 的预设，什么都不会删除）；`local_only`。
