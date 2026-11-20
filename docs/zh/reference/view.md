# 视图流：界面直接画的条目和会话状态

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

核心把会话日志计算成一条条供界面直接画出来的条目（视图投影），头（连接核心的界面程序，例如终端界面、网页界面）按这些条目画出界面即可，不需要自己从事件推算。获取条目的方式有两种：用 `view.page {view: true}` 翻页，或者用 `subscribe {stream: "view"}` 订阅视图流。

## 订阅

`subscribe {"session", "stream": "view"}` 的回应带有最新一页的条目（和 `view.page {view: true}` 相同：`entries`、`more`、`first`、`last`、`capped`）、整份会话状态 `status`，以及事件流订阅回应里的那几个字段。之后核心会推送下面这些消息：

| 推送 | 是什么 |
|---|---|
| `view.add {session, entry, after}` | 新的一条条目，排在 `after` 那一条的后面（`null` 表示排在最前面） |
| `view.update {session, entry}` | 一条条目换成新的内容（状态变了）；位置也变了时带 `after` |
| `view.append {session, id, text}` | 正在写的回复、思考的文字，或者正在写参数的工具的 `args`：新的文字接在后面 |
| `view.hidden {session, ids, hidden}` | 撤销、恢复时：把这几条藏起来，或者重新显示出来 |
| `view.remove {session, id}` | 拿掉一条条目（打断时丢掉的半截内容、一轮结束时还没有压缩完的压缩） |
| `view.status {session, status}` | 会话状态变了：推送整份状态 |
| `resync {session, stream: "view"}` | 连接掉了队：需要重新订阅 |

视图流不推送原始事件。条目里的文字按连接的语言写好；连接的语言改了以后，之后的条目使用新的语言。

## 条目

每一条条目是一个对象，共有的字段是：`id`、`kind`、`turn`（属于哪一轮）、`hidden`（是否被撤销藏起来了）、`at`（开始的时刻）。

**编号**按日志里的位置计算，翻页和订阅算出来的编号相同：`m<序号>` 是一条消息，`b<seen>.<块号>` 是回复里的一块，`g…` 是时间线的一段，`c<序号>` 是一次压缩，`e<序号>` 是一轮的收尾和旁白，`s<序号>.<第几张>` 是 AI 发出去的图，`x<序号>.<第几个>` 是只在视图流里出现的旁白。

| `kind` | 格 |
|---|---|
| `user` | `text`；`attachments`（`kind`、`name`、`media_type`、`blob`，图片另有 `width`、`height`，带了路径的附件有 `path`；只给了路径的附件 `kind` 是 `file`，没有 `media_type`、`blob`，有 `name`、`path`、`path_only: true`，头写了大小时还有 `size`）；`from`（从其他地方来的话的来处，写法和事件的 `by` 相同；场所里的人说的话还带 `name`：它是那条消息的 `venue.name`（群名片、昵称），只能当作显示名，不可信，头和平常一样对它转义，例如画成「昵称（qq:号）」；没有写名字时不带）；`poke: true`（场所里的人戳了 AI 一下，和那条消息的 `venue.poke` 相同；没有戳时不写）；`poke_text`（戳一戳的后缀，和 `venue.poke_text` 相同，例如「的小脑袋」：不可信，头和平常一样对它转义；没有写时不带）；`level`；`queued: true`（这句话还在排队，AI 还没有听到）；`withdrawn: true`（排着队的话被退回了：头把文字放回输入框）；`round`（长期目标的续轮是第几轮）；`shell`（人用 `!` 输入的命令，`text` 是那一行命令：还没有运行完时是 `{"running": true}`，运行完以后是 `exit` 或 `signal`、`text`（输出）、`omitted`、`duration_ms`，异常结束时另有 `reason`：`timed_out`、`interrupted`、`aborted`） |
| `reply` | `text`（Markdown 原文，解析和渲染由头负责）；`open: true`（还在写）；`persona`：这一轮是哪个人格回复的（人格的编号，不使用人格时是 `null`）。会话中途换了人格时，以前的回复仍然记成旧人格：人更换人格的那一刻先记下来，真正换上（下一个回合开始）以后的回复才记成新人格 |
| `media` | AI 发出去的一张图：`image`、`alt`、`sticker` |
| `voice` | AI 说出来的一段语音（工具返回的 `sent` 里写了 `voice` 的那一项，编号和图片一样是 `s<序号>.<第几项>`）：`blob`、`media_type`、`name`、`duration_ms`，以及开始这一轮的是谁：`by`（开始这一轮的那句话的 `by`，写法和事件的相同；在本机的头上说的话带 `head`，到点叫醒的是 `schedule`；这一轮没有触发它的那句话时不写）、`cause`（开始这一轮的那条请求的编号，和 `turn.started` 的 `cause` 相同）。头画一个播放条；如果 `cause` 是自己发出的那条 `session.send` 的编号，说明这是在这个头上说的话，当场播放；如果 `by` 是 `schedule` 或者没有写，说明这是没有人开口的一轮，打开着这个会话的头都播放；在其他头上说的话只画播放条，不播放 |
| `group` | 时间线的一段（几步思考和工具）：`steps`、`open`、`summary`、`summary_en`（收起时显示的那一行）、`took_ms`、`failed` |
| `thought` | 一步思考：`group`、`text`、`open`、`took_ms` |
| `tool` | 一步工具：`group`、`call`、`name`、`hosted`（模型在供应商那边自己运行的工具写 `true`，见下面「供应商侧工具」；核心执行的工具不写）、`title`、`state`、`args`、`diff`（`{added, removed}`）、`images`、`job`、`approval`（需要人确认的请求：`access`、`rule`、`detail`，决定以后多出 `decision`、`reason`、`by`；带 `expires_at` 时，到那一刻还没有人确认就自动拒绝，头根据它画倒计时，这时被拒绝的 `by` 是 `kernel`）、`took_ms`、`files`（`[{path, action}]`，`action` 是 `created`、`changed`、`trashed`）、`skill`、`shown`（模型用 `show` 展示的文件：`{path, name, media_type, size, modified, title?, language?, width?, height?}`，`modified` 是展示那一刻文件的修改时间，写法和事件的 `at` 相同；文件内容不在核心里，本机的头按 `path` 读取，远程的头用 [`session.shown`](blob.md#sessionshown) 带上这一步的 `call` 分段读取；同一个 `path` 再展示一次时，新的那一张换掉旧的）、`detail`（工具返回的结构化详情，例如搜到的几条结果；带 `kind` 的是三种通用形状 `links`、`table`、`images` 之一，见 [扩展](extensions.md#toolcalltoolcancel)）、`output`（结果原文的开头一段：最多 100 行、8 KiB；被截断时带 `output_cut: true`，整份用 `view.detail` 获取）、`questions`（提问的那一步问的那一组问题，提问时就有）、`nested`（脚本里调用过工具的 `shell`：`{count, last}`，`last` 是序号最大的那一个 `{n, name, title, state, started?}`；头在那一行后面写 `· #n 显示名`，整份用 `view.detail` 获取）、`progress`（运行中最新的一行进度：工具输出里以 `miyu-progress: ` 开头的那一行，去掉前缀，最长 200 个字；同一步最多每 0.5 秒推送一次；结果到了以后去掉，翻页、重新订阅时没有这一项） |
| `end` | 一轮的收尾：`reason`、`model`、`endpoint`、`took_ms`、`usage`、`speed`（`{output, ms}`：这一轮每次主请求的输出 token 数，和从首字到结束的毫秒数，分别加起来，不算调用工具、等人、旁路请求的时间；没有记下用时时不写；整段攒齐以后一口气到达的那一次请求不算在内——从首字到结束不到 100 毫秒，或者算出来每秒超过 5000 个），出错时另有 `error`、`explain` |
| `notice` | 旁白：`what` 的取值见下面 |

**`tool` 的 `state`**：`preparing`（还在写参数）、`running`、`ok`、`error`、`denied`、`cancelled`、`skipped`。

**`tool` 的 `title`**：`{"name", "object", "said", "via"}`：分别是给人看的工具名、对象（路径、命令的短标题……）、结果的那一句话、经谁完成（例如网络搜索的来源 `Exa`，写在名字后面的括号里；没有时不写）。被拒绝的步骤（`state` 是 `denied`）不论是哪件工具，都带结果的那一句话，说明是谁拦下的、为什么（例如 `acme-guard 拦下了：理由`、`只读模式，未执行`）；结果没有带给人看的说法时不写。

**供应商侧工具**（`hosted: true`）：这是模型在供应商那边自己运行的搜索、读网页，核心不执行，画成同样的一步工具。`name` 是供应商自己的工具名，原样保留；核心能识别的工具（`search_web`、`web_search`、`WebSearch`，`read_url_content`、`web_fetch`、`WebFetch`），`title` 按核心的网络搜索、抓网页写显示名和对象，不能识别的工具 `title.name` 写原名，没有对象。`title.via` 说明是由谁运行的：写这条回复的供应商的显示名（配置里写的，或者目录里那一家的），没有显示名时写供应商编号。状态：参数还在写时是 `preparing`，参数收全以后是 `running`，结果到了是 `ok`（供应商说失败了时是 `error`）；回复已经落盘、但还没有结果时，如果回复被打断了写 `cancelled`，否则写 `ok`。这类工具没有 `approval`、`said`，也不会是 `denied`。`output`、`output_cut` 按供应商交来的结果填写，整份用 `view.detail` 带上这一步的 `call` 获取；`call` 在回复落盘以后才有。

**收起那一行**（`summary`）：由几段 `{"text", "tone"?}` 组成，`tone` 是 `added`、`removed`、`error`。例如 `Ran 2 commands · 1 edit +3 -1 · 5s`。

**`notice` 的 `what`**：

| `what` | 格 |
|---|---|
| `compaction` | `trigger`（`auto`、`manual`、`overflow`）、`state`（`running`、`done`、`failed`）、`written`、`expected`、`prepared`、`before`、`after`、`took_ms`、`usage`、`instructions`、`truncated`（摘要请求太长、截掉了最老的几组时，这一项是截掉的组数，最早的那一段没有进入摘要；没有截过时不写）；摘要原文用 `view.detail` 获取 |
| `cleared` | 上下文被清空了 |
| `paused` | 自动压缩暂停了：`reason`、`failures` |
| `reverted` | 撤销了几轮：`turns`、`said`、`jobs` |
| `job` | 一件后台任务结束了：`job`、`job_kind`、`title`、`mark`（`done`、`failed`、`stopped`）、`report`、`command`、`output`（`{blob, chars}`）、`exit_code`、`signal`、`took_ms`、`nested`（后台命令的脚本调用过工具时才有，和 `tool` 的相同） |
| `lines` | 被监视的后台命令输出了一批行（`job.lines`）：`job`、`title`、`command`、`text`（这一批的全文，最多 4000 个字，截断过的中间有一行写明省略了多少）、`lines`（有几行）、`omitted`（截掉了几个字）。这不表示任务结束，`jobs` 里那一行仍然在运行 |
| `answered` | 一组提问得到了回答：`questions`、`answers`、`by` |
| `persona` | 会话中途换了人格、预设，在真正换上的那一刻出现：`persona: {from, to}`、`preset: {from, to}`，换了哪一项就写哪一项；写的是编号，没有时是 `null` |
| `recap` | 一段回顾：`text`、`covers` |
| `model` | 更换了模型：`endpoint`、`model`、`from`、`why`（`failover`、`replaced`） |
| `workspace` | 更换了工作区：`cwd`、`dirs` |
| `peer` | 正在等待的会话空闲下来了，或者等不到了：`session`、`reason`、`status` |
| `retitle_failed` | 人要求的重起标题（[`session.retitle`](sessions.md#sessionretitle)）没有成功，标题没有变：`text`（按连接的语言写好的那一句：`更改标题失败`）、`error`（那一次请求的错误）。这一条只在视图流里出现，翻页、重新订阅时没有 |

## 会话状态 `status`

| 名字 | 说明 |
|---|---|
| `state` | `idle`、`running`、`waiting`（有还没有了结的确认、提问） |
| `waiting` | `[{what: "approve" | "ask", entry, call}]` |
| `since` | 这一轮开始的时刻 |
| `waited_ms` | 这一轮等人（确认、回答提问）已经等完的时间一共多少毫秒，同时等几件事只算一次；没有等过时不写。等人的时间不算进用时 |
| `waiting_since` | 正在等人的这一段从什么时候开始；没有在等时不写。用时按「现在 − `since` − `waited_ms` − (现在 − `waiting_since`)」计算，等人时用时停住；条目里的 `took_ms` 也扣掉了等人的时间 |
| `doing` | 正在做什么：`thinking`、`writing`、`tool`（带 `entry`）；`retrying`（`attempt`、`limit`、`at`、`class`、`message`）；`compacting`（`trigger`、`written`、`expected`） |
| `context` | `{window, compaction_line, context, used}`：分别是上下文窗口（设了会话的上下文上限时，是生效的窗口，取模型的窗口和上限里较小的那个）、压缩线、会话的上下文上限（没有设时不写）、最近一次用了多少（那次请求的输入加输出；刚压缩完时是压缩以后的量） |
| `speed` | `{output, ms}`：最近一次有速度的请求的输出 token 和毫秒数（整段攒齐以后一口气到达的那一次不算，也不会替换掉它，计算口径和 `end` 的 `speed` 相同） |
| `cooling_until` | 所有候选都在冷却时，最早恢复的时刻 |
| `preparing` | 后台正在提前压缩：`{seen, since}` |
| `usage` | 本会话累计的用量，写法和 `usage.query` 的一行相同（连同 `converted`、`converted_parts`） |
| `model`、`permission`、`todos`、`workspace`、`persona`、`preset` | 和订阅回应里的那几个字段相同；会话中途换了人格、预设时会跟着推送。无工具的会话 `preset` 是 `null`，另外带 `no_tools: true`，和会话列表相同 |
| `vision` | 会话钉住的多模态模型（`session.configure` 的 `vision`）：模型或 `@池`，原样保存；钉住以后马上推送，没有钉住（跟着 `models.vision`）时不写 |
| `compaction_line` | 会话钉住的压缩触发线（`session.configure` 的 `compaction_line`）：是窗口的百分之几；钉住以后马上推送，没有钉住（跟着 `compaction.line`）时不写。生效的压缩线要看 `context.compaction_line` |
| `todos_done` | 待办全部做完而被清空时，这一项是清空前的那一份（头先打勾、停一下再收起） |
| `goal` | 长期目标：`{text, limit, state, reason, round, since, usage}`，`state` 是 `running`、`paused`、`completed`、`blocked`、`waiting` |
| `jobs` | 这个会话直接派出去的任务（正在运行的，和最近做完的 20 个）：`{job, what, title, state, started, ended, session, command, exit_code, signal, why, nested, fork}`（`nested` 和 `tool` 的相同，只有脚本调用过工具的后台命令才有；`fork` 只有 fork 子代理才有，值是 `true`，头根据它换图标），`state` 是 `running`、`done`、`failed`、`stopped`、`aborted`。场所会话里桥分出去的并行支线（`session.fork`）也列在这里，排在任务后面：`{job, what: "agent", fork: true, session, title, state, started, ended?}`，`job`、`session` 都是支线的会话编号，`title` 是支线的标题，没有标题时是第一句话的第一行，`state` 是 `running`（正在运行，还没有运行完第一轮）、`done`、`stopped`（被打断）；正在运行的支线全部列出，做完的只保留最近的 20 条；点开时按 `session` 订阅它的视图流 |
| `title` | 会话的标题：起了标题才有这一项；标题改了时推送一份新的状态。场所会话不在会话列表里，头根据它写会话名 |
