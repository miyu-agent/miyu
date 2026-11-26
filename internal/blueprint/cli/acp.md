## `miyu acp`

### 是什么

Miyu 当 ACP（Agent Client Protocol，规范 <https://agentclientprotocol.com>，v1）的 agent：Zed、JetBrains 这类编辑器把 `miyu acp` 当子进程拉起，在标准输入、标准输出上说 JSON-RPC 2.0，一行一条。它连本机的核心（照 `miyu ask` 一样连、没在跑就拉起来），把 ACP 翻成核心的会话协议。不另起模型、不另存会话：编辑器里的会话就是 Miyu 的会话，编号同一个，终端、网页里也看得到、接得上（设计 73 第五节，施工 ACP-6）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `acp`；换上帮助页；拉起核心用的命令是自己加上 `core`（`cli/main.md`） |
| `crates/miyu-cli/src/acp.rs` | 找数据根、连核心（`connect_or_start`），拿标准输入、标准输出交给主循环；开局的错误写在标准错误上 |
| `crates/miyu-cli/src/acp/server.rs` | 主循环：编辑器的一行照方法分派，核心的推送照会话翻，翻出来的事办掉（报、要对比、回请求、问人、报配置项） |
| `crates/miyu-cli/src/acp/editor.rs` | 对着编辑器的那一头：回应、出错、`session/update`、向编辑器发的请求 |
| `crates/miyu-cli/src/acp/setup.rs` | 握手、`session/new`、`load`、`resume`、`list`、`close`、`set_mode`、`set_config_option` |
| `crates/miyu-cli/src/acp/turns.rs` | `session/prompt`、`session/cancel`、斜杠命令、`available_commands_update` |
| `crates/miyu-cli/src/acp/prompt.rs` | 内容块凑成一句话和附件，附件传给核心 |
| `crates/miyu-cli/src/acp/session.rs` | 一个会话记着什么；一轮怎么结束写成 `stopReason` |
| `crates/miyu-cli/src/acp/view.rs`、`status.rs` | 视图流的条目、会话状态翻成 `session/update`（纯逻辑） |
| `crates/miyu-cli/src/acp/tool.rs` | 一步工具写成 `tool_call`、`tool_call_update`，对比 |
| `crates/miyu-cli/src/acp/asking.rs`、`forms.rs` | 确认、提问翻成 `session/request_permission`、`elicitation/create`，回答交回核心 |
| `crates/miyu-cli/src/acp/options.rs` | 模式、会话配置项 |
| `crates/miyu-cli/src/language/acp.rs` | 给人看的字 |
| `crates/miyu-cli/src/help/{zh,en}/acp.txt` | 帮助页 |

### 对外的样子

- 没有参数。标准输出上只有协议的行；连不上核心这类开局的错误照 `miyu ask` 的写法写在标准错误上（`miyu: …`），退出码 1。编辑器关了管道：退出码 0，正在进行的回合留在核心里接着跑。核心断了：等着回的 `session/prompt` 都回出错（`-32603`），退出码 1。
- 用到的环境变量：`MIYU_HOME`（数据根）；拉起的核心照它自己的一套（`core.md`）。
- 握手时向核心报 `head.kind` 是 `acp`、`caps.input` 是 `true`（有人能确认，核心照常等）、不带 `prefix_ask`。

### 握手

`initialize` 回：

```json
{"protocolVersion": 1,
 "agentCapabilities": {"loadSession": true,
   "promptCapabilities": {"image": true, "audio": false, "embeddedContext": true},
   "sessionCapabilities": {"list": {}, "resume": {}, "close": {}}},
 "agentInfo": {"name": "miyu", "title": "Miyu", "version": "<版本>"},
 "authMethods": []}
```

- 编辑器报的版本不管多少都回 `1`（规范：不支持对方的版本就回自己支持的最高版本）。
- 不声明 `mcpCapabilities`：核心的 MCP 服务一个核心一份、所有会话共用（`mcp.md`「一、说哪一版」第 3 条），没有只对一个会话的接法；`session/new` 等带来的 `mcpServers` 不收，运行日志里记一行有几个。
- 不声明 `systemPrompt`、`additionalDirectories`；不用编辑器的 `fs/*`、`terminal/*`。
- 编辑器的能力只看 `elicitation.form`：有的，提问走表单。
- 握手以前来的别的请求回 `-32600`；不认识的方法 `-32601`；读不懂的行 `-32700`。

### 会话

| ACP | 核心 |
|---|---|
| `session/new {cwd}` | `cwd` 要是绝对路径（不是的 `-32602`），去掉 `.`；`session.create {cwd}`，别的照配置（权限照 Miyu 的默认，项目主人 2026-11-26 定）；再照下面接上 |
| `session/load {sessionId, cwd}` | `subscribe {stream: "view"}`；回应里那一页之前还有的，照 `view.page {before: first, turns: 50, view: true}` 一页页往前翻到头；条目照先后重放（见「一轮怎么翻」，人说的话都报 `user_message_chunk`），会话状态报一遍，再回 |
| `session/resume {sessionId, cwd}` | 同上，只是条目只记下、不报 |
| 接上以后 | 回 `{modes, configOptions}`（`new` 的另带 `sessionId`）；接着报 `available_commands_update`；有在等人的接着问 |
| `session/list {cwd?}` | `session.list {dream: false}`，去掉子会话（`parent` 不是 `null` 的）；`cwd`（`~/…` 照握手回的 `host.home` 写成绝对路径）和要的那个解开链接以后比，同一个的才列；一项 `{sessionId, cwd, title?, updatedAt?}`，`title` 是标题，没有的是 `preview`，`updatedAt` 是 `last_active`；不分页 |
| `session/close {sessionId}` | 有一句在等的先 `session.interrupt {queued: "return"}`、回那一句 `cancelled`；`unsubscribe`，忘掉会话和问过编辑器还没回的；回 `{}`。没开着的也回 `{}`。会话本身留在 Miyu 里 |

- 没有这个会话（`subscribe` 回 `session_not_found`），和要的会话没在这个编辑器里开着：`-32002`，`data.sessionId` 是要的那个。
- `load`、`resume` 不换会话的工作目录：编辑器给的 `cwd` 只用来补全工具参数里的相对路径。

### 说一句

`session/prompt {sessionId, prompt}`：

1. 同一个会话上一句还没回的 `-32600`。
2. 内容块：`text` 照先后接起来；`resource_link` 写进话里，本机文件（`file://`，去掉主机名 `localhost`、解开 `%xx`，Windows 去掉盘符前的 `/`）写成路径，别的照地址原样，前后没有空白的补一个空格；`image` 解开 base64 当附件，名字 `image-<第几个>.<jpg|png|gif|webp>`；`resource` 带 `text` 的照 UTF-8、带 `blob` 的解开 base64 当附件，名字是地址的最后一段，没有的 `file-<第几个>`，类型照 `mimeType`；`audio` 不收。base64 解不开的整句不收（`-32602`）；没有字、没有附件的 `-32602`。
3. 没有附件、`/` 开头的先看是不是斜杠命令（下一节）。
4. 附件照先后传：不超过 512 KiB 的 `blob.put {data, name, media_type?}`，大的 `blob.open`、`blob.write`（一块 512 KiB）、`blob.close`；`session.send {session, text, attachments?}`。被拒的照核心的原话回 `-32603`。
5. 记下这一句（`m<序号>`）：编辑器自己画了，之后不报回去；它有了所属的一轮，等那一轮的收尾（`end`）来了回 `stopReason`。

`session/cancel`（通知）：在等的那一句还没所属的一轮的，先 `session.withdraw {message: <序号>}`，退回了当场回 `cancelled`；退不了的（已经听到了）和已经开了一轮的发 `session.interrupt {queued: "return"}`，那一轮结束时回 `cancelled`（收到过取消的，不管怎么结束都回 `cancelled`，规范要求）。

`stopReason`：

| 一轮的收尾 | 回 |
|---|---|
| `completed` | `end_turn` |
| `interrupted` | `cancelled` |
| `step_limit` | `max_turn_requests` |
| 出错，分类 `content_policy` | `refusal` |
| 出错，分类 `output_limit` | `max_tokens` |
| 别的出错、`aborted`、`restarted` | JSON-RPC 出错 `-32603`，话是收尾的 `explain`，没有的是错误原话 |

### 斜杠命令

- 接上会话时问 `command.catalog {session}`，报 `available_commands_update`：一条命令 `{name, description, input?}`，`description` 是 `summary`，`input.hint` 是 `argument`；打开整页的（带 `opens`）不列；另加头自己的 `/compact`。
- 一句话（没有附件）去掉首尾空白以后 `/` 开头、第一个词是表里的名字或别名：`command.run {session, text: <这一行>, cwd: <编辑器给的目录>}`。回的 `said` 不空的报成一段 `agent_message_chunk`；回的 `events` 里有引出的一句话的，等它那一轮结束；开的一轮在回应以前就结束了的（`/clear`），照它回；都没有的，命令一回就回 `end_turn`。
- `/compact [要点]`（表里没有同名的时候）：`session.compact {session, instructions?}`，等它开的那一轮结束。
- 被拒（没有能压的、有回合在进行）：照核心的原话报一段 `agent_message_chunk`，回 `end_turn`。
- 不是表里的：照平常的话说。

### 一轮怎么翻

视图流的推送照会话翻（`view.md`）：

每一段流式的字（`agent_message_chunk`、`agent_thought_chunk`、`user_message_chunk`）带 `messageId`，是它那一条条目的编号：编号变了编辑器另起一块，上一轮打断了的思考和下一轮的思考不连成一句（ACP-7 在真 Zed 里看到过）。斜杠命令的回执、被拒的原话是 `said-<编辑器那条请求的编号>`。

| 条目、推送 | `session/update` |
|---|---|
| `reply` | `agent_message_chunk`：第一次见报整份，之后 `view.append` 的那一截、`view.update` 里报过的以后多出来的；前面对不上的（重写过）不报 |
| `thought` | `agent_thought_chunk`，同上 |
| `user`，不是编辑器发的 | `user_message_chunk`（在终端、网页里接着说的），只报一次；退回的、空的不报 |
| `tool` | 第一次见 `tool_call`，之后 `tool_call_update` 只写变了的格（见「工具步骤」） |
| `end` | 回等着的 `session/prompt` |
| 一轮头一次报内容（正文、思考、工具），这一轮还没有开头 | 先报一句开头 `user_message_chunk`（`messageId` 是 `turn-<那一轮>`），编辑器才另起一条消息：后台任务报回来叫醒的（旁白 `job` 记下的最近一件）是 `后台任务「<标题>」已完成`（`失败`、`已停止`、`已结束`），别的是 `Miyu 自己开始了新的一轮`。有开头的：人说的话所属的一轮（编辑器发的、别处来的都算），编辑器在等的那一句还没听到时的一轮、等的就是它的那一轮。只记下的不报 |
| 别的条目、`view.hidden`、`view.remove` | 不报（ACP 没有收回报过的内容的写法） |
| `resync` | 重新订阅；回应里的那一页照「正在发生」再过一遍，报过的只报变了的 |

### 工具步骤

- `toolCallId` 是条目编号（还在写参数时就有）；`name` 是工具名。
- `title`：工具卡片的那一句：`title.name`，有 `via` 的接 ` (<via>)`，有 `object` 的空一格接上，有 `said` 的接 ` · <said>`。
- `kind`：`read` → `read`；`edit`、`write` → `edit`；`trash` → `delete`；`glob`、`grep`、`web_search` 和供应商侧的 `search_web`、`WebSearch` → `search`；`shell` → `execute`；`web_fetch` 和供应商侧的 `read_url_content`、`WebFetch` → `fetch`；`todowrite` → `think`；别的 `other`。
- `status`：`preparing` → `pending`；`running` 有确认没定的 → `pending`，别的 → `in_progress`；`ok` → `completed`；`error`、`denied`、`cancelled`、`skipped` → `failed`。
- `rawInput`：参数写完了的（不是 `preparing`）照参数原文解开。
- `locations`：结果里改了的文件（`files`，真实位置的绝对路径）；还没结果的，`read`、`edit`、`write` 照参数的 `file_path`，相对的照编辑器给的目录补全。
- `content`（每次整份换）：执行命令的（`shell`，参数写完了的）先是一段代码块 `$ <命令原文>`：标题只有她写的说明，规范要编辑器把命令写全，确认框里尤其要看到；再是结果：有结果的，输出原文的开头一段围进代码块（围栏比字里最长的一串反引号多一个，至少三个）；没有输出的写结果那一句（被拒的为什么）。做成了、新建或者改了文件的，再要 `view.detail {session, call}`，一个文件的每一段 `@@` 写成一项 `{"type": "diff", path, oldText, newText}`（改前是上下文加删的行，改后是上下文加加的行，新建的文件改前是 `null`），有对比的整个 `content` 换成对比；扔进回收站的不要对比。重放时不要对比。

### 确认和提问

会话状态的 `waiting` 里每一件（调用编号，脚本里调的带序号）只问编辑器一次；了结了的（别的头上答了、到时被拒）忘掉。

- **确认**（`approve`）：`session/request_permission {sessionId, toolCall: {toolCallId, title, kind, status: "pending", rawInput?, content?}, options}`，`content` 同工具卡片（执行命令的是命令原文）。选项照先后：`allow_once`（`仅允许本次`，`decision: once`）；带了放行范围（`approval.rule`）的再有 `allow_session`（`在本会话内允许`，kind `allow_always`，`decision: session`）；`reject`（`拒绝`，kind `reject_once`，`decision: deny`）。编辑器选了的照 `session.answer {session, call, decision, nested?}` 回；取消了的、认不得的一律拒绝。
- **提问**（`ask`），编辑器能弹表单的：`elicitation/create {sessionId, toolCallId, mode: "form", message, requestedSchema}`：第 N 道题是 `qN`（单选带 `enum` 的字、多选元素带 `enum` 的数组、没选项的字），有选项的另有 `qN_other`（标题 `其他`）。题目只写一处，都不写说明（ACP-7 在真 Zed 里看到同一句出现三次）：一道题的，题目是 `message`，格的标题只写 `header`，没有的不写；几道的，`message` 是 `回答下面的问题`，格的标题是题目，有 `header` 的写成 `<header>：<题目>`。`accept` 的照内容读（不是选项的不算、空白的字不算），别的每道都没答。
- **提问**，编辑器不能弹表单的：一道一道问 `session/request_permission`，`title` 是题目，每个选项一项（`c<第几个>`，kind `allow_once`），加一项 `skip`（`跳过`，kind `reject_once`）；没有选项的题当没答。
- 回答照 `session.answer {session, call, answers, nested?}` 交；核心回 `bad_answer` 的，照每道都没答再交一次。

### 会话状态

| 会话状态 | `session/update` |
|---|---|
| `todos` | `plan`：一项 `{content, priority: "medium", status}`；变了才报，从来没有待办的不报空的 |
| `context.used`、`context.window` | `usage_update {used, size, cost?}`：两个都有才报，变了才报；`cost` 只有一种币的照原样，几种币的照 `converted`，有 `rest`、有 `unpriced` 的不带 |
| `title` | `session_info_update {title}`，变了才报 |
| `permission`、`model.ref`、`model.effort.level` | 变了的报 `current_mode_update` 和整份 `config_option_update` |
| `waiting` | 见「确认和提问」 |

### 模式和配置项

- 模式两种：`full`（`开放权限`）、`read-only`（`只读`），照会话状态的 `permission.read_only`。`session/set_mode`：`full` 发 `session.set_permission_level {level: "full", read_only: false}`，`read-only` 发 `{read_only: true}`，回 `{}`；认不得的 `-32602`。
- 配置项照先后三项：`mode`（`权限`，category `mode`）；`model`（`模型`，category `model`，现在的模型知道才有）；`effort`（`思考强度`，category `thought_level`，现在的模型有思考档位、会话状态里有这一档才有）。
- 能选的模型：第一次接上会话时问一次 `model.list`：每家供应商一组（组名是供应商的名字），有密钥（`state` 不是 `no_key`）、不是向量模型的模型，名字照 `facts.name`；能用的池另一组 `模型池`（`@<名字>`）。现在用的不在里面的单列在最前面一组。思考档位照 `facts.reasoning`。
- `session/set_config_option {configId, value}`：`mode` 同上；`model` 要是能选的或者现在的，`session.configure {model}`；`effort` 要是现在的模型的档位，`session.configure {effort}`。回整份配置项（照换了的写，核心推来的会话状态随后再对一遍）；认不得的 `-32602`，核心拒了的 `-32603`。编辑器自己切的不再另报 `current_mode_update`。

### 给人看的字

模式、配置项、确认的选项的名字和回给编辑器的出错照握手以后的界面语言（`language/acp.rs`），中文、英文各一份。不给模型看的字一个都没加。

| 用处 | 中文 | 英文 |
|---|---|---|
| 模式 | `开放权限`、`只读` | `Full access`、`Read-only` |
| 配置项 | `权限`、`模型`、`思考强度`；`模型池` | `Permission`、`Model`、`Thinking effort`；`Model pools` |
| 确认 | `仅允许本次`、`在本会话内允许`、`拒绝`；`跳过` | `Allow once`、`Allow for this session`、`Deny`；`Skip` |
| 表单 | `回答下面的问题`、`其他` | `Answer the questions below`、`Other` |
| 一轮的开头 | `后台任务「<标题>」已完成`（`失败`、`已停止`、`已结束`）、`Miyu 自己开始了新的一轮` | `Background job "<title>" finished`（`failed`、`stopped`、`ended`）、`Miyu started a new turn on its own` |
| `/compact` | `压缩上下文`，提示 `摘要要保留的要点` | `Compact the context`, hint `what the summary should keep` |
| 出错 | `无法解析的消息`、`需要先发送 initialize`、`不支持的方法：<方法>`、`工作目录必须是绝对路径`、`会话不存在`、`上一条消息还没有回答完`、`图片或文件的内容无法解码`、`消息是空的`、`不支持的选项`、`与核心的连接已断开` | `unreadable message`、`initialize first`、`unsupported method: <method>`、`the working directory must be an absolute path`、`no such session`、`the previous prompt is still running`、`cannot decode an image or file`、`the prompt is empty`、`unsupported option`、`lost connection to the core` |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-cli/src/acp/view/tests.rs` | 每段字带它那一条的 `messageId`；不是人说话开的一轮先报一句开头、说是哪件后台任务、一轮只报一次，人说话开的、编辑器在等的不报；正文只报一次（第一次、接上的一截、整份里多出来的）；思考；编辑器自己的话不报、别处的报一次，重放都报、只记下的不报；藏起来的不报；等的那一句听到了换成等它那一轮、别的一轮结束不回；只记下的在跑的工具不记；改了文件的要对比、扔进回收站的不要 |
| `crates/miyu-cli/src/acp/status/tests.rs` | 待办、用量（两个数、准的金额）、标题、权限和模型变了、在等的交出去和了结了的忘掉 |
| `crates/miyu-cli/src/acp/tool/tests.rs` | 执行命令的内容先是命令原文、标题不变、写到一半的不报；`kind`、标题、写参数时 `pending`、等确认 `pending`、只写变了的、围栏、统一格式差异拆成对比 |
| `crates/miyu-cli/src/acp/prompt/tests.rs` | 内容块照先后、`file://` 解成路径、解不开的整句不收 |
| `crates/miyu-cli/src/acp/forms/tests.rs`、`options/tests.rs`、`setup/tests.rs` | 表单的格和读回答；能选的模型、配置项的先后、换的值查过；`~` 写成绝对路径、只收绝对的目录 |
| `crates/miyu-cli/tests/acp.rs`、`tests/acp/{turns,sessions,asking}.rs` | 真的核心、假模型，测试当编辑器：握手的能力、握手前的请求、新会话（目录、默认权限、斜杠命令）、相对目录、一轮的思考和正文、工具步骤（标题、`kind`、位置、对比）、打断、被拒、斜杠命令；载入重放（先后、往前翻页的长历史）、接上不重放、没有的会话、照目录列、切模式、别处切的模式报过来、关掉；确认（选项、拒绝回核心）、提问一道一道问、提问走表单 |

### 出处

- 设计 `73-ACP.md` 第五节（方向二：`miyu acp`）、第六节（决定）。
- ACP 规范 v1：initialization、session-setup、prompt-turn、content、tool-calls、session-modes、session-config-options、slash-commands、agent-plan、session-list、elicitation、error（2026-11-26 读的原文）。
- `cli/ask.md`：连核心、握手的写法；`view.md`：视图流；`sessions.md` 参考：会话的方法。

### 还没有的

- 编辑器给的 MCP 服务（`mcpServers`）：核心没有只对一个会话的接法（设计 73 第五节第 6 条）。
- 编辑器的文件和终端（`fs/*`、`terminal/*`）：Miyu 的工具读写磁盘，编辑器里没保存的改动看不到（第 8 条）。
- 子代理的会话（规范的 `subagent_update`）、子代理要的确认：只报主会话，子会话里在等的确认编辑器问不到，到时照 `permission.confirm.timeout` 被拒。
- 压缩（`compaction_update`）、旁白（`notice`）不报；`plan_update` 不用（只报 `plan`）。
- 重放时人附的图、文件不报；工具改文件的对比只在正在发生时要。
- `session/list` 不分页。
