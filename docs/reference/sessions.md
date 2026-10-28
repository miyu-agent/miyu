# 会话：`session.*`、`view.*`

会话是一段对话：一份只追加的事件日志，核心照它算出现在的样子。头造会话、往里说话、订阅它的事件来画界面。英文版：[sessions.en.md](sessions.en.md)。

| 方法 | 做什么 |
|---|---|
| [`session.create`](#sessioncreate) | 造一个会话 |
| [`session.list`](#sessionlist) | 列出会话 |
| [`session.send`](#sessionsend) | 说一句话 |
| [`session.interrupt`](#sessioninterrupt) | 打断在进行的回合 |
| [`session.withdraw`](#sessionwithdraw) | 不打断，退回一条排着队的话 |
| [`session.tools`](#sessiontools) | 她下一轮能用的工具 |
| [`session.context`](#sessioncontext) | 上下文一块块各占多少 |
| [`session.usage_parts`](#sessionusage_parts) | 累计用量照上下文的块分开 |
| [`session.answer`](#sessionanswer) | 回答一次确认，或者一组提问 |
| [`session.revert`、`session.unrevert`](#sessionrevertsessionunrevert) | 撤销、恢复最近一次撤销 |
| [`session.redo`](#sessionredo) | 重做最后一轮 |
| [`session.compact`](#sessioncompact) | 手动压缩上下文 |
| [`session.clear`](#sessionclear) | 清空上下文 |
| [`session.recap`](#sessionrecap) | 要一句回顾 |
| [`session.set_permission_level`](#sessionset_permission_level) | 切权限级别、只读开关 |
| [`session.set_workspace`](#sessionset_workspace) | 换工作目录 |
| [`session.set_meta`](#sessionset_meta) | 改标题、置顶 |
| [`session.terminal`、`session.set_terminal`](#sessionterminalsessionset_terminal) | 终端会话：拿到它、指到别的会话 |
| [`session.configure`](#sessionconfigure) | 换模型、人格、预设 |
| [`session.fork`](#sessionfork) | 从一个会话分叉出子会话 |
| [`session.delete`](#sessiondelete) | 删除会话 |
| [`view.page`](#viewpage) | 按页读历史 |
| [`view.detail`](#viewdetail) | 一次工具调用的完整差异、一次压缩的摘要原文 |

场所会话（通讯平台上的群、私聊）另有几个方法，见 [场所](venues.md)。

## 通用

- **会话编号**是 UUIDv7 字符串，照造的先后排。
- **谁能调**：下面的方法人都能调。扩展照它的账号能看到的会话调，另要能力：读的（`session.list`、`session.recap`、`session.tools`、`session.context`、`session.usage_parts`、`view.page`、`view.detail`）要 `events.read`；造会话、说话、打断、撤销、恢复、重做、压缩、清空、换配置、分叉、改标题要 `sessions.drive`；`session.answer`、`session.set_permission_level`、`session.set_workspace`、`session.terminal`、`session.set_terminal`、`session.delete` 只给人（`local_only`）。扩展不带 `as` 说的话记成它自己（`harness`，名字是包的编号），它不能写 `from`。
- **会话没在跑的先载入**：核心重启过、闲够了退下的会话，第一次用到时自动载入，不用头操心。
- **改了什么看推送**：多数方法只回 `{}` 或几条事件的序号，变化本身照事件推给订阅着这个会话的头（[推送](pushes.md)）。
- **找会话的出错**，每个带 `session` 的方法都可能回：

| `data.reason` | 什么时候 |
|---|---|
| `session_not_found` | 没有这个会话，删了的也是 |
| `session_broken` | 会话载入不了：日志坏了、读不了 |
| `session_stopped` | 会话停了（写不进去、出了 bug）；再发一次会重新载入 |
| `session_sealed` | 会话封存了（分叉时写了 `seal_after`、跑满了），改对话的命令不收 |
| `restoring` | 撤销还没做完时来的命令（照常碰不到） |

## `session.create`

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `cwd` | 字符串 | 头的工作目录，人看到的写法，例如 `~/src/miyu`。不写的用这个会话自己的默认工作区；远程的连接不能写（施工 Y-1 下） |
| `persona` | 字符串或 `null` | 用哪个人格；`null` 是不用人格；不写照配置 `persona.default` |
| `preset` | 字符串或 `null` | 用哪个预设；不写照 `preset.default`，都没有是 `full`；`null` 是无工具：一件工具都不给（施工 P-16 下；以前的写法，照旧认。新开的选「无功能」写 `none`，施工 P-17） |
| `model` | 字符串 | `<供应商>/<模型>` 或 `@<池>`；不写照 `models.chat` |
| `memory` | `persona`、`session`、`off` | 记忆跟着人格、只在这个会话里、不用；不写照人格的设置 |
| `dirs` | 字符串的数组 | 另外加进来的目录，和工作目录一样能读能写 |
| `chosen` | 布尔，默认 `false` | 照收不看（以前用来区分人选的）：太宽的 `cwd`（根目录；Windows 上还有家目录）一律照用、回应带 `wide`，能随便写的范围是这个会话自己的默认工作区 |
| `oneshot` | 布尔，默认 `false` | 一次性的会话（命令行 `miyu ask` 那种） |
| `images` | 布尔，默认 `true` | 你显示得了图片吗 |
| `titles` | 布尔，默认 `true` | 自动起标题吗：`false` 的从此不自动起，人手改照旧改得了 |
| `project` | 字符串 | 在哪个项目里建（[项目](projects.md)）：没写的 `persona`、`preset` 照项目的，建好了记一条 `session.meta_changed`（回应的 `events` 是 `[1, 2]`） |

**回应**

| 名字 | 说明 |
|---|---|
| `session` | 新会话的编号 |
| `events` | `[1]`：`session.created` 那一条；带了 `project` 的是 `[1, 2]` |
| `cwd` | 会话实际在哪个目录干活，人看到的写法（家目录下的写成 `~/…`，同 `session.list`） |
| `wide` | `true`：目录太宽（根目录；Windows 上还有家目录、包含数据目录的）、照人选的用着，能随便写的是这个会话的默认工作区 |
| `untrusted_project` | 这个目录有项目配置、还没决定信不信任的，写它在哪 |

```json
→ {"jsonrpc":"2.0","id":"c1","method":"session.create","params":{"cwd":"~/src/miyu"}}
← {"id":"c1","jsonrpc":"2.0","result":{"cwd":"~/src/miyu","events":[1],"session":"01a14aca-d2c4-768c-acc6-a67e35106d22"}}
```

同一个 `id` 再发，交回上一次造的那一个，不再造。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 人格、预设编号写法不对；`memory` 不是那三种；`model` 不是字符串 |
| `unknown_persona`、`persona_invalid` | 没有这个人格；人格的文件写错了（`data.problem` 说哪一层、哪个文件第几行） |
| `unknown_preset`、`preset_invalid` | 同上，预设的 |
| `unknown_model` | `model` 解析不出：写法不对、没有这家供应商、没有这个池 |
| `dir_too_wide` | `dirs` 里有一条太宽（家目录、根目录、包含数据目录的） |
| `internal_error`（`-32603`） | 核心造不出来 |

## `session.list`

**参数**：`oneshot`（布尔，`true` 只要一次性的）、`dream`（布尔，`true` 只要整理会话，`false` 不要整理会话，不写都列）、`limit`（最多几个，不写是全部）。

**回应** `{"sessions": [<一项>…]}`，从新到旧。一项：

| 名字 | 说明 |
|---|---|
| `session` | 编号 |
| `cwd` | 工作目录，人看到的写法 |
| `last_active` | 最后一条事件的时刻 |
| `oneshot` | 是不是一次性的 |
| `parent` | 子会话的父会话，主会话是 `null` |
| `title` | 标题，没有的不写 |
| `named` | `true`：标题是人起的 |
| `preview` | 没有标题时：第一句话的第一行，最多 50 个字 |
| `pinned` | `true`：置顶了 |
| `busy` | `true`：有回合在进行 |
| `persona`、`preset` | 用哪个人格、预设，没有的不写；无工具的 `preset` 是 `null`，另带 `no_tools: true`（施工 P-16 下） |
| `tokens` | 连同它派出去的子会话（一层层往下）一共用了多少 token：每次模型调用（主请求、起标题、压缩都算）的输入（没命中、缓存读、缓存写）加输出，和终端底栏的 `Σ` 同一个算法；还是 0 的不写 |
| `context` | `{"used", "window"?}`：上下文用了多少 token（最近一次主请求的输入：没命中、缓存读、缓存写，同终端底栏、会话状态的上下文），压缩以后、还没说过话的不写；`window` 是那一次请求的模型照这时的配置查到的上下文窗口，查不到的不写 |
| `project` | 在哪个项目，不在的不写 |
| `kind`、`dream` | 整理会话（整理记忆跑在里面的会话）才有：`kind` 是 `"dream"`，`dream` 是 `{"persona"?, "session"?, "manual"?, "from"?}`：哪个人格、哪个会话的记忆，人叫的写 `manual: true`，在哪个会话里叫的写 `from`；这一趟跑完了的多 `result`：`{"given", "revised", "retired", "summary", "stopped"?}`，新记、改了、作废了几条，有没有写摘要，叫停的写 `stopped: true`，没跑完的没有。主列表一般不列它们（传 `dream: false`），记忆页照 `dream` 列那一份记忆的 |
| `origin` | 终端集成开的会话（`session.terminal` 开的）才有，是 `"terminal"`：头的会话列表照它写「终端集成」。以前开的没有 |
| `forked` | fork 子代理的会话才有，是 `true`：头照它换图标。普通子代理、主会话没有 |
| `default_workspace` | 这个会话的默认工作区，绝对路径：主会话是它自己的 `…/home/<账号>/workspace/<会话编号>/`，子会话、分叉出来的是主会话的那一个。`cwd` 和它一样的可以写「默认工作区」 |

只列本机的会话；场所会话不在里面（见 [场所](venues.md) 的 `venue.sessions`）。要一直跟着最新的，订阅 `sessions` 流（[连接](connection.md)）。

**出错**：`bad_params`；读不了会话目录 `internal_error`。

## `session.send`

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `text` | 字符串，必写 | 要说的话 |
| `attachments` | 数组 | 附件，每项是 `blob.put` 的回应里的 `blob`、`name`、`media_type`，可另带 `path`（原来在本机哪儿） |
| `urgent` | 布尔，默认 `false` | 急着插话：她正忙的，下一步就看到 |
| `prefix` | `"keep"` 或 `"compact"` | 前缀要变时人选的：直接发送、压缩上下文并发送（下面「前缀要变」） |
| `from` | 字符串 | 你是别的 agent 程序：写上你的名字，这句话记成你说的，不是本人 |
| `cwd` | 字符串 | 先换到这个工作目录再说（终端无缝集成：每一轮的工作目录跟着 shell 走）：查法、太宽的 `wide` 同 [`session.set_workspace`](#sessionset_workspace)；和现在一样的什么都不记，换了的推 `session.workspace_changed`。场所会话不收 |
| `as`、`venue` | 对象 | 只给场所会话，见 [场所](venues.md) |

**回应**：`events`（这一句的序号）、`cwd`（换过以后的）、`untrusted_project`（同 `session.create`）；换到太宽的目录多 `wide: true`。`cwd` 写错的（`path_unreadable`、`not_a_directory`、`path_forbidden`）整句不收。

1. 她闲着，这一句开一轮；正忙，排队（`urgent` 的插进下一步）。开的那一轮 `turn.started` 的 `cause` 是这一条的 `id`，照它认出是你开的。
2. 这一句落了盘就回，不等她答完：她怎么答看推送。
3. 附件先查再找会话：附件的 blob 要在、照内容再认一遍（见 [附件](blob.md)）。

**前缀要变**（握手 `caps.prefix_ask: true` 的头才有）：属主的工具说明、提示词覆盖、人格、预设目录里的文件改了（技能不算：会话中途技能变了走消息尾巴，前缀不变），下一轮的请求前缀会变、缓存失效。会话空着时开新一轮（`session.send`、`session.redo`、开轮的 `command.run`）不带 `prefix` 的，回 `prefix_changing`，`data.files` 是改了的文件（照数据根写的相对路径，最多 20 个），多的在 `data.more`。给人弹「直接发送」「压缩上下文并发送」「取消」，选了再带 `"prefix": "keep"` 或 `"compact"` 发一次。`compact` 先压缩再说，压不了的回压缩的拒绝（例如 `nothing_to_compact`），这句没发。放行过的这一批不再问。

```json
→ {"jsonrpc":"2.0","id":"m1","method":"session.send","params":{"session":"01a14aca-…","text":"看看这张图","attachments":[{"blob":"sha256:9f86d0…","name":"shot.png","media_type":"image/png"}]}}
← {"id":"m1","jsonrpc":"2.0","result":{"cwd":"~/src/miyu","events":[42]}}
```

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 附件缺了格、写法不对；`from` 不是字符串或去掉控制字符以后是空的 |
| `empty_message` | 没有字、没有附件 |
| `unknown_attachment` | 附件的 blob 核心里没有 |
| `attachment_too_big` | 附件里有超了上限的图 |
| `venue_session` | 对场所会话说话却没带 `as` |

## `session.interrupt`

**参数**：`session`；`queued`（必写）：排着队还没发的话怎么办，`send` 打断以后马上发，`return` 退回来，`keep` 留着不发。

**回应**：`{"events": [<这一次追加的事件>…]}`。

**出错**：`not_running`：没有回合在进行。

## `session.withdraw`

不打断这一轮，退回一条排着队、她还没听到的话。

**参数**：`session`；`message`（可以不写）：退哪一条，那句话的 `message.user` 的序号；不写的退回最新的一条。

**回应**：`{"events": [<那条 message.withdrawn 的序号>]}`。推送照 `session.interrupt` 退回的一样：一条 `message.withdrawn`，`messages` 里只有这一条；视图里那一句标 `withdrawn: true`。这一轮照跑，回完了由剩下排着的接着开。

**出错**：`already_heard`：写的那一条不在队里，请求已经带上了（或者由它接着开了一轮）；`nothing_queued`：没写哪一条，又没有排着的。

## `session.tools`

她下一次请求会拿到的工具。

**参数**：`session`（可以不写：不写的照新会话会拿到的算）；不写 `session` 时可带 `persona`（`null` 是无人格）、`preset`（`null` 是无工具），同 `session.create`。

**回应**：`{"tools": [{"name", "source": {"kind", "id", "name"}, "summary"?}]}`，照发给模型的先后（名字的字节序）。`kind` 是 `builtin`、`package`、`mcp`、`script`；`source.name` 照连接的语言挑。`summary` 只有脚本有。工具一律全量（施工 FL-1），以前只露名字的那一格 `loaded: false` 不再出现。

```json
{"tools":[{"name":"edit","source":{"kind":"builtin","id":"basesystem","name":"基础系统"}},{"name":"mcp__notes__search","source":{"kind":"mcp","id":"notes","name":"notes"}}]}
```

**出错**：`bad_params`（写了 `session` 又写了 `persona`、`preset`）；找会话的那几种。

## `session.context`

这个会话这时的上下文一块块的：system、工具说明、示范对话、检查点摘要、对话各占多少。只在调用时算，不影响会话。

**参数**：`session`（可以不写：不写的照新会话会拿到的算）；不写 `session` 时可带 `persona`（`null` 是无人格）、`preset`（`null` 是无工具），同 `session.create`。

**回应**：`{"window"?, "line"?, "used"?, "basis", "anchor"?, "estimated_total", "parts": [...]}`。`window`、`line` 是上下文窗口和压缩线（没交过限额的不写）；`used` 是这时用了多少，口径同底栏；`basis` 是 `measured`（照最近一次请求供应商报的 `anchor` 起算）或 `estimated`（全是本地估的）。`parts` 每块 `{"part", "label", "tokens", "children"?}`，`part` 是稳定的编号，`label` 照连接的语言，是 0 的块不交。有 `used` 的各块已经照它摊好，加起来正好是 `used`（摊成 0 的不交）；`estimated_total` 是摊之前本地估的和。

块的编号：`system`（下面照拼的先后 `system.persona`、`system.venue`、`system.group`、`system.profile`、`system.project`、`system.core`、`system.clicall`、`system.show`、`system.skills`、`system.knowledge`、`system.style_lock`、`system.appended:<包>`；老会话里和这时的字对不上的那几截并成 `system.other`）、`tools`（下面 `tools.builtin`、`tools.package:<包>`、`tools.mcp:<服务>`、`tools.scripts`）、`demos`、`summary`、`messages`（下面 `messages.user`、`messages.assistant`、`messages.memory`（下面 `messages.memory.resident`、`messages.memory.recall`）、`messages.knowledge`、`messages.facts`、`messages.project_docs`（仓库里的 AGENTS.md）、`messages.reports`（子代理、后台命令的回报）、`messages.tool_results`）。包、服务的 `label` 是原名。

```json
{"window":128000,"line":102400,"used":45231,"basis":"measured","anchor":41000,"estimated_total":43800,"parts":[{"part":"system","label":"系统提示","tokens":6100,"children":[{"part":"system.persona","label":"人设","tokens":4300},{"part":"system.core","label":"核心说明","tokens":900},{"part":"system.skills","label":"技能目录","tokens":900}]},{"part":"messages","label":"对话","tokens":37700,"children":[{"part":"messages.user","label":"你说的","tokens":4200},{"part":"messages.assistant","label":"AI 说的","tokens":33500}]}]}
```

**出错**：`bad_params`（写了 `session` 又写了 `persona`、`preset`）；找会话的那几种；新会话拼不出来的 `internal`。

## `session.usage_parts`

这个会话累计用了多少，照上下文的块分开（和 `session.context` 同一套块），外加一块 AI 输出。只算主请求；压缩、子代理、回顾这些照用途用 `usage.query` 查。

**参数**：`session`（必写）；`tree`（布尔，可以不写）连它派的子会话一起算。

**回应**：`{"requests", "input", "output", "parts": [...]}`。`requests` 主请求几次，`input` 报的输入（没命中、命中、写缓存）加起来，`output` 输出加起来。`parts` 的块和 `session.context` 一样，最后多一块 `output`；加起来正好是 `input + output`。每次请求的输入照那一刻各块的比例摊开再加起来；命中、没命中不分开摊，所以命中会有一部分算到新内容头上。

```json
{"requests":503,"input":80000000,"output":203822,"parts":[{"part":"system","label":"系统提示","tokens":220000},{"part":"tools","label":"工具说明","tokens":1900000},{"part":"messages","label":"对话","tokens":77880000},{"part":"output","label":"AI 输出","tokens":203822}]}
```

**出错**：`bad_params`；找会话的那几种。

## `session.answer`

她做要确认的事时会停下来问（推送 `tool.approval_requested`），她用 `ask_user` 提问时也会等（推送 `question.asked`）。用它回答。

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `call` | 字符串，必写 | 那一次调用的 `call_id` |
| `decision` | `once`、`session`、`deny` | 回答确认：允许这一次、这个会话都允许、拒绝 |
| `reason` | 字符串 | 拒绝的理由，她看得到；只有拒绝能带 |
| `attachments` | 数组 | 拒绝时附的图、文件，写法同 `session.send`（`blob.put` 交回的 `blob`、`name`、`media_type`，可带 `path`）；跟着理由给她；只有拒绝能带 |
| `answers` | 数组 | 回答提问：照题目先后一题一项 `{"picked": [选的选项标题…], "text": <自己写的>, "notes": <备注>, "attachments": [<同上>]}`，几格都可不写；附件跟着那一题给她 |
| `nested` | 正整数 | 她在命令里经 `miyu call` 调的工具要确认时，请求里带 `nested`（序号）：回答时照写。不写也行，答的就是那次命令下面正在问的那一件 |

`decision`、`answers` 正好写一个。**回应**：`{"events": [序号…]}`。

**选 `session` 记住的是什么**：推送里的 `rule`（没有就不能选 `session`），紧凑的 JSON：`tool` 哪件工具，`write` 放行写的几个目录（连同下面的）。命令要越过沙盒时，`write` 是工作目录，工具认得出命令前缀的另有 `prefix`（例如 `"docker ps"`、`"npm run"`）：选了以后，这个会话里工作目录在这个目录下面时，同一前缀的命令越过沙盒不再问。连起来的几段前缀不一样、开头有变量赋值、包了 `sudo`、`env`、`sh -c` 这类一层的命令没有 `prefix`，照旧每次问。头可以把 `prefix` 显示成「以后 `docker ps` 开头的命令在这个目录不再询问」。

```json
{"prefix":"docker ps","tool":"shell","write":["/home/me/proj"]}
```

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 两样都写或都不写；提问带了 `reason` 或外面的 `attachments`；`decision` 不是那三种；附件缺格、写法不对 |
| `unknown_attachment`、`attachment_too_big` | 同 `session.send`：附件的 blob 没有、太大 |
| `not_asking` | 这个调用没在等回答：答过了、结束了、等的不是这一种 |
| `no_rule` | 选了 `session`，这次确认却没有能记住的范围 |
| `unexpected_reason` | 允许却带了理由、附件 |
| `bad_answer` | 回答和题目对不上：条数不对、选了没有的、单选选了几项 |

## `session.revert`、`session.unrevert`

撤销：把最后一轮（或者从某一轮起）从对话里撤掉，她改过的文件不动。恢复：撤回最近一次撤销。

**参数**：`session`；`revert` 另可写 `turn`：从哪一轮起撤（那一轮 `turn.started` 的序号），不写撤最后一轮。

**回应**

| 名字 | 说明 |
|---|---|
| `events` | 这一次的事件：`turn.reverted` 或 `turn.unreverted` |
| `cwd` | 会话的工作目录（真实位置） |
| `turns` | 撤了（恢复了）几轮 |
| `said` | 第一轮里人说的那句话的第一行，没有的不写 |
| `compactions`、`clears` | 只有撤销：撤掉的几轮里有几次压缩、清空，上下文回到了那之前；0 的不写 |
| `jobs` | 只有撤销：停掉的后台任务 `[{"job", "what", "title"}]`，没有的不写 |

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | `turn` 写了 0 |
| `turn_running` | 有回合在进行：先打断 |
| `nothing_to_revert` | 一轮都没有 |
| `unknown_turn` | 那一轮不在现在的历史里 |
| `nothing_to_unrevert` | 没有能恢复的：没撤过，或者撤了以后开过新的一轮、压缩过 |

## `session.redo`

撤掉最后一轮，把开它的话再发一次，开新的一轮。网页里「编辑上一句」也走它。

**参数**：`session`；`text`：换掉开这一轮那句话的字（不写照原来的）；`attachments`：换掉它的附件（不写照原来的，空数组是不要附件）；`prefix`：同 `session.send`「前缀要变」。

**回应**：同撤销，`events` 最后是重发的每一句的序号。新的一轮 `turn.started` 的 `cause` 是这一条的 `id`。

**出错**：`not_redoable`（最后一轮不是人说话开的，或者一轮都没有）、`turn_running`、`empty_message`（换完一块都不剩），附件的同 `session.send`，`prefix_changing`。

## `session.compact`

**参数**：`session`；`instructions`：人附的要求，例如「重点保留数据库设计的讨论」。

**回应**：`{"events": [<那一轮 turn.started 的序号>]}`，开了就回，不等压完。进度看推送里的 `compaction.progress`、`compaction.done`、`context.compacted`、`turn.ended`。

**出错**：`turn_running`；`nothing_to_compact`（上一次压缩以后没有新的内容）。

## `session.clear`

清空上下文：开一轮只记一个空的检查点，不请求模型。待办也一起清掉。

**参数**：`session`。**回应**：同 `session.compact`；回应到的时候，那一轮的开头、空的检查点、结束都已经推过来了。撤掉这一轮，上下文回到清空以前。

**出错**：`turn_running`；`nothing_to_clear`（本来就是空的）。

## `session.recap`

要一句回顾：这个会话在做什么、做完了什么、卡在哪。单独请求一次模型，只喂最近几轮的对话。

**参数**：`session`。**回应** `{"text", "upto", "cached"}`：那一句、照到第几条、是不是交回的上一句（之后没有新内容，没请求）。

成了的，订阅着的头都会先收到 `model.called`（`purpose` 是 `recap`）和 `session.recapped`。

**出错**：`nothing_to_recap`（她还一句话都没说过）；`recap_failed`（请求出错，或者回复里没有字）。

## `session.set_permission_level`

**参数**：`session`；`level`：`workspace`（工作区）或 `full`（完全放开）；`read_only`：只读开关。改哪样写哪样，至少写一个。

**回应**：`{}`。变了的推 `session.policy_changed`。收紧的当场生效；放宽的下一次请求生效。改成完全放开之前要不要问人一声，是头的事。

**出错**：`bad_params`：两样都没写；`level` 不是那两种。

## `session.set_workspace`

**参数**：`session`；`cwd`：新的工作目录（绝对路径或 `~` 开头）；`dirs`：新的加进来的目录，整份换掉（空数组是去掉全部）。至少写一个。

`cwd` 写账号的工作区本身（握手的 `host.workspace`）就是回到这个会话自己的默认工作区：换成它（会话列表、订阅回应的 `default_workspace`）。

**回应**：`{"cwd", "dirs"}`，太宽的另带 `wide: true`。变了的推 `session.workspace_changed`；这一轮照旧，下一轮开始照新的。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 两样都没写 |
| `path_unreadable` | 目录不在、读不了 |
| `not_a_directory` | 是文件 |
| `path_forbidden` | 落在 Miyu 的数据目录里（账号自己的默认工作区除外） |
| `dir_too_wide` | `dirs` 里有一条太宽 |

## `session.terminal`、`session.set_terminal`

终端会话：在 shell 里说的话进的那个一直续下去的会话（终端无缝集成用，设计 `22-命令行.md` 第七节）。每个账号一个，带了 `terminal` 键的每个终端各一个（施工 T-22）；哪个算、什么时候换新的由核心定。

**`session.terminal`**：参数 `cwd`（可以不写）：要新开时在哪个目录，没写的照默认工作区；`fresh`（可以不写，默认假）：真的不管指着谁，照下面第 3 条开一个新的、指过去（`/new`）；`terminal`（可以不写）：哪个终端，头生成的键，1 到 128 个字、没有控制字符，不合的拒 `bad_params`，带了的照这个键各记一个指针，不带的用账号原来那一个；`create`（可以不写，默认真）：假的只问不开，下面第 2、3 条的情形回 `{"session": null}`，不新开、不挪指针（终端刚开、还没说话时问一下在哪个会话），和 `fresh: true` 一起写的拒 `bad_params`。回应 `{"session"}`：

1. 指着的会话还在、是这个账号自己的本机主会话（不是子会话、场所会话、一次性的），就是它。
2. 它闲置超过 `terminal.idle_new_after` 分钟（0 是不换）：开一个新的，指针挪过去，回应多 `renewed: {"last"}`（旧会话最后一次动静的时刻），旧会话不动。指过去（`session.set_terminal`、新开的）的那一刻也算一次动静：主动切到一个很久没说话的会话，闲置从切过去时算起。
3. 没指过、指着的删了、不算数的：开一个新的，回应多 `created: true`。

新开的照 `terminal.persona`、`terminal.preset`（没设的跟随 `persona.default`、`preset.default`）。同一时刻几个连接一起要（同一个键），只开一个。键各记最后一次用到的时刻，7 天没用的写指针时顺手清掉，最多留 64 个（最久没用的先清）；清掉的键再来当没指过。

**`session.set_terminal`**：参数 `session`：把终端会话指到它（`/sessions` 选中的）；`terminal`（可以不写）：改哪个终端的指针，写法同上。只收这个账号自己的本机主会话，别的拒 `session_not_found`。回应 `{"session"}`。

## `session.set_meta`

**参数**：`session`；`title`：新标题，1 到 200 个字，`null` 是去掉标题；`pinned`：`true` 置顶、`false` 取消；`project`：项目编号是移进去，`null` 是移出来（只有本机的主会话能进项目：场所会话、子代理、分叉出来的拒 `bad_params`，`data.field` 是 `project`）。至少写一个。置顶和项目是两格：拖进项目时要不要取消置顶，头自己另写 `pinned`。

**回应**：`{}`。变了的推 `session.meta_changed`。没起名的会话，核心在第一轮答完以后自己起一个标题，同样推 `session.meta_changed`（`by` 是内核）。

**出错**：`bad_params`：一样都没写；标题空的、超过 200 个字；`project` 写空字符串、这个会话不能进项目（`data.field` 是 `project`）。`unknown_project`：没有这个项目。

## `session.configure`

换这个会话用的模型、人格、预设，下一轮开始生效。

**参数**：`session`；下面三格至少写一格：
- `model`：`<供应商>/<模型>` 或 `@<池>`；`null` 是不再钉着哪一个，回到跟着配置的 `models.chat`（每一轮照那时的配置，配置换了跟着换）。
- `persona`：人格编号；`null` 是无人格。
- `preset`：预设编号；`null` 是换成无工具（施工 P-16 下）。

换人格、预设时先压缩一轮：核心马上照旧人格把前面的对话压成摘要，下一轮换上新的，从摘要接着聊。跟着人格的记忆从这一轮起记在新人格那一间，以前的留在原来那一间。

**回应**：`{}`。变了的推 `session.policy_changed`；换模型生效的那一刻推 `model.changed`。`session.list`、`subscribe` 回应里的 `persona`、`preset` 马上照换成的写（换成无工具的写 `preset: null`、`no_tools: true`）。

**出错**：`bad_params`（三格都没写、空字符串、类型不对；子代理换人格、预设，`data.field` 是那一格）；`unknown_model`；`unknown_persona`；`unknown_preset`；`turn_running`：换人格、预设时有回合在进行（等这一轮做完）。

## `session.fork`

从一个主会话分叉出子会话：子会话带着它到分叉点为止的历史接着说，前面的请求前缀和它逐字节一样，缓存照用。

**参数**：`session`（父会话，要是主会话）；`seal_after`：跑完几轮就封存（1 起），不写不封存。

**回应**：`{"session": <子会话编号>, "upto": <分叉点的序号>}`。父会话正在跑一轮的，分叉点是这一轮开始之前。

**出错**：`bad_params`：父会话是子代理、分叉出来的；`seal_after` 不是 1 起的整数。

## `session.delete`

挪进回收处，留 7 天。它派出去的子会话一起删。

**参数**：`session`。**回应**：`{}`，挪好了才回。订阅着它的头收到 `resync`。

**出错**：`turn_running`：有回合在进行（先打断）；`internal_error`：挪不动。

## `view.page`

按页读历史，从最新往前翻，页的边界落在两轮之间。

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `before` | 正整数 | 只要序号小于它的；不写是最新一页。往前翻拿上一页的 `first` |
| `turns` | 1 到 50，默认 20 | 最多几轮 |
| `view` | 布尔，默认 `false` | `true` 交给界面直接画的条目（视图投影），不交原始事件 |

**回应**：`events`（或者 `entries`）、`more`（前面还有没有），有内容的另带 `first`、`last`；超过 1 MiB 少给了轮数的带 `capped: true`；这一页里报完、更早派出的任务带 `jobs`。

最新一页的 `last` 就是这一刻的最后一条：接着 `subscribe {"after": last}` 只接新的。

**出错**：`bad_params`；`session_not_found`；`session_broken`。

## `view.detail`

**参数**：`session`；`call`（一次工具调用的 `call_id`）、`job`（一个后台任务的编号）或 `compaction`（一次压缩替代到的序号），正好写一个；可以另写 `nested_after`（整数）：嵌套调用只要序号比它大的，加上它以前还在跑的。

**回应**

- 调用：`{"files": [...], "output"}`。`output` 是结果原文，不截；`files` 一个文件一项：`path`、`action`（`write` 或 `trash`）、`diff`（统一格式，上下文 3 行）、`added`、`removed`，算不出差异的写 `skipped`（`too_big`、`binary`、`missing`）。
- 压缩：`{"summary"}`：那一次的摘要原文。
- 脚本里调过工具的（`miyu call`）：调用的回应多 `nested`，后台任务的回应只有 `{"nested"}`。照序号一项一个：`n`（序号）、`name`、`title`（`{name, object}`，显示名照连接的语言）、`args`（参数原文）、`state`（`running`、`ok`、`error`、`denied`、`needs_approval`、`cancelled`），有的另带 `started`（在跑的开始时刻）、`took_ms`、`reason`（拒绝、出错的那一句）、`result`（`{blob, bytes}`，交给脚本的那份结果，用 `blob.get` 取）。那次 `shell` 还在跑、还没结果的，也交 `{"nested"}`。

同一次调用的结果不会变，问一次就够；下面调的工具在跑时会多，用 `nested_after` 只拿新的。

**出错**：`bad_params`；`session_not_found`；`unknown_call`（没有这次调用的结果，或者没有这次压缩）；`session_broken`。
