# 会话：`session.*`、`view.*`

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

会话（session）就是一段对话：它是一份只追加的事件日志，核心根据这份日志算出会话现在的状态。头（连接核心的界面程序，例如终端界面、网页界面）创建会话、向会话发送消息，并且订阅会话的事件来绘制界面。

| 方法 | 做什么 |
|---|---|
| [`session.create`](#sessioncreate) | 创建一个会话 |
| [`session.list`](#sessionlist) | 列出会话 |
| [`session.send`](#sessionsend) | 向会话发送一句话 |
| [`session.interrupt`](#sessioninterrupt) | 打断正在进行的回合 |
| [`session.withdraw`](#sessionwithdraw) | 在不打断回合的情况下，退回一条正在排队的消息 |
| [`session.tools`](#sessiontools) | 查看 AI 下一轮能使用的工具 |
| [`session.context`](#sessioncontext) | 查看上下文由哪几块组成、每一块各占多少 |
| [`session.usage_parts`](#sessionusage_parts) | 查看累计用量，按上下文的各块分开统计 |
| [`session.quota`](#sessionquota) | 查看模型供应商的额度：每个窗口用了多少、什么时候重置，或者账户余额 |
| [`session.answer`](#sessionanswer) | 回答一次确认请求，或者一组提问 |
| [`session.revert`、`session.unrevert`](#sessionrevertsessionunrevert) | 撤销；恢复最近一次撤销 |
| [`session.redo`](#sessionredo) | 重做最后一轮 |
| [`session.compact`](#sessioncompact) | 手动压缩上下文 |
| [`session.clear`](#sessionclear) | 清空上下文 |
| [`session.recap`](#sessionrecap) | 生成一句回顾 |
| [`session.set_permission_level`](#sessionset_permission_level) | 切换权限级别、只读开关 |
| [`session.set_workspace`](#sessionset_workspace) | 更换工作目录 |
| [`session.set_meta`](#sessionset_meta) | 修改标题、置顶 |
| [`session.retitle`](#sessionretitle) | 让模型重新起一个标题 |
| [`session.viewing`](#sessionviewing) | 头告诉核心自己正在显示哪个会话 |
| [`session.terminal`、`session.set_terminal`](#sessionterminalsessionset_terminal) | 获取终端会话，或者把终端会话指向其他会话 |
| [`session.configure`](#sessionconfigure) | 更换模型、多模态模型、人格、预设、思考强度、压缩触发线 |
| [`session.fork`](#sessionfork) | 从一个会话分叉出子会话 |
| [`session.delete`](#sessiondelete) | 删除会话 |
| [`session.prompts`](#sessionprompts) | 列出会话里问过的话 |
| [`session.export`](#sessionexport) | 把会话导出成原始包或者 Markdown |
| [`session.import`](#sessionimport) | 导入原始包 |
| [`view.page`](#viewpage) | 按页读取历史 |
| [`view.detail`](#viewdetail) | 读取一次工具调用的完整差异，或者一次压缩的摘要原文 |

场所会话（通讯平台上的群聊、私聊）另外还有几个方法，详细的见 [场所](venues.md)。

## 通用

- **会话编号**是 UUIDv7 字符串，按创建的先后排序。
- **谁能调用**：人可以调用下面所有的方法；对于关在沙盒里的场所会话（群聊、陌生人的私聊），人只能以只读的方式查看，读取类的几个方法仍然可用，其他方法返回 `session_not_found`（见[场所](venues.md)「主人看场所会话」）。扩展只能对它看得到的会话调用这些方法（平台桥只看得到它自己平台上的场所会话，其他扩展看得到本机的会话和你自己的私聊），并且还需要相应的能力：读取类的方法（`session.list`、`session.recap`、`session.tools`、`session.context`、`session.usage_parts`、`session.quota`、`session.prompts`、`view.page`、`view.detail`）需要 `events.read`；创建会话、发送消息、打断、撤销、恢复、重做、压缩、清空、更换配置、分叉、修改标题、重起标题需要 `sessions.drive`；`session.answer`、`session.set_permission_level`、`session.set_workspace`、`session.terminal`、`session.set_terminal`、`session.delete`、`session.export`、`session.import` 只给人调用（`local_only`）。扩展发送消息时如果不带 `as`，这句话记成扩展自己说的（`harness`，名字是软件包的编号）；扩展不能填写 `from`。
- **没有在运行的会话会先载入**：核心重启过，或者会话闲置太久已经退下，这样的会话在第一次用到时会自动载入，头不需要处理。
- **变化通过推送告知**：多数方法只返回 `{}` 或者几条事件的序号，变化本身以事件的形式推送给订阅了这个会话的头（见 [推送](pushes.md)）。
- **查找会话时的出错**：每个带 `session` 参数的方法都可能返回下面这些错误：

| `data.reason` | 什么时候 |
|---|---|
| `session_not_found` | 没有这个会话；已经删除的会话也返回这个错误 |
| `session_broken` | 会话无法载入：日志损坏，或者无法解析 |
| `session_read_only` | 会话日志中间有一行损坏：损坏的那一行以前的内容仍然可以用 `view.page` 查看，但是会话无法载入，也不能接着对话，要继续就新开一个会话；`data.segment` 是哪一段日志（只是文件名），`data.line` 是第几行 |
| `session_unavailable` | 会话此刻无法读取：读取日志时发生了读写错误，日志本身没有损坏，过一会儿再试即可 |
| `session_stopped` | 会话已经停止（写不进日志，或者出了 bug）；再发送一次会重新载入会话 |
| `session_sealed` | 会话已经封存（分叉时写了 `seal_after`，并且已经运行满了这么多轮），修改对话的命令不再接受 |
| `restoring` | 撤销还没有完成时收到了命令（正常情况下不会遇到） |

## `session.create`

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `cwd` | 字符串 | 头的工作目录，用人看到的写法，例如 `~/src/miyu`。不填写时使用这个会话自己的默认工作区；远程连接不能填写这个参数 |
| `persona` | 字符串或 `null` | 使用哪个人格；`null` 表示不使用人格；不填写时使用配置项 `persona.default` |
| `preset` | 字符串或 `null` | 使用哪个预设；不填写时使用 `preset.default`，两者都没有时使用 `full`；`null` 表示无工具，即一件工具都不给（这是以前的写法，仍然接受；新开的会话选「无功能」时写 `none`） |
| `model` | 字符串 | `<供应商>/<模型>` 或 `@<池>`；不填写时记下此刻的 `models.chat`（如果它是池，就记下池），以后修改 `models.chat` 不会影响这个会话。三个固定的池（`@lite`、`@normal`、`@flagship`）是空的时，按没有填写处理 |
| `vision` | 字符串 | 这个会话用哪个模型来看图，写法和 `model` 相同；不填写时跟随配置的 `models.vision`；填写空的固定池时也按没有填写处理。填写了并且解析得出时，会话创建好以后会记一条 `session.policy_changed`，和 `session.configure` 的 `vision` 相同 |
| `compaction_line` | 整数 | 这个会话的压缩触发线，是窗口的百分比，取值 20 到 95；不填写时跟随配置的 `compaction.line`。会话创建好以后会记一条 `session.policy_changed`，和 `session.configure` 的 `compaction_line` 相同 |
| `memory` | `persona`、`off` | `persona` 表示记忆跟随人格，`off` 表示不使用记忆；不填写时跟随人格。其他值（包括以前的 `session`）返回 `bad_params` |
| `dirs` | 字符串的数组 | 另外加入的目录，和工作目录一样可以读写 |
| `oneshot` | 布尔，默认 `false` | 是否是一次性的会话（例如命令行 `miyu ask` 创建的会话） |
| `images` | 布尔，默认 `true` | 你的界面是否能显示图片 |
| `titles` | 布尔，默认 `true` | 是否自动起标题：设为 `false` 以后，核心不再自动起标题，人仍然可以手动修改标题 |
| `project` | 字符串 | 在哪个项目里创建会话（见 [项目](projects.md)）：没有填写的 `persona`、`preset` 使用项目的设置；创建好以后会记一条 `session.meta_changed`（此时回应的 `events` 是 `[1, 2]`） |

**回应**

| 名字 | 说明 |
|---|---|
| `session` | 新会话的编号 |
| `events` | `[1]`，即 `session.created` 那一条事件；带了 `project` 时是 `[1, 2]`；带了 `vision`、`compaction_line` 时各再多一条（按这个先后，排在项目那一条的后面） |
| `cwd` | 会话实际工作的目录，用人看到的写法（家目录下的路径写成 `~/…`，和 `session.list` 相同） |
| `wide` | 为 `true` 时表示目录太宽（根目录；在 Windows 上还包括家目录和包含数据目录的目录）；因为这是人选的目录，所以仍然使用，但是这个会话能随意写入的只有它的默认工作区 |
| `untrusted_project` | 如果这个目录有项目配置，并且还没有决定是否信任它，这里写出项目配置的位置 |

```json
→ {"jsonrpc":"2.0","id":"c1","method":"session.create","params":{"cwd":"~/src/miyu"}}
← {"id":"c1","jsonrpc":"2.0","result":{"cwd":"~/src/miyu","events":[1],"session":"01a14aca-d2c4-768c-acc6-a67e35106d22"}}
```

用同一个 `id` 再发送一次时，核心返回上一次创建的那个会话，不会再创建新的会话。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 人格或预设的编号写法不对；`memory` 不是 `persona`、`off` 之一；`model`、`vision` 不是字符串；`compaction_line` 不是 20 到 95 之间的整数 |
| `unknown_persona`、`persona_invalid` | 没有这个人格；或者人格的文件写错了（`data.problem` 说明是哪一层、哪个文件的第几行） |
| `unknown_preset`、`preset_invalid` | 和上一行相同，只是对象换成预设 |
| `unknown_model` | 无法解析 `model`、`vision`：写法不对、没有这家供应商、没有这个池，或者自己建的池是空的（三个固定的池是空的时不算，按没有填写处理） |
| `dir_too_wide` | `dirs` 里有一项太宽（家目录、根目录，或者包含数据目录的目录） |
| `internal_error`（`-32603`） | 核心无法创建会话 |

## `session.list`

**参数**：`oneshot`（布尔，为 `true` 时只列出一次性的会话）、`dream`（布尔，为 `true` 时只列出整理会话，为 `false` 时不列出整理会话，不填写时全部列出）、`limit`（最多列出几个，不填写时列出全部）。

**回应**是 `{"sessions": [<一项>…]}`，按从新到旧排列。每一项的字段如下：

| 名字 | 说明 |
|---|---|
| `session` | 会话编号 |
| `cwd` | 工作目录，用人看到的写法 |
| `last_active` | 最后一条事件的时刻 |
| `last_spoken` | 人最后一次发话的时刻，即人说的最后一句话（主人在头上说的，或者通讯平台上的人说的）的时刻；由循环、定时任务、回报、续跑开启的那几轮不算。头按它排列会话列表：只切换进会话不会改变它的位置，人说了话才会把它移到顶部。还没有人说过话时不写这个字段 |
| `oneshot` | 是否是一次性的会话 |
| `parent` | 子会话的父会话；主会话的这个字段是 `null` |
| `title` | 标题；没有标题时不写 |
| `named` | 为 `true` 时表示标题是人起的 |
| `preview` | 没有标题时提供：第一句话的第一行，最多 50 个字 |
| `pinned` | 为 `true` 时表示已经置顶 |
| `busy` | 为 `true` 时表示有回合正在进行 |
| `viewed` | 为 `true` 时表示至少有一个头正在显示它（见 [`session.viewing`](#sessionviewing)），后台通知由那个头负责显示 |
| `persona`、`preset` | 使用的人格、预设；没有时不写；无工具时 `preset` 是 `null`，并且另外带 `no_tools: true` |
| `tokens` | 这个会话连同它派出去的子会话（一层层往下算）一共用了多少 token：每次模型调用（主请求、起标题、压缩都算）的输入（未命中、缓存读、缓存写）加上输出，和终端底栏的 `Σ` 用同一个算法；还是 0 时不写 |
| `context` | `{"used", "window"?}`：上下文用了多少 token（即最近一次主请求的输入：未命中、缓存读、缓存写，和终端底栏、会话状态里的上下文相同）；刚压缩完、还没有再说过话时不写；`window` 是根据此刻的配置查到的、那一次请求所用模型的上下文窗口，查不到时不写 |
| `project` | 所在的项目；不在项目里时不写 |
| `kind`、`dream` | 只有整理会话（整理记忆时运行的会话）才有：`kind` 是 `"dream"`，`dream` 是 `{"persona"?, "session"?, "manual"?, "from"?}`，表示整理的是哪个人格的记忆（`session` 只出现在以前的会话里）；由人发起的整理写 `manual: true`，`from` 写在哪个会话里发起；这一次整理已经运行完时多一个 `result`：`{"given", "revised", "retired", "summary", "stopped"?}`，分别是新记、修改、作废了几条记忆，有没有写摘要，被叫停时写 `stopped: true`；没有运行完时没有 `result`。主列表一般不列出整理会话（传 `dream: false`），记忆页按 `dream` 列出那一份记忆的整理会话 |
| `origin` | 会话是从哪里开启的：终端集成开启的会话（由 `session.terminal` 开启）是 `"terminal"`，头据此在会话列表里显示「终端集成」；AI 向顾问提问时开启的顾问会话（子会话，带 `parent`）是 `"advisor"`。其他会话和以前开启的会话没有这个字段 |
| `forked` | 只有 fork 子代理的会话才有，值是 `true`：头据此换用另一个图标。普通子代理和主会话没有这个字段 |
| `archived` | 只有导入的只读存档才有，值是 `true`：头把它排在单独的一段里，并且把输入框换成一行「这是存档」 |
| `default_workspace` | 这个会话的默认工作区，是绝对路径：主会话使用它自己的 `…/workspace/<会话编号>/`，子会话和分叉出来的会话使用主会话的那一个。如果 `cwd` 和它相同，界面上可以显示为「默认工作区」 |

这个方法只列出本机的会话，场所会话不在其中（场所会话见 [场所](venues.md) 的 `venue.sessions`）。要一直跟踪最新的列表，就订阅 `sessions` 流（见 [连接](connection.md)）。

**出错**：`bad_params`；读取不了会话目录时返回 `internal_error`。

## `session.send`

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 发送到哪个会话 |
| `text` | 字符串，必写 | 要说的话 |
| `attachments` | 数组 | 附件，按先后排列。每一项是 `blob.put` 的回应里的 `blob`、`name`、`media_type`，还可以另外带 `path`（文件原来在本机的位置）；也可以是只给路径的一项 `{"path", "name", "size"?}`，这种项没有 `blob`（见下面第 3 条） |
| `urgent` | 布尔，默认 `false` | 急着插话：如果 AI 正在忙，下一步就会看到这句话 |
| `shell` | 布尔，默认 `false` | 人在输入框里用 `!` 输入的一条命令：`text` 是命令行，核心在会话的工作目录里运行它（见下面第 4 条） |
| `skills` | 字符串数组 | 人在输入框里用 `$` 点名的技能，按先后排列（名字从 [`command.catalog`](packages.md#commandcatalog) 的 `skills` 里选）：每个技能的正文套上一层 `<skill name=… path=…>` 外壳，排在这句话前面发给 AI；时间线上仍然按 `text` 原样显示。技能不存在或者不许人点名时返回 `unknown_skill`；会话的预设关掉了这个技能时返回 `command_not_allowed`（`data.why` 说明是哪个预设）；同一个技能写了两遍、和 `shell` 一起写、发给场所会话，或者是其他人说的话，都返回 `bad_params` |
| `prefix` | `"keep"` 或 `"compact"` | 前缀要变时人做的选择：直接发送，或者压缩上下文并发送（见下面「前缀要变」） |
| `from` | 字符串 | 如果你是其他 agent 程序，就在这里写上你的名字，这句话会记成你说的，而不是本人说的 |
| `cwd` | 字符串 | 先换到这个工作目录再发送（终端无缝集成：每一轮的工作目录跟着 shell 走）：检查的方法和太宽时的 `wide` 都和 [`session.set_workspace`](#sessionset_workspace) 相同；和现在的目录一样时什么都不记，换了目录时推送 `session.workspace_changed`。场所会话不接受这个参数 |
| `as`、`venue` | 对象 | 只用于场所会话，见 [场所](venues.md) |

**回应**：`events`（这一句的序号）、`cwd`（换过以后的工作目录）、`untrusted_project`（和 `session.create` 相同）；换到太宽的目录时另外带 `wide: true`。如果 `cwd` 写错了（`path_unreadable`、`not_a_directory`、`path_forbidden`），整句话都不接受。

1. 如果 AI 闲着，这一句会开启一轮；如果 AI 正在忙，这一句会排队（`urgent` 的消息插进下一步）。开启的那一轮的 `turn.started` 里，`cause` 是这一条请求的 `id`，你可以据此识别出这一轮是你开启的。
2. 这一句写进磁盘就返回，不等 AI 回答完；AI 怎么回答，要看推送。
3. 核心先检查附件，再查找会话：附件的 blob 必须存在，并且核心会根据内容再识别一遍（见 [附件](blob.md)）。
   - **只给路径的一项**：超过上传上限（20 MiB）的文件不需要上传内容，只提交它在核心所在机器上的路径。`path` 是绝对路径（以 `/`、`~/`、Windows 的盘符或 `\\` 开头），不含控制字符，最多 4096 字节；`name` 是文件名（不带路径，最多 255 字节）；`size` 是字节数，为非负整数，可以不填写。写法不对时返回 `bad_params`。
   - 核心不读取这个文件，也不检查它是否存在：AI 看到的是一句写着名字、路径、大小的说明，文件内容没有附上；AI 要查看内容时，需要自己按路径用工具读取（读取时按权限判断）。这种项可以和带 `blob` 的项混在一起发送，先后按写的顺序。
   - 它记成 `message.user` 里的一块 `{"type": "path", "name", "path", "size"?}`（见 [事件](events.md)）；在视图里它仍然是一件附件，但是没有 `blob`、`media_type`，并且多一个 `path_only: true`（见 [视图](view.md)）。
   - 只有 `session.send`、`session.redo` 接受这种项：场所会话（带 `as` 的消息）和 [`session.answer`](#sessionanswer) 的附件不接受，会返回 `bad_params`。`path` 是核心所在机器上的路径，所以远程连接带了这种项时仍然返回 `local_machine_only`（见 [连接](connection.md)）。
4. `shell: true`：只接受人在头里输入的命令，不判断权限、不进沙盒、不弹确认。它记成一条写了 `shell` 的 `message.user`，写进磁盘就返回；在 AI 听到它的那一刻（AI 闲着时是新开一轮的第一件事，AI 正在忙时是下一步请求模型以前），核心在会话那时的工作目录里运行它，结果记成一条 `shell.ran`（见 [事件](events.md)），然后 AI 接着回答。还在排队的命令可以像普通消息一样退回，退回的命令不会运行。命令运行满 120 秒时，整个进程组会被杀掉；命令运行时打断（`session.interrupt`）也会杀掉整个进程组，并且 AI 不会回答。和 `attachments`、`venue` 一起写时返回 `bad_params`。视图流里那条 `user` 下面多一格 `shell`（见 [视图](view.md)）。

**前缀要变**（只有握手时带 `caps.prefix_ask: true` 的头才会遇到）：如果你的工具说明、提示词覆盖、人格、预设目录里的文件改了（技能不算：会话中途技能变化时放在消息的尾部，前缀不变），下一轮请求的前缀就会变化，缓存也会失效。核心只根据这个会话自己的情况判断：如果改的是其他人格、头像背景这类图片，或者这个会话用不到的覆盖，它的前缀一个字都没有变，就不会询问。会话空闲时开启新的一轮（`session.send`、`session.redo`、会开启一轮的 `command.run`），如果不带 `prefix`，会返回 `prefix_changing`：`data.files` 是改动过、并且真正进入这个会话前缀的文件（写成相对数据根的路径，最多 20 个），超出的部分在 `data.more` 里。换过人格、预设的会话已经先压缩过，不会再询问。头要给人弹出「直接发送」「压缩上下文并发送」「取消」三个选项，人选好以后，再带上 `"prefix": "keep"` 或 `"compact"` 发送一次。`compact` 会先压缩再发送；如果无法压缩，会返回压缩的拒绝（例如 `nothing_to_compact`），这句话不会发送。放行过的这一批改动不会再询问。

```json
→ {"jsonrpc":"2.0","id":"m1","method":"session.send","params":{"session":"01a14aca-…","text":"看看这张图","attachments":[{"blob":"sha256:9f86d0…","name":"shot.png","media_type":"image/png"}]}}
← {"id":"m1","jsonrpc":"2.0","result":{"cwd":"~/src/miyu","events":[42]}}
```

只给路径的附件：

```json
→ {"jsonrpc":"2.0","id":"m2","method":"session.send","params":{"session":"01a14aca-…","text":"看看这个镜像","attachments":[{"path":"/home/alice/Downloads/disk.iso","name":"disk.iso","size":31457280}]}}
← {"id":"m2","jsonrpc":"2.0","result":{"cwd":"~/src/miyu","events":[47]}}
```

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 附件缺少字段，或者写法不对；只给路径的附件发给了场所会话；`from` 不是字符串，或者去掉控制字符以后是空的 |
| `empty_message` | 既没有文字，也没有附件 |
| `unknown_attachment` | 核心里没有附件的 blob |
| `attachment_too_big` | 附件里有超过上限的图片 |
| `venue_session` | 对场所会话发送消息，但是没有带 `as` |
| `shell_not_allowed` | `shell` 不是人在头里输入的：带了 `from` 或 `as`、是扩展发送的，或者是发给场所会话的 |
| `shell_unavailable` | 使用 `shell` 时，已安装的软件包里没有能运行命令的工具（没有安装，或者已经关闭） |

## `session.interrupt`

**参数**：`session`；`queued`（必须填写）：正在排队、还没有发送的消息怎么处理：`send` 表示打断以后马上发送，`return` 表示退回，`keep` 表示留着不发送。

**回应**：`{"events": [<这一次追加的事件>…]}`。

**出错**：`not_running`：没有回合正在进行。

## `session.withdraw`

在不打断这一轮的情况下，退回一条正在排队、AI 还没有听到的消息。

**参数**：`session`；`message`（可以不填写）：退回哪一条，值是那句话的 `message.user` 的序号；不填写时退回最新的一条。

**回应**：`{"events": [<那条 message.withdrawn 的序号>]}`。推送和 `session.interrupt` 退回消息时一样：一条 `message.withdrawn`，它的 `messages` 里只有这一条；视图里那一句标上 `withdrawn: true`。这一轮仍然继续运行，运行完以后，由剩下排队的消息接着开启下一轮。

**出错**：`already_heard`：指定的那一条不在队列里，请求已经带上了它（或者已经由它接着开启了一轮）；`nothing_queued`：没有指定哪一条，并且也没有正在排队的消息。

## `session.tools`

查看 AI 下一次请求会拿到的工具。

**参数**：`session`（可以不填写：不填写时按新会话会拿到的工具计算）；不填写 `session` 时可以带 `persona`（`null` 表示无人格）、`preset`（`null` 表示无工具），和 `session.create` 相同。

**回应**：`{"tools": [{"name", "source": {"kind", "id", "name"}, "summary"?}]}`，按发给模型的先后排列（即名字的字节序）。`kind` 是 `builtin`、`package`、`mcp`、`script` 之一；`source.name` 按连接的语言选择。只有脚本有 `summary`。工具一律完整提供，不会出现只给名字的 `loaded: false`。

```json
{"tools":[{"name":"edit","source":{"kind":"builtin","id":"basesystem","name":"基础系统"}},{"name":"mcp__notes__search","source":{"kind":"mcp","id":"notes","name":"notes"}}]}
```

**出错**：`bad_params`（写了 `session`，又写了 `persona` 或 `preset`）；以及查找会话时的那几种错误。

## `session.context`

查看这个会话此刻的上下文由哪几块组成：system、工具说明、示范对话、检查点摘要、对话各占多少。核心只在调用时计算，不影响会话。

**参数**：`session`（可以不填写：不填写时按新会话会拿到的上下文计算）；不填写 `session` 时可以带 `persona`（`null` 表示无人格）、`preset`（`null` 表示无工具），和 `session.create` 相同。

**回应**：`{"window"?, "line"?, "context"?, "used"?, "basis", "anchor"?, "estimated_total", "parts": [...]}`。`window`、`line` 是上下文窗口（设了上下文上限时是生效的窗口）和压缩线（还没有拿到过限额时不写）；`context` 是会话的上下文上限（没有设置时不写）；`used` 是此刻用了多少，口径和底栏相同；`basis` 是 `measured`（从最近一次请求时供应商报告的 `anchor` 开始计算）或 `estimated`（全部是本地估算的）。`parts` 的每一块是 `{"part", "label", "tokens", "children"?}`：`part` 是稳定的编号，`label` 按连接的语言给出，值为 0 的块不提供。有 `used` 时，各块已经按它分摊好，加起来正好等于 `used`（分摊成 0 的块不提供）；`estimated_total` 是分摊之前本地估算的总和。

块的编号如下：`system`（下面按拼接的先后有 `system.persona`、`system.venue`、`system.group`、`system.profile`、`system.project`、`system.core`、`system.clicall`、`system.show`、`system.skills`、`system.knowledge`、`system.style_lock`、`system.appended:<包>`；老会话里和此刻的文字对不上的那几段合并成 `system.other`）、`tools`（下面有 `tools.builtin`、`tools.package:<包>`、`tools.mcp:<服务>`、`tools.scripts`）、`demos`、`summary`、`messages`（下面有 `messages.user`、`messages.assistant`、`messages.memory`（下面有 `messages.memory.resident`、`messages.memory.recall`）、`messages.knowledge`、`messages.facts`、`messages.project_docs`（仓库里的 AGENTS.md）、`messages.reports`（子代理、后台命令的回报）、`messages.tool_results`）。软件包、服务的 `label` 是它们的原名。

```json
{"window":128000,"line":102400,"used":45231,"basis":"measured","anchor":41000,"estimated_total":43800,"parts":[{"part":"system","label":"系统提示词","tokens":6100,"children":[{"part":"system.persona","label":"人设","tokens":4300},{"part":"system.core","label":"核心说明","tokens":900},{"part":"system.skills","label":"技能目录","tokens":900}]},{"part":"messages","label":"对话","tokens":37700,"children":[{"part":"messages.user","label":"你说的","tokens":4200},{"part":"messages.assistant","label":"AI 说的","tokens":33500}]}]}
```

**出错**：`bad_params`（写了 `session`，又写了 `persona` 或 `preset`）；查找会话时的那几种错误；新会话拼不出上下文时返回 `internal`。

## `session.usage_parts`

查看这个会话累计用了多少，按上下文的块分开统计（和 `session.context` 用同一套块），另外再加上一块 AI 输出。这里只计算主请求；压缩、子代理、回顾这些用途的用量，用 `usage.query` 按用途查询。

**参数**：`session`（必须填写）；`tree`（布尔，可以不填写）：连同它派出去的子会话一起计算。

**回应**：`{"requests", "input", "output", "parts": [...]}`。`requests` 是主请求的次数，`input` 是供应商报告的输入（未命中、命中、写缓存）的总和，`output` 是输出的总和。`parts` 的块和 `session.context` 一样，最后多一块 `output`；各块加起来正好等于 `input + output`。每次请求的输入按那一刻各块所占的比例分摊，再累加起来；命中和未命中的输入不分开分摊，所以有一部分命中的输入会算到新内容上。

```json
{"requests":503,"input":80000000,"output":203822,"parts":[{"part":"system","label":"系统提示词","tokens":220000},{"part":"tools","label":"工具说明","tokens":1900000},{"part":"messages","label":"对话","tokens":77880000},{"part":"output","label":"AI 输出","tokens":203822}]}
```

**出错**：`bad_params`；以及查找会话时的那几种错误。

## `session.quota`

查看这个会话接下来请求的那一家模型供应商的额度：每个窗口各用了百分之几、什么时候重置，或者账户里还剩多少钱。只有能查询额度的供应商才有这些信息（目前是 opencode Go 的窗口和 DeepSeek 官方的余额），头的用量框根据它绘制。会话每完成一轮，核心都会在后台查询一次；同一家供应商在 60 秒内使用上一次的结果。查询的是 `subscribe` 回应里 `model.endpoint` 的那一家：如果刚换了模型、还没有开启新的回合，就是换成的那一家（换成池时要到下一轮开始才有）。

**参数**：`session`（必须填写）。

**回应**：`{"windows": [...]}`，按 `rolling`、`weekly`、`monthly` 的先后排列，供应商没有报告的窗口不提供。每一项包括：`id`（`rolling` 是 5 小时的滚动窗口，另外两个是 `weekly`、`monthly`），`label`（按连接的语言给出：`5 小时`、`每周`、`每月`），`percent`（用了百分之几，原样给出，可能带小数），`status`（供应商给出的状态，原样给出，例如 `ok`），`resets_at`（什么时候重置）。如果这一家查不到额度（不支持、轮换的池、连接不上、返回的内容不对），返回 `{}`，不报错。

```json
{"windows":[{"id":"rolling","label":"5 小时","percent":4,"status":"ok","resets_at":"2026-11-14T08:31:28.000Z"},{"id":"weekly","label":"每周","percent":12,"status":"ok","resets_at":"2026-11-17T00:00:00.000Z"},{"id":"monthly","label":"每月","percent":30,"status":"ok","resets_at":"2026-12-01T00:00:00.000Z"}]}
```

使用 DeepSeek 官方的供应商（没有填写地址，或者地址的主机就是 `api.deepseek.com`）时，回应是余额 `{"balance": {...}}`，没有查到时不带这一项：`available`（是否可用，原样给出 DeepSeek 的值），`items`（每种货币一项，按 DeepSeek 给出的先后排列：`currency` 是币种，`total` 是总额，`granted` 是赠送的金额，`topped_up` 是充值的金额，金额都是 DeepSeek 原样给出的字符串）。通过中转站使用 DeepSeek 模型的供应商，以及地址写成环境变量的供应商，都不查询余额，因为那个密钥不是 DeepSeek 官方的。`windows` 和 `balance` 互相独立，有其中一项不影响另一项。

```json
{"balance":{"available":true,"items":[{"currency":"CNY","total":"12.34","granted":"2.00","topped_up":"10.34"}]}}
```

带 `windows` 或 `balance` 的回应还会带 `provider`：`{"id", "name"}`，表示查询的是哪一家：`id` 是配置里这一家供应商的编号，`name` 是给人看的名字（配置写了 `providers.<id>.name` 时使用它；没有写时使用目录里对应的那一家的名字；再没有时使用编号；和 `model.list` 的 `name` 相同）。头根据它显示「{供应商} 额度情况」「{供应商} 余额」，不根据自己记住的当前模型去猜。返回 `{}` 时不带这一项。

```json
{"provider":{"id":"deepseek","name":"DeepSeek"},"balance":{"available":true,"items":[{"currency":"CNY","total":"12.34","granted":"2.00","topped_up":"10.34"}]}}
```

**出错**：`bad_params`；以及查找会话时的那几种错误。

## `session.answer`

AI 做需要确认的事情时会停下来询问（推送 `tool.approval_requested`）；AI 用 `ask_user` 提问时也会等待（推送 `question.asked`）。这个方法用来回答这两种情况。如果确认请求带了 `expires_at`，到了那一刻还没有人回答，就会自动拒绝（由配置 `permission.confirm.timeout` 决定，出厂值是 5 分钟）；提问会一直等待。

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `call` | 字符串，必写 | 那一次调用的 `call_id` |
| `decision` | `once`、`session`、`deny` | 回答确认请求：分别是允许这一次、在这个会话里都允许、拒绝 |
| `reason` | 字符串 | 拒绝的理由，AI 能看到；只有拒绝时才能带 |
| `attachments` | 数组 | 拒绝时附上的图片、文件，写法和 `session.send` 相同（`blob.put` 返回的 `blob`、`name`、`media_type`，可以带 `path`）；这些附件跟着理由一起交给 AI；只有拒绝时才能带 |
| `answers` | 数组 | 回答提问：按题目的先后，一题一项 `{"picked": [选的选项标题…], "text": <自己写的>, "notes": <备注>, "attachments": [<同上>]}`，其中每一格都可以不填写；附件跟着那一题一起交给 AI |
| `nested` | 正整数 | AI 在命令里通过 `miyu call` 调用的工具需要确认时，请求里会带 `nested`（序号），回答时原样填写。不填写也可以，此时回答的就是那次命令下面正在询问的那一件 |

`decision`、`answers` 两者正好写一个。**回应**：`{"events": [序号…]}`。

**选 `session` 记住的是什么**：记住的是推送里的 `rule`（如果没有 `rule`，就不能选 `session`，选了会返回 `no_rule`）。目前核心发出的确认请求都不带 `rule`（包括命令、高危命令、安装软件包、扩展的守卫发出的确认），所以只能允许这一次或者拒绝；以前在只读模式下读取其他程序的私密目录时，询问里带过 `rule`（放行读取的那几个目录），私密目录去掉以后就没有了。头根据有没有 `rule` 决定是否提供「在本会话内允许」这个选项。

```json
{"read":["/home/me/.ssh"],"tool":"read"}
```

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | `decision` 和 `answers` 两个都写了，或者都没写；回答提问时带了 `reason` 或者外层的 `attachments`；`decision` 不是那三种取值之一；附件缺少字段，或者写法不对 |
| `unknown_attachment`、`attachment_too_big` | 和 `session.send` 相同：附件的 blob 不存在，或者太大 |
| `not_asking` | 这次调用没有在等待回答：已经回答过、已经结束、等待的不是这一种回答，或者已经过了请求的 `expires_at`、被自动拒绝了 |
| `no_rule` | 选了 `session`，但是这次确认没有可以记住的范围 |
| `unexpected_reason` | 允许的同时带了理由或附件 |
| `bad_answer` | 回答和题目对不上：条数不对、选了不存在的选项，或者单选题选了多项 |

## `session.revert`、`session.unrevert`

撤销会把最后一轮（或者从某一轮开始的所有轮）从对话里撤掉，AI 改过的文件保持原样。恢复会撤回最近一次撤销。

**参数**：`session`；`revert` 另外可以填写 `turn`：从哪一轮开始撤销（值是那一轮 `turn.started` 的序号），不填写时撤销最后一轮。

**回应**

| 名字 | 说明 |
|---|---|
| `events` | 这一次追加的事件：`turn.reverted` 或 `turn.unreverted` |
| `cwd` | 会话的工作目录（真实位置） |
| `turns` | 撤销（或者恢复）了几轮 |
| `said` | 第一轮里人说的那句话的第一行；没有时不写 |
| `compactions`、`clears` | 只有撤销才有：撤掉的几轮里有几次压缩、几次清空，上下文已经回到它们之前的状态；值为 0 时不写 |
| `jobs` | 只有撤销才有：停掉的后台任务 `[{"job", "what", "title"}]`；没有时不写 |

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | `turn` 写了 0 |
| `turn_running` | 有回合正在进行，需要先打断 |
| `nothing_to_revert` | 一轮都没有 |
| `unknown_turn` | 那一轮不在现在的历史里 |
| `nothing_to_unrevert` | 没有可以恢复的撤销：没有撤销过，或者撤销以后又开启过新的一轮、压缩过 |

## `session.redo`

撤掉最后一轮，把开启这一轮的话再发送一次，开启新的一轮。网页界面里的「编辑上一句」也通过这个方法实现。

**参数**：`session`；`text`：替换开启这一轮的那句话的文字（不填写时使用原来的文字）；`attachments`：替换那句话的附件（不填写时使用原来的附件，空数组表示不要附件）；`prefix`：和 `session.send` 的「前缀要变」相同。

**回应**：和撤销相同，`events` 的最后是重新发送的每一句话的序号。新的一轮的 `turn.started` 里，`cause` 是这一条请求的 `id`。

**出错**：`not_redoable`（最后一轮不是由人说话开启的，或者一轮都没有）、`turn_running`、`empty_message`（替换以后一块内容都不剩），附件相关的错误和 `session.send` 相同，以及 `prefix_changing`。

## `session.compact`

**参数**：`session`；`instructions`：人附上的要求，例如「重点保留数据库设计的讨论」。

**回应**：`{"events": [<那一轮 turn.started 的序号>]}`，压缩一开始就返回，不等压缩完成。进度要看推送里的 `compaction.progress`、`compaction.done`、`context.compacted`、`turn.ended`。

**出错**：`turn_running`；`nothing_to_compact`（上一次压缩以后没有新的内容）。

## `session.clear`

清空上下文：核心开启一轮，只记一个空的检查点，不请求模型。待办也会一起清掉。

**参数**：`session`。**回应**：和 `session.compact` 相同；收到回应时，那一轮的开头、空的检查点和结束都已经推送过来了。如果撤掉这一轮，上下文会回到清空以前的状态。

**出错**：`turn_running`；`nothing_to_clear`（上下文本来就是空的）。

## `session.recap`

生成一句回顾：这个会话在做什么、做完了什么、卡在哪里。核心会单独请求一次模型，只提供最近几轮的对话。

**参数**：`session`。**回应**是 `{"text", "upto", "cached"}`：分别是那一句回顾、回顾截止到第几条事件、是不是返回了上一句回顾（之后没有新的内容，所以没有请求模型）。

生成成功时，订阅了这个会话的头都会先收到 `model.called`（`purpose` 是 `recap`）和 `session.recapped`。

**出错**：`nothing_to_recap`（AI 还一句话都没有说过）；`recap_failed`（请求出错，或者回复里没有文字）。

## `session.set_permission_level`

**参数**：`session`；`level`：只能是 `full`（完全放开）；`read_only`：只读开关。要改哪一项就写哪一项，至少写一个。

两种模式能读取的范围一样，数据根也可以读取（包括 `etc/private/secrets.toml`）；只读模式只多了一条规则：任何地方都不能写入，命令在只读的沙盒里运行。在完全放开模式下，数据根和其他目录一样可以写入，只有删除整个数据根（通过命令或 `trash` 删除都算，把整个数据根挪走、扔进回收站的命令也算）时，核心会按照「受保护的命令」里删除的那一类先询问人，人点了允许才会删除；数据根里面的文件和其他地方的文件一样处理。

**回应**：`{}`。设置变化时推送 `session.policy_changed`：`permission` 是换成的设置，`from` 是换之前的设置，头可以据此画出这一条事件以前的那几句，不需要往前翻（以前的日志里没有 `from`）。收紧权限当场生效；放宽权限从下一次请求开始生效。改成完全放开之前要不要先询问人，由头决定。

**出错**：`bad_params`：两项都没有写；或者 `level` 不是 `full`。

## `session.set_workspace`

**参数**：`session`；`cwd`：新的工作目录（绝对路径，或者以 `~` 开头）；`dirs`：新的加入目录，会整份替换原来的目录（空数组表示去掉全部）。至少写一个。

如果 `cwd` 写的是工作区本身（握手时的 `host.workspace`），就表示回到这个会话自己的默认工作区：核心会把它换成默认工作区（即会话列表、订阅回应里的 `default_workspace`）。

**回应**：`{"cwd", "dirs"}`，目录太宽时另外带 `wide: true`。目录变化时推送 `session.workspace_changed`；正在进行的这一轮仍然使用原来的目录，从下一轮开始使用新的目录。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 两项都没有写 |
| `path_unreadable` | 目录不存在，或者无法读取 |
| `not_a_directory` | 写的是一个文件 |
| `path_forbidden` | 目录落在 Miyu 的数据目录里（工作区 `workspace/` 除外） |
| `dir_too_wide` | `dirs` 里有一项太宽 |

## `session.terminal`、`session.set_terminal`

终端会话是你在 shell 里说的话所进入的那个一直延续下去的会话（用于终端无缝集成）。默认只有一个；带了 `terminal` 键的调用，每个终端各有一个。哪个会话算作终端会话、什么时候换成新的会话，由核心决定。

**`session.terminal`** 的参数：`cwd`（可以不填写）：需要新开会话时在哪个目录开，不填写时使用默认工作区；`fresh`（可以不填写，默认为假）：为真时不管现在指向哪个会话，都按下面第 3 条开一个新的会话并指向它（即 `/new`）；`terminal`（可以不填写）：哪个终端，值是头生成的键，长度 1 到 128 个字，不含控制字符，不符合时拒绝并返回 `bad_params`；带了这个键时，核心按键分别记一个指针，不带时使用原来那一个指针；`create`（可以不填写，默认为真）：为假时只查询、不新开，遇到下面第 2、3 条的情形时返回 `{"session": null}`，不新开会话，也不移动指针（用于终端刚打开、还没有说话时查询它在哪个会话里），和 `fresh: true` 一起写时拒绝并返回 `bad_params`。回应是 `{"session"}`，按下面的规则决定：

1. 如果指向的会话还在，并且是本机的主会话（不是子会话、场所会话或一次性的会话），就返回它。
2. 如果它闲置超过了 `terminal.idle_new_after` 分钟（设为 0 表示不更换），就开一个新的会话，把指针移过去，回应里多一个 `renewed: {"last"}`（旧会话最后一次有动静的时刻），旧会话保持原样。被指向的那一刻（通过 `session.set_terminal` 指向，或者新开）也算一次动静：如果你主动切换到一个很久没有说话的会话，闲置时间从切换的那一刻开始计算。
3. 如果还没有指向过会话，或者指向的会话已经删除、不符合条件，就开一个新的会话，回应里多一个 `created: true`。

新开的会话使用 `terminal.persona`、`terminal.preset`（没有设置时跟随 `persona.default`、`preset.default`）。如果需要新开会话，而 `cwd` 落在数据根里（并且不在数据根的 `workspace/` 里），核心不会新开会话，也不移动指针，而是拒绝并返回 `path_forbidden`，带着这个目录的 `session.send` 也是这样拒绝的；如果指向的会话还在、不需要新开，仍然返回它。同一时刻有几个连接一起请求（同一个键）时，只会开一个会话。每个键都记下最后一次使用的时刻，7 天没有使用的键会在写指针时顺便清掉，最多保留 64 个（最久没有使用的先清掉）；被清掉的键再次使用时，当作从来没有指向过。

**`session.set_terminal`** 的参数：`session`：把终端会话指向这个会话（即 `/sessions` 里选中的会话）；`terminal`（可以不填写）：修改哪个终端的指针，写法和上面相同。只接受本机的主会话，其他会话会被拒绝并返回 `session_not_found`。回应是 `{"session"}`。

## `session.set_meta`

**参数**：`session`；`title`：新标题，1 到 200 个字，`null` 表示去掉标题；`pinned`：`true` 表示置顶，`false` 表示取消置顶；`project`：写项目编号表示移进这个项目，`null` 表示移出项目（只有本机的主会话能移进项目：场所会话、子代理、分叉出来的会话会被拒绝并返回 `bad_params`，`data.field` 是 `project`）。至少写一个。置顶和项目是两个独立的字段：拖进项目时要不要取消置顶，由头另外写 `pinned` 决定。

**回应**：`{}`。变化时推送 `session.meta_changed`。对于没有起名的会话，核心会在第一轮回答完以后自动起一个标题，同样推送 `session.meta_changed`（`by` 是内核）。

**出错**：`bad_params`：一项都没有写；标题是空的，或者超过 200 个字；`project` 写了空字符串，或者这个会话不能移进项目（`data.field` 是 `project`）。`unknown_project`：没有这个项目。

## `session.retitle`

让模型根据最近的对话重新起一个标题：它使用和核心自动起标题相同的指令、相同的模型，同样记录用量（`model.called` 的 `purpose` 是 `title`），区别只是它看的是最近回答完的至多三轮（自动起标题只看第一轮）。每一轮只带人说的话和 AI 最后的正文回答，不带工具调用和思考；还没有回答完的那一轮不带。人起过的标题也会被替换；创建时写了 `titles: false` 的会话、一次性的会话、子会话、场所会话也同样会起标题。

**参数**：`session`。

**回应**：`{}`：请求发出去就返回，不等待模型。标题起好以后推送 `session.meta_changed`，`by` 是发起请求的人，所以这个标题算作人起的：会话列表里那一项会写 `named`，以后核心自动起标题也不会覆盖它。如果已经有一次起标题的请求正在进行，就不再发送，仍然返回 `{}`。正在进行的那一轮不带，也不会被打断；如果第一轮还没有回答完，就等这一轮结束以后再起标题。如果没有起成功（模型出错、发出请求 180 秒后还没有说完、没有写出标题、要求调用工具），标题保持不变，核心记一条出错的 `model.called`，订阅了事件流的头会收到瞬时事件 `session.retitle_failed`（见 [事件](events.md#瞬时事件)），视图流里多一条 `retitle_failed` 旁白（见 [视图流](view.md)），头据此告诉人「更改标题失败」；如果发起请求以后、标题起好之前，人又用 `session.set_meta` 修改了标题，就保留人改的标题，什么都不推送。核心自动起标题失败时不推送。

**出错**：`bad_params`：没有写 `session`；或者没有可以用来起标题的内容（AI 还没有一句带正文的回复，此时也没有回合正在进行）。没有这个会话时返回 `session_not_found`。

## `session.viewing`

头告诉核心自己此刻正在显示哪个会话：切换会话、新开会话、刚启动时各发送一次。这个方法只给人使用的头调用，扩展调用时返回 `needs_approval`。

**参数**：`session`：正在显示的会话；`null` 或者不填写表示没有显示任何会话。

**回应**：`{}`。核心按连接记录，连接断开以后就忘掉。至少有一个头正在显示某个会话时，会话列表里那一项写 `viewed: true`，变化时像 `busy` 一样推送 `sessions.changed`。后台会话回答完、正在提问、需要确认时的通知，由正在显示它的那个头照常显示；其他头看到 `viewed` 就不再另外弹出，避免几个头各弹一遍。

**出错**：`bad_params`；没有这个会话时返回 `session_not_found`。

## `session.configure`

更换这个会话使用的模型、多模态模型、人格、预设、思考强度、压缩触发线，这些从下一轮开始生效；上下文上限从下一次请求开始生效。更换模型、思考强度时，显示当场变化，请求从下一轮开始变化：`subscribe` 回应和 `view.status` 里的 `model` 当场按换成的值写，切换到其他会话再切回来、重新连接以后也是新的值。

**参数**：`session`；下面七项至少写一项：
- `model`：`<供应商>/<模型>` 或 `@<池>`；`null` 表示不再固定使用某一个模型，回到跟随配置的 `models.chat`（每一轮按那时的配置，配置改了就跟着改）。填写三个固定的池（`@lite`、`@normal`、`@flagship`）之一、这个池又是空的时，和 `null` 相同。
- `vision`：多模态模型，写法和 `model` 相同：`<供应商>/<模型>` 或 `@<池>`；`null` 表示不再固定，回到跟随配置的 `models.vision`；填写三个固定的池之一、这个池又是空的时，和 `null` 相同。会话的模型不能看图时，核心用这个模型把图片转成文字（没有固定时使用 `models.vision`）；如果固定的模型后来解析不出来（供应商或池被删除了），那张图片按没有转成文字处理，不会退回使用 `models.vision`。
- `persona`：人格编号；`null` 表示无人格。
- `preset`：预设编号；`null` 表示换成无工具。
- `effort`：思考强度的一档，按这个会话此刻所用模型的档位检查（即 `model.list` 的 `facts.reasoning`；如果同一次调用还换了模型，就按换成的模型检查）。使用池的会话不能设置这一项，写了会返回 `bad_params`（`data.field` 是 `effort`），池里的每个成员按它自己的配置；`null` 表示回到跟随配置。这一项只影响这个会话：配置里的 `providers.<id>.models.<model>.effort` 是新会话的默认值。换到没有这一档的模型时使用配置，换回来以后又会用上这一档。
- `context`：上下文上限，单位是 token；`null` 表示去掉上限。生效的窗口取模型的窗口和这个上限中较小的一个（模型没有报告窗口时直接使用这个上限），压缩线按生效的窗口计算；上限跟着会话走，换模型、池更换成员以后仍然使用。如果上限太小，按它算不出压缩线，就返回 `bad_params`（`data.field` 是 `context`，`data.min` 是最少需要多少），并且什么都不更换。人输入 `/context` 修改的也是这一项。
- `compaction_line`：压缩触发线，是窗口的百分比，取值为 20 到 95 的整数；`null` 表示不再固定，回到跟随配置的 `compaction.line`。上下文用到这条线时会自动压缩。压缩线仍然不会超过「窗口 − 输出预留 − 余量」：20 万的窗口调到 85 以上不起作用，百万级的窗口才能达到 95。

更换人格、预设时会先压缩一轮：核心马上用旧人格把前面的对话压缩成摘要，下一轮换上新的人格或预设，从摘要接着对话。跟随人格的记忆从这一轮开始记在新人格的那一间里，以前的记忆留在原来那一间里。

**回应**：`{}`。变化时推送 `session.policy_changed`；更换模型、思考强度时，还会当场推送一条 `model.changed`（`why` 是 `turn`）：换成模型时带 `endpoint`、`model`，`effort` 能算出来时才带；换成池时只有 `ref`，窗口、压缩线保持原样；下一轮开始真正换过去时，如果和刚才推送的一样，就不再推送，如果不一样（池固定了某个成员、窗口变了），就再推送一条。更换上下文上限时，`subscribe` 回应和 `view.status` 里的 `limits` 当场更换（`window` 是生效的窗口，`context` 是上限，没有设置时不写）。此后 `subscribe` 回应和 `model.changed` 里的 `model.effort` 是 `{"level", "from"}`：`from` 为 `session` 表示这是会话记下的那一档，为 `system` 表示来自配置。`session.list`、`subscribe` 回应里的 `persona`、`preset` 马上按换成的值写（换成无工具时写 `preset: null`、`no_tools: true`）。固定了多模态模型时，`view.status` 马上带上 `vision`，去掉以后不写；固定了压缩触发线时，同样马上带上 `compaction_line`，并且从下一轮开始，`limits.compaction_line` 按它更换。

**出错**：`bad_params`（一项都没有写、写了空字符串、类型不对；子代理更换人格、预设时，`data.field` 是那一项；`effort` 不在此刻所用模型的档位里时，`data.field` 是 `effort`，什么都不更换；`context` 不是非负整数；`context` 太小时，`data.field` 是 `context`、`data.min` 是最少需要多少，什么都不更换；`compaction_line` 不是整数，或者不在 20 到 95 之间时，`data.field` 是 `compaction_line`，什么都不更换）；`unknown_model`：无法解析 `model`、`vision`，什么都不更换；`unknown_persona`；`unknown_preset`；`turn_running`：更换人格、预设时有回合正在进行（需要等这一轮完成）。

## `session.fork`

从一个主会话分叉出子会话：子会话带着主会话到分叉点为止的历史接着对话，前面的请求前缀和主会话逐字节相同，所以缓存可以继续使用。

**参数**：`session`（父会话，必须是主会话）；`seal_after`：运行完几轮以后封存（从 1 开始），不填写时不封存。

**回应**：`{"session": <子会话编号>, "upto": <分叉点的序号>}`。如果父会话正在运行一轮，分叉点是这一轮开始之前。

**出错**：`bad_params`：父会话是子代理，或者是分叉出来的会话；`seal_after` 不是从 1 开始的整数。

## `session.delete`

把会话挪进回收处，保留 7 天。它派出去的子会话会一起删除。

**参数**：`session`。**回应**：`{}`，挪好以后才返回。订阅了这个会话的头会收到 `resync`。

**出错**：`turn_running`：有回合正在进行（需要先打断）；`internal_error`：无法挪动会话。

## `session.prompts`

按先后列出一个会话里你问过的所有话。界面根据它列出「问话跳转」：如果某一句还没有读进来，就按它的 `seq`，用 `view.page` 的 `before` 往前读到那一页，再跳过去。这个方法只读日志，不载入会话。

**参数**：`session`。

**回应**：`prompts`，一项对应一句话：

| 名字 | 说明 |
|---|---|
| `seq` | 那条 `message.user` 的序号 |
| `turn` | 它所在的回合：在回合进行中排队的话，是那一轮；闲着时说的话，是它开启的那一轮；两者都没有时不写 |
| `at` | 它的时刻，写法和事件的 `at` 相同 |
| `text` | 第一行，去掉前后的空白，最多 100 个字；由斜杠命令模板展开的话，显示你输入的那一行（取法和 `session.list` 的 `preview` 相同）。只有图片、文件时是空的 |

- **只列出你本人在这里说的话**：本机说的话、平台桥替你从平台私聊里转进来的话都会列出；子会话或父会话发来的话、平台上其他人说的话（群里识别出是你的也算平台上的人）、注入的话（例如长期目标的续轮）、其他 harness 或定时任务提交进来的话都不列出。
- **按有效历史列出**：撤销掉的话不列出，恢复的话放回来，重做（编辑上一句）的话按最后那一版列出，撤回的排队消息不列出；压缩过的话仍然列出。
- **分叉出来的会话**只列出它自己的话：分叉前的那部分话在父会话里列出。

**出错**：`bad_params`；`session_not_found`；`session_broken`。

## `session.export`

把一个会话导出来，写到你给的位置，或者放进回应：原始包（ZIP）用于存档、换机器、排查问题，以后的导入也只接受原始包；Markdown 是给人看的。这个方法只给人使用的头调用。

**参数**：

| 名字 | 说明 |
|---|---|
| `session` | 会话编号 |
| `format` | `bundle` 表示原始包；`markdown` 表示 Markdown |
| `to` | `{"path": "<绝对路径>"}` 表示核心写到这个文件；`"inline"` 表示放进回应的 `text`，只适用于 Markdown |
| `for` | 只适用于 Markdown，表示给谁用：`read` 是给自己看（不填写时也是这个值），`agent` 是给其他 agent（例如 Claude Code、codex），`chat` 是贴进网页版的 chat |
| `range` | 只适用于 Markdown，表示导出多少：`all` 是全部（`read`、`chat` 不填写时是这个值）；`context` 是 AI 此刻的上下文，从最近一次压缩开始（`agent` 不填写时是这个值）；`{"turns": N}` 是最近 N 轮 |
| `thinking` | 只适用于 Markdown，布尔值，表示是否带上思考。不填写时按 `for` 决定（`read` 带上，其他值不带） |
| `tools` | 只适用于 Markdown，表示工具调用写多少：`line` 写一行；`seen` 写 AI 当时看到的内容；`full` 写全文，后台命令的输出、脚本里调用的工具返回的内容也会取来。不填写时按 `for` 决定（`chat` 是 `line`，其他值是 `seen`） |
| `children` | 只适用于 Markdown，表示子代理怎么写：`reports`（不填写时也是这个值）按 `for` 写交代和回报；`append` 还会把每个子会话按同样的写法整份附在后面，按任务编号排列 |
| `assets` | 只适用于 Markdown，表示图片、附件怎么处理：`placeholder`（不填写时也是这个值）写成占位；`files` 连同图片一起导出：拷进 Markdown 旁边的 `<文件名>_files/` 文件夹，Markdown 里写相对路径。只能和 `to: {"path"}` 一起使用 |
| `locale` | 只适用于 Markdown，表示显示名、旁白用哪种语言（`zh`、`ja`，其他值都是英文）；不填写时使用握手时的语言 |

**回应**（原始包）：`path`、`bytes`（包的大小）、`upto`（这个会话导出到第几条事件）、`sessions`（包里有哪几个会话）、`blobs`（有几个附件、大内容）。

**回应**（Markdown）：`path` 或 `text`、`bytes`（多少字节）、`tokens`（估算值，和底栏上下文用量的算法相同）、`upto`（这个会话导出到第几条事件）、`redacted`（隐去了哪几个密钥、各隐去几处，例如 `{"openai": 2}`，附在后面的子会话也算在内）、`children`（附在后面的子会话，`[{"job": "j1", "session": "…", "title": "…"}]`，没有写 `append` 时是 `[]`）；写了 `assets: files` 时还有 `assets`：`{"dir": "<文件夹的绝对路径>", "files": 3, "missing": ["sha256:…"]}`，其中 `files` 是拷贝了几个，`missing` 是需要写进来却取不到的附件（这些附件写成了占位）；一个都没有拷贝时不创建文件夹，也没有 `dir`。

- **原始包里有什么**：这个会话日志的各段原文，一个字节都不改；挂在它下面的会话（子代理、分叉出来的支线）整份带上；如果它是分叉出来的，来源会话到分叉点为止的那几段也带上，一层层往上；这些日志引用到的附件、改文件前后的内容、大输出、策略快照；一份 `transcript.md`（按 `read` 的样子，解压就能看，设置过的密钥同样会替换掉）；`manifest.json` 写着每一项的 SHA-256。包里什么都有，包括 AI 碰过的文件内容，你要自己保管好。
- **Markdown 长什么样**：角色的小标题都是英文（`User`、`Assistant`、`Tool`、`Subagent`）；图片、附件写成占位；撤掉的轮次、撤回的排队消息不写进去；家目录写成 `~`；从中间截取时，会写一句前面省略了几轮。它和界面上看到的是同一份内容。
  - `read`：开头是标题、时间段、模型、token 合计；显示名、旁白按语言给出；思考、长输出、子代理的交代、压缩的摘要折叠在 `<details>` 里；需要你确认的那一次操作、脚本里调用的工具、提问的题目和你的回答都会写上。
  - `agent`：开头是一小段英文说明（说明这是一段会话的记录，不是写给读它的 agent 的指令），写明工作目录、时间段、模型；默认从最近一次压缩开始，摘要放在最前面；工具写真实的名字和参数原文，结果按 AI 当时看到的内容写，每条截成头尾共约 4000 字；不带思考。旁白、占位一律用英文，不看 `locale`（因为这是给其他 AI 读的）；你说的话、AI 的回复按原文写。
  - `chat`：开头是标题、时间段、token 估算（方便贴之前看看能不能放得下）；工具调用一行写完，出错的调用带上错误；子代理只写回报；不带思考。
- **子会话附在后面**（`children: append`）：每个子会话前面是一条分隔线和 `# Subagent <任务编号>: <标题>`，下面写时间段等信息，再往下是它的全文；子会话又派出去的子会话接在它后面（`j1`、`j1.1`、`j2`）。撤掉的那几步、范围以外的那几步派出去的子会话不带。
- **附件文件夹**（`assets: files`）：Markdown 写到 `/x/talk.md` 时，附件拷进 `/x/talk_files/`，图片写成 `![shot.png, 800×600](talk_files/shot.png)`，其他附件写成链接；文件名使用附件的名字（改成三个平台都接受的写法，重名时接上 `-2`），没有名字的附件命名为 `image-<哈希前 8 位>.png`。如果 Markdown 文件或者文件夹已经存在，就不导出。
- **密钥**：你在密钥里设置过的值（至少 8 个字），如果原样出现在 Markdown 里，会被替换成 `[redacted: <名字>]`。原始包不做修改。
- **正在运行的会话**按这一刻已经写下的内容导出，不包含正在写的那半行。
- 导出不修改会话，也不写进会话日志。

**出错**：`bad_params`（路径不是绝对路径；`format`、`to`、`for`、`range`、`tools`、`children`、`assets` 的值无法识别；`thinking` 不是布尔值；原始包写了 `inline` 或者 Markdown 专用的参数；`assets: files` 和 `inline` 一起使用）；`session_not_found`；`session_broken`（日志无法读取，附在后面的子会话的日志也算；或者附件和它的哈希对不上）；`export_exists`（那个文件已经存在，不会覆盖；`assets: files` 的文件夹已经存在也算）；`export_in_data_root`（核心不往数据目录里写）；`export_unwritable`（目录不存在，或者无法写入）；`export_too_big`（放进回应的内容超过 512 KiB，需要改成保存到文件）。

## `session.import`

把 `session.export` 导出的原始包导入进来：换机器、换数据目录时用它搬运会话，旧版 Miyu 的迁移程序也使用它。这个方法只给人使用的头调用。

**参数**：

| 名字 | 说明 |
|---|---|
| `blob` | 先用 `blob.put` 上传的包 |
| `path` | 本机上的包，写绝对路径（命令行使用）；`blob`、`path` 只写其中一个 |
| `archive` | 布尔，默认 `false`：为真时导入成只读存档 |

**回应**：`main`（包里的主会话）、`sessions`（这一次新导入的会话）、`present`（早就存在、跳过了的会话）。

- **先核对，再写入磁盘**：包里每一项的哈希、日志的序号都对得上才会写入；有一项不对，整个包都不接受，数据目录里不会多出任何东西。
- **编号保持原样**：如果已经有同样编号、内容也一样的会话，就当作已经导入过，跳过它；内容不一样时拒绝导入。同一个包再导入一次，什么都不会变化。
- **存档**：可以翻看、搜索、导出，AI 搜索以前的对话时也能搜到；但是不能接着对话，发送消息、撤销、压缩这些修改会话的操作会返回 `archived`。包里标明是存档的会话，导入以后仍然是存档。不是存档的会话导入以后就能接着对话。
- 导入完马上就能搜索到，不需要先打开一次。会话列表会推送一条变化。

**出错**：`bad_params`；`unknown_attachment`（`blob` 不存在）；`bad_bundle`（不是会话包）；`bundle_mismatch`（包已损坏）；`bundle_too_new`（包是更新的版本导出的）；`session_exists`（已经有同样编号、内容不一样的会话，`data.session` 说明是哪一个）。

## `view.page`

按页读取历史，从最新的一页往前翻，每一页的边界都落在两轮之间。

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `before` | 正整数 | 只要序号小于它的事件；不填写时返回最新的一页。往前翻时，填写上一页的 `first` |
| `turns` | 1 到 50，默认 20 | 最多返回几轮 |
| `view` | 布尔，默认 `false` | 为 `true` 时返回界面可以直接绘制的条目（视图投影），不返回原始事件 |

**回应**：`events`（或者 `entries`）、`more`（前面还有没有内容），有内容时另外带 `first`、`last`；因为超过 1 MiB 而少给了轮数时，带 `capped: true`；这一页里已经报告完成、但是更早派出的任务，带在 `jobs` 里；日志中间有一行损坏、这一页只给到损坏的那一行以前时，带 `broken`。

最新一页的 `last` 就是此刻的最后一条事件：接着用 `subscribe {"after": last}`，就只会收到新的事件。

`broken` 是 `{"segment", "line"}`：表示哪一段日志（只是文件名，例如 `000000000001.jsonl`）的第几行损坏了。如果这一页要读的行里遇到了损坏的行，这一页就像日志在损坏行的前一条结束了一样；如果损坏的行不在这一页要读的范围里，这一页照常返回，不带 `broken`。这样的会话无法载入，发送消息、订阅时返回 `session_read_only`：界面按 `view.page` 以只读的方式显示，说明损坏在哪里，要继续就新开一个会话。核心不修改、也不修复日志。

**出错**：`bad_params`；`session_not_found`；`session_broken`（日志无法解析，属于连第一条都读不出来的这一类，没有可以查看的内容）。

## `view.detail`

**参数**：`session`；`call`（一次工具调用的 `call_id`）、`job`（一个后台任务的编号）或 `compaction`（一次压缩替代到的序号），三者正好写一个；另外可以写 `nested_after`（整数）：嵌套调用只要序号比它大的，以及在它之前开始、现在还在运行的调用。

**回应**

- 调用：`{"files": [...], "output"}`。`output` 是结果原文，不截断；`files` 一个文件一项：`path`、`action`（`write` 或 `trash`）、`diff`（统一格式，上下文 3 行）、`added`、`removed`，算不出差异时写 `skipped`（`too_big`、`binary`、`missing`）。
- 供应商侧工具（视图里 `hosted: true` 的那一步）：`{"files": [], "output"}`，`output` 是供应商返回的结果（超过 64 KiB 时会截断，截断过的另外带 `output_cut: true`）；还没有结果时，按没有这次调用处理。
- 压缩：`{"summary"}`，即那一次压缩的摘要原文。
- 脚本里调用过工具（`miyu call`）时：调用的回应多一个 `nested`，后台任务的回应只有 `{"nested"}`。`nested` 按序号一项对应一次调用：`n`（序号）、`name`、`title`（`{name, object}`，显示名按连接的语言给出）、`args`（参数原文）、`state`（`running`、`ok`、`error`、`denied`、`needs_approval`、`cancelled`），有的还带 `started`（正在运行的调用开始的时刻）、`took_ms`、`reason`（拒绝或出错的那一句说明）、`result`（`{blob, bytes}`，交给脚本的那份结果，用 `blob.get` 获取）。如果那次 `shell` 还在运行、还没有结果，也返回 `{"nested"}`。

同一次调用的结果不会变化，查询一次就够了；下面调用的工具在运行时会越来越多，用 `nested_after` 就只获取新的调用。

**出错**：`bad_params`；`session_not_found`；`unknown_call`（没有这次调用的结果，或者没有这次压缩）；`session_broken`。
