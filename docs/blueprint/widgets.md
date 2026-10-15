## 界面块

状态：图纸（2026-10-15 主会话起草，设计 `33-界面扩展.md`）。X-15 上（清单）、X-15 中（内容和推送）、X-15 下（整页的命令）做完（2026-10-15）；两个头照它画版式图、接协议。终端同一天对过：四个位置放得下、不加新位置，内容补三样（`copy` 动作、进度条的 `mark`、程序改标题），上限和版式图在终端的 `tui.md`「界面块」。

### 是什么

扩展包往终端、网页上放的东西：首页一段、侧边栏一块、底栏一小段、一整页（设计 33 第三节）。清单声明有哪几块，内容由包的程序推给核心，核心转给在看的头，头自己画。协议里叫 `widget`：`view` 已经是会话的视图流（`view.md`）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-config/src/package/widget.rs` | 清单的 `[[widget]]`、`Manifest::slash_names`（这个包占的斜杠命令名：`[[slash]]` 的加整页的）（X-15 上） |
| `crates/miyu-endpoint/src/packages.rs` | `package.list` 的 `widgets`（X-15 上） |
| `crates/miyu-config/src/package/capability.rs` | 扩展能力多一个 `widgets`（X-15 上） |
| `crates/miyu-policy/src/widget.rs` | 内容的写法：六种段、上限、字里不许有的、三种动作、换掉的标题；照语言挑字（X-15 中，纯逻辑） |
| `crates/miyu-endpoint/src/widgets.rs` | 记内容、记谁在看，变了广播（X-15 中） |
| `crates/miyu-endpoint/src/widgets/set.rs` | `widget.set`、`widget.clear`（X-15 中） |
| `crates/miyu-endpoint/src/widgets/head.rs`、`entry.rs`、`crates/miyu-endpoint/src/subscriptions/widgets.rs` | 头的订阅：读参数、记成在看、推 `widget.changed`、限速；推给头的一块（X-15 中） |
| `crates/miyu-endpoint/src/commands/pages.rs`、`commands/templates.rs`、`commands/catalog.rs` | 整页的命令：认、列进菜单、打了回 `opens`（X-15 下） |
| `crates/miyu-endpoint/src/widgets/watch.rs`、`subscriptions/extension_config.rs` | 告诉程序 `widget.watch`、`widget.unwatch`，连接断了清它的内容；随推包自己的配置的任务一起起（X-15 中） |

### 对外的样子

**清单**（只有带 `[process]` 的包能写，别的种类 `wrong_kind`）：

```toml
[process]
capabilities = ["widgets"]

[[widget]]
id = "today"                              # 包里唯一：小写字母开头，小写字母、数字、-，最多 32 个
place = "sidebar"                         # home、sidebar、status、page
title = { en = "Weather", zh = "天气" }   # sidebar、page 必写；home、status 可以不写
scope = "session"                         # global（不写就是它）；session 只给 sidebar
command = "weather"                       # 只给 page：斜杠命令名，写法同斜杠命令（slash.md）
```

1. 一个包最多 16 块。`title` 写法同 `[package] name`（语言到字的表）。
2. 写错了报 `bad_widget`，报在那一段，`detail` 是哪一格（例如 `widget.today.place`）：编号写法不对、重了，位置不认，侧边栏、整页没写标题，`scope` 不认、`session` 写在侧边栏以外，`command` 写法不对、写在整页以外、和这个包的 `[[slash]]` 或别的整页重了，超过 16 块（`detail` 是 `widget`），没在 `[process] capabilities` 里写 `widgets`（`detail` 是 `widget`，报在第一段）。
3. `command` 和斜杠命令一个名字空间（`slash.md`「认」第 2 条）：装包查冲突（`packages.md`「冲突」的 `slash`），和内置的同名报 `slash_builtin`。

**扩展能力** `widgets`：在界面上显示它自己的内容。和别的扩展能力一样，开的时候人批（`extensions.md`「能力」）；名字、一句说明在内核那份 `human` 的 `extensions/capabilities/widgets`、`…/summary`。

**`package.list` 每一项多** `widgets`：`[{"id", "place", "title"?, "scope", "command"?}]`，`title` 照连接的语言挑；没声明的不带这一格。

**内容**：一块的内容是一个数组，一项一段：

```json
[
  {"type": "heading", "text": "本周"},
  {"type": "text", "text": "晴 23°C", "tone": "accent"},
  {"type": "pairs", "items": [{"label": "湿度", "value": "40%"}]},
  {"type": "progress", "value": 0.42, "text": "索引 42%"},
  {"type": "list", "items": [{"title": "构建 #128", "detail": "main", "right": "3m", "state": "doing", "action": {"call": "build.open", "params": {"id": 128}}}]},
  {"type": "buttons", "items": [{"label": "刷新", "action": {"call": "refresh"}}, {"label": "详情", "action": {"open": "detail"}}]}
]
```

| 段 | 格 | 上限 |
|---|---|---|
| `heading` | `text` | |
| `text` | `text`，`tone` 可以不写 | 只有它能换行（`\n`） |
| `pairs` | `items`：`{"label", "value", "tone"?}` | 50 对 |
| `progress` | `value`：0 到 1 的数；`text`、`mark` 可以不写：`mark` 是 0 到 1 的数，标在条上的那一处（上下文的压缩线这类） | |
| `list` | `items`：`{"title", "detail"?, "right"?, "state"?, "action"?}` | 200 条 |
| `buttons` | `items`：`{"label", "action"}` | 8 个 |

1. **字**：每一处字是一个字符串，或者语言到字符串的表（同 `title`），核心照连接的语言挑（这种语言、`en`、`zh`、`ja`）。`text` 段的字最多 2000 个，别的最多 200 个。不许有控制字符（`text` 段的 `\n` 除外）、改写方向的字符（U+202A–U+202E、U+2066–U+2069）。不认 Markdown、HTML：头一律当字画。
2. **语气** `tone`：`normal`（不写就是它）、`dim`、`accent`、`good`、`warn`、`bad`。**状态** `state`：`todo`、`doing`、`done`、`failed`、`off`。颜色、图标由头照自己的主题定。
3. **动作** `action`：`{"call": <方法名>, "params"?: <对象>}` 调这个包的程序登记的方法（`package-pages.md` 的 `package.methods`，名字写法同那里）；`{"open": <块的编号>}` 打开同一个包的一个 `page` 块；`{"copy": <字>}` 人点了把这段字写进剪贴板（最多 2000 个字，终端 2026-10-15 要的：会话的短编号点了复制完整编号，扩展的构建号、地址同理）。
4. **整块**：最多 64 段；编成 JSON 最多 64 KiB。`status` 的只能是一段 `text`、一行、显示宽度最多 40 列（中文这类宽字算两列，照 Unicode 的宽度）；`home` 的最多 8 段。头画的上限另由各自的版式图定（终端：首页最多 3 块、一块最多 6 行；侧边栏一块最多 10 行；底栏最多 3 格）。

**程序发给核心**（只在核心拉起的那个连接上；别的连接 `not_an_extension`）：

| 方法 | 参数 | 回应 |
|---|---|---|
| `widget.set` | `{"widget", "session"?, "title"?, "content"}`：`title` 换掉清单里的标题（写法同清单的，最多 200 个字；终端 2026-10-15 要的：「待办 2/5」「构建 3/5」这种会变的），不写的照清单的 | `{"kept": true}`；跟着会话的块，这个会话没人在看的，核心不留，回 `{"kept": false}` |
| `widget.clear` | `{"widget", "session"?}` | `{}`：回到没有内容 |

拒绝：清单里没有这一块 `unknown_widget`（`data.widget`）；全局的写了 `session`、跟着会话的没写，`bad_params`；内容写法不对 `bad_widget_content`，`data.path` 是错在哪（例如 `content[4].items[2].title`），`data.problem` 是 `type`、`missing`、`too_long`、`too_many`、`too_big`、`control_char`、`tone`、`state`、`value`、`action`、`place` 之一，原来的内容不动。

**核心告诉程序**（通知，不带 `id`）：

- `widget.watch {"widget", "session"?}`：这一块从没人看变成有人看了（跟着会话的照会话说）。程序这时取数据、推内容。
- `widget.unwatch {"widget", "session"?}`：最后一个人也不看了。程序停下手里为它干的活。
- 握手的回应写出去以后，核心把这一刻有人在看的各推一条 `widget.watch`：程序握了手马上就可能收到，和它自己发的请求的回应穿插着来，照 JSON-RPC 认 `id` 分开。
- 读得慢掉了队的，核心照这一刻有人在看的重对一遍：说过、现在没人看的补 `widget.unwatch`，新的补 `widget.watch`。

**头**：

1. `subscribe {"stream": "widgets", "session"?, "showing": [<位置>…], "page"?: {"package", "widget"}}`：`session` 是正在看的会话（首页不带；看子会话时写那个子会话）；`showing` 是屏上这一刻有的位置（`home`、`sidebar`、`status`）；`page` 是开着的那一整页。终端的写法（2026-10-15 对过）：首页 `["home", "status"]`，宽屏对话 `["sidebar", "status"]`，窄屏 `["status"]`，开着整页时 `showing` 是空的、只带 `page`。一个连接至多一个，换会话、换版面、开关整页都重新订阅，换一个新的；`unsubscribe {"stream": "widgets"}` 停掉。
2. 回应 `{"widgets": [<一块>…]}`：装着、开着的扩展声明的全部块，照包的编号、清单里的先后排；没带 `session` 的不列跟着会话的。
3. 一块：`{"package", "id", "place", "title"?, "scope", "command"?, "source", "icon"?, "running", "content"?}`。`title` 是这一刻生效的（程序推的，没推过的照清单的）；`source` 是包的名字（照连接的语言），`icon` 是包的图标（清单 `[package] icon` 的 Lucide 图标名，终端画不了的可以照一张自己的表换成字符，认不出的不画），头照它们让人认得出是谁放的（设计 33 第三节第 2 条）。`running` 是程序在不在跑。`content` 只有 `showing` 里的位置、开着的那一整页才带：有内容的是数组，程序还没推、清掉了的是 `null`。
4. 之后推 `widget.changed {"package", "id", "widget": <一块> | null}`：内容变了、程序起来了或停了、清单变了；关掉、卸掉的是 `null`。同一块最快每 250 毫秒推一次，中间的只留最新的。读得太慢、掉了队推 `resync {"stream": "widgets"}`，这个订阅停了，头重新订阅。
5. 字都照这个连接的语言挑好；头一律当字画（网页用 `textContent`，不用 `innerHTML`）。
6. 点了 `call` 动作：头发 `package.call {"package", "method": <call>, "params"}`（`package-pages.md`），回应不用看；拒了的（`method_failed`、`method_timeout`、`program_not_running`……）弹一句。点了 `open`：头打开那一整页（重新订阅，带上 `page`）。点了 `copy`：头把字写进剪贴板，不找核心。

**整页的命令**（X-15 下）：

1. `command.catalog` 多列声明了 `command` 的整页：`{"name", "aliases": [], "summary": <整页的标题>, "opens": {"package", "widget"}}`；自己放的同名模板盖住它。不算功能，不归预设管：只是开一页，不碰会话。场所会话（通讯平台的群、私聊）里不列。
2. `command.run` 打了这个名字：回 `{"command", "opens": {"package", "widget"}, "said": ""}`，头打开那一整页；不记 `command.ran`，不进会话日志。场所会话里照没有这个命令（`unknown_command`）。

### 怎么走

1. **谁在看**：核心照每个头的订阅算：全局的块，`showing` 里有它的位置（`page` 的是开着它）就算一个人在看；跟着会话的块，照订阅带的 `session` 一个会话一份。数从 0 变 1 发 `widget.watch`，从 1 变 0 发 `widget.unwatch`；头断了、取消订阅、重新订阅都照新的算。程序没在跑的时候不发，它起来握了手再补。
2. **记**：全局的块记最新的一份；跟着会话的只记有人在看的会话的，没人看了就扔。全在内存里，核心重启、程序重启都从没有开始（程序收到 `widget.watch` 再推）。
3. **清**：程序停了，它的块内容全清，推 `running: false`；关掉、卸掉、清单里去掉了的块推 `null`；升级以后清单变了的照新的推。
4. **限速**：给每个连接、每一块记上次推的时间，不到 250 毫秒的等到点再推最新的那一份；空闲时什么都不推。
5. **日志**：内容不进运行日志、不进事件。拒了的写法记一行 `WARN widget content rejected package=… widget=… problem=…`。

### 出错

| 代码 | 什么时候 |
|---|---|
| `bad_widget` | 清单的 `[[widget]]` 写错了（报在那一段） |
| `unknown_widget` | `widget.set`、`widget.clear` 的块不在它的清单里 |
| `bad_widget_content` | 内容写法不对 |

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-config/src/package/widget/tests.rs`（X-15 上） | 每一格、每一种写错、报在那一段、没声明能力、只有扩展能写、最多 16 块、整页的命令名和自己的斜杠命令、别的整页不重 |
| `crates/miyu-store/src/packages/conflicts/tests.rs` 的 `page_commands_count_like_slash_commands`、`crates/miyu-endpoint/tests/widgets.rs`（X-15 上） | 整页的命令名和别的包的斜杠命令撞了不装；`package.list` 列出来、标题照语言、没声明的不带；能力名照语言；`miyu check` 报撞了内置的 |
| `crates/miyu-policy/src/widget/tests.rs`、`crates/miyu-endpoint/src/widgets/tests.rs`（X-15 中） | 六种段的每一格、上限、控制字符和改方向的字符、语言表、两种动作、`status` 和 `home` 的限制、错在哪的路径 |
| `crates/miyu-endpoint/tests/widget_content.rs`（X-15 中） | 真核心、测试用的扩展：头订阅以后程序收到 `widget.watch`，推的内容转到头、照语言挑；换会话、换版面照新的算，最后一个走了 `widget.unwatch`；没人看的会话不留；限速只推最新的；写法不对的拒、原来的不动；程序停了内容清空、新的程序握了手又被告诉一遍；关掉推 `null`；订阅的参数不对 `bad_params`；登记表：只有第一个人来、最后一个人走时说，跟着会话的没人看不留 |
| `crates/miyu-endpoint/tests/widget_commands.rs`（X-15 下） | 菜单列出整页的命令、带 `opens`，会话里照样列；打了回 `opens`、日志一条不多；自己放的模板盖住它；关着的扩展不列不认；场所会话里 `unknown_command`（装包时撞名在 X-15 上守着） |

### 起草时定的

- 有人看才叫程序干活（设计 33 W5）：程序不用定时刷新；跟着会话的块只为有人看的会话算。
- 头说清屏上有哪几个位置：窄屏的终端不该叫醒只放侧边栏的程序。
- 内容只在内存里：它是程序这一刻的状态，程序重启后会重新推；记盘没有用处，还得清。
- 限速放在核心：程序推得再快，头每 250 毫秒最多重画这一块一次；头那边照旧一帧 33 毫秒。

### 还没有的

- 链接、图片（设计 33 第七节）。
- 没有程序的包放固定的块。
- 成员的头：现在只有管理员的头订阅得到，随多用户那一段。
