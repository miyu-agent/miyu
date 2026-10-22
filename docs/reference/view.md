# 视图流：界面直接画的条目和会话状态

核心把会话日志算成一条条给界面画的条目（视图投影），头照它画，不用自己从事件算。拿法有两种：`view.page {view: true}` 翻页，`subscribe {stream: "view"}` 订阅视图流。英文版：[view.en.md](view.en.md)。

## 订阅

`subscribe {"session", "stream": "view"}` 的回应带最新一页的条目（同 `view.page {view: true}`：`entries`、`more`、`first`、`last`、`capped`）、整份会话状态 `status`，和事件流订阅回应的那几格。之后推：

| 推送 | 是什么 |
|---|---|
| `view.add {session, entry, after}` | 新的一条，排在 `after` 那一条后面（`null` 是最前） |
| `view.update {session, entry}` | 一条换成这样（状态变了）；位置变了的带 `after` |
| `view.append {session, id, text}` | 正在写的回复、思考的字，正在写参数的工具的 `args`：接在后面 |
| `view.hidden {session, ids, hidden}` | 撤销、恢复：这几条藏起、显示回来 |
| `view.remove {session, id}` | 拿掉一条（打断时丢掉的半截、一轮结束时还没压完的压缩） |
| `view.status {session, status}` | 会话状态变了：整份 |
| `resync {session, stream: "view"}` | 掉了队：重新订阅 |

视图流不推原始事件。字照连接的语言写好；连接的语言改了，之后的照新的。

## 条目

一条一个对象，共有的格：`id`、`kind`、`turn`（属于哪一轮）、`hidden`（撤销藏起的）、`at`（开始的时刻）。

**编号**照日志里的位置算，翻页、订阅算出来的一样：`m<序号>` 一条消息，`b<seen>.<块号>` 回复里的一块，`g…` 时间线的一段，`c<序号>` 压缩，`e<序号>` 一轮的收尾和旁白，`s<序号>.<第几张>` 她发出去的图，`x<序号>.<第几个>` 只在视图流里有的旁白。

| `kind` | 格 |
|---|---|
| `user` | `text`；`attachments`（`kind`、`name`、`media_type`、`blob`，图有 `width`、`height`）；`from`（别处来的话的来处）；`level`；`queued: true`（排着、她还没听到）；`withdrawn: true`（排着的被退回了：头把字放回输入框）；`round`（长期目标的续轮是第几轮） |
| `reply` | `text`（Markdown 原文，解析和渲染是头的事）；`open: true`（还在写）；`persona`：这一轮是哪个人格回的（编号，无人格的是 `null`）。会话中途换了人格的，以前的回复照旧是旧人格：人换的那一下先记着，真换上（下一个回合开始）以后的回复才是新的 |
| `media` | 她发出去的一张图：`image`、`alt`、`sticker` |
| `group` | 时间线的一段（几步思考和工具）：`steps`、`open`、`summary`、`summary_en`（收起那一行）、`took_ms`、`failed` |
| `thought` | 一步思考：`group`、`text`、`open`、`took_ms` |
| `tool` | 一步工具：`group`、`call`、`name`、`title`、`state`、`args`、`diff`（`{added, removed}`）、`images`、`job`、`approval`、`took_ms`、`files`（`[{path, action}]`，`action` 是 `created`、`changed`、`trashed`）、`skill`、`detail`（工具交的结构化详情，例如搜到的几条；带 `kind` 的是三种通用形状 `links`、`table`、`images`，见 [扩展](extensions.md#toolcalltoolcancel)）、`output`（结果原文的前面一段：最多 100 行、8 KiB；切了的带 `output_cut: true`，整份用 `view.detail` 取）、`questions`（提问那一步问的那一组，问的时候就有）、`nested`（脚本里调过工具的 `shell`：`{count, last}`，`last` 是序号最大的那一个 `{n, name, title, state, started?}`；头在那一行后面写 `· #n 显示名`，整份用 `view.detail` 取） |
| `end` | 一轮的收尾：`reason`、`model`、`endpoint`、`took_ms`、`usage`，出错的 `error`、`explain` |
| `notice` | 旁白：`what` 见下 |

**`tool` 的 `state`**：`preparing`（还在写参数）、`running`、`ok`、`error`、`denied`、`cancelled`、`skipped`。

**`tool` 的 `title`**：`{"name", "object", "said", "via"}`：给人看的工具名、对象（路径、命令的短标题……）、结果那一句、经谁做的（例如网络搜索的来源 `Exa`，写在名字后面的括号里；没有的不写）。

**收起那一行**（`summary`）：几段 `{"text", "tone"?}`，`tone` 是 `added`、`removed`、`error`。例如 `Ran 2 commands · 1 edit +3 -1 · 5s`。

**`notice` 的 `what`**：

| `what` | 格 |
|---|---|
| `compaction` | `trigger`（`auto`、`manual`、`overflow`）、`state`（`running`、`done`、`failed`）、`written`、`expected`、`prepared`、`before`、`after`、`took_ms`、`usage`、`instructions`；摘要原文用 `view.detail` 取 |
| `cleared` | 上下文清空了 |
| `paused` | 暂停了自动压缩：`reason`、`failures` |
| `reverted` | 撤销了几轮：`turns`、`said`、`jobs` |
| `job` | 一件后台任务了结了：`job`、`job_kind`、`title`、`mark`（`done`、`failed`、`stopped`）、`report`、`command`、`output`（`{blob, chars}`）、`exit_code`、`signal`、`took_ms`、`nested`（后台命令的脚本调过工具的，同 `tool` 的） |
| `answered` | 一组提问答了：`questions`、`answers`、`by` |
| `persona` | 会话中途换了人格、预设，真换上的那一刻：`persona: {from, to}`、`preset: {from, to}`，换了哪样写哪样；写编号，没有的是 `null` |
| `recap` | 一段回顾：`text`、`covers` |
| `model` | 换了模型：`endpoint`、`model`、`from`、`why`（`failover`、`replaced`） |
| `workspace` | 换了工作区：`cwd`、`dirs` |
| `peer` | 等的会话空下来了、等不到了：`session`、`reason`、`status` |

## 会话状态 `status`

| 名字 | 说明 |
|---|---|
| `state` | `idle`、`running`、`waiting`（有没了结的确认、提问） |
| `waiting` | `[{what: "approve" | "ask", entry, call}]` |
| `since` | 这一轮开始的时刻 |
| `waited_ms` | 这一轮等人（确认、回答提问）等完了的一共多少毫秒，同时等几件只算一次；没等过的不写。等人的时间不算用时 |
| `waiting_since` | 正在等人的这一段从什么时候起；没在等的不写。用时照「现在 − `since` − `waited_ms` − (现在 − `waiting_since`)」算，等人时停住；条目里的 `took_ms` 也刨掉了等人的时间 |
| `doing` | 正在做什么：`thinking`、`writing`、`tool`（带 `entry`）；`retrying`（`attempt`、`limit`、`at`、`class`、`message`）；`compacting`（`trigger`、`written`、`expected`） |
| `context` | `{window, compaction_line, used}`：上下文窗口、压缩线、最近一次用了多少（那次请求的输入加输出；刚压完的是压完的量） |
| `speed` | `{output, ms}`：最近一次的输出 token 和毫秒 |
| `cooling_until` | 候选都在冷却时最早恢复的时刻 |
| `preparing` | 后台在提前压缩：`{seen, since}` |
| `usage` | 本会话累计用量 |
| `model`、`permission`、`todos`、`workspace`、`persona`、`preset` | 同订阅回应的那几格；中途换了人格、预设的跟着推 |
| `todos_done` | 待办全部做完而清空时，清空前那一份（头打勾停一下再收） |
| `goal` | 长期目标：`{text, limit, state, reason, round, since, usage}`，`state` 是 `running`、`paused`、`completed`、`blocked`、`waiting` |
| `jobs` | 这个会话直接派出去的任务（在跑的和最近做完的 20 个）：`{job, what, title, state, started, ended, session, command, exit_code, signal, why, nested}`（`nested` 同 `tool` 的，脚本调过工具的后台命令才有），`state` 是 `running`、`done`、`failed`、`stopped`、`aborted` |
