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
| [`session.quota`](#sessionquota) | 供应商的额度：几个窗口用了多少、什么时候重置 |
| [`session.answer`](#sessionanswer) | 回答一次确认，或者一组提问 |
| [`session.revert`、`session.unrevert`](#sessionrevertsessionunrevert) | 撤销、恢复最近一次撤销 |
| [`session.redo`](#sessionredo) | 重做最后一轮 |
| [`session.compact`](#sessioncompact) | 手动压缩上下文 |
| [`session.clear`](#sessionclear) | 清空上下文 |
| [`session.recap`](#sessionrecap) | 要一句回顾 |
| [`session.set_permission_level`](#sessionset_permission_level) | 切权限级别、只读开关 |
| [`session.set_workspace`](#sessionset_workspace) | 换工作目录 |
| [`session.set_meta`](#sessionset_meta) | 改标题、置顶 |
| [`session.retitle`](#sessionretitle) | 让模型重起一个标题 |
| [`session.viewing`](#sessionviewing) | 头告诉核心自己开着哪个会话 |
| [`session.terminal`、`session.set_terminal`](#sessionterminalsessionset_terminal) | 终端会话：拿到它、指到别的会话 |
| [`session.configure`](#sessionconfigure) | 换模型、看图模型、人格、预设、思考强度、压缩触发线 |
| [`session.fork`](#sessionfork) | 从一个会话分叉出子会话 |
| [`session.delete`](#sessiondelete) | 删除会话 |
| [`session.prompts`](#sessionprompts) | 列出会话里问过的话 |
| [`session.export`](#sessionexport) | 把会话导出成原始包或者 Markdown |
| [`session.import`](#sessionimport) | 导入原始包 |
| [`view.page`](#viewpage) | 按页读历史 |
| [`view.detail`](#viewdetail) | 一次工具调用的完整差异、一次压缩的摘要原文 |

场所会话（通讯平台上的群、私聊）另有几个方法，见 [场所](venues.md)。

## 通用

- **会话编号**是 UUIDv7 字符串，照造的先后排。
- **谁能调**：下面的方法人都能调；关在沙盒里的场所会话（群、陌生人的私聊）人只读地看，读的几个照常，别的回 `session_not_found`（见[场所](venues.md)「主人看场所会话」）。扩展照它看得到的会话调（平台桥只看得到它的平台的场所会话，别的扩展看得到本机的、你自己的私聊），另要能力：读的（`session.list`、`session.recap`、`session.tools`、`session.context`、`session.usage_parts`、`session.quota`、`session.prompts`、`view.page`、`view.detail`）要 `events.read`；造会话、说话、打断、撤销、恢复、重做、压缩、清空、换配置、分叉、改标题、重起标题要 `sessions.drive`；`session.answer`、`session.set_permission_level`、`session.set_workspace`、`session.terminal`、`session.set_terminal`、`session.delete`、`session.export`、`session.import` 只给人（`local_only`）。扩展不带 `as` 说的话记成它自己（`harness`，名字是包的编号），它不能写 `from`。
- **会话没在跑的先载入**：核心重启过、闲够了退下的会话，第一次用到时自动载入，不用头操心。
- **改了什么看推送**：多数方法只回 `{}` 或几条事件的序号，变化本身照事件推给订阅着这个会话的头（[推送](pushes.md)）。
- **找会话的出错**，每个带 `session` 的方法都可能回：

| `data.reason` | 什么时候 |
|---|---|
| `session_not_found` | 没有这个会话，删了的也是 |
| `session_broken` | 会话载入不了：日志坏了、读不懂 |
| `session_read_only` | 会话日志中间坏了一行：坏行以前的用 `view.page` 照样看得到，会话载入不了、不能接着聊，要继续新开一个会话；`data.segment` 是哪一段（只是文件名），`data.line` 是第几行 |
| `session_unavailable` | 会话这一刻读不了：读日志时读写出错，日志没坏，过一会儿再试 |
| `session_stopped` | 会话停了（写不进去、出了 bug）；再发一次会重新载入 |
| `session_sealed` | 会话封存了（分叉时写了 `seal_after`、跑满了；人生会话结束了，见[人生会话](life.md)），改对话的命令不收 |
| `restoring` | 撤销还没做完时来的命令（照常碰不到） |

## `session.create`

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `cwd` | 字符串 | 头的工作目录，人看到的写法，例如 `~/src/miyu`。不写的用这个会话自己的默认工作区；远程的连接不能写 |
| `persona` | 字符串或 `null` | 用哪个人格；`null` 是不用人格；不写照配置 `persona.default` |
| `preset` | 字符串或 `null` | 用哪个预设；不写照 `preset.default`，都没有是 `full`；`null` 是无工具：一件工具都不给（以前的写法，照旧认；新开的选「无功能」写 `none`） |
| `model` | 字符串 | `<供应商>/<模型>` 或 `@<池>`；不写的照这一刻的 `models.chat` 记下（写的是池就记池），以后改 `models.chat` 这个会话照旧 |
| `vision` | 字符串 | 这个会话替看图用哪个模型：写法同 `model`；不写的跟着配置的 `models.vision`。造好以后记一条 `session.policy_changed`，同 `session.configure` 的 `vision` |
| `compaction_line` | 整数 | 这个会话的压缩触发线：窗口的百分之几，20 到 95；不写的跟着配置的 `compaction.line`。造好以后记一条 `session.policy_changed`，同 `session.configure` 的 `compaction_line` |
| `memory` | `persona`、`off` | 记忆跟着人格、不用；不写是跟着人格。别的值（以前的 `session` 也算）回 `bad_params` |
| `dirs` | 字符串的数组 | 另外加进来的目录，和工作目录一样能读能写 |
| `oneshot` | 布尔，默认 `false` | 一次性的会话（命令行 `miyu ask` 那种） |
| `images` | 布尔，默认 `true` | 你显示得了图片吗 |
| `titles` | 布尔，默认 `true` | 自动起标题吗：`false` 的从此不自动起，人手改照旧改得了 |
| `project` | 字符串 | 在哪个项目里建（[项目](projects.md)）：没写的 `persona`、`preset` 照项目的，建好了记一条 `session.meta_changed`（回应的 `events` 是 `[1, 2]`） |

**回应**

| 名字 | 说明 |
|---|---|
| `session` | 新会话的编号 |
| `events` | `[1]`：`session.created` 那一条；带了 `project` 的是 `[1, 2]`，带了 `vision`、`compaction_line` 的各再多一条（照这个先后，排在项目那一条后面） |
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
| `bad_params`（`-32602`） | 人格、预设编号写法不对；`memory` 不是那三种；`model`、`vision` 不是字符串；`compaction_line` 不是 20 到 95 的整数 |
| `unknown_persona`、`persona_invalid` | 没有这个人格；人格的文件写错了（`data.problem` 说哪一层、哪个文件第几行） |
| `unknown_preset`、`preset_invalid` | 同上，预设的 |
| `unknown_model` | `model`、`vision` 解析不出：写法不对、没有这家供应商、没有这个池 |
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
| `viewed` | `true`：至少一个头开着它（[`session.viewing`](#sessionviewing)），后台通知由那个头出 |
| `persona`、`preset` | 用哪个人格、预设，没有的不写；无工具的 `preset` 是 `null`，另带 `no_tools: true` |
| `tokens` | 连同它派出去的子会话（一层层往下）一共用了多少 token：每次模型调用（主请求、起标题、压缩都算）的输入（没命中、缓存读、缓存写）加输出，和终端底栏的 `Σ` 同一个算法；还是 0 的不写 |
| `context` | `{"used", "window"?}`：上下文用了多少 token（最近一次主请求的输入：没命中、缓存读、缓存写，同终端底栏、会话状态的上下文），压缩以后、还没说过话的不写；`window` 是那一次请求的模型照这时的配置查到的上下文窗口，查不到的不写 |
| `project` | 在哪个项目，不在的不写 |
| `kind`、`dream` | 整理会话（整理记忆跑在里面的会话）才有：`kind` 是 `"dream"`，`dream` 是 `{"persona"?, "session"?, "manual"?, "from"?}`：哪个人格的记忆（`session` 只在以前的会话里有），人叫的写 `manual: true`，在哪个会话里叫的写 `from`；这一趟跑完了的多 `result`：`{"given", "revised", "retired", "summary", "stopped"?}`，新记、改了、作废了几条，有没有写摘要，叫停的写 `stopped: true`，没跑完的没有。主列表一般不列它们（传 `dream: false`），记忆页照 `dream` 列那一份记忆的 |
| `origin` | 从哪开的：终端集成开的会话（`session.terminal` 开的）是 `"terminal"`，头的会话列表照它写「终端集成」；她问顾问开的顾问会话（子会话，带 `parent`）是 `"advisor"`。别的、以前开的没有 |
| `forked` | fork 子代理的会话才有，是 `true`：头照它换图标。普通子代理、主会话没有 |
| `archived` | 导进来的只读存档才有，是 `true`：头把它排在单独一段，输入框换成一行「这是存档」 |
| `life` | 人生会话才有，是 `true`（见[人生会话](life.md)）：日志里记着 `life.started`。别的会话没有 |
| `default_workspace` | 这个会话的默认工作区，绝对路径：主会话是它自己的 `…/workspace/<会话编号>/`，子会话、分叉出来的是主会话的那一个。`cwd` 和它一样的可以写「默认工作区」 |

只列本机的会话；场所会话不在里面（见 [场所](venues.md) 的 `venue.sessions`）。要一直跟着最新的，订阅 `sessions` 流（[连接](connection.md)）。

**出错**：`bad_params`；读不了会话目录 `internal_error`。

## `session.send`

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `text` | 字符串，必写 | 要说的话 |
| `attachments` | 数组 | 附件，照先后。每项是 `blob.put` 的回应里的 `blob`、`name`、`media_type`，可另带 `path`（原来在本机哪儿）；或者是只给路径的一项 `{"path", "name", "size"?}`，没有 `blob`（下面第 3 条） |
| `urgent` | 布尔，默认 `false` | 急着插话：她正忙的，下一步就看到 |
| `shell` | 布尔，默认 `false` | 人在输入框里用 `!` 打的一条命令：`text` 是命令行，核心照会话的工作目录跑（下面第 4 条） |
| `skills` | 字符串数组 | 人在输入框里用 `$` 点名的技能，照先后（名字从 [`command.catalog`](packages.md#commandcatalog) 的 `skills` 挑）：每个技能的正文套一层 `<skill name=… path=…>` 外壳，排在这句话前面发给她；时间线上照 `text` 原样显示。没有、不许人点名的 `unknown_skill`；会话的预设关着的 `command_not_allowed`（`data.why` 说哪个预设）；写了两遍、和 `shell` 一起写、场所会话、别人说的 `bad_params` |
| `prefix` | `"keep"` 或 `"compact"` | 前缀要变时人选的：直接发送、压缩上下文并发送（下面「前缀要变」） |
| `from` | 字符串 | 你是别的 agent 程序：写上你的名字，这句话记成你说的，不是本人 |
| `cwd` | 字符串 | 先换到这个工作目录再说（终端无缝集成：每一轮的工作目录跟着 shell 走）：查法、太宽的 `wide` 同 [`session.set_workspace`](#sessionset_workspace)；和现在一样的什么都不记，换了的推 `session.workspace_changed`。场所会话不收 |
| `as`、`venue` | 对象 | 只给场所会话，见 [场所](venues.md) |

**回应**：`events`（这一句的序号）、`cwd`（换过以后的）、`untrusted_project`（同 `session.create`）；换到太宽的目录多 `wide: true`。`cwd` 写错的（`path_unreadable`、`not_a_directory`、`path_forbidden`）整句不收。

1. 她闲着，这一句开一轮；正忙，排队（`urgent` 的插进下一步）。开的那一轮 `turn.started` 的 `cause` 是这一条的 `id`，照它认出是你开的。
2. 这一句落了盘就回，不等她答完：她怎么答看推送。
3. 附件先查再找会话：附件的 blob 要在、照内容再认一遍（见 [附件](blob.md)）。
   - **只给路径的一项**：超过上传上限（20 MiB）的文件不用传内容，只交它在核心那台机器上的路径。`path`：绝对路径（`/`、`~/`、Windows 的盘符或 `\\` 开头），没有控制字符，最多 4096 字节；`name`：文件名（不带路径，最多 255 字节）；`size`：字节数，非负整数，可不写。写法不对的 `bad_params`。
   - 核心不读它、不查它在不在：她看到一句写着名字、路径、大小的说明，内容没有附上，要看自己照路径用工具读（读的时候照权限判）。能和有 `blob` 的混着发，先后照写的。
   - 记成 `message.user` 里的一块 `{"type": "path", "name", "path", "size"?}`（见 [事件](events.md)）；视图里照样是一件附件，没有 `blob`、`media_type`，多 `path_only: true`（见 [视图](view.md)）。
   - 只有 `session.send`、`session.redo` 收：场所会话（带 `as` 的）、[`session.answer`](#sessionanswer) 的附件不收，`bad_params`。`path` 是核心那台机器上的路径，远程的连接带了的照旧 `local_machine_only`（见 [连接](connection.md)）。
4. `shell: true`：只收人在头里打的，不判权限、不进沙盒、不弹确认。记成一句写了 `shell` 的 `message.user`，落了盘就回；在她听到它的那一刻（闲着时开一轮的第一件，正忙时下一步问模型以前）照那时会话的工作目录跑，结果记一条 `shell.ran`（见 [事件](events.md)），她接着回。排着的照普通消息退回，退回的不跑。跑满 120 秒整组杀掉；跑着时打断（`session.interrupt`）整组杀掉、她不回。和 `attachments`、`venue` 一起写的 `bad_params`。视图流里那条 `user` 下面多一格 `shell`（见 [视图](view.md)）。

**前缀要变**（握手 `caps.prefix_ask: true` 的头才有）：你的工具说明、提示词覆盖、人格、预设目录里的文件改了（技能不算：会话中途技能变了走消息尾巴，前缀不变），下一轮的请求前缀会变、缓存失效。只照这个会话自己判：改的是别的人格、头像背景这些图片、它用不到的覆盖，它的前缀一个字没变，不问。会话空着时开新一轮（`session.send`、`session.redo`、开轮的 `command.run`）不带 `prefix` 的，回 `prefix_changing`，`data.files` 是改了的、真进它前缀的文件（照数据根写的相对路径，最多 20 个），多的在 `data.more`。换了人格、预设的会话已经先压缩过，不再问。给人弹「直接发送」「压缩上下文并发送」「取消」，选了再带 `"prefix": "keep"` 或 `"compact"` 发一次。`compact` 先压缩再说，压不了的回压缩的拒绝（例如 `nothing_to_compact`），这句没发。放行过的这一批不再问。

```json
→ {"jsonrpc":"2.0","id":"m1","method":"session.send","params":{"session":"01a14aca-…","text":"看看这张图","attachments":[{"blob":"sha256:9f86d0…","name":"shot.png","media_type":"image/png"}]}}
← {"id":"m1","jsonrpc":"2.0","result":{"cwd":"~/src/miyu","events":[42]}}
```

只给路径的：

```json
→ {"jsonrpc":"2.0","id":"m2","method":"session.send","params":{"session":"01a14aca-…","text":"看看这个镜像","attachments":[{"path":"/home/alice/Downloads/disk.iso","name":"disk.iso","size":31457280}]}}
← {"id":"m2","jsonrpc":"2.0","result":{"cwd":"~/src/miyu","events":[47]}}
```

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 附件缺了格、写法不对；只给路径的附件发给了场所会话；`from` 不是字符串或去掉控制字符以后是空的 |
| `empty_message` | 没有字、没有附件 |
| `unknown_attachment` | 附件的 blob 核心里没有 |
| `attachment_too_big` | 附件里有超了上限的图 |
| `venue_session` | 对场所会话说话却没带 `as` |
| `shell_not_allowed` | `shell` 不是人在头里打的：带了 `from`、`as`，扩展说的，或者是场所会话 |
| `shell_unavailable` | `shell`：装着的软件包里没有能跑命令的工具（没装或者关了） |

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

**回应**：`{"tools": [{"name", "source": {"kind", "id", "name"}, "summary"?}]}`，照发给模型的先后（名字的字节序）。`kind` 是 `builtin`、`package`、`mcp`、`script`；`source.name` 照连接的语言挑。`summary` 只有脚本有。工具一律全量，不会有只露名字的 `loaded: false`。

```json
{"tools":[{"name":"edit","source":{"kind":"builtin","id":"basesystem","name":"基础系统"}},{"name":"mcp__notes__search","source":{"kind":"mcp","id":"notes","name":"notes"}}]}
```

**出错**：`bad_params`（写了 `session` 又写了 `persona`、`preset`）；找会话的那几种。

## `session.context`

这个会话这时的上下文一块块的：system、工具说明、示范对话、检查点摘要、对话各占多少。只在调用时算，不影响会话。

**参数**：`session`（可以不写：不写的照新会话会拿到的算）；不写 `session` 时可带 `persona`（`null` 是无人格）、`preset`（`null` 是无工具），同 `session.create`。

**回应**：`{"window"?, "line"?, "context"?, "used"?, "basis", "anchor"?, "estimated_total", "parts": [...]}`。`window`、`line` 是上下文窗口（设了上下文上限的是生效的窗口）和压缩线（没交过限额的不写）；`context` 是会话的上下文上限（没设的不写）；`used` 是这时用了多少，口径同底栏；`basis` 是 `measured`（照最近一次请求供应商报的 `anchor` 起算）或 `estimated`（全是本地估的）。`parts` 每块 `{"part", "label", "tokens", "children"?}`，`part` 是稳定的编号，`label` 照连接的语言，是 0 的块不交。有 `used` 的各块已经照它摊好，加起来正好是 `used`（摊成 0 的不交）；`estimated_total` 是摊之前本地估的和。

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

## `session.quota`

这个会话接下来请求的那一家供应商的额度：几个窗口各用了百分之几、什么时候重置。只有查得到额度的供应商有（现在是 opencode Go），用量框里照它画。会话每说完一轮核心在后台查一次，同一家 60 秒内照上一次的。

**参数**：`session`（必写）。

**回应**：`{"windows": [...]}`，照 `rolling`、`weekly`、`monthly` 的先后，供应商没报的窗口不交。一项：`id`（`rolling` 是 5 小时滚动的，`weekly`、`monthly`），`label`（照连接的语言：「5 小时」「每周」「每月」），`percent`（用了百分之几，原样，可能带小数），`status`（供应商说的状态，原样，例如 `ok`），`resets_at`（什么时候重置）。这一家查不到额度（不支持、轮换的池、连不上、回的不对）的回 `{}`，不报错。

```json
{"windows":[{"id":"rolling","label":"5 小时","percent":4,"status":"ok","resets_at":"2026-11-14T08:31:28.000Z"},{"id":"weekly","label":"每周","percent":12,"status":"ok","resets_at":"2026-11-17T00:00:00.000Z"},{"id":"monthly","label":"每月","percent":30,"status":"ok","resets_at":"2026-12-01T00:00:00.000Z"}]}
```

**出错**：`bad_params`；找会话的那几种。

## `session.answer`

她做要确认的事时会停下来问（推送 `tool.approval_requested`），她用 `ask_user` 提问时也会等（推送 `question.asked`）。用它回答。确认请求带 `expires_at` 的，到那一刻还没人回答就自动拒绝（配置 `permission.confirm.timeout`，出厂 5 分钟）；提问一直等。

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

**选 `session` 记住的是什么**：推送里的 `rule`（没有就不能选 `session`），紧凑的 JSON：`tool` 哪件工具，`read`、`write` 放行读、写的几个目录（连同下面的）。选了以后，这个会话里这件工具再碰这些目录不再问。命令的确认请求不带 `rule`，只能允许这一次或者拒绝。

```json
{"read":["/home/me/.ssh"],"tool":"read"}
```

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 两样都写或都不写；提问带了 `reason` 或外面的 `attachments`；`decision` 不是那三种；附件缺格、写法不对 |
| `unknown_attachment`、`attachment_too_big` | 同 `session.send`：附件的 blob 没有、太大 |
| `not_asking` | 这个调用没在等回答：答过了、结束了、等的不是这一种，过了请求的 `expires_at` 已经自动拒了 |
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

**参数**：`session`；`level`：只有 `full`（完全放开）；`read_only`：只读开关。改哪样写哪样，至少写一个。

**回应**：`{}`。变了的推 `session.policy_changed`。收紧的当场生效；放宽的下一次请求生效。改成完全放开之前要不要问人一声，是头的事。

**出错**：`bad_params`：两样都没写；`level` 不是那两种。

## `session.set_workspace`

**参数**：`session`；`cwd`：新的工作目录（绝对路径或 `~` 开头）；`dirs`：新的加进来的目录，整份换掉（空数组是去掉全部）。至少写一个。

`cwd` 写工作区本身（握手的 `host.workspace`）就是回到这个会话自己的默认工作区：换成它（会话列表、订阅回应的 `default_workspace`）。

**回应**：`{"cwd", "dirs"}`，太宽的另带 `wide: true`。变了的推 `session.workspace_changed`；这一轮照旧，下一轮开始照新的。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 两样都没写 |
| `path_unreadable` | 目录不在、读不了 |
| `not_a_directory` | 是文件 |
| `path_forbidden` | 落在 Miyu 的数据目录里（工作区 `workspace/` 除外） |
| `dir_too_wide` | `dirs` 里有一条太宽 |

## `session.terminal`、`session.set_terminal`

终端会话：在 shell 里说的话进的那个一直续下去的会话（终端无缝集成用）。一个，带了 `terminal` 键的每个终端各一个；哪个算、什么时候换新的由核心定。

**`session.terminal`**：参数 `cwd`（可以不写）：要新开时在哪个目录，没写的照默认工作区；`fresh`（可以不写，默认假）：真的不管指着谁，照下面第 3 条开一个新的、指过去（`/new`）；`terminal`（可以不写）：哪个终端，头生成的键，1 到 128 个字、没有控制字符，不合的拒 `bad_params`，带了的照这个键各记一个指针，不带的用原来那一个；`create`（可以不写，默认真）：假的只问不开，下面第 2、3 条的情形回 `{"session": null}`，不新开、不挪指针（终端刚开、还没说话时问一下在哪个会话），和 `fresh: true` 一起写的拒 `bad_params`。回应 `{"session"}`：

1. 指着的会话还在、是本机的主会话（不是子会话、场所会话、一次性的），就是它。
2. 它闲置超过 `terminal.idle_new_after` 分钟（0 是不换）：开一个新的，指针挪过去，回应多 `renewed: {"last"}`（旧会话最后一次动静的时刻），旧会话不动。指过去（`session.set_terminal`、新开的）的那一刻也算一次动静：主动切到一个很久没说话的会话，闲置从切过去时算起。
3. 没指过、指着的删了、不算数的：开一个新的，回应多 `created: true`。

新开的照 `terminal.persona`、`terminal.preset`（没设的跟随 `persona.default`、`preset.default`）。同一时刻几个连接一起要（同一个键），只开一个。键各记最后一次用到的时刻，7 天没用的写指针时顺手清掉，最多留 64 个（最久没用的先清）；清掉的键再来当没指过。

**`session.set_terminal`**：参数 `session`：把终端会话指到它（`/sessions` 选中的）；`terminal`（可以不写）：改哪个终端的指针，写法同上。只收本机的主会话，别的拒 `session_not_found`。回应 `{"session"}`。

## `session.set_meta`

**参数**：`session`；`title`：新标题，1 到 200 个字，`null` 是去掉标题；`pinned`：`true` 置顶、`false` 取消；`project`：项目编号是移进去，`null` 是移出来（只有本机的主会话能进项目：场所会话、子代理、分叉出来的拒 `bad_params`，`data.field` 是 `project`）。至少写一个。置顶和项目是两格：拖进项目时要不要取消置顶，头自己另写 `pinned`。

**回应**：`{}`。变了的推 `session.meta_changed`。没起名的会话，核心在第一轮答完以后自己起一个标题，同样推 `session.meta_changed`（`by` 是内核）。

**出错**：`bad_params`：一样都没写；标题空的、超过 200 个字；`project` 写空字符串、这个会话不能进项目（`data.field` 是 `project`）。`unknown_project`：没有这个项目。

## `session.retitle`

让模型照最近的对话重起一个标题：和核心自动起标题用同一句指令、同一个模型，同样记用量（`model.called` 的 `purpose` 是 `title`），只是看的是最近答完的至多三轮（自动起的只看第一轮）。每轮只带人说的话和她最后的正文回答，工具调用、思考不带；还没答完的那一轮不带。人起过名的也换掉；造会话时写了 `titles: false` 的、一次性的会话、子会话、场所会话照样起。

**参数**：`session`。

**回应**：`{}`：请求发出去就回，不等模型。起好了推 `session.meta_changed`，`by` 是要它的人，算人起的：会话列表那一项写 `named`，以后核心自动起标题也不会盖掉它。已经有一次起标题的请求在路上的，不再发，照样回 `{}`。正在进行的那一轮不带、不打断；第一轮还没答完的，等这一轮结束再起。没起成的（模型出错、发出去 180 秒还没说完、没写出标题、要了工具），标题不变，记一条出错的 `model.called`，订阅着事件流的头收到瞬时事件 `session.retitle_failed`（见 [事件](events.md#瞬时事件)），视图流里多一条 `retitle_failed` 旁白（见 [视图流](view.md)），头照它告诉人「更改标题失败」；要了以后、起好之前人又用 `session.set_meta` 改了标题的，留人改的，什么都不推。核心自己起标题没起成的不推。

**出错**：`bad_params`：没写 `session`；没东西可起（她一句带正文的回复都还没有，这时也没有回合在进行）。没有这个会话 `session_not_found`。

## `session.viewing`

头告诉核心自己这时开着哪个会话：切会话、新开会话、刚起来时各发一次。只给人的头用，扩展调回 `needs_approval`。

**参数**：`session`：开着的会话；`null` 或不写是哪个都没开。

**回应**：`{}`。核心照连接记，连接断了就忘。会话列表那一项至少一个头开着时写 `viewed: true`，变了照 `busy` 那样推 `sessions.changed`。后台会话答完、在问、要确认的通知，开着它的那个头照常出，别的头见到 `viewed` 就不再另弹，免得几个头弹几遍。

**出错**：`bad_params`；没有这个会话 `session_not_found`。

## `session.configure`

换这个会话用的模型、看图模型、人格、预设、思考强度、压缩触发线，下一轮开始生效；上下文上限下一次请求起生效。

**参数**：`session`；下面七格至少写一格：
- `model`：`<供应商>/<模型>` 或 `@<池>`；`null` 是不再钉着哪一个，回到跟着配置的 `models.chat`（每一轮照那时的配置，配置换了跟着换）。
- `vision`：替看图用的模型，写法同 `model`：`<供应商>/<模型>` 或 `@<池>`；`null` 是不再钉着，回到跟着配置的 `models.vision`。会话的模型看不了图时，核心照它把图转成文字（没钉的照 `models.vision`）；钉的后来解析不出（供应商、池删了），那张图照没转成办，不退回 `models.vision`。
- `persona`：人格编号；`null` 是无人格。
- `preset`：预设编号；`null` 是换成无工具。
- `effort`：思考强度的一档，照这个会话这时的模型的档位查（`model.list` 的 `facts.reasoning`；同一条换了模型的照换成的）。用着池的会话不能设，写了回 `bad_params`（`data.field` 是 `effort`），池里每个成员照它自己的配置；`null` 是回到跟着配置。只管这个会话：配置里的 `providers.<id>.models.<model>.effort` 是新会话的默认。换到没有这一档的模型照配置，换回来又用上。
- `context`：上下文上限，token 数；`null` 是去掉。生效的窗口取模型的窗口和它里小的（模型没报窗口的照它），压缩线照生效的窗口算；跟着会话，换模型、池换了成员都照用。太小、照它算不出压缩线的回 `bad_params`（`data.field` 是 `context`，`data.min` 是最少要多少），什么都不换。人打 `/context` 也是改它。
- `compaction_line`：压缩触发线，窗口的百分之几，20 到 95 的整数；`null` 是不再钉着，回到跟着配置的 `compaction.line`。上下文用到这条线就自动压缩。压缩线照旧不超过「窗口 − 输出预留 − 余量」：20 万的窗口调到 85 以上不起作用，百万的窗口才到得了 95。

换人格、预设时先压缩一轮：核心马上照旧人格把前面的对话压成摘要，下一轮换上新的，从摘要接着聊。跟着人格的记忆从这一轮起记在新人格那一间，以前的留在原来那一间。

**回应**：`{}`。变了的推 `session.policy_changed`；换模型、思考强度生效的那一刻推 `model.changed`；换了上下文上限，`subscribe` 回应、`view.status` 的 `limits` 当场换（`window` 是生效的窗口，`context` 是上限，没设的不写）。之后 `subscribe` 回应、`model.changed` 的 `model.effort` 是 `{"level", "from"}`：`from` 是 `session` 的是会话记的这一档，`system` 的是配置。`session.list`、`subscribe` 回应里的 `persona`、`preset` 马上照换成的写（换成无工具的写 `preset: null`、`no_tools: true`）。钉了看图模型的，`view.status` 马上带 `vision`，去掉了的不写；钉了压缩触发线的同样马上带 `compaction_line`，下一轮开始 `limits.compaction_line` 照它换。

**出错**：`bad_params`（一格都没写、空字符串、类型不对；子代理换人格、预设，`data.field` 是那一格；`effort` 不在这时的模型的档位里，`data.field` 是 `effort`，什么都不换；`context` 不是非负整数；`context` 太小，`data.field` 是 `context`、`data.min` 是最少要多少，什么都不换；`compaction_line` 不是整数，或者不在 20 到 95 里，`data.field` 是 `compaction_line`，什么都不换）；`unknown_model`：`model`、`vision` 解析不出，什么都不换；`unknown_persona`；`unknown_preset`；`turn_running`：换人格、预设时有回合在进行（等这一轮做完）。

## `session.fork`

从一个主会话分叉出子会话：子会话带着它到分叉点为止的历史接着说，前面的请求前缀和它逐字节一样，缓存照用。

**参数**：`session`（父会话，要是主会话）；`seal_after`：跑完几轮就封存（1 起），不写不封存。

**回应**：`{"session": <子会话编号>, "upto": <分叉点的序号>}`。父会话正在跑一轮的，分叉点是这一轮开始之前。

**出错**：`bad_params`：父会话是子代理、分叉出来的；`seal_after` 不是 1 起的整数。

## `session.delete`

挪进回收处，留 7 天。它派出去的子会话一起删。

**参数**：`session`。**回应**：`{}`，挪好了才回。订阅着它的头收到 `resync`。

**出错**：`turn_running`：有回合在进行（先打断）；`internal_error`：挪不动。

## `session.prompts`

列出一个会话里你问过的所有话，照先后。界面照它列「问话跳转」：没读进来的那一句，照它的 `seq` 用 `view.page` 的 `before` 往前读到那一页再跳。只读日志，不载入会话。

**参数**：`session`。

**回应**：`prompts`，一项一句：

| 名字 | 说明 |
|---|---|
| `seq` | 那条 `message.user` 的序号 |
| `turn` | 它的回合：回合里排着的是那一轮，闲着时说的是它开的那一轮；都没有的不写 |
| `at` | 它的时刻，写法同事件的 `at` |
| `text` | 第一行，去掉前后空白，最多 100 个字；斜杠命令模板展开的照你敲的那一行（同 `session.list` 的 `preview` 的取法）。只有图、文件的是空的 |

- **只列你本人在这里说的**：本机说的、桥替你在平台私聊里转进来的都列；子会话、父会话发来的，平台上别的人说的（群里认出是你的也算平台上的人），注入的（长期目标的续轮这些），别的 harness、定时任务交进来的都不列。
- **照有效历史**：撤销掉的不列，恢复的放回，重做（编辑上一句）照最后那一版，撤回的排着的话不列；压缩过的照样列。
- **分叉出来的会话**只列它自己的：底子里的话在父会话里列。

**出错**：`bad_params`；`session_not_found`；`session_broken`。

## `session.export`

把一个会话导出来写到你给的位置，或者放进回应：原始包（ZIP）存档、换机器、查问题都用它，以后的导入只认它；Markdown 给人看。只给人的头。

**参数**：

| 名字 | 说明 |
|---|---|
| `session` | 会话编号 |
| `format` | `bundle`：原始包。`markdown`：Markdown |
| `to` | `{"path": "<绝对路径>"}`：核心写到这个文件。`"inline"`：放进回应的 `text`，只对 Markdown |
| `for` | 只对 Markdown，给谁用：`read` 自己看（不写的也是）、`agent` 给别的 agent（Claude Code、codex 这些）、`chat` 贴进网页版 chat |
| `range` | 只对 Markdown，导多少：`all` 全部（`read`、`chat` 不写的是它）；`context` 她现在的上下文，从最近一次压缩起（`agent` 不写的是它）；`{"turns": N}` 最近 N 轮 |
| `thinking` | 只对 Markdown，布尔：带不带思考。不写的照 `for`（`read` 带，别的不带） |
| `tools` | 只对 Markdown，工具写多少：`line` 一行；`seen` 她当时看到的；`full` 全文，后台命令的输出、脚本里调的工具交回的也取来。不写的照 `for`（`chat` 是 `line`，别的是 `seen`） |
| `children` | 只对 Markdown，子代理：`reports`（不写的也是，照 `for` 写交代、回报）；`append` 再把每个子会话照同样的写法整份附在后面，按任务编号排 |
| `assets` | 只对 Markdown，图片、附件：`placeholder`（不写的也是，写占位）；`files` 连图一起导：拷进 Markdown 旁边的 `<文件名>_files/` 文件夹，Markdown 里写相对路径。只能配 `to: {"path"}` |
| `locale` | 只对 Markdown：显示名、旁白用哪种语言（`zh`、`ja`、别的是英文）；不写的照握手时的 |

**回应**（原始包）：`path`、`bytes`（包多大）、`upto`（这个会话导到第几条）、`sessions`（包里有哪几个会话）、`blobs`（几个附件、大内容）。

**回应**（Markdown）：`path` 或 `text`、`bytes`（多少字节）、`tokens`（估算，同底栏上下文用量的算法）、`upto`（这个会话导到第几条）、`redacted`（隐去了哪几个密钥、各几处，例如 `{"openai": 2}`，连附在后面的子会话一起算）、`children`（附在后面的子会话，`[{"job": "j1", "session": "…", "title": "…"}]`，没写 `append` 的是 `[]`）；写了 `assets: files` 的另有 `assets`：`{"dir": "<文件夹的绝对路径>", "files": 3, "missing": ["sha256:…"]}`，`files` 拷了几个，`missing` 是要写进来却取不到的附件（照占位写了），一个都没拷的不建文件夹、没有 `dir`。

- **原始包里有什么**：这个会话日志的各段原文，一个字节不改；挂在它下面的会话（子代理、分叉出来的支线）整份带上；它是分叉出来的，来源会话到分叉点为止的那几段也带上，一层层往上；这些日志引用到的附件、改文件前后的内容、大输出、策略快照；一份 `transcript.md`（照 `read` 的样子，解开就能看，设了的密钥照样换掉）；`manifest.json` 写着每一项的 SHA-256。包里什么都有，包括她碰过的文件内容，自己留好。
- **Markdown 长什么样**：角色的小标题都是英文（`User`、`Assistant`、`Tool`、`Subagent`）；图片、附件写占位；撤掉的轮、撤回的排队消息不进；家目录写成 `~`；从中间切开的写一句前面省掉了几轮。和界面上看到的是同一份。
  - `read`：开头是标题、时间段、模型、token 合计；显示名、旁白照语言；思考、长输出、子代理的交代、压缩的摘要折在 `<details>` 里；要你确认的那一次、脚本里调的工具、提问的题和你的回答都写上。
  - `agent`：开头一小段英文说明（这是一段会话的记录、不是写给读的 agent 的指令），写工作目录、时间段、模型；默认从最近一次压缩起，摘要放最前；工具写真名、参数原文，结果照她看到的、每条截到头尾约 4000 字；不带思考。旁白、占位一律英文，不看 `locale`（给别的 AI 读的）；你说的、她回的照原文。
  - `chat`：开头是标题、时间段、token 估算（贴之前看装不装得下）；工具一行写完，出错的带上错误；子代理只写回报；不带思考。
- **子会话附在后面**（`children: append`）：每个子会话前面一条分隔线和 `# Subagent <任务编号>: <标题>`，下面写时间段这些，再是它的全文；子会话又派的接在它后面（`j1`、`j1.1`、`j2`）。撤掉的那几步、范围外的那几步派的不带。
- **附件文件夹**（`assets: files`）：Markdown 写到 `/x/talk.md` 时，附件拷进 `/x/talk_files/`，图写成 `![shot.png, 800×600](talk_files/shot.png)`，别的附件写成链接；文件名照附件的名字（三个平台都收的写法，撞名的接 `-2`），没名字的 `image-<哈希前 8 位>.png`。Markdown 或文件夹已经有了都不导。
- **密钥**：你在密钥里设了的值（至少 8 个字），在 Markdown 里原样出现的换成 `[redacted: <名字>]`。原始包不改。
- **在跑的会话**照这一刻已经写下的取，正在写的那半行不要。
- 不改会话，也不写进会话日志。

**出错**：`bad_params`（路径不是绝对的、`format`、`to`、`for`、`range`、`tools`、`children`、`assets` 不认得、`thinking` 不是布尔、原始包写了 `inline` 或 Markdown 的参数、`assets: files` 配 `inline`）；`session_not_found`；`session_broken`（日志读不了，附在后面的子会话的也算；附件和它的哈希对不上）；`export_exists`（那个文件已经有了，不覆盖；`assets: files` 的文件夹已经有了也算）；`export_in_data_root`（不往数据目录里写）；`export_unwritable`（目录不在、写不进）；`export_too_big`（放进回应的超过 512 KiB，改成存文件）。

## `session.import`

把 `session.export` 打的原始包导进来：换机器、换数据目录时搬会话，旧版 Miyu 的迁移程序也用它。只给人的头。

**参数**：

| 名字 | 说明 |
|---|---|
| `blob` | 先用 `blob.put` 传上来的包 |
| `path` | 本机上的包，绝对路径（命令行用）；`blob`、`path` 只写一样 |
| `archive` | 布尔，默认 `false`：导成只读存档 |

**回应**：`main`（包的主会话）、`sessions`（这一次新导进来的）、`present`（早就在的，跳过了）。

- **先核对再落盘**：包里每一项的哈希、日志的序号都对得上才写；有一样不对整个不收，数据目录里什么都不多。
- **编号照原样**：已经有同编号、内容一样的当导过了跳过，内容不一样的拒。再导一次同一个包什么都不变。
- **存档**：能翻、能搜、能导出，她搜以前的对话也搜得到；不能接着说，说话、撤销、压缩这些改会话的回 `archived`。包里写了是存档的，导进来照样是存档。不是存档的导进来就能接着聊。
- 导完马上能搜到，不用先打开一次。会话列表推一条变化。

**出错**：`bad_params`；`unknown_attachment`（`blob` 不在）；`bad_bundle`（不是会话包）；`bundle_mismatch`（包坏了）；`bundle_too_new`（更新的版本打的包）；`session_exists`（已有同编号、内容不一样的会话，`data.session` 是哪一个）。

## `view.page`

按页读历史，从最新往前翻，页的边界落在两轮之间。

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `before` | 正整数 | 只要序号小于它的；不写是最新一页。往前翻拿上一页的 `first` |
| `turns` | 1 到 50，默认 20 | 最多几轮 |
| `view` | 布尔，默认 `false` | `true` 交给界面直接画的条目（视图投影），不交原始事件 |

**回应**：`events`（或者 `entries`）、`more`（前面还有没有），有内容的另带 `first`、`last`；超过 1 MiB 少给了轮数的带 `capped: true`；这一页里报完、更早派出的任务带 `jobs`；日志中间坏了一行、这一页只给到坏行以前的带 `broken`。

最新一页的 `last` 就是这一刻的最后一条：接着 `subscribe {"after": last}` 只接新的。

`broken` 是 `{"segment", "line"}`：哪一段日志（只是文件名，例如 `000000000001.jsonl`）、这一段的第几行坏了。要读的行里碰到坏行的，这一页好像日志到坏行前一条为止；坏行不在这一页要读的行里的，照常给、不带它。这样的会话载入不了，说话、订阅回 `session_read_only`：界面照 `view.page` 只读地显示，说一句坏在哪，要继续新开一个会话。日志不改、不修。

**出错**：`bad_params`；`session_not_found`；`session_broken`（日志读不懂，连第一条都读不成的这一类，没有能看的）。

## `view.detail`

**参数**：`session`；`call`（一次工具调用的 `call_id`）、`job`（一个后台任务的编号）或 `compaction`（一次压缩替代到的序号），正好写一个；可以另写 `nested_after`（整数）：嵌套调用只要序号比它大的，加上它以前还在跑的。

**回应**

- 调用：`{"files": [...], "output"}`。`output` 是结果原文，不截；`files` 一个文件一项：`path`、`action`（`write` 或 `trash`）、`diff`（统一格式，上下文 3 行）、`added`、`removed`，算不出差异的写 `skipped`（`too_big`、`binary`、`missing`）。
- 供应商侧工具（视图里 `hosted: true` 的那一步）：`{"files": [], "output"}`，`output` 是供应商交来的结果（超过 64 KiB 的截过，截过的另带 `output_cut: true`）；还没结果的照没有这次调用。
- 压缩：`{"summary"}`：那一次的摘要原文。
- 脚本里调过工具的（`miyu call`）：调用的回应多 `nested`，后台任务的回应只有 `{"nested"}`。照序号一项一个：`n`（序号）、`name`、`title`（`{name, object}`，显示名照连接的语言）、`args`（参数原文）、`state`（`running`、`ok`、`error`、`denied`、`needs_approval`、`cancelled`），有的另带 `started`（在跑的开始时刻）、`took_ms`、`reason`（拒绝、出错的那一句）、`result`（`{blob, bytes}`，交给脚本的那份结果，用 `blob.get` 取）。那次 `shell` 还在跑、还没结果的，也交 `{"nested"}`。

同一次调用的结果不会变，问一次就够；下面调的工具在跑时会多，用 `nested_after` 只拿新的。

**出错**：`bad_params`；`session_not_found`；`unknown_call`（没有这次调用的结果，或者没有这次压缩）；`session_broken`。
