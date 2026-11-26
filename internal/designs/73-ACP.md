## ACP：Miyu 接别的 agent 程序，编辑器接 Miyu

[Agent Client Protocol](https://agentclientprotocol.com)（ACP）是编辑器和 agent 程序之间的协议：JSON-RPC 2.0，agent 是编辑器拉起的
子进程，走标准输入输出。官方登记表里现在有 46 个 agent，Claude Code、Codex、Gemini CLI、Google Antigravity、OpenCode、Cursor、
GitHub Copilot、Kimi、Qwen Code 都在。这份设计做两个方向：

- **方向一（客户端）**：Miyu 当客户端，一个扩展包 `acp` 把登记表里的 agent 当成 Miyu 的模型驱动来用。
- **方向二（Agent）**：Miyu 当 agent，`miyu acp` 让 Zed、JetBrains 这类编辑器把 Miyu 当成一个 AI 助手来用。

状态：定稿（2026-11-26 core1 写；项目主人当天定：两个方向都做、现在就做；对方自带的工具做成可选配置；照官方登记表、用到时自动装；
编辑器里的会话照 Miyu 的默认权限、在编辑器里能切）。

### 一、为什么

- 项目主人 2026-11-25 发现 Antigravity 有官方的 ACP 服务端，「有了 ACP 之后就能接大部分 agent 程序」。现在每接一家借订阅的 agent
  CLI 就要单写一个驱动、解析一种私有的输出格式（`antigravity` 包解析 agy 的 stream-json），ACP 把这一层统一了。
- LiteMiyu 已经有 `litemiyu acp`（方向二），编辑器里能用；Miyu 没有。
- 2026-11-23、24 主人的群会话两次卡死，根因是 `antigravity` 驱动把一轮里几次请求的输入加起来报给核心，核心拿它当上下文大小（施工
  AG-12 是驱动侧的快修，代价是记账偏少）。旧版 0.7 一次调用记两份用量：记账用的加总、上下文用的最后一次请求（旧版
  `context_meter.rs` 只认 `last_request_usage`）。重制版的驱动协议只留了一份，这个区分丢了。ACP 的 agent 报的是 `usage_update`
  （上下文用了多少、窗口多大），本来就是第二份；不补这一格，方向一的上下文也会算错。

### 二、ACP 里和这件事有关的（2026-11-26 读的协议原文，加实测）

| 事 | 协议 | 实测 |
|---|---|---|
| 握手 | `initialize`：双方交换能力；`authenticate`：agent 列出登录方式 | Antigravity 1.5.2 列四种登录（谷歌账号、企业、Gemini API 密钥、Vertex）；Claude 适配器 0.112.0 一种都不列（用本机已登录的 Claude Code） |
| 对话归谁 | agent 自己管历史、自己压缩（`compaction_update`）；客户端交不进历史 | — |
| 指令 | `session/new` 的 `systemPrompt`：`append` 追加，`override` 整个替换，各要一项能力 | Claude 适配器声明 `override`；Antigravity 写在自己的 `_meta.antigravity.systemPrompt.append` 里 |
| 工具 | `session/new` 的 `mcpServers`：stdio 每个 agent 都必须支持，http、sse 可选 | 两家都声明 http、sse |
| 一轮 | `session/prompt`（内容块：文字、图片、嵌入的资源）；agent 发 `session/update`：`agent_message_chunk`、`agent_thought_chunk`、`tool_call`、`tool_call_update`、`plan`、`usage_update` 等；回 `stopReason` | 两家都收图片 |
| 用量 | `usage_update {used, size, cost?}`：这时上下文用了多少、窗口多大，花费可选 | — |
| 权限 | agent 要动手时向客户端发 `session/request_permission`，客户端回选了哪一项 | — |
| 文件、终端 | agent 可以请客户端读写文件、开终端（`fs/*`、`terminal/*`），客户端声明了才会用 | — |
| 会话 | `session/load` 重放历史、`session/resume` 不重放、`session/list` | 两家都有 |

### 三、核心要补的：驱动另报上下文大小（施工 ACP-1）

1. `driver.call` 的回答里，`usage` 多一格可选的 `context`：这一次调用结束时上下文一共多少 token（输入加这一次的输出）。不写的照旧。
   内置的三个驱动不写；`antigravity` 写最后一次请求的；`acp` 写 agent 最后一次 `usage_update` 的 `used`。
2. 内核的 `Usage` 多 `context: Option<u64>`，落进 `model.called` 的 `usage`（没有的不写，旧日志照读）。
3. `estimate::anchor` 的 `reported`：有 `context` 用它，没有照旧是 `uncached + cache_read + cache_write + output`。
4. 记账、金额照旧用 `uncached`、`cache_read`、`cache_write`、`output`，不看 `context`。`antigravity` 随后（施工 AG-13）把输入改回
   加总、另写 `context`，撤掉 AG-12 的记账偏少。

### 四、方向一：扩展包 `acp`（Miyu 当客户端）

仓库 `miyu-agent/miyu-acp`，包编号 `acp`，进官方软件源。做法照 `antigravity` 包：agent 只当模型，工具交回核心（设计 71 第一节），
进程、MCP 桥、会话对应这几块的代码照 `antigravity` 包搬，改成说 ACP。

1. **列哪些 agent**：照官方登记表（`https://cdn.agentclientprotocol.com/registry/v1/latest/registry.json`）。每个 agent 一个驱动，
   驱动名是登记表的编号，供应商里写 `driver = "acp/claude-acp"`。登记表缓存在包的数据目录，`driver.models` 时过期了（一天）才重取；
   取不到用缓存。登记表以外的，设置里能手写一条：名字、启动命令、参数、环境变量，驱动名 `acp/<名字>`。
2. **怎么装**：第一次用到时照登记表的 `distribution` 装进包的数据目录：
   - `binary`：下载这个系统、这种架构的压缩包，核对 `sha256`，解开，照 `cmd`、`args` 起；
   - `npx` / `uvx`：机器上有 `npx`（`uvx`）就照包名和版本起，包缓存放包的数据目录；没有就报「要先装 Node.js（uv）」；
   - 这个系统、这种架构没有的，`driver.models` 不列它，用到时报不支持。
   装的过程照核心的进度报给头（`package.progress`）；版本跟着登记表，换版本时旧的删掉。
3. **登录**：`initialize` 回的 `authMethods` 不空、`session/new` 报要登录（`auth_required`）时，这一次调用回 `auth_required` 一类
   的错，说话写清楚：这个 agent 要先登录，照登记表的网址、或者在终端里运行它自己的登录命令。不替主人走 OAuth，不碰别家程序的凭据。
   `gemini-api-key` 这类用密钥的，供应商里填了 `key` 的照它走 `authenticate`。
4. **会话对应**：Miyu 每次 `driver.call` 交整段对话，agent 自己记历史。照 `antigravity` 包 AG-2 的指纹前缀：这一次的消息指纹以
   某个 agent 会话记着的为前缀、别的指纹一样的，就是那段对话的下一步，只把新增的用户消息、工具结果交过去；对不上的（压缩过、
   改过、换过人格）新开一个 agent 会话，把历史写成一段文字放在第一次提问前面（`antigravity` 包的转录做法）。进程按 Miyu 会话复用，
   闲 10 分钟收掉；agent 支持 `session/resume` 的，下次接着用。
5. **指令**：Miyu 的 system 照 agent 的能力交：有 `override` 的整个替换（Claude 适配器），只有 `append` 的追加（Antigravity 写进
   它的 `_meta`），都没有的放在第一次提问的最前面。回合尾巴照 Miyu 的规矩放在这一轮用户消息后面。
6. **工具**：Miyu 的工具照 `antigravity` 包的做法，经包自己的 stdio MCP 服务交给 agent（`mcpServers`，名字 `miyu`）；agent 调它时
   交回核心执行，权限、沙盒、确认框和别的模型一样。参数格式照 agent 认的整形（Gemini 系照 `antigravity` 包的整形）。
7. **对方自带的工具**（项目主人 2026-11-26 定：做成可选配置）。包设置「对方自带的工具」两档：
   - **只用 Miyu 的工具**（出厂）：只读的搜索、抓网页类（ACP 的 `kind` 是 `search`、`fetch`、`think`，或者不带 `kind`、标题认得出
     是搜索的）放行，照设计 71 画成供应商侧工具；别的（`edit`、`delete`、`move`、`execute`、`other`）对方来要权限时回拒绝；对方
     不要权限就自己跑的，照样画成供应商侧工具，并在包的日志里记一条警告。能关掉的照 agent 的说法关（Claude 适配器的
     `disallowedTools` 一类，放在 `session/new` 的 `_meta` 里）。
   - **也放行对方的工具**：对方要权限时，包经核心的确认框问主人（施工 ACP-8 的 `driver.approve`：同工具调用的确认那一套，只能允许
     这一次 / 拒绝，选项照对方给的；项目主人 2026-11-26 看了 Zed 的做法以后定用 Miyu 自己的确认框，没人看着的会话一律拒绝），选了
     允许的让它自己跑，照供应商侧工具画；跑命令、改文件不在 Miyu 的沙盒里，设置页这一档的说明写清楚。
   不给 agent 开 `fs/*`、`terminal/*`（不声明这两项能力）：文件、命令都走 Miyu 的工具。
8. **一轮怎么交**：`agent_message_chunk` → 正文块；`agent_thought_chunk` → 思考块；`tool_call`/`tool_call_update` → 交回核心的工具
   调用（MCP 那边到了的）或供应商侧工具块；`plan` 不交（Miyu 有自己的待办）；`usage_update` 记下最后一次的 `used`、`size`，回答里
   写 `usage.context`，`size` 用来校正模型窗口；`cost` 有就照它记金额（币种不是美元的不记）；没有 token 计数的 agent，记账照 Miyu
   的估算（照设计 66 第十一节借订阅照 API 价算）。`stopReason`：`end_turn` 正常结束，`max_tokens` → `truncated`，`refusal` →
   `content_policy`，`cancelled` 是 Miyu 打断的。
9. **模型和窗口**：`driver.models` 列每个装得上的 agent 一项，模型名是 agent 报的（`session/new` 回的模型或配置项）；报不出来的
   只列一个 `default`。窗口照 `usage_update` 的 `size` 记下，第一次之前照登记表没有、照 agent 的惯例写一个保守值。

### 五、方向二：`miyu acp`（Miyu 当 agent）

核心命令行多一个子命令 `miyu acp`：编辑器拉起它，它连本机的核心（照 `miyu ask` 一样连、没开就起），把 ACP 翻成核心的会话协议。
不另起模型、不另存会话：编辑器里的会话就是 Miyu 的会话，终端、网页里也看得到、接得上。

1. **握手**：`initialize` 回能力：`loadSession`、`promptCapabilities`（图片、嵌入的资源）、`mcpCapabilities`（stdio、http，第 6 条）、
   `sessionCapabilities`（`list`、`resume`、`close`）、`systemPrompt` 不声明（人格照 Miyu 的）。`authMethods` 空：连的是本机核心，
   用本机令牌，不要编辑器登录。
2. **会话**：`session/new {cwd}` → `session.create`，工作目录是 `cwd`，权限照 Miyu 的默认（项目主人 2026-11-26 定）；`session/load` →
   打开那个会话，把视图流里的历史照第 3 条重放成 `session/update`；`session/resume` 只接上不重放；`session/list` 列 Miyu 的会话
   （照 `cwd` 过滤）。
3. **一轮**：`session/prompt` → `session.say`（图片、嵌入的资源照附件交）；视图流照下面翻成 `session/update`，这一轮结束回
   `stopReason`：
   - 正文 → `agent_message_chunk`；思考 → `agent_thought_chunk`；
   - 工具步骤 → `tool_call`（`title` 用工具卡片给人看的标题，`kind` 照工具：`read`、`edit`、`delete`、`move`、`search`、`execute`、
     `think`、`fetch`、`other`；改文件的带 `locations` 和 diff 内容），结果 → `tool_call_update`；
   - 待办（`todowrite`）→ `plan`；上下文用量 → `usage_update`（`used`、`size`，金额照 Miyu 算的、币种 `USD`）；
   - 回合正常结束 `end_turn`，到了轮数上限 `max_turn_requests`，被拒 `refusal`，打断 `cancelled`；出错照 JSON-RPC 的错误回。
4. **确认**：核心要问人时（`tool.approval_requested`、`question.asked`），翻成 `session/request_permission`：选项照核心给的（允许
   这一次、总是允许、拒绝），编辑器选了哪一项照着回核心；提问（`ask_user`）的选项也走这一路，补充的字走 `elicitation`（编辑器声明了
   才用，没声明的只给选项）。
5. **模式**：`session/set_mode` 对应 Miyu 的权限：只读、开放（照 Miyu 已有的两档）；`current_mode_update` 报切换。模型、思考档位
   照 ACP 的会话配置项（`config_option_update`）给编辑器选。
6. **编辑器给的 MCP 服务**：`session/new` 的 `mcpServers` 加进这个会话（核心已有的 MCP 接入，只对这个会话生效、会话结束拆掉）；
   核心没有「只对一个会话」的接法时，第一版不收、`mcpCapabilities` 不声明，记在第八节。
7. **斜杠命令**：`available_commands_update` 列 Miyu 的命令（照命令表），编辑器里打 `/compact` 这类照 Miyu 的命令跑。
8. **编辑器的文件和终端**：第一版不用（Miyu 的工具读写磁盘）；编辑器里没保存的改动看不到，写进第八节。

### 六、决定

| 已定 | 没选 | 重开条件 |
|---|---|---|
| 两个方向都做（项目主人 2026-11-26） | 只做一个 | 无 |
| agent 只当模型，工具交回核心 | 让 agent 自己跑一切、Miyu 只转话 | 项目主人要 |
| 对方自带的工具可选，出厂只用 Miyu 的（项目主人 2026-11-26「可选配置」） | 一律关、一律开 | 无 |
| 照官方登记表、用到时自动装、核校验和（项目主人 2026-11-26） | 只认手写命令 | 登记表停更 |
| 不替主人走 OAuth，不碰别家的凭据 | 包里做登录 | 无 |
| 驱动另报上下文大小，记账照旧 | 只报一份 | 无 |
| `miyu acp` 是核心的子命令，编辑器里的会话就是 Miyu 的会话 | 另起一个独立的 agent 程序 | 无 |
| 编辑器里的会话照 Miyu 的默认权限、能切（项目主人 2026-11-26） | 一律只读起步 | 无 |

### 七、施工

| 步 | 做什么 |
|---|---|
| ACP-1 | 核心：驱动回答的 `usage.context`，内核 `Usage.context`、`model.called` 落盘，`estimate::anchor` 优先用它；协议参考、蓝图、扩展开发文档 |
| AG-13 | `antigravity` 包：输入改回加总，另报最后一次请求的上下文大小（撤掉 AG-12 的记账偏少）；0.1.7 |
| ACP-2 | `acp` 包骨架：仓库、清单、登记表的取和缓存、装（binary、npx、uvx，核 sha256）、`driver.models` |
| ACP-3 | `acp` 包：进程、握手、登录的报错、会话对应（指纹前缀、转录）、一轮的增量、`usage.context`、`stopReason` |
| ACP-4 | `acp` 包：MCP 桥（Miyu 的工具）、对方自带的工具两档、权限请求 |
| ACP-8 | 核心：扩展驱动在一次调用进行中请核心问人（`driver.approve`），同工具调用的确认那一套，头不用改；`acp` 包改用它（ACP-4 补） |
| ACP-5 | `acp` 包：真 agent 实测（Claude 适配器、Antigravity、Gemini CLI），README、发版 |
| ACP-6 | 核心 `miyu acp`：握手、会话（new、load、resume、list）、一轮的翻译、确认、模式、斜杠命令 |
| ACP-7 | `miyu acp` 用真编辑器（Zed）实测；用户手册（中英） |
| ACP-9 | `miyu acp`：能选的模型每次接上会话、列表变了时重问；核心断了不退出、再连上 |
| ACP-11 | `miyu acp`：重放时叫醒的那一轮的开头照先后报；没说明的出错照界面语言说；编辑器拉起的核心用编辑器的环境，写进手册 |

### 八、做下来和这里不一样的、没做的

施工 ACP-6（方向二，2026-11-26）：

- 第五节第 6 条：核心的 MCP 服务一个核心一份、所有会话共用，没有只对一个会话的接法。`miyu acp` 不收编辑器给的 `mcpServers`、不声明
  `mcpCapabilities`（规范说 stdio 的每个 agent 都必须支持，这一条现在没守）。
- 第五节第 3 条的金额：只有一种币的照原样写那种币，几种币的照换算成 `usage.currency` 的那一个；有算不出钱的请求、有换算不了的不报
  金额。
- 第五节第 5 条：别的头切的权限、模型报 `current_mode_update`、`config_option_update`；编辑器自己切的不另报。
- 第五节第 2 条：`session/load`、`resume` 不换会话的工作目录，编辑器给的 `cwd` 只用来补全工具参数里的相对路径。
- 第五节第 4 条：编辑器不能弹表单时，提问一道一道走 `session/request_permission`，没有选项的题、多选题多选的那几项收不到。
- 第五节第 7 条：头自己加了 `/compact`（核心的命令表里没有它，同终端界面）。
- 没做的：子代理的会话（`subagent_update`）和子会话里的确认、压缩（`compaction_update`）、旁白（`notice`）、`plan_update`、
  `session/list` 分页；重放时人附的图、文件不报，改文件的对比只在正在发生时要。

施工 ACP-9（方向二，Zed 实测撞到的，2026-11-26）：

- 第五节第 5 条：能选的模型以前一个进程只问一次 `model.list`。新装的 `acp` 包带的供应商不要密钥，核心照常列，但头一次问的时候它的
  列表还在后台拉（核心先照手头的答空的，拉到了推 `models.changed`），`miyu acp` 不订这个推送，之后一直列不出。改成每次接上会话
  重问，订配置的推送、列表或模型这一块的配置变了也重问，变了报 `config_option_update`。终端界面、网页本来就照 `models.changed`
  重问，不受影响。
- 核心断了以前 `miyu acp` 直接退出。改成不退出：正在进行的那一轮回出错，当场照头自己重连的连法试一次（人停的不拉），没连上的
  等编辑器下一次要用时照 `miyu ask` 的连法连（核心没开就拉起），连上了重新订阅开着的会话。

施工 ACP-11（方向二，Zed 实测撞到的，2026-11-26）：

- 第五节第 2 条：重放时不是人说话开的那一轮（后台任务报回来叫醒的），开头那一句以前排在 `session/load` 的回应以后，编辑器里整段对话
  最后多一句开头、那一轮的回答并进了前一条助手消息。改成同实时一样照先后报。
- 第五节第 3 条：出错收尾、核心没给说明也没有错误原话的（有计划的重启打断的 `restarted`、崩了补的 `aborted`），以前回的是原因的原文，
  Zed 里只见 `restarted`。改成照怎么结束的说、照界面语言。
- 核心没开时由 `miyu acp` 拉起，同所有的头一样，核心继承拉起它的进程的环境，也就是编辑器的：从程序坞打开的编辑器 `PATH` 短，没有
  shell 配置文件里的变量，之后终端界面、网页连的也是这个核心。这一次只写进手册（建议先在终端里 `miyu core start`）。装了用户服务的
  走服务管理器拉不解决 `PATH`（systemd、launchd 给的也不是 shell 的环境），核心起来时读登录 shell 的环境才解决，两样都是改所有头
  拉起核心的规矩。项目主人 2026-11-26 定：做「核心起来时读一次登录 shell 的环境、只补 `PATH`」，单独一项设计，放在首个
  正式版以后；经服务管理器拉起不做。

施工 ACP-2 到 ACP-4（方向一，2026-11-26）：

- 第四节第 1 条驱动名：清单的 `[[driver]]` 只能写死，做不到每个 agent 一个 `acp/<编号>`。做成一个驱动 `acp/acp`、一家自带的
  供应商 `acp`，agent 写在模型名里（`acp/claude-acp`、`acp/claude-acp/sonnet`）；代码也认 `acp/<agent>` 的驱动名，以后改成
  每个 agent 一段 `[[driver]]` 接得上。
- 第四节第 1 条手写的 agent：核心不许多行的文字项默认为空，参数、环境变量不单列，写成一行启动命令（`名字=值 … 程序 参数…`，
  认引号，不经 shell）。
- 第四节第 2 条装的进度：`package.progress` 是核心推给发起安装的那条连接的，扩展推不了；下载进度只写进包的日志。
- 第四节第 7 条：Antigravity 只留 `search_web`，`read_url_content` 照 `antigravity` 包的先例不开。
- 第四节第 8 条用量：agent 在 `session/prompt` 的回应里带了 token 数（Claude 适配器带）的照它记四项加 `context`；没带的整格不报
  （内核的 `Usage` 四项必填，`context` 不能单报），这时上下文照 Miyu 的估算（agent 自己会压缩，影响小）。
- 设计没写到、顺手定的：会话共用包的状态目录下一个空目录当工作目录；窗口没学到之前 Claude 适配器写 200k、别的 128k；清单里的
  命令叫 `acp-agents`（`acp` 会被核心的 `miyu acp` 盖掉）。
- 没做的：进程收掉以后用 `session/resume` 接着同一个会话（现在照新开、重放）；`effort` 对应 agent 的思考档位；一步里并行的几件
  Miyu 工具（第二件拒，照 `antigravity` 包）；工具结果里的图交回 agent；机器上没有 npx、uvx 时照样列出、用到才报。

施工 ACP-8、ACP-4 补（2026-11-26）：

- 第四节第 7 条「也放行对方的工具」：ACP-4 先做成交回核心一次 `ask_user` 问主人（扩展发不起确认框，时间线上多一次「AI 提问」）；
  项目主人看了 Zed 的做法以后定用 Miyu 自己的确认框。核心加了 `driver.approve`（施工 ACP-8）：确认画在那一块供应商侧工具上，
  `by` 是包，`detail` 照跑命令（有命令的）或者包问人（别的）的样子写，头照已有的样子画、不用改；只能允许这一次或者拒绝，对方的
  「总是允许」不映射成「本会话内允许」（两个头都只认允许这一次、拒绝，核心发的确认也都不带规则），允许时照对方的「允许这一次」
  回、没有的才照「总是允许」。没人能确认的会话（场所会话、不是能输入的头开的会话）、只读时要写要跑命令的当场拒；问着时这一次
  调用不计时限。`acp` 包去掉了借 `ask_user` 的做法；旧核心回「没有这个方法」的照「只用 Miyu 的工具」那一档拒绝，日志里写一句。
- 没做的：崩了以后载入时，在等的那一问不补决定（不对着调用），视图里那一步留着没定的请求；收紧成只读时不回头拦下已经在问的。

施工 ACP-10（方向一，真人实测撞到的三处，2026-11-26）：

- 第四节第 7 条「也放行对方的工具」：对方不来要权限就自己跑了 Bash。根因是 Claude 适配器 0.112.0 起会话的权限模式不由客户端定：
  有 `permissions.defaultMode` 的照主人本机给 Claude Code 的设置（主人设的是 `bypassPermissions`），没有的起在 `auto`（它自己判
  要不要问），两种都不发 `session/request_permission`。改成开会话以后照 ACP 的会话模式切到「每次都问」的那一档：`session/new` 报了
  `modes` 的发 `session/set_mode`，只有配置项的发 `session/set_config_option`；哪一档照编号认，`default`（Claude 适配器、Gemini
  CLI、Antigravity）、`manual`、`ask`、`read-only`（Codex 适配器没有每次都问的一档，这一档最紧），都对不上的不切、日志里记一句。
  两档都切（只用 Miyu 的工具时主人本机的 MCP 服务这类也不该不问就跑）。Claude 适配器另在 `_meta.claudeCode.options` 写
  `allowDangerouslySkipPermissions: false`，主人设的 `bypassPermissions` 当场退回 `default`、模式表里也不列。仍然不问就跑了会动
  本机的工具（`read`、搜索、抓网页、思考以外的）的，供应商侧工具块的结果前面照界面语言写「未经确认已执行」（协议里结果模型和人都看，
  没有只给人的一格）。主人本机设置里的允许规则（`permissions.allow`）还会放行，包不改主人的设置，靠这一句标出来。
- 第四节第 4 条转录：登录失败那一轮没有回答，核心把「这一轮出错了」那一句（`<turn-ended reason="error">`）和下一句人话攒在同一条
  user 消息里。包以前把整条当这一轮交，agent 两句都答、还说自己是被打断后重试。改成照那一句分开：它和前面的人话归进历史（`User:`
  那一段以那一句结尾），后面的才是这一轮；字一个不加。
- 第四节第 9 条窗口、模型表：Claude 适配器的模型表里简称（`opus`、`haiku`）跟着当代走，全名（`claude-haiku-4-5-20251001`）钉在
  那一版，不是同一个模型；`haiku` 记的 1M 是 Haiku 5.5 的真窗口，全名那一行的 200k 是还没用过、照保守值写的，Haiku 4.5 本来就是
  200k。改成 `usage_update` 的 `_meta["_claude/model"]` 报了真用的模型时，窗口也记在那个全名上、记下「简称 → 全名」，列表里简称
  照它解到的全名取窗口。驱动的模型表协议（`driver.models`）没有名字这一格，包交不出 agent 给的显示名（「Haiku 5.5」），简称在
  编辑器里照编号显示、全名照核心的目录显示名字，看着像两套：项目主人 2026-11-26 定照旧两套都列，不加显示名、不删旧版本。主人在 Claude Code 设置里写的允许规则照它放行、不改（同日定「不用管」），对方没问就跑了的照样标「未经确认已执行」。
