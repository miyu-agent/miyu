## 视图投影

状态：定稿，2026-10-09 主会话起草，同一天和终端界面、网页的会话对过形状（施工 9-8 上起）。设计 `04-核心协议.md` 第五节 P3、`01-架构.md` D1：核心决定「显示什么」，头决定「怎么画」。

### 是什么

把会话的事件（落了盘的和瞬时的）算成头直接画的一条条**条目**：种类、状态、属于哪一轮、标题那一句、收起那一行的字。原来每个头自己从事件算（终端约 3300 行：整理正文和时间线 2650、时间线的字 700），两个头各算一遍、字还得对齐；搬进核心以后只有一份。

不搬的：怎么画（布局、颜色、图标、动效、预览几行、滚动、同一时刻只转一处），人亲手点开收起，底栏、运行状态行的词库、侧边栏，预览工作区放哪些文件，待办（另有流）。

分步（主会话定）：9-8 上 投影本身；9-8 中 `view.page` 交条目、握手、`view.detail` 多交的；9-8 下 视图流；9-8 补 会话状态 `view.status`（补上本会话的，补下整棵会话树，2026-10-10 和两个头对过）。原来排的 9-8 再补「Markdown 由核心解析」拿掉了：核心只交 Markdown 原文，解析和渲染都归头（2026-10-10 项目主人定：核心保持输出文字）；两个头照一页共用的约定解析，理解才一致。

### 在哪

| 文件 | 干什么 |
|---|---|
| `crates/miyu-view/src/projector.rs`、`projector/`（新） | 投影这台小机器：喂一条事件交出它引起的变化（`Change`）；`users.rs` 消息和排着的话，`turns.rs` 开轮、收尾、每次请求，`blocks.rs` 回复的一块块和起止，`tools.rs` 调工具的一步、确认、提问，`notices.rs` 旁白，`undo.rs` 撤销恢复 |
| `crates/miyu-view/src/entry.rs`、`notice.rs`、`change.rs`、`id.rs` | 条目、旁白、变化、编号的格；都能读回来（施工 V-D） |
| `crates/miyu-view/src/entry/job.rs` | 任务了结那一条旁白的几格（施工 V-D 从 `entry.rs` 挪出来）；监视的命令出的一批行那一条（`WatchedLines`，施工 MN-1 下） |
| `crates/miyu-view/src/relay.rs` | 读回来时 `By`、`RawJson` 那几格经通用的 JSON 转一道（施工 V-D） |
| `crates/miyu-view/src/projector/nested.rs`、`entry/nested.rs`、`nested.rs` | 脚本里调的工具（施工 M-5 上）：`tool.nested_started`、`tool.nested` 照编号认父级，那一步、那一行任务、任务了结的旁白带 `nested`；订阅时补上在跑的（`Projector::nested_running`）；点开时的整份（`listed`） |
| `crates/miyu-view/tests/nested.rs`（施工 M-5 上） | 前台 `shell` 下面调三次（成、拒、出错）个数和最近一个跟着变，在跑的带开始时刻；没调过的没有；后台命令那一行、了结的旁白带上；订阅时补上在跑的 |
| `crates/miyu-view/tests/shell.rs`（施工 SH-1 下） | 人用 `!` 打的命令：那条 `user` 带 `shell`，跑完了是退出码、输出、截掉几个字、用时；在跑的是 `{"running": true}`，跑完换成结果、异常结束的带 `reason`；别的话没有这一格；翻页和接着推的一样 |
| `crates/miyu-view/src/summary.rs`、`title.rs`、`estimate.rs`、`explain.rs`、`words.rs` | 收起那一行、标题那一句、照参数估的行数、出错说明；给人看的字的接口（`Words`、`Texts`）和工具分类（`Kinds`） |
| `crates/miyu-view/tests/` | 照内核的替身跑真会话，每个测试都对照视图流和翻页最后一样（`support/mod.rs` 的 `same`）；`random.rs` 三百份随机剧本；`notices.rs` 另有切在派任务之后的一页（9-8 中）、回报的退出码和用时（9-8 三补）；`lines.rs` 另有改了文件的一步带 `files`（9-8 三补）、工具的名字照记下的、照这一刻工具目录里的、照工具名（9-8 六补）；`outputs.rs` 一步带结果原文的前面一段，切在 100 行、8 KiB、字的边界上（9-8 十四补） |
| `crates/miyu-endpoint/tests/view_entries.rs`、`view_page.rs`、`view_detail.rs` | 真核心走一遍（9-8 中）：握手报版本；交条目的页、边界照旧、字照连接的语言；更早派出的任务照样有标题；调用的结果原文、压缩的摘要 |
| `crates/miyu-view/tests/status.rs`（9-8 补上） | 闲着、在跑、在等人（确认、提问到了结）；正在写、在调工具；重试和都在冷却出现又消失；上下文用了多少；任务从派出到了结、停了的为什么 |
| `crates/miyu-endpoint/src/diffs/tests.rs`（施工 V-18） | `view.detail` 的差异和条目 `diff` 的真数共用的两边：效果里写着、哈希对得上的原文先用，一个 blob 都没有也算得出；新建的改前当空的；原文和哈希对不上的当没写、照哈希取 blob；以前的日志没写原文的照旧取 blob；不是 UTF-8、太大、取不出来各说原因 |
| `crates/miyu-endpoint/tests/view_tree.rs`（9-8 补下） | 派出去的子代理在跑一轮：那一项带 `busy`、`spawned`、`running_deep`、`usage`，整份带 `running_deep`、`usage_tree`；子代理派了孙代理也数进来；连子代理一起的用量（等「子代理那一次算进树里」，不等树上恰好是哪个数：中间数可能并进一次推送，施工 BUG-4） |
| `crates/miyu-endpoint/tests/view_status.rs`（9-8 补上） | 订阅回应带整份状态；跑一轮推 `running` 再推 `idle`、用量跟着变、没变的不推；切权限也推 |
| `crates/miyu-endpoint/tests/view_stream.rs`、`src/subscriptions/view/tests.rs` | 视图流（9-8 下）：回应排在推送前面、推送拼到那一页上和翻页一样、`turn.started` 另推；改了语言之后的照新的字；带 `after` 的拒、退订了不推；接着的 `view.append` 并成一条 |
| `resources/core/view.json` | 工具算哪一类（命令、编辑、子代理、留言）、参数里哪一格是会话编号：数据，不登记 |
| `crates/miyu-endpoint/src/view/` | `view.page` 多交条目、`view.detail` 多交的（9-8 中，`project.rs` 接上投影要的字和改了多少行）；视图流订阅时的最新一页（9-8 下，`newest`）；一页只读要的那一截（施工 V-41 上，`read.rs`：先数行、不解，再只解最后几轮和切点前要的那几行） |
| `crates/miyu-endpoint/src/view/read/tests/`（施工 V-41 上） | 以前的读法（整份读、整份切、整份学）照抄在 `old.rs`；`script.rs` 照种子造会话日志（撤销、恢复、重做、撤回、压缩、撤销跨压缩、派后台命令和子代理、留言、脚本里调的工具、回报、分叉、子会话、在跑一轮当中、内核写不出来但读得成的几种样子），写成几段真文件（空段、末尾半行）；`same.rs` 三千份（平时每份三个，`MIYU_PAGES_FULL=1` 全跑，CI 的 randomized 那一项设，施工 CI-2）新旧逐字比：几种轮数、字节上限从最新一页翻到头，日志里每一处当 `before`，交事件的、交条目的；视图流订阅到各处的最新一页连同会话状态，后面的事件接着喂两台投影每一条一样；一行坏了的，读到的报坏了、没读到的和没坏的一样（平时 40 份，全量 160 份） |
| `crates/miyu-endpoint/src/subscriptions/view.rs`、`connection/streams.rs` 的 `subscribe_view` | 视图流（9-8 下）：订阅、转发、合并 `view.append`、换字；压着的工具进度到点推出来（施工 X-45 补） |
| `crates/miyu-view/src/projector/progress.rs` | 在跑的工具的进度（施工 X-45 补）：攒半行、认进度行、0.5 秒合并 |
| `crates/miyu-view/tests/progress.rs`、`src/projector/progress/tests.rs`、`crates/miyu-endpoint/tests/view_progress.rs` | 工具进度（施工 X-45 补）：切半行、只认前缀、取最后一条、截长、半行不涨；视图流带上、结果到了去掉、翻页没有；0.5 秒里一条、压着的到点推、两个调用各算；真订阅里真 `shell` 印的进度到得了、结果以后不带 |
| `crates/miyu-endpoint/src/view/whole.rs`（施工 SE-1 中下上） | 整个会话喂投影：导出要整份，`view.page` 一页最多 50 轮；照 `read_range` 一块块读、喂完就放掉，不整份读成事件，喂的是同一台投影、同一套字，条目和从头翻到尾的 `view.page`（`view: true`）一样；每一条另交给调的一方看一眼（导出照它记条目里没有的原文）。`whole/tests.rs` 照翻页对照测试的造法造四十份会话，和整份读再喂的条目、交出的事件逐条比 |
| `crates/miyu-view/src/markdown.rs`、`markdown/`（施工 SE-1 中下上、中下中、中下下，SH-2） | 会话导出成 Markdown 的渲染器：一个不连线的头，纯函数，只吃条目、开关（`options.rs`：三个预设 `read`、`agent`、`chat`，写了的开关盖过预设）、给人看的字（`core/export/…`、思考用时照 `view/summary/thought-for`）、时区、配了的密钥、附在后面的子代理（`Child`）、附件文件夹（`Files`）；`render` 是 `draft`（一个会话一份草稿，交回写进去的那几步派的子代理）加 `finish`（接起来的几份一起换密钥、家目录、token 估算）；`sources.rs` 照事件记条目里没有的原文（切过的工具结果全文、压缩的摘要、回复的 `seen` 和序号、脚本里调的工具、`range: context` 要的有效历史、`job.started` 记的子代理，`tools: full` 要的 blob 原文由读日志的一方放进来），`select.rs` 照范围挑条目，`head.rs` 三种开头和附在后面的子会话的开头（`agent` 那一段英文是常量 `AGENT_NOTE`），`steps.rs` 思考和 `read` 的工具，`shell.rs` 人用 `!` 跑的命令（`console` 代码块里的 `$ <命令>`、怎么结束的、用时、输出照工具的写法），`raw.rs` `agent`、`chat` 的工具，`line.rs` 一行的工具，`extras.rs` 要人确认的那一次、脚本里调的工具、提问，`notices.rs` 旁白和收尾，`text.rs` 代码块、折叠块、列表、截头尾的写法，`scrub.rs` 换密钥、家目录写成 `~`，`agent.rs` `agent` 预设的旁白、占位（固定英文的常量），`files.rs` `assets: files` 起文件名、写相对链接（`files/tests.rs` 守名字和链接的写法）。怎么写见下面「导出成 Markdown」 |
| `crates/miyu-view/tests/markdown.rs`（施工 SE-1 中下上） | `read` 预设一整轮逐字对；撤掉的、撤回的、撤销说明不进（另照真会话撤一轮）；真会话压缩一次，旁白照时间线、摘要折在下面；长输出折起来、切过的照日志取全文；正文里没合上的代码块、反引号、mermaid、思考里的 `</details>`；子代理的交代和回报；附件占位；家目录只换整段路径；密钥精确匹配、同一处取长的、太短的不换 |
| `crates/miyu-view/tests/markdown_presets.rs`、`markdown_ranges.rs`、`markdown_extras.rs`（施工 SE-1 中下中） | `agent`、`chat` 各一整段逐字对，结果截头尾、`full` 不截，开关盖过预设；`context` 照真会话从检查点起、压缩撤掉了回到更早、`{"turns": N}`；要人确认的那一次、提问、脚本里调的工具（真会话）三种写法，`full` 取 blob |
| `crates/miyu-view/tests/markdown_attached.rs`（施工 SE-1 中下下） | `agent` 连接是中文时旁白、占位整段英文逐字对，`read` 照旧跟语言；`assets: files` 人附的、结果里的、她发的图写成链接，取不到的照占位、记下；`children: append` 的子会话照任务编号、撤掉的那一步派的不带，小标题和开头列表三种写法，密钥、token 估算照整份 |
| `crates/miyu-view/tests/markdown_attached.rs` 的 `a_path_only_attachment_is_written_as_its_path_and_never_copied`（施工 AT-1） | 只有路径的附件：条目没有 `blob`、`media_type` 照样读写；占位写路径；`assets: files` 不拷、不算取不到 |
| `resources/core/human/<语言>.json` | `said` 的 `view/…`：收起那一行的字（照网页演示 `timeline.summary` 搬来，两个头一字不差）、准备中的显示名、「会话 短编号」「父会话」、出错说明（照终端的 `error_classes`、`status_hints`） |

### 对外的样子

**握手**（9-8 中）：`hello` 的回应多 `view: 1`（视图投影的版本）。没有这一格的是旧核心，头照旧自己算。

**头读回来**（施工 V-D，终端换成照视图流画要的）：条目、旁白、推送（`Change`）、会话状态（`Status`）、`view.detail` 的嵌套调用（`NestedItem`）都能照写出去的 JSON 读回来（`Deserialize`），经门面 `miyu_client::view`（feature `view`）给头。
1. 可以没有的格缺着照没有（空的列表、`false`、0 也是）；不认识的格不管：核心加格子，旧的头照样读得懂。
2. 只认文字的几格原来写成 `&'static str`（附件的 `kind`、换模型的 `why`、任务的 `why`），改成 `String`，写出去的一个字节不变。
3. 内核里「先整块读原文、再照 `kind` 分派」的 `By`、`RawJson`，在照 `kind` 分派、`flatten` 的条目里读不成（serde 先把内容攒成中间的样子，它们只认原文）：那几格先读成 `serde_json::Value` 再交给它们（`relay.rs`）。只在读的时候用。
4. 推送的 `view.update` 的 `after`：没有是位置没变，`null` 是挪到最前（施工 V-D 修，终端报的：serde 读两层的 `Option` 把 `null` 也当没有，那一格经 `relay::present` 读）。线上的推送是 `view.<op>` 加参数、不带 `op`，`Change::read(method, params)` 照它读回来（带的 `session` 不管）。
5. 测试：`tests/support/mod.rs` 的 `same` 把每个测试、三百份随机剧本里投影交出的每一条条目、每一个推送写成 JSON 再读回来、照线上的写法写一条推送再用 `Change::read` 读回来（`on_the_wire`），都和原来的一样；会话状态同样（`tests/status.rs`）；挪到最前的那一种另有一条（`tests/web.rs`）。

**条目**：一条一个 JSON 对象，共有的格：

| 格 | 写法 | 说明 |
|---|---|---|
| `id` | 字符串 | 稳定编号：跨页、跨重连、核心重启都不变（下面「编号」） |
| `kind` | 下面几种之一 | |
| `turn` | 回合编号 | 属于哪一轮；不属于哪一轮的不写 |
| `hidden` | `true` | 撤销藏起来的；显示着的不写 |
| `at` | 时刻，同事件的 `at` | 开始的时刻：流式来的照第一段增量，翻页算的照引出它的那条事件 |

**编号**：照日志里的位置算，谁算都一样。

| 条目 | 编号 |
|---|---|
| 一条消息（`message.user` 和别处来的话） | `m<序号>` |
| 模型回复里的一块（正文、思考、工具调用） | `b<seen>.<块号>`：`seen` 是那次请求的，块号同 `model.delta` 的 `index`；流式时就有，落了盘照 `message.assistant` 的 `seen` 和 `indexes` 算出同一个 |
| 时间线的一段 | `g` 加它第一步的编号去掉 `b`，例如 `g41.0` |
| 压缩那一条 | `c<序号>`：替代到的那一条（`compaction.progress` 的 `seen`、摘要请求的 `seen`、`context.compacted` 的 `upto` 是同一个），流式的进度、摘要请求的记录、落了盘的检查点改的是同一条 |
| 别的（一轮的收尾、旁白） | `e<序号>`：引出它的那一条事件 |
| 她发出去的图（`media`） | `s<序号>.<第几张>`：序号是那条 `tool.result` 的（施工 X-19） |
| 只在视图流里有的旁白（瞬时事件引出的：出错换了模型） | `x<序号>.<第几个>`：排在序号是它的那一条后面的第几个瞬时旁白。不进日志，翻页、重新订阅都没有它 |

**`message.assistant` 多一格 `indexes`**：每一块在流里是第几块（`model.delta` 的 `index`），照 `blocks` 的先后。空块、打断时丢掉的半截调用不进回复（`kernel/session.md`「收回复」），后面的块的位置和块号就错开了；只有错开时才写，和位置一样的不写，以前的日志没有这一格、照位置算。

**种类**：

| `kind` | 格 |
|---|---|
| `user` | `text`（命令模板展开成的是人敲的那一行，施工 X-14 上，`slash.md`）；`attachments`（照 `message.user` 的附件块：`kind`（`image`、`file`）、`name`、`media_type`、`blob`，图有 `width`、`height`，带了路径的有 `path`；大小头照 `blob` 取。只有路径的附件（施工 AT-1，`path` 块）照样出一件：`kind` 是 `file`，没有 `media_type`、`blob`，有 `name`、`path`，另带 `path_only: true`，头写了大小的有 `size`；头照 `path_only` 认，不去 `blob.get`）；`from`（别处来的话的来处：别的会话、子代理、父会话、别的 harness、场所里的人，和 `by` 一个写法）；`level`（发出去那一刻的权限级别）；`queued: true`（排着、她还没听到，听到了换成不写）；`withdrawn: true`（排着的被退回了：`message.withdrawn`，打断时没听到的那几条；不画，视图流里推到的那一刻头把字放回输入框，翻页、重新订阅看到的不放回）；`round`（长期目标的续轮是第几轮，施工 X-8 补：`from` 是模块 `goal` 的，照那条消息的 `cause` 读，`protocol.md` 的 `command.run` 第 10 条；网页画成一行「目标 · 第 N 轮」，不把续轮的原文给人看，2026-10-12 项目主人定）；`shell`（施工 SH-1 下，设计 57 第四节第 4 条：人用 `!` 打的命令，`text` 是那一行；还没跑完的是 `{"running": true}`，排着的也是，头照 `queued` 分；收到 `shell.ran` 换成 `exit` 或 `signal`、`text`、`omitted`、`duration_ms`，异常结束的另有 `reason`；对不上一条画着的话的 `shell.ran` 不理；别的话没有这一格） |
| `reply` | `text`（Markdown 原文）；`open: true`（还在写，写完不写）；`persona`（施工 9-8 十三补，网页要的）：这一轮是哪个人格回的，无人格的是 `null`。投影照 `session.created` 的人格起头，带 `persona` 的 `session.policy_changed` 先记着，下一条带 `policy` 的（快照真换上了）起算新的：同内核、会话那一层（`kernel/session.md`「换人格、预设」） |
| `media` | 她发出去的一张图（施工 X-19，`tool.result.sent`）：`image`（同 `user` 的附件那一格，宽高一定有）、`alt`?、`sticker`?。一张一条，排在那一步工具后面，和 `reply` 一样把前面那段 `group` 收起（那一步照旧在 `group` 里） |
| `group` | 时间线的一段：`steps`（几步的编号，照先后；多了一步，这一段跟着 `view.update`，`steps` 和收起那一行一起变）；`open: true`（还在进行：她没开口、这一轮没结束）；`summary`、`summary_en`（收起那一行，下面「收起那一行」）；`took_ms`（从第一步开始到最后一步结束，刨掉中间等人的，见「会话状态」的 `waited_ms`） |
| `thought` | 一步思考：`group`；`text`；`open: true`（还在想）；`took_ms` |
| `tool` | 一步工具：`group`；`call`（调用编号，流式时还没有）；`name`；`title`（下面「标题那一句」）；`state`；`args`（参数原文，流式时边收边接）；`diff`（编辑、写入的 `{added, removed}`，有了结果以后是真数，之前照参数估、带 `estimated: true`）；`images`（结果里的图：`[{blob, media_type, width, height}]`，照先后，头照 `blob.get` 取；`read` 读了图片的那一步）；`job`（派子代理、后台命令的任务编号）；`to_title`（留言发给子代理的，那个子代理的标题：派它那一步的 `description`）；`approval`（要人确认的：`access`、`rule`、`detail` 照 `tool.approval_requested` 原样，定了以后多 `decision`、`reason`、`by`；确认记在这一步上，不另起旁白。脚本里调的要确认的（施工 M-7 上）记在父 `shell` 那一步上，多 `nested`（序号），`detail` 的 `tool` 是那件工具的名字；它还没开始跑，不进 `nested` 的列表）；`took_ms`；`files`（改了哪些文件，照结果的效果：`[{path, action}]`，`path` 是换成真实位置以后的绝对路径，`action` 是 `created`（改之前没有这个文件）、`changed`（改了已有的）或 `trashed`（移进回收站），照先后；读的不算，没有的不写。施工 9-8 三补，网页的预览工作区照它认产物、差异卡片照它写「新建」「修改」）；`skill`（读的是技能的 `SKILL.md` 的那一步：`{name, title}`，名字照结果的 `file.read` 效果，`title` 是给人看的名字，照连接的语言挑，同 `preset.get` 里 `skill-<名字>` 那个功能的 `name`，没写的照名字；施工 9-8 四补，网页照它写「使用技能 · 写技能」。施工 9-8 五补起照读的那一刻记在效果里的 `display_name` 挑（这种语言没写的照英文、中文、日文的先后），卸了包、改了名字历史照旧；以前的日志没记的照这一刻装着的查）；`shown`（`show` 给人看的文件：格同结果的 `file.shown` 效果，`path`、`blob`、`name`、`media_type`、`size`，有的再带 `title`、`language`、`width`、`height`；施工 D-5，`tools/show.md`，头照 `media_type` 选怎么显示，同一个 `path` 再来一张换掉那一张；没有的不写）；`detail`（给头看的结构化结果：照结果的 `detail` 原样带上，形状由那件工具定，施工 W-11；没有的不写）；`output`（结果原文的前面一段，施工 9-8 十四补，终端换视图流要的：写法同 `view.detail` 的 `output`，最多 100 行、8 KiB，先到的算，切在字的边界上；切了的多 `output_cut: true`，整份照 `view.detail` 要；没有字的、还没结果的不写；有了结果那一次带上）；`questions`（提问那一步问的那一组，照 `question.asked` 的 `questions`，问的时候就带上、答了照旧留着，施工 9-8 十四补；答案在 `answered` 旁白）；`nested`（下面「脚本里调的工具」，前台那次 `shell` 下面的）；`progress`（在跑时最新的一行进度，施工 X-45 补：工具输出（瞬时的 `tool.progress`）里一整行以 `miyu-progress: ` 开头的，去了前缀和首尾空白，空的不算，最长 200 个字；照调用编号攒半行，一段里几行取最后一条，变了才推。同一个调用最多每 0.5 秒推一次，压着的最新那条最迟 0.5 秒内推出来（`Projector::due`、`tick`，视图流的转发循环到点调）。只在视图流里有，结果到了那一次去掉，翻页、重新订阅没有；任何工具的调用都认，后台任务、嵌套调用不带） |
| `end` | 一轮的收尾：`reason`（`completed`、`interrupted`、`error`、`restarted`……同 `turn.ended`）；`model`、`endpoint`；`took_ms`；`usage`（这一轮加起来，写法同 `model.called`；回顾、起标题这些辅助请求不算，整理会话的主请求 `purpose: memory` 照主请求算，施工 D-1 下补）；`speed`（这一轮的出字速度 `{output, ms}`，施工 SP-1：每次主请求的输出 token、首字到结束的毫秒各自加起来，口径同「会话状态」的 `speed`，没报首字的那一次照整次；调工具、等人不在请求里，旁路请求不算；一次都没记下用时的不写，以前的日志重新投影照样算得出来。每秒多少由头算）；出错的 `error`（`class`、`status`、`message`）和 `explain`（出错说明，照连接的语言） |
| `notice` | 旁白：`what` 是下面几种之一，带各自的格 |

`tool` 的 `state`：`preparing`（还在写参数）、`running`（参数写完了、还没结果；排着队的也是它，头照「同一时刻只转一处」自己挑）、`ok`、`error`、`denied`、`cancelled`、`skipped`，后五种同 `tool.result` 的状态。这一轮结束时还没结果的记 `cancelled`。

`notice` 的 `what`（第一步先这几种，结构化，字由头写）：

| `what` | 格 |
|---|---|
| `compaction` | `trigger`（`auto`、`manual`、`overflow`；在压时照进度带的，施工 6-11 再补）；`instructions`（手动压缩附的要求）；`state`（`running`、`done`、`failed`）；`written`、`expected`、`seen`（摘要写到哪了，同 `compaction.progress`，只在 `running` 时有，视图流里照进度 `view.update`；`written` 变小是重来）；`prepared: true`（换上的是提前压好的，不出进度；施工 6-11 三补起照 `context.compacted` 的 `prepared`，翻页也有，头照它不画结果行）；`before`、`after`（只在视图流里有：`compaction.done` 是瞬时的，翻页没有）；`took_ms`、`usage`；`truncated`（摘要请求截掉了最老的几组，照 `context.compacted` 的，翻页也有；没截过的不写，施工 UP-2）；失败的 `error`（`class`、`status`、`message`）和 `explain`。摘要原文不进条目，点开时 `view.detail {session, compaction: <序号>}` 取。手动压缩那一轮的用时、用量接在这一条上，不另起 `end` |
| `cleared` | 上下文清空了；那一轮不另起 `end` |
| `paused` | 暂停了自动压缩（`context.compaction_paused`）：`reason`、`failures`、`entry`，原样 |
| `reverted` | 撤销了几轮：`turns`、`said`（撤掉的第一句的头一行）；（施工 U-1 拿掉了 `files`：撤销不再改回文件；以前日志里的 `files.restored` 不进视图）；`jobs`（停掉的后台任务的编号）；撤掉的条目另外 `hidden` |
| `job` | 一件后台任务了结了：`job`、`job_kind`（`agent`、`command`；不叫 `what`，旁白的种类占了那个名字）、`title`、`mark`（`done`、`failed`、`stopped`）、`reason`（回报里的原样）、`report`（子代理的报告全文）；后台命令的 `command`（派它那次调用的命令原文）、`output`（`{blob, chars}`，照回报，头点开时照 `blob.get` 取）、`exit_code`、`signal`、`took_ms`（照回报，没有的不写；施工 9-8 三补）；`nested`（这个后台命令下面最后那一份，下面「脚本里调的工具」） |
| `lines` | 监视的后台命令（`shell` 写了 `watch`）出了一批行（`job.lines`，施工 MN-1 下，设计 56 第四节第 4 条）：`job`、`title`（派它那一步的描述）、`command`（派它那次调用的命令原文）、`text`（这一批全文，照事件原样，最多 4000 个字，截过的中间有一行省了多少）、`lines`、`omitted`。不是任务了结：状态里那一行照旧 `running`；头照回报那样画一行，点开看全文 |
| `answered` | 一组题答了：`questions`（问的那一组，照 `question.asked`）、`answers`（照 `question.answered`）、`by` |
| `persona` | 中途换了人格、预设（施工 9-8 十四补，P-13）：`persona: {from, to}`、`preset: {from, to}`，换了哪样写哪样；写编号，无人格、没有预设的是 `null`，显示名头照列表查。人换的那一下（带 `persona`、`preset` 的 `session.policy_changed`）先记着，快照真换上（下一条带 `policy` 的）时出这一条，同 `reply` 的 `persona` 起算的时机 |
| `recap` | 一段回顾：`text`、`covers`（讲到的最后一轮，撤了那一轮跟着藏） |
| `model` | 换了模型：`endpoint`、`model`；`from`（换之前的 `{endpoint, model}`）；`why`：`failover`（出错换了，瞬时的 `model.changed`，只在视图流里有，带换之前最近一次出错的 `class`）、`replaced`（钉着的没了、退回默认的，照 `session.policy_changed` 的 `replaced`）。回合开始重新解析的（`why: turn`）不出旁白 |
| `workspace` | 换了工作区：`cwd`、`dirs`，原样（照 `session.workspace_changed`，家目录写 `~` 由头做）。「她在哪干活」是头拿自己的目录比的，留在头里 |
| `peer` | 等的会话空下来了、等不到了（`peer.idle`）：`session`、`reason`（`idle`、`expired`、`gone`）、`status`。终端现在不画，网页画 |

**标题那一句**（`tool` 的 `title`）：`{"name", "object", "said"}`，照连接的语言。派子代理的 `object` 是描述（`description`），任务编号在 `job`，两个头都写成「任务编号 · 描述」，还没派出去的只有描述；点开的交代是 `args` 里的 `prompt`。留言（`send_message`）的 `object` 照 `to`：子代理写任务编号（标题另在 `to_title`），父会话写「父会话」，别的会话写「会话 短编号」；送到了不写 `said`（和对象重了）。`name` 是显示名：先照结果里记下的 `display_name`（施工 9-8 六补：脚本、MCP 的、扩展的工具派出去那一刻的，照连接的语言挑，这种语言没写的照英文、中文、日文的先后；卸了包、关了服务历史照旧），没有的照 `human` 的 `tools.<名字>.name`（核心自带的），再没有的照这一刻工具目录里那件工具的显示名（还在跑的、内核替它写的拒绝和取消、以前的日志都走这一条），都没有的照工具名；`object` 是对象（`subject` 那个参数的值，`subject` 照 `human` 的 `tools.<名字>.subject`，没有的照这一刻工具目录里那件工具自己说的：扩展登记时写的、MCP 和脚本带的（施工 9-8 十一补，网页报的：原来只照 `human` 的，扩展的工具只有名字没有对象）；路径在家目录里的写 `~/…`，命令写短标题 `description`，参数带会话编号的另给 `session` 短编号）；`said` 是结果那一句（`tool.result` 的 `human` 换成字，换不成的不写）。被拒的（`denied`）不分工具都写（施工 HK-1补，网页实测 HK-1 时 core1 报的）：守卫拒的照 `core/guard/…`（「acme-guard 拦下了：理由」），权限策略拒的照 `core/permissions/…`，只读时内核拦下的、人拒的照 `core/tool-results/…`；不然写入、编辑、命令被拒了，人只能点开看给模型的英文那一句。被拒的结果没带 `human` 的照旧不写；命令、编辑出错的照旧不写（有预览、加减的行数）。`via` 是经谁做的（施工 N-2 再补，项目主人 2026-10-19 定：网络搜索的来源挑到名字后面）：结果的 `human` 带 `via` 这一格的照它给，头写成「网络搜索（Exa）」，没带的不写。准备中的 `name` 是「准备执行命令」这种。

**收起那一行**：`summary` 照连接的语言，`summary_en` 固定英文（界面语言是自动的头用英文那份）。一行拆成几段，带颜色的单拎出来：

```json
[{"text": "Ran 2 commands"}, {"text": " · 1 edit "}, {"text": "+3", "tone": "added"}, {"text": " "}, {"text": "-1", "tone": "removed"}, {"text": " · "}, {"text": "1 err", "tone": "error"}, {"text": " · 5s"}]
```

`tone`：`added`、`removed`、`error`。这一段只有一条命令、它出错了的，整行红：`group` 多 `failed: true`。字和数的规矩照终端蓝图 `tui.md`「时间线」第 17 条，原样搬进核心。

「记住」单算一类（施工 9-8 十补，项目主人 2026-10-17 在终端定，两个头一样）：`remember` 一次算一条（`core/view.json` 的 `kinds`），排在留言、给会话的留言之后，别的工具之前。打头写「记住了 N 条」`Remembered 1 memory`、`Remembered N memories`（日文「記憶を N 件保存」），不打头写「N 条记忆」`1 memory`、`N memories`（「記憶 N 件」）：`Remembered 1 memory · 1 tool · 1 thought · 2s`。出错、被拒的没记成，同编辑，算成用过一件工具。`memory_search` 不单算，算在工具里。

「网络搜索」「抓网页」各单算一类（施工 9-8 十二补，项目主人 2026-10-19 在终端定，两个头一样）：`web_search`、`web_fetch` 一次算一条（`core/view.json` 的 `kinds` 是 `search`、`fetch`），排在记住之后、别的工具之前，先搜索后抓取。搜索打头写「搜索了 N 次网络」`Searched the web 1 time`、`Searched the web N times`（日文「ウェブを N 回検索」），不打头写「N 次搜索」`1 search`、`N searches`（「検索 N 回」）；抓取打头写「抓取了 N 个网页」`Fetched 1 page`、`Fetched N pages`（「ページを N 件取得」），不打头写「N 个网页」`1 page`、`N pages`（「ページ N 件」）。出错的照记住，算工具。

只为发一张图的那一段（施工 X-24，项目主人 10-16 定）：这一段的工具全是同一个包的（工具目录里这一刻的包编号，不在目录里的不算），`tool.result.sent` 一共一张，打头写发出那张的那件工具的显示名，不再数「用了几件工具」：`[{"text": "发送表情包 · 5s"}]`（先搜再发也算这一种）。`summary_en` 写英文的显示名（结果里记下的英文那一份，没有的照英文的字、再没有的照工具名）。想过的、出错的那几格照旧接在后面。混了别的包的工具、发了不止一张的照旧数。终端自己数这一行，经 `human.get` 的 `packages` 照同一条规矩判。

**`view.page`**（9-8 中）多一格 `view`：写 `true` 的回应把 `events` 换成 `entries`（这一页的条目，照显示的先后，读的那一刻的状态），别的格照旧。排着队的 `user` 照它被听到的位置排；撤销藏起的照样在、带 `hidden`。更早的一页拼上来，头照 `id` 去重。

**`view.detail`**（9-8 中）多交 `output`：这次调用的结果原文（工具的输出，整份）。另认 `{session, compaction: <序号>}`：交回那一次压缩的 `summary`（检查点的摘要原文）。施工 M-5 上多交 `nested`（整份嵌套调用），另认 `{session, job}`、`nested_after`（`protocol.md`「`view.detail`」）。

**脚本里调的工具**（施工 M-5 上，设计 `39-脚本调工具.md` 第七节第 4 条，项目主人 2026-10-22 定只做视图这一层、两个头都照它画）：`nested` 是 `{count, last}`。`count` 是这个父级下面到这一刻见过几个（开始了的、了结的，照序号去重）；`last` 是序号最大的那一个 `{n, name, title, state, started?}`：`title` 同「标题那一句」的显示名、对象（照参数挑，结果那一句不写；开始了的瞬时事件没带参数，在跑时只有显示名），`state` 是 `running` 或 `tool.nested` 的 `status`，`started` 只有在跑的有（开始了的瞬时事件的时刻）。头在那一行后面接 `· #n 显示名`。
1. 父级照编号认：`call_…#n` 是前台那次调用，改它那一步（`calls` 里找得到才改）；`j…#n` 是后台任务，改会话状态 `jobs` 里那一行，任务了结时的旁白带最后那一份（内核把还没了结的记成 `cancelled`，排在 `job.reported` 前面）。
2. 只带个数和最近一个：`view.update` 推整条、`view.status` 推整份，一个脚本调几百次时整份列表会越推越长（施工单 M-5 拍板第 1 条）。整份点开时照 `view.detail` 要。
3. 翻页：前台的和那一步在同一轮、同一页；后台任务的照切点前的日志也学（`learn_rows`），那一行的个数照整份日志算。开始了的只在视图流里有，翻页的样子只有了结的；视图流和翻页最后一样。
4. 订阅（头重连）：日志里还没有在跑的，订阅时照会话交的 `Current.nested`（`{id, name, args, at}`）补上，回应的条目照补过的写。

**视图流**（9-8 下）：`subscribe {session, stream: "view"}`，回应带最新一页的条目（同 `view.page`），之后的推送和这份在会话 actor 的同一步里拿，不重不漏：

| 推送 | 什么时候 |
|---|---|
| `view.add {session, entry, after}` | 新的一条，排在 `after` 那一条后面（`null` 是最前） |
| `view.update {session, entry}` | 一条换成这样：状态变了（开始、结果到了、出错、被打断、写完、听到了）。位置变了的带 `after` |
| `view.append {session, id, text}` | 正在写的 `reply`、`thought` 的字，正在写参数的 `tool` 的 `args`：接在后面 |
| `view.hidden {session, ids, hidden}` | 撤销、恢复：这几条藏起、显示回来，编号不删 |
| `view.remove {session, id}` | 拿掉一条：流式时开了、落了盘的回复里却没有的块（出错时去掉的、打断时丢掉的半截工具调用；空了的那一段跟着拿掉），一轮结束时还在压的压缩（被打断了，翻页本来就没有它）。正文、思考出了字才开条目，空块不会推 |

慢的头：同一条的 `view.append` 先合并；还放不下推 `resync`（`stream: "view"`），头重新订阅拿新的一页（`04-核心协议.md` 第七节）。订阅时的那一页和订阅在会话 actor 的同一步里拿（`subscribe_after(0)` 交回的订阅和日志），回应排在所有推送前面；之后的推送接着喂同一台投影（9-8 下，`crates/miyu-endpoint/src/subscriptions/view.rs`）。连接的 `ui.language` 改了，下一批起换字（`Projector::retext`）。

### 会话状态（施工 9-8 补上，2026-10-10 和两个头对过）

视图流订阅的回应带一格 `status`；之后一批推送算完，和上一份比，变了推整份 `view.status {session, status}`，不推增量、不按秒推。视图流不推原始事件，在跑没跑、在等什么都看它。

| 格 | 说明 |
|---|---|
| `state` | `idle`、`running`、`waiting`（有没了结的确认、提问） |
| `waiting` | `[{what: "approve"\|"ask", entry, call, nested?}]`：没了结的确认、提问，照先后；脚本里调的确认带 `nested`（施工 M-7 上）；要确认的工具和参数、题目在那一步的条目里（`args`、`approval`）。没有的不写 |
| `since` | 这一轮开始的时刻；用时由头算。没在跑的不写 |
| `waited_ms`、`waiting_since` | 等人不算用时（施工 9-8 十四补，项目主人 2026-10-23 定，各处统一）：`waited_ms` 是这一轮等人等完了的那几段一共多少毫秒，没等过的不写；`waiting_since` 是正在等的这一段从什么时候起，没在等的不写。等人照内核的事件认：`tool.approval_requested` 起、定了或有了结果止，`question.asked` 起、答了或有了结果止；等的从没有到有时起一段、全了结时止，同时等几件只算一段。头照「现在 − `since` − `waited_ms` − (现在 − `waiting_since`)」算，等人时计时停住。条目里的用时（`tool`、`group`、`end` 的 `took_ms`）照同一个函数刨掉这几段（内核的 `miyu_kernel::waits::Waits`，施工 E-M 下挪进内核，9-8 十四补修起视图也调它，内核记的用时一个口径） |
| `doing` | 在跑时正在做什么：`{what: "thinking"\|"writing"\|"tool", entry}`；`{what: "retrying", attempt, limit, at, class, message, status?, failover?}`（`at` 是再试的时刻，照 `status` 瞬时事件）；`{what: "compacting", entry, trigger, written, expected}`（`trigger` 是 `auto`、`manual`、`overflow`，施工 6-11 三补：头照它分，自动的只写一行，手动的画进度）。没在跑的不写 |
| `context` | `{window?, compaction_line?, used?}`：限额同订阅回应的 `limits`；`used` 是最近一次主请求的输入（三项加起来）加输出（施工 9-8 十四补，终端、核心主会话 2026-10-22 定：她回的下一次请求要带上，算上更接近这一刻占了多少），压完的照 `compaction.done` 的 `after`，压缩落了盘还没有新请求的不写 |
| `speed` | `{output, ms}`：最近一次主请求的输出 token、首字到结束的毫秒数（没报首字的照整次）；每秒多少由头算 |
| `cooling_until` | 候选都在冷却时最早恢复的时刻（照 `class: cooling` 的重试算），说成了一次就不写。只认订阅以后看到的 |
| `preparing` | 后台在提前压：`{seen, since}`（施工 6-11 三补）。起压时（`compaction.started`）有，那一次说完了（成没成都算）、换上了、压过了以后不写。头看它从没有变成有，弹一句「已触发上下文压缩」。只认订阅以后看到的，重连的不补 |
| `usage` | 本会话累计，写法同 `usage.query` 的一行（含子代理的随 9-8 补下） |
| `compactable` | 这一刻有没有能压的（施工 W-12，网页要的：换人格、预设之前问不问「切换前会压缩」）：`session.compact` 会不会回 `nothing_to_compact`，判法同内核（`Session::compactable`）。向会话要，每落一批盘重取，变了跟着推；不由投影算 |
| `model` | 同订阅回应的 `model`（带思考强度 `effort`） |
| `permission`、`todos`、`workspace` | 同订阅回应的那几格 |
| `todos_done` | 因为全部做完而清空了待办（`todos` 是空的）时，清空前那一份，同事件流 `todos.changed` 的 `done`；头照它全打勾停一下再收（施工 9-8 九补，网页的会话要的）。别的时候不写，再写一份没做完的就没了 |
| `persona`、`preset` | 同订阅回应的那几格；中途换了的（P-13）照会话这一刻的跟着推（施工 9-8 十四补对齐蓝图，代码 P-13 起就是这样） |
| `goal` | 长期目标（施工 X-6，`goal.md`）：`{text, limit?, state, reason?, round, resumes, since, usage}`。`state` 是 `running`、`paused`、`completed`、`blocked`、`waiting`（跑着、在等人开口：续轮开的一轮没调工具，施工 X-7；核心重启以后载入的目标记成停着，是 `paused`）；`reason` 卡住的才有；`round` 是设下以后开过几轮（停着时开的不算）；`resumes` 是停过以后又接着跑了几次（施工 GOAL-1：照 `goal.resumed` 数，人续上的、改停着的顺带续上的、核心重启载入记成停着以后续上的都算，0 也写）；`since` 设下的时刻；`usage` 是这个目标花了多少，写法同 `usage`（跑着时的请求）。没设过、清掉了的不写；结束了的照最后的样子留着，再设一个时换掉。向会话要（会话层照整份日志算），不由投影算 |
| `jobs` | 这个会话直接派出去的任务：在跑的和最近做完的 20 个，照派出的先后。一项 `{job, what, title, state, started, ended?, session?, command?, exit_code?, signal?, why?, nested?, fork?}`（`fork` 施工 A-F 补：fork 子代理写 `true`，照 `job.started`；别的不写）（`nested` 是后台命令下面脚本里调过的工具，「脚本里调的工具」，施工 M-5 上）：`what` 是 `command`、`agent`；`state` 是 `running`、`done`（命令退出码 0；子代理那一轮结束了，还能留言叫醒，叫醒了回到 `running`）、`failed`（退出码不是 0、被信号杀了）、`stopped`（`why` 是 `stopped` 人停或她自己停的、`undone` 撤销时停的、`restarted` 有计划重启时停的、`timed_out` 跑到最长时间停的、`too_large` 输出太多停的，施工 7-12 下；`too_chatty` 监视的命令一小时记满了 120 批停的，施工 MN-1 下）、`aborted`（核心崩了，断了）。翻页、订阅时切点前派出、了结的照日志补上 |

| `running_deep` | 这个会话在跑的任务一共几个，连子孙的（施工 9-8 补下） |
| `usage_tree` | 连子代理一起累计用了多少，写法同 `usage`：自己的加上每个子代理那一支的（施工 9-8 补下） |

子代理任务（`jobs` 里 `what: agent` 的）另带四格（施工 9-8 补下，2026-10-10 和终端界面对过）：`busy`（子会话这会儿有没有在跑一轮）、`spawned`（它自己派出、还在跑的有几个）、`running_deep`（它那一支在跑的一共几个，不算它自己）、`usage`（它那一支一共用了多少，照账本连子会话一起查）。子代理此刻在做哪一步不在这里：要的头订阅那个子会话的视图流，看它的 `status.doing`。

1. 投影算得出的那一半（`state`、`waiting`、`since`、`waited_ms`、`waiting_since`、`doing`、`used`、`speed`、`cooling_until`、`preparing`、`jobs`）在 `miyu-view` 的 `projector/status.rs`，喂事件时顺手记；别的向会话要，在 `miyu-endpoint` 的 `view/status.rs` 拼。
2. 一批里有落了盘的事件才向会话 actor 重要一份「当前的」（用量、权限、工作区）；只有增量的不打扰它。
3. 会话树在 `view/tree.rs` 量：只看载入了的会话（在跑的子代理一定载入着，没载入的那一支算没有在跑的），往下最多走 8 层；用量照账本（`usage::branch`）。任务表变了（编号、状态），或者有子代理时别的会话动了（会话列表报的：一轮开始、空下来、改名、删了，`Listing::touched`；子代理新派的孙会话还不知道是谁的，所以不只看已知的子孙；施工 9-8 补下修起，会话的任务表变了（派出、了结）也报一声，`Listing::moved`，只告诉会话树、不推会话列表：孙代理开轮那一声可能先于子代理记下它的任务，只靠开轮、空下来会少数一个）才重量，量完照样和上一份比，变了才推。

### 怎么走

1. 投影是一台小机器：事件照先后一条条喂进去（落了盘的照序号，瞬时的照到的先后），每喂一条交出这一条引起的变化（加、换、接字、藏起）。从哪一条开始喂都一样：编号只看日志里的位置，不看喂过多少。
2. 一页的条目：先照切点前的日志学派出去的后台任务（`Projector::learn`：只记标题、命令，不出条目，9-8 中），再照这一页的事件喂一遍，取最后的样子。切点前的日志只交投影学了会改东西的那几条（施工 V-41 上，`crates/miyu-endpoint/src/view/read.rs`）：派出、留言叫醒任务的工具结果，回报、子会话回报、脚本里调的工具，派任务那几次调用所在的回复（调用编号带着回复的序号，照它读回那一条）；`learn` 多认了别的种类、别的效果，`read.rs` 要跟着改，对照测试会逮到。视图流：每个订阅一台，喂它转发的每一条，变化照上面的表推。语言照这个连接的（`hello` 第 6 条），改了 `ui.language` 的从下一条起照新的；已经交过的不重推，头换了语言重新订阅、重新要页，换掉手里的。
3. 时间线的一段：思考、调工具记进在进行的那一段；她开口说话、插进一条旁白、这一轮结束，这一段结束（终端蓝图「时间线」第 1、21 条）。一块什么时候算完：这一块的 `end`，或者同一次请求里下一块开始了，先到的算（同第 10 条）。
4. 只算显示什么，不管画成什么样。终端、网页各自的配置（收不收、铺不铺开、预览几行）留在头里。
5. 视图流和翻页最后一样：块的开始时刻、思考用了多久照 `model.called` 的 `blocks`（请求发出去的时刻加 `start_ms`），压缩那一条的开始时刻照摘要请求发出去的时刻；只在视图流里有的只有 `x…` 旁白和压缩前后的用量。测试每一份都对照。
6. 排着的话照 `seen` 认被听到：哪一次请求（第一段增量、落了盘的回复、`model.called`，先到的算）看到了它，它就挪到末尾、前面那段收起。开轮时开它的那几条和排在它前面还排着的一起归到这一轮；本来不排着的（空闲时说的、旁听的）只记上这一轮、不挪。用了斜杠命令、没在回答的，留着的排队话挪到末尾、不归到哪一轮（`/stop`）。
7. 一段的 `group` 在加步、步变了状态时跟着 `view.update`；一段收起时算 `took_ms`（第一步开始到最后一步结束：思考照起止，工具照结果到的时刻）。

### 导出成 Markdown（施工 SE-1 中下上、中下中、中下下）

`session.export {format: "markdown"}`（`protocol.md`「`session.export`」）复用投影：整个会话喂一台投影（`view/whole.rs`），条目交给 `miyu_view::markdown` 渲染，界面上有的导出里才有，不另写「显示什么」（调研 `docs/reviews/2026-09-30-会话导出调研.md` 第四节第 3 条）。条目里没有的原文照事件记（`Sources`）：条目只带前面一段的工具结果（`output_cut`）取她当时看到的全文，压缩那一条取检查点的摘要，回复里的块照 `seen` 找回那条回复，脚本里调的工具照父级记了结的 `tool.nested`。

**三个预设**（调研第五节第 2 条；`for` 定预设，写了的 `range`、`thinking`、`tools` 盖过它）：

| 预设 | 范围 | 思考 | 工具 | 子代理 | 开头 |
|---|---|---|---|---|---|
| `read` 自己看 | `all` | 带 | `seen`：显示名、摘要，输出照她看到的，长的折起来 | 交代和回报 | 标题、时间段、模型、token 合计 |
| `agent` 给别的 agent | `context` | 不带 | `seen`：真名、参数原文，结果截到头尾约 4000 字 | 交代（在参数原文里）和回报 | 一段英文事实 |
| `chat` 贴网页 chat | `all` | 不带 | `line`：一行 | 回报 | 标题、时间段、token 估算 |

`children`：`reports`（不写的也是，照预设写交代、回报）或 `append`（另把子会话附在后面，见下面「子会话附在后面」）。`assets`：`placeholder`（不写的也是，写占位）或 `files`（连图一起导成文件夹，见下面「附件文件夹」）。

**都一样的**：

1. 角色的小标题一律英文：消息 `## User`（别处来的写来处：`User (<平台身份>)`、`Session (<短编号>)`、`Module (<模块>)`、`Harness (<名字>)`、`Schedule`、`Kernel`）；她开口 `## Assistant`，接着说的不再起，旁白、回报以后再开口重起；一步工具 `### Tool: …`（一行的写成列表里的 `- Tool: …`，连着的几步一个列表）；子代理的回报 `## Subagent: <标题>`。
2. 正文原样（消息、回复、思考、回报、回顾、摘要）：没合上的代码块在末尾替它合上；放进折叠块的，代码块外面的 `</details` 写成 `&lt;/details`。mermaid 留源码代码块。
3. 思考（带的时候）折在 `<details>` 里，那一行是「思考了 3s」（没记下用时的写「思考」）。
4. 图片、附件写占位：`[图片：<名字>, <宽>×<高>]`、`[文件：<名字>]`，什么都没有的只写是什么；不内联 base64。只有路径的附件（施工 AT-1）写 `[文件：<路径>]`：没有内容可拷，`assets: files` 也照占位写、不算取不到的。`assets: files` 的写成相对链接（下面「附件文件夹」）。
5. 旁白一行一个引用块（`> 上下文压缩了`）：压缩好了的摘要折在下面，在压的、被打断的不写；清空、暂停自动压缩、后台命令了结（下面脚本里调的工具照各自的写法；`tools: full` 再接 blob 里的输出全文）、监视的命令出的一批行（行折在下面）、答了提问（一题一行；`chat` 一行写完）、回顾（正文跟在下面）、换模型、换人格和预设、换工作目录、等的会话怎么了。一轮出错、被打断、步数到顶、核心停了没走完的写一行；走完的不写。`read`、`chat` 的显示名、旁白照 `locale`；`agent` 的旁白、占位一律英文（第 8 条）。
6. 不进的：撤掉的轮（`hidden`）、撤回的排队消息（`withdrawn`）、撤销说明（撤了就当没发生过，调研第五节第 2 条）；注入的上下文、策略、系统提示词本来就不是条目。
7. 最后过一遍：配了的密钥（`secrets.toml` 的值，不到 8 个字的不换）原样出现的换成 `[redacted: <名字>]`，几个值重叠的从左往右取、同一处起头的取最长的，报每个名字几处；再把家目录写成 `~`，只换整段路径（前后不是路径里的字）。
8. 给人看的字在 `resources/core/human/<语言>.json` 的 `said` 的 `export/…`，语言照参数的 `locale`、没写的照连接的；`[redacted: …]`、角色小标题、`agent` 的开头、`agent`、`chat` 的工具写法固定英文。`agent` 预设的旁白、占位（`> Context compacted`、`[Image: …]`、`> The turn was interrupted` 这些）也固定英文，不跟 `locale`（core1 2026-11-04 定：它是给别的模型读的机械文本）：写成渲染器里的常量（`markdown/agent.rs`，编号同给人看的字，字段照 `{名字}` 换），不进 `resources/core/human`；那里没有的编号（只有 `read` 走得到的）照英文那一份。人说的话、她回的话照原文。给模型看的字一个不加：导出不进请求。

**范围**（`select.rs`）：

1. `all`：全部。`{"turns": N}`：照条目的 `turn`，最近 N 轮从那一轮的头一条起、后面的都要（不属于哪一轮的旁白也要）；比有的多的当全部。
2. `context`：她现在的上下文。读日志时每一条另交给内核的有效历史（`History::whole`，工具结果的内容、改了的文件原文不交，只算范围），喂完落到检查点上（`settle`，同载入时从日志重建），和请求组装看到的是同一份：撤掉的、撤回的、压缩掉的不在，撤掉了压缩的回到更早那一次。条目照引出它的那条事件对：`m<序号>`、`e<序号>`、`s<序号>.<n>` 照那一条，`b<seen>.<块号>` 照 `seen` 找回那条回复；被动压缩留下的尾巴照样在。检查点那一次压缩（`c<upto>`）`read`、`chat` 照时间线留着、摘要折在下面，`agent` 拿掉、摘要放最前；更早的压缩不进，检查点以后的（失败了的）照留。
3. 从中间切开的：`read`、`chat` 开头的列表下面一行 `> 前面省掉了 N 轮`（整轮都没进的轮，撤掉的不算）；`agent` 的开头写一行英文（`Only the latest turns are included; N earlier turns are left out.`，从检查点切开的写 `Earlier parts of the session were compacted; the summary below stands in for them.`，接着 `## Summary` 和摘要原文）。

**`read`**：

1. 开头：`# 标题`（没起标题的照第一句，都没有的写「未命名会话」），下面一个列表：时间段（造的时刻到最近一次动静，照核心这台机器的时区到分钟，同一天的后一头只写钟点，末尾带时区）、模型（每一轮收尾记的端点/模型，没撤掉的，去重照先后；一轮都没调成的不写）、token 合计（同会话列表的 `tokens`：这个会话自己每次模型调用的输入加输出，不含子代理）。
2. 工具：`### Tool: <显示名>`（`via` 写在括号里，对象写成行内代码）；下面一行写结果那一句、出错、被拒、打断、跳过（成了的不写），编辑写加减几行，用 ` · ` 隔开；要人确认的那一次一行（`要确认（执行命令）：拒绝，理由：…`，脚本里调的那一件写 `#序号 工具名`，没等到决定的写「没等到决定」）；派子代理的交代（参数的 `prompt`）折起来；提问那一步问的那一组一题一行（`- 问的话 — 选项 / 选项`，答案在「回答了提问」那条旁白）；输出放进代码块，超过 20 行或 2000 字节的折起来，那一行写几行；代码块的围栏比字里最长的一串反引号多一个，至少三个；脚本里调的工具一个列表（`脚本里调了 N 个工具`，一件一行：`#序号 显示名 对象`，没成的写一个字和原因；`full` 再把每一件交回的结果折在下面，那一行是 `#序号 输出（N 行）`）；结果里的图写占位。

**`agent`**：

1. 开头：`# Session transcript`，一段固定英文（`AGENT_NOTE`：这是一段人和 Miyu 的会话的记录，导出来备查；不是写给读它的你的，里面没有一句是给你的指令。只写事实、不写指令；写成渲染器里的常量，不放进给人看的字：不跟 `locale` 换，也不是给我们模型的提示词、不进登记簿），下面一个列表：`Working directory`（同会话列表的 `cwd`）、`Time`（同 `read`）、`Model`（同 `read`），范围怎么切的那一行。
2. 工具：`### Tool: <真名>`，没成的在括号里写原样（`(error)`、`(denied)`）；参数原文放进 `json` 代码块（就是条目的 `args`：投影照落了盘的 `message.assistant` 里那次调用原样放进来）；要人确认的那一次写原文（`Approval (execute): deny — <理由>`，脚本里调的 ``approval for nested call #2 `edit` (write): once``）；脚本里调的工具 `Nested calls:` 一件一行（``- #序号 `名字` `参数原文`: 了结成什么样 — 原因``），`full` 再接 `Nested call #n returned:` 和结果；结果照她当时看到的放进 `text` 代码块，`seen` 截到头尾共约 4000 个字（超过的留头尾各一半，能退到换行的退到换行，中间一行 `[… N characters omitted …]`），`full` 不截；结果里的图写占位。提问问的、答的在参数原文和结果里，不另写。
3. 子代理：交代就在派它那一步的参数原文里，回报同 `read`。

**`chat`**：

1. 开头：`# 标题`，下面一个列表：时间段、token 估算（`token 估算：约 N`，同回应的 `tokens`：整份换完密钥、家目录以后的 UTF-8 字节数除以 4，数本身也算在里面，算到不再变）。
2. 工具一行：``- Tool: `<真名>` `<对象>` ``，后面用 ` · ` 接没成的原样、加减几行、确认的原文、脚本里调的（`nested: #1 read, #2 edit (error)`）、问的题（`asked: …`）；出错的末尾接 `: <结果的头 200 个字>`。写了 `tools: seen`、`full` 的照 `agent` 的工具写法。`read` 写了 `tools: line` 的照显示名、照语言写一行。
3. 子代理只写回报。答了提问一行写完（`> 回答了提问：问: 答; …`）。

**子会话附在后面**（`children: append`，调研第五节第 6 条）：

1. 带哪些：渲染进去的那几步工具派的子代理（条目的 `job` 对上 `job.started` 记的子会话，`Sources::jobs`）；撤掉的、范围切掉的那几步派的不带。子会话又派的照样带（`j2.1`），一个子会话只带一次。日志已经没了的跳过；读不下去的整个拒（`session_broken`）。
2. 怎么写：照同一套开关（`for`、`thinking`、`tools`、`assets`）整份渲染，范围一律 `all`；开头换成一条分隔线 `---`、`# Subagent <任务编号>: <标题>`（标题是派它时的 `description`），下面的列表 `read` 时间段、模型、token 合计，`chat` 时间段，`agent` `Working directory`、`Time`、`Model`；`AGENT_NOTE`、token 估算只在最前面写一次。
3. 先后：照任务编号（`j1`、`j1.1`、`j2`），接在主会话后面。换密钥、家目录写成 `~`、`chat` 的 token 估算照接起来的整份算，`redacted` 是合计。

**附件文件夹**（`assets: files`，调研第五节第 7 条）：

1. 只配写文件：Markdown 写到 `<目录>/<名字>.md` 时，图片、附件拷进 `<目录>/<名字>_files/`（名字去掉扩展名，没有扩展名的照整个名字）。人附的、她发的图、工具结果里的图都算。
2. 链接：图 `![<说明>](<名字>_files/<文件名>)`，别的附件 `[<说明>](…)`；说明同占位里的（名字、宽高），什么都没有的照文件名。路径里的空格、括号、方括号、尖括号、`#`、`?`、`%`、反斜杠、反引号、引号、控制字符写成 `%XX`。
3. 文件名：附件的名字，没有的照路径的最后一截，都没有的 `image-<哈希前 8 位>.<类型>`（`file-…`）；只留最后一截，`<>:"/\|?*`、控制字符换成 `_`，头尾的空格和点去掉，最多 80 个字（扩展名留着），`CON`、`NUL`、`COM1` 这类前面垫 `_`。同一个 blob 只拷一份；不同的撞了名（不分大小写）的在扩展名前接 `-2`、`-3`。
4. 取不到的：渲染前照 blob 库查（核对哈希），取不到的照占位写，回应的 `assets.missing` 列出哈希。

### 还没有的

- 两个头共用的 Markdown 约定（公式分隔符、认哪些 HTML、没写完的块怎么算、本机路径），主会话起草。
- 运行状态行的词库挂在人格上，等项目主人定。
