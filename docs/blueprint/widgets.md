## 界面块

状态：图纸（2026-10-15 主会话起草，设计 `33-界面扩展.md`；第三节的位置等两个头对过）。施工 X-15 上、中、下照它做，两个头照它画版式图、接协议。

### 是什么

扩展包往终端、网页上放的东西：首页一段、侧边栏一块、底栏一小段、一整页（设计 33 第三节）。清单声明有哪几块，内容由包的程序推给核心，核心转给在看的头，头自己画。协议里叫 `widget`：`view` 已经是会话的视图流（`view.md`）。

### 在哪（施工时补）

| 代码 | 管什么 |
|---|---|
| `crates/miyu-config/src/package/widget.rs` | 清单的 `[[widget]]`（X-15 上） |
| `crates/miyu-config/src/package/capability.rs` | 扩展能力多一个 `widgets`（X-15 上） |
| `crates/miyu-policy/src/widget.rs` | 内容的写法：几种段、上限、字里不许有的、动作（X-15 中，纯逻辑） |
| `crates/miyu-endpoint/src/widgets.rs` | 记内容、算谁在看、告诉程序、推给头、限速（X-15 中） |

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

1. 一个包最多 16 块。`title` 写法同 `[package] name`（一个字，或者语言到字的表）。
2. 写错了报 `bad_widget`，报在那一段：`data.problem` 是 `id`、`duplicate`、`place`、`title`、`scope`、`command`、`too_many`、`capability`（没在 `[process] capabilities` 里写 `widgets`）之一。
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
| `progress` | `value`：0 到 1 的数；`text` 可以不写 | |
| `list` | `items`：`{"title", "detail"?, "right"?, "state"?, "action"?}` | 200 条 |
| `buttons` | `items`：`{"label", "action"}` | 8 个 |

1. **字**：每一处字是一个字符串，或者语言到字符串的表（同 `title`），核心照连接的语言挑（这种语言、`en`、`zh`、`ja`）。`text` 段的字最多 2000 个，别的最多 200 个。不许有控制字符（`text` 段的 `\n` 除外）、改写方向的字符（U+202A–U+202E、U+2066–U+2069）。不认 Markdown、HTML：头一律当字画。
2. **语气** `tone`：`normal`（不写就是它）、`dim`、`accent`、`good`、`warn`、`bad`。**状态** `state`：`todo`、`doing`、`done`、`failed`、`off`。颜色、图标由头照自己的主题定。
3. **动作** `action`：`{"call": <方法名>, "params"?: <对象>}` 调这个包的程序登记的方法（`package-pages.md` 的 `package.methods`，名字写法同那里）；`{"open": <块的编号>}` 打开同一个包的一个 `page` 块。
4. **整块**：最多 64 段；编成 JSON 最多 64 KiB。`status` 的只能是一段 `text`、一行、最多 40 个字；`home` 的最多 8 段。

**程序发给核心**（只在核心拉起的那个连接上；别的连接 `not_an_extension`）：

| 方法 | 参数 | 回应 |
|---|---|---|
| `widget.set` | `{"widget", "session"?, "content"}` | `{"kept": true}`；跟着会话的块，这个会话没人在看的，核心不留，回 `{"kept": false}` |
| `widget.clear` | `{"widget", "session"?}` | `{}`：回到没有内容 |

拒绝：清单里没有这一块 `unknown_widget`（`data.widget`）；全局的写了 `session`、跟着会话的没写，`bad_params`；内容写法不对 `bad_widget_content`，`data.path` 是错在哪（例如 `content[4].items[2].title`），`data.problem` 是 `type`、`missing`、`too_long`、`too_many`、`too_big`、`control_char`、`tone`、`state`、`value`、`action`、`place` 之一，原来的内容不动。

**核心告诉程序**（通知，不带 `id`）：

- `widget.watch {"widget", "session"?}`：这一块从没人看变成有人看了（跟着会话的照会话说）。程序这时取数据、推内容。
- `widget.unwatch {"widget", "session"?}`：最后一个人也不看了。程序停下手里为它干的活。
- 握手的回应写出去以后，核心把这一刻有人在看的各推一条 `widget.watch`。

**头**：

1. `subscribe {"stream": "widgets", "session"?, "showing": [<位置>…], "page"?: {"package", "widget"}}`：`session` 是正在看的会话（首页不带）；`showing` 是屏上这一刻有的位置（`home`、`sidebar`、`status`），终端窄屏没有侧边栏就不写 `sidebar`；`page` 是开着的那一整页。一个连接至多一个，换会话、换版面、开关整页都重新订阅，换一个新的；`unsubscribe {"stream": "widgets"}` 停掉。
2. 回应 `{"widgets": [<一块>…]}`：装着、开着的扩展声明的全部块，照包的编号、清单里的先后排；没带 `session` 的不列跟着会话的。
3. 一块：`{"package", "id", "place", "title"?, "scope", "command"?, "source", "icon"?, "running", "content"?}`。`source` 是包的名字（照连接的语言），`icon` 是包的图标，头照它们让人认得出是谁放的（设计 33 第三节第 2 条）。`running` 是程序在不在跑。`content` 只有 `showing` 里的位置、开着的那一整页才带：有内容的是数组，程序还没推、清掉了的是 `null`。
4. 之后推 `widget.changed {"package", "id", "widget": <一块> | null}`：内容变了、程序起来了或停了、清单变了；关掉、卸掉的是 `null`。同一块最快每 250 毫秒推一次，中间的只留最新的。读得太慢、掉了队推 `resync {"stream": "widgets"}`，这个订阅停了，头重新订阅。
5. 字都照这个连接的语言挑好；头一律当字画（网页用 `textContent`，不用 `innerHTML`）。
6. 点了 `call` 动作：头发 `package.call {"package", "method": <call>, "params"}`（`package-pages.md`），回应不用看；拒了的（`method_failed`、`method_timeout`、`program_not_running`……）弹一句。点了 `open`：头打开那一整页（重新订阅，带上 `page`）。

**整页的命令**（X-15 下）：

1. `command.catalog` 多列声明了 `command` 的整页：`{"name", "summary": <整页的标题>, "opens": {"package", "widget"}}`。不算功能，不归预设管：只是开一页，不碰会话。场所会话（通讯平台的群、私聊）里不列。
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

### 守着它的（施工时补）

| 测试 | 守什么 |
|---|---|
| `crates/miyu-config/src/package/widget/tests.rs`（X-15 上） | 每一格、每一种写错、报在那一段、没声明能力、只有扩展能写、最多 16 块 |
| `crates/miyu-policy/src/widget/tests.rs`（X-15 中） | 六种段的每一格、上限、控制字符和改方向的字符、语言表、两种动作、`status` 和 `home` 的限制、错在哪的路径 |
| `crates/miyu-endpoint/tests/widgets.rs`（X-15 中） | 真核心、测试用的扩展：头订阅以后程序收到 `widget.watch`，推的内容转到头、照语言挑；换会话、换版面照新的算，最后一个走了 `widget.unwatch`；没人看的会话不留；限速只推最新的；写法不对的拒、原来的不动；程序停了 `running: false`、关掉卸掉推 `null`；点按钮经 `package.call` 调到 |
| `crates/miyu-endpoint/tests/widget_commands.rs`（X-15 下） | 菜单列出整页的命令、带 `opens`；打了回 `opens`、不记日志；场所会话里不列、`unknown_command`；装包时和别的斜杠命令撞名拦下 |

### 起草时定的

- 有人看才叫程序干活（设计 33 W5）：程序不用定时刷新；跟着会话的块只为有人看的会话算。
- 头说清屏上有哪几个位置：窄屏的终端不该叫醒只放侧边栏的程序。
- 内容只在内存里：它是程序这一刻的状态，程序重启后会重新推；记盘没有用处，还得清。
- 限速放在核心：程序推得再快，头每 250 毫秒最多重画这一块一次；头那边照旧一帧 33 毫秒。

### 还没有的

- 链接、图片（设计 33 第七节）。
- 没有程序的包放固定的块。
- 成员的头：现在只有管理员的头订阅得到，随多用户那一段。
