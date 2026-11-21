## 施工单 WD-4：核心给人看的字表清理成标准的软件短句

状态：做完了，等合并（分支 `step/WD-4-核心给人看的字清理`，从 main 的 36bd3c4e4 开，交之前变基到 dfffbf7f3，也就是 FX-7 到 FX-12 之后）。主会话 2026-11-21 派；项目主人同日决定字表直接修改、不出样稿。

### 为什么

核心给人看的字表 `resources/core/human/{zh,en,ja}.json` 开工时一共 931 句（英文另有 21 句单数形式）：设置项的名字和说明、报错、命令的回执、导出里的小标题。其中不少是施工时随手写的口语和半截话，例如「不认识的键 {detail}」「这个脚本没登记上：{detail}」「{package} 没有在运行，已拦下」「系统包管理器装」。WD-1 只统一了「密钥」和称呼，没有逐句清理。这一步把整张表按 `AGENTS.md`「文档的规矩」逐句过一遍，改成标准的软件短句。

### 改成什么样

- 报错写成「未知的键：{detail}」「{detail} 必须是 1 到 100 之间的整数」「功能编号格式不正确：{detail}」「脚本未注册：{detail}」「已跳过这个脚本：{detail}」「{key} 必须是 {expected}，当前为 {got}」这样的句式。
- 状态词写成「已拒绝」「已中断」「已跳过」「待确认」「运行中」「后台命令已完成：{title}」「本轮已达到步数上限」。
- 不用施工时的口语（「不认识的」「写法不对」「登记」「拦下」「这一份」「写的是」），也不用文言、官腔字眼（「此」「尚未」「无需」「如需」「即可」「您」）。
- 设置项的说明画在设置页上那一项的按钮、输入框旁边，里面不再写配置文件的写法（项目主人 2026-11-21 定）。例如 `models.chat` 的说明从「新会话默认用的模型，写成 供应商/模型，例如 deepseek/deepseek-flash；也能写 @池。」改成「新会话默认使用的模型。」。说明里提到选项时使用选项的名字（「跟随系统」），不使用配置里的值（`auto`）。
- 英文、日文只修改有同样问题的句子：生硬的直译（`Badly written …` 改成 `Invalid …`）、配置文件的说法（`Left out,` 改成 `When unset,`）、和中文说的不是同一件事的句子。

这些写法补进了 `internal/blueprint/config.md`「给人看的字」的「用词的规矩」。

### 数量

| | 中文 | 英文 | 日文 |
|---|---|---|---|
| 修改的句子 | 360 | 129 | 60 |

一共涉及 382 个键。配置键、错误码、占位符、句子首尾的空白和末尾的标点都没有变化（`human_strings.py check` 逐句校验）。

### 没有修改的，和原因

- **FX-9 改过的几句**保持它的意思：`models.vision` 的名字和说明（「多模态模型」）、`usage/purpose/vision`（「多模态」）没有修改；`net.search.source` 的说明只把「代抓」写成「代为抓取」。FX-9 删掉的 `config/option-needs-key`、`config/option-needs-address` 没有恢复。
- **FX-10、FX-12 新加的四句**按同一套规矩润色：`commands/loop-fixed-held`、`loop-dynamic-held`、`loop-auto-held` 的前半句和不带 `-held` 的三句保持一致，后半句写成「有一次撤回还没有处理，循环暂不运行；发送一条消息或运行 /restore 后开始」；`package-problems/tool_not_portable` 写成「工具说明 {detail} 在非字符串类型上使用了 enum，部分模型供应商会拒绝整个请求」，英文句首改成大写。
- **「社区包」四句提示**（`said/packages/notice/*`）没有修改。它们是项目主人 2026-11-12 定的原话（设计 61），手册有五处逐字引用。按词表应该写「软件包」，要不要改由项目主人决定。
- **英文的 `export/end/aborted`、`export/shell/aborted`、`export/omitted`** 没有修改。给 AI 读的导出预设（`agent`）不论界面语言都使用英文这一份，这几句属于给模型的字。中文和日文已经把「Miyu 停了」改成「核心已停止」，英文仍然写 `… when Miyu stopped`，实际停止的是核心；要不要改交主会话决定。
- **一两个词的标签**（上下文各块的名字、用途、额度窗口、预设分栏）大多已经规范，没有修改。
- **`view/error/class/*` 里的「上下文太长」「额度用完」「模型都在冷却」**没有修改：`crates/miyu-cli/src/language.rs` 里有一张并行的表写着同样的字，只改一处会造成两处不一致。
- **测试自己写的假字表**（例如 `crates/miyu-config/src/test_support.rs` 的 `words()`）没有修改，它们不读真的字表。

### 回代码核对后改正的事实

| 键 | 原来的说法 | 实际行为和出处 |
|---|---|---|
| `package-problems/bad_pages_dir` | 「资源目录里的相对目录」 | 相对的是包目录，和同组的 `bad_page_dir` 一致 |
| `package-problems/script_state_left` | `{detail}` 当作目录 | `{detail}` 填的是脚本的名字 |
| `extensions/capabilities/memory.write/summary` | 中文只说知识库里的内容，英文、日文只说整个知识库 | 这项能力同时管 `kb.create`、`kb.delete`、`kb.mount` 和 `kb.add`、`kb.update`、`kb.remove`、`kb.reindex`（`crates/miyu-endpoint/src/connection/rights.rs`） |
| `config/items/providers.<id>.quota.url/description`（中文） | 「模型目录里的地址」 | 查额度的地址和格式在随程序发的供应商档案里（`resources/models/profiles.toml`、`crates/miyu-models/src/quota.rs` 的 `settled`），不在 models.dev 的模型目录里；英文、日文原来就是对的 |
| `export/end/aborted`、`export/shell/aborted`（日文） | 「Miyu が停止」 | 停止的是核心，和中文一致 |
| `config/items/usage.currency/name`（英文、日文） | `Currency shown first`、「先に表示する通貨」 | 说明三种语言都写按汇率换算成这个币种显示，名字改成 `Display currency`、「表示する通貨」 |
| `package-layers/home`（英文） | `installed` | 拼进 `Covered by the {by} copy` 以后分不出是你安装的还是系统包管理器安装的，改成 `user-installed` |

`package-layers/*` 只在 `crates/miyu-endpoint/src/packages.rs` 的 `shadowed_sentence` 里使用，填进 `package-problems/shadowed` 的 `{by}`。两句一起改，拼出来是「已被系统包管理器安装的版本覆盖（这个版本是 0.2.0，正在使用的是 0.3.0），不加载这个版本」。

配置报错是几句拼成的（`crates/miyu-config/src/words.rs`、`problem/tell.rs`），相关的句子一起改，拼出来是「log.level 只能是 error、warn、info、debug、trace 或 off，当前为 "verbose"。可以改为其中一个，例如 log.level = "info"。暂用 "info"（默认值）。」。

### 从设置项说明里拿掉的配置文件写法

| 键 | 拿掉的原话 | 手册里写在哪 |
|---|---|---|
| `models.chat` | 「写成 供应商/模型，例如 deepseek/deepseek-flash；也能写 @池」 | 配置键全表「能写」一栏：`供应商/模型`、`@池`；「模型引用」类型 |
| `pools.<id>.models` | 「每个写成 供应商/模型」 | 配置键全表：每项 `供应商/模型` |
| `providers.<id>.key` | 「写成 { secret = "名字" }（用 miyu provider login 保存）或 { env = "环境变量" }。」 | 配置键全表「密钥引用」类型 |
| `external.bindings.<external>` | 「键填平台上的身份（例如 qq:10001），值填 owner。」 | 配置键全表这一行和它下面的示例 |
| `ui.language` | 「auto 表示跟随终端或浏览器的语言。」（改成用选项名「跟随系统」） | 配置键全表：`auto` 表示跟随系统 |
| `log.level` | 「排查问题时可以调成 debug。」（改成用选项名「详细（用于排查）」） | 配置键全表：排查问题时调成 `debug` |

这六处手册的配置键全表（`docs/zh/guide/config-keys.md`）原来都已经写了，这一步没有往手册里补内容。

### 连带的修改

- **测试**：53 个测试文件里逐字断言的旧话换成新话（`miyu`、`miyu-cli`、`miyu-core`、`miyu-endpoint`、`miyu-store`、`miyu-view`）。断言的写法没有变化，只换期望的字面量。
- **新测试**：`crates/miyu-core/tests/human_wording.rs` 加一条 `chinese_words_avoid_construction_slang`，中文字表里不出现 18 个口语和文言字眼。基点的字表里有 71 行会被它拦住。
- **样本**：`internal/designs/samples/config/` 的四份（两种语言的 Schema 和参考文件）用 `MIYU_SAMPLE_WRITE=1` 重新生成，`internal/blueprint/config.md` 里的中文参考文件样本块同步。
- **蓝图**：四页里照抄句子的表格格子换成新话，一共 101 格（`config.md` 94 格，`kernel/tools.md`、`session/guard.md`、`tools/interface.md` 7 格）；「用词的规矩」补四条。蓝图行文里引用的旧话是当时的记录，没有修改。
- **手册和协议参考**：18 页里写在反引号里的原话换成新话（中文 13 页，英文 5 页），例如排查页的启动报错、扩展页的 `miyu check` 警告、数据页的数据根报错。手册「能力」表里 `memory.write` 的说明补上「创建、删除知识库」。
- 以前的施工单和设计文档里引用的旧话是当时的记录，没有修改。

### 留给其他会话的

- **通讯平台、终端界面、网页界面的仓库**里断言了核心原话的测试，换钉时按下面的对照表修改。`~/.cache/miyu-docs-2026-11-14/wd4-other-repos.txt` 列出了这三个仓库和托盘的主分支里出现了核心旧话（五个字以上的片段）的 83 行，其中多数是注释，供核对时参考。主会话点名的 `crates/miyu-im-app/tests/{called,commands,group,receipts}.rs` 断言的是短句，不在这份清单里，需要按对照表逐句查找。
- **`crates/miyu-endpoint/src/refusal/message.rs`** 里还有一张写在代码里的拒绝原因表（中英成对），里面有同样的口语（「登录没成。」「这个服务没登录过。」）。它不在字表里，由 `docs/zh/reference/errors.md` 和测试守着，这一步没有修改，建议另开一步。
- **`crates/miyu-cli/src/language.rs`** 的命令行文案同样没有在这一步修改。

### 验收

- `human_strings.py check`：占位符、首尾空白、末尾标点逐句不变。
- 全仓 `cargo test --workspace` 通过；门禁十二项通过；Windows 版 clippy、三个平台的 CI 通过。
- `python3 ~/.cache/miyu-docs-2026-11-14/lang/check.py .`：中英各 62 页，问题 0 个。
- 手册里引用的界面原话用 `uiscan.py` 和基点对比：新增的两处是把占位符填成例子的引用（`preset.default 引用的预设 … 不存在，…`、`已被 acme-guard 阻止：理由`），逐句核对过，和字表一致。

### 对照表

一共 549 行，每一组里按字表的先后排列（以变基后的 main 为基准）。键是字表里的路径：`config/…` 是设置页上的字，`said/…` 是报错和回执。

#### 设置页（`config/*`：设置项的名字、说明、选项、分组）

共 97 句。

| 键 | 语言 | 原来 | 现在 |
|---|---|---|---|
| `config/items/show.system_hint/description` | zh | `让 AI 把做出来或拿到的图片、网页、文档、音视频用「展示」直接给你看，而不只是给一个路径。` | `让 AI 把生成或获取的图片、网页、文档、音视频用「展示」直接显示给你，而不是只给出路径。` |
| `config/items/compaction.prepare/description` | zh | `上下文快满时，在后台先把旧的那一段压成摘要，到了要压缩的时候直接换上，不用停下来等。关闭后，到了压缩触发线才开始压缩。` | `上下文即将占满时，在后台提前把较早的部分压缩成摘要，到了需要压缩的时候直接替换，不需要停下来等待。关闭后，到达压缩触发线才开始压缩。` |
| `config/items/net.search.deepseek.key/description` | zh | `联网服务选择 DeepSeek 时需要这个密钥。不填写时使用模型供应商里官方 DeepSeek 的密钥（设置里显示「已继承」）。搜索和代抓按次收费。` | `联网服务选择 DeepSeek 时需要这个密钥。不填写时使用模型供应商里官方 DeepSeek 的密钥（设置里显示「已继承」）。搜索和抓取网页按次收费。` |
| `config/items/net.internal_domains/description` | zh | `其他人（群成员、陌生人）让 AI 上网时，AI 不会访问这些域名和它们的子域名，无论域名解析到什么地址、是否经过代理。你自己不受影响。写法例如 example-corp.com。` | `其他人（群成员、陌生人）让 AI 上网时，AI 不会访问这些域名和它们的子域名，无论域名解析到什么地址、是否经过代理。你自己不受影响。例如 example-corp.com。` |
| `config/items/net.search.source/description` | zh | `AI 搜索网页、抓取网页使用哪一家服务。「自动」只使用免费额度：搜索时依次尝试 Exa、Parallel、Firecrawl、AnySearch、You.com，被限额的服务会暂停一段时间；Miyu 自己抓取失败的网页交给 Exa、Firecrawl、Parallel 代抓。这几家服务都在国外，搜索词和网址会发送给它们。如果选择了其中一家，就优先使用它（选择「DeepSeek 搜索接口」时，抓取网页也优先交给它）；它不可用时再按「自动」的顺序尝试。` | `AI 搜索网页、抓取网页使用哪一家服务。「自动」只使用免费额度：搜索时依次尝试 Exa、Parallel、Firecrawl、AnySearch、You.com，被限额的服务会暂停一段时间；Miyu 自己抓取失败的网页交给 Exa、Firecrawl、Parallel 代为抓取。这几家服务都在国外，搜索词和网址会发送给它们。如果选择了其中一家，就优先使用它（选择「DeepSeek 搜索接口」时，抓取网页也优先交给它）；它不可用时再按「自动」的顺序尝试。` |
| `config/items/memory.extract_max_turns/description` | zh | `群里一直有人说话、对话停不下来时，距离上次整理满这么多轮就直接在后台整理。` | `群里一直有人说话、对话一直没有停顿时，距离上次整理满这么多轮就直接在后台整理。` |
| `config/items/memory.extract_max_wait/description` | zh | `群里一直有人说话、对话停不下来时，最早一轮未整理的对话超过这么久就直接在后台整理。` | `群里一直有人说话、对话一直没有停顿时，最早一轮未整理的对话超过这么久就直接在后台整理。` |
| `config/items/memory.merge_every/description` | zh | `每隔这么久，AI 把新记忆和旧记忆放在一起整理一遍：合并重复的，作废过时的，并写一段对你的了解。` | `每隔这么久，AI 把新记忆和旧记忆放在一起整理一遍：合并重复的记忆，作废过时的记忆，并写一段对你的了解。` |
| `config/items/memory.dream_keep/description` | zh | `每份记忆只保留最近这么多次整理记忆的过程，更早的移到回收处。0 是整理完就删。` | `每份记忆只保留最近这么多次整理记忆的过程，更早的会移到回收处。0 表示整理完成后立即删除。` |
| `config/items/log.level/description` | zh | `运行日志记录到哪一级。排查问题时可以调成 debug。如果设置了环境变量 MIYU_LOG，那一次启动以环境变量为准。` | `运行日志记录到哪一级。排查问题时可以选择「详细（用于排查）」。如果设置了环境变量 MIYU_LOG，那一次启动以环境变量为准。` |
| `config/items/log.level/options/error` | zh | `只记错误` | `只记录错误` |
| `config/items/log.level/options/debug` | zh | `详细，排查用` | `详细（用于排查）` |
| `config/items/log.level/options/off` | zh | `不记` | `不记录` |
| `config/items/ui.language/description` | zh | `终端界面、网页界面和命令行显示的文字使用哪种语言。auto 表示跟随终端或浏览器的语言。` | `终端界面、网页界面和命令行显示的文字使用哪种语言。「跟随系统」表示跟随终端或浏览器的语言。` |
| `config/items/ui.startup/description` | zh | `打开终端界面或网页时，开一个新会话，还是接着最近的那一个。` | `打开终端界面或网页界面时，是新建一个会话，还是继续最近的会话。` |
| `config/items/persona.default/description` | zh | `新会话默认用哪个人格。不设置时，新会话不使用人格，记忆单独保存一份。` | `新会话默认使用哪个人格。不设置时，新会话不使用人格，记忆单独保存一份。` |
| `config/items/preset.default/description` | zh | `新会话默认用哪个预设。` | `新会话默认使用哪个预设。` |
| `config/items/profile.name/description` | zh | `AI 平时怎么叫你，例如名字、昵称。` | `AI 平时对你的称呼，例如名字、昵称。` |
| `config/items/profile.preferred_reply_language/description` | zh | `AI 默认用哪种语言回复，对话里可以随时让它换。` | `AI 默认使用哪种语言回复，你可以在对话里随时让 AI 更换。` |
| `config/items/permission.confirm.git/description` | zh | `强制推送、reset --hard、clean -f、丢弃改动、删除 stash 前先问。` | `执行强制推送、reset --hard、clean -f、丢弃改动、删除 stash 之前先向你确认。` |
| `config/items/permission.confirm.delete/description` | zh | `删工作目录或它的上级、删 Miyu 的数据、删运行时才知道目标的整个目录前先问。` | `删除工作目录或它的上级目录、删除 Miyu 的数据、删除运行时才能确定目标的整个目录之前先向你确认。` |
| `config/items/permission.confirm.power/description` | zh | `关机、重启前先问。` | `关机、重启之前先向你确认。` |
| `config/items/permission.confirm.pipe_to_shell/description` | zh | `curl … \| sh、irm … \| iex 这类前先问。` | `运行 curl … \| sh、irm … \| iex 这类命令之前先向你确认。` |
| `config/items/permission.confirm.container/description` | zh | `容器要宿主的设备、进程或网络命名空间前先问。` | `容器使用宿主的设备、进程或网络命名空间之前先向你确认。` |
| `config/items/permission.confirm.timeout/description` | zh | `等你确认超过这么多秒就自动拒绝。0 为一直等。` | `等待你确认超过这么多秒就自动拒绝。0 表示一直等待。` |
| `config/items/skills.external.enabled/name` | zh | `读取其他目录的 Skills` | `读取其他目录的技能` |
| `config/items/skills.external.dirs/description` | zh | `「读取其他目录的 Skills」开启时读取的目录，按填写的先后顺序读取；如果有同名的技能，使用排在前面的目录里的那一个。路径写成 ~/… 或绝对路径。` | `「读取其他目录的技能」开启时读取的目录，按填写的先后顺序读取；如果有同名的技能，使用排在前面的目录里的那一个。路径填写为 ~/… 或绝对路径。` |
| `config/items/permission.start_read_only/name` | zh | `新会话开局只读` | `新会话默认只读` |
| `config/items/permission.start_read_only/description` | zh | `打开以后，新会话一开始就是只读。AI 只能查、写计划，要改文件时你再关掉只读。项目配置里只能把它打开。` | `开启后，新会话一开始就处于只读状态。AI 只能查看和写计划，需要修改文件时你再关闭只读。项目配置里只能开启这一项。` |
| `config/items/clicall.tools.<name>/description` | zh | `开启后，AI 在命令里也可以用 miyu call 调用这件工具，结果交给脚本处理；关闭后只能直接调用。不填写时使用工具自己登记的设置：MCP 服务和脚本工具默认可以调用，自带工具里只有搜索、抓取网页、搜索记忆、知识库几件可以调用，扩展的工具默认不能调用。写入、发消息这类工具即使开启也不能用 miyu call 调用。` | `开启后，AI 在命令里也可以用 miyu call 调用这件工具，结果交给脚本处理；关闭后只能直接调用。不填写时使用工具注册时的设置：MCP 服务和脚本工具默认可以调用，自带工具里只有搜索、抓取网页、搜索记忆、知识库几件可以调用，扩展的工具默认不能调用。写入、发消息这类工具即使开启也不能用 miyu call 调用。` |
| `config/items/packages.<id>.enabled/description` | zh | `是否启用这个软件包。关闭后，它提供的工具、技能、命令和内容都不可用，扩展和它带的服务会停止，出厂的内置包会被卸载。可以随时重新启用。` | `是否启用这个软件包。关闭后，它提供的工具、技能、命令和内容都不可用，扩展和它带的服务会停止，出厂的内置软件包会被卸载。可以随时重新启用。` |
| `config/items/models.chat/description` | zh | `新会话默认用的模型，写成 供应商/模型，例如 deepseek/deepseek-flash；也能写 @池。` | `新会话默认使用的模型。` |
| `config/items/pools.<id>.models/description` | zh | `几个模型编成一组，每个写成 供应商/模型。` | `把几个模型编成一组。` |
| `config/items/pools.<id>.description/description` | zh | `AI 派子代理时看到的一句，接在池名后面。用英文写，一行。` | `AI 派子代理时看到的一句说明，接在池名后面。使用英文填写，只填写一行。` |
| `config/items/providers.<id>.key/description` | zh | `写成 { secret = "名字" }（用 miyu provider login 保存）或 { env = "环境变量" }。一家供应商只有一个密钥；如果要同时使用几份额度，把它们配置成几家供应商，再放进同一个池。不填写时请求不带认证头。` | `一家供应商只有一个密钥；如果要同时使用几份额度，把它们配置成几家供应商，再放进同一个池。不填写时请求不带认证头。` |
| `config/items/providers.<id>.models.<model>.catalog/description` | zh | `对应 models.dev 目录里的哪一个模型，写成 <供应商>/<模型>。不填写时按名字查找。` | `对应 models.dev 目录里的哪一个模型，填写为 <供应商>/<模型>。不填写时按名字查找。` |
| `config/items/providers.<id>.models.<model>.inputs/description` | zh | `这个模型能读的：文字、图片、PDF、音频、视频。` | `这个模型能读取的输入类型：文字、图片、PDF、音频、视频。` |
| `config/items/providers.<id>.models.<model>.price.cache_read/name` | zh | `读缓存的价` | `缓存读取价` |
| `config/items/providers.<id>.models.<model>.price.cache_read/description` | zh | `每一百万读缓存 token 的价格。` | `每一百万缓存读取 token 的价格。` |
| `config/items/providers.<id>.models.<model>.price.cache_write/name` | zh | `写缓存的价` | `缓存写入价` |
| `config/items/providers.<id>.models.<model>.price.cache_write/description` | zh | `每一百万写缓存 token 的价格。` | `每一百万缓存写入 token 的价格。` |
| `config/items/providers.<id>.quota.url/description` | zh | `查询这家供应商额度的完整地址，优先于模型目录里的地址。如果模型目录里这家供应商没有额度接口，填写了也不会查询。对于 DeepSeek 这类能识别官方地址的供应商，接口地址不是官方地址时同样不会查询。` | `查询这家供应商额度的完整地址，优先于 Miyu 自带的地址。如果 Miyu 自带的资料里这家供应商没有额度接口，填写了也不会查询。对于 DeepSeek 这类能识别官方地址的供应商，接口地址不是官方地址时同样不会查询。` |
| `config/items/packages.mirrors/description` | zh | `官方软件源连接不上或者速度慢时，按填写的先后顺序先尝试这几个地址。下载的内容仍然会验证签名和哈希，所以镜像无法篡改内容。` | `官方软件源无法连接或者速度慢时，按填写的先后顺序先尝试这几个地址。下载的内容仍然会验证签名和哈希，所以镜像无法篡改内容。` |
| `config/items/packages.hold/name` | zh | `不升级的包` | `不升级的软件包` |
| `config/items/packages.other_systems/description` | zh | `软件源的列表里也列出不支持本机系统的软件包，并标注「不支持当前系统」。这些包无法安装。` | `软件源的列表里也列出不支持本机系统的软件包，并标注「不支持当前系统」。这些软件包无法安装。` |
| `config/items/packages.sources.<id>.url/description` | zh | `存放索引的那一层目录的地址。地址和公钥都填写后才算一个软件源；添加一个软件源就表示你信任它。这一项写在系统配置里。` | `索引所在目录的地址。地址和公钥都填写后才算一个软件源；添加一个软件源就表示你信任它。这一项在系统配置中设置。` |
| `config/items/external.bindings.<external>/description` | zh | `通讯平台上哪几个账号是你本人。键填平台上的身份（例如 qq:10001），值填 owner。在私聊里，这个账号就代表你。` | `通讯平台上哪几个账号是你本人。在私聊里，这个账号就代表你。` |
| `config/group_descriptions/protected` | zh | `下面几类命令即使在完全放开时也会先询问你，关闭后不再询问。另外有几类命令始终拒绝，不能关闭：删除根目录、家目录、系统目录，格式化磁盘，向裸设备写入，容器接管整个宿主。` | `下面几类命令即使在完全放开时，执行之前也会先向你确认，关闭后不再确认。另外有几类命令始终拒绝，不能关闭：删除根目录、家目录、系统目录，格式化磁盘，向裸设备写入，容器接管整个宿主。` |
| `config/items/jobs.max_running/name` | en | `Longest background command` | `Maximum background command runtime` |
| `config/items/compaction.prepare/description` | en | `When the context is nearly full, the older part is summarized in the background, so the switch happens without a wait when compaction is due. Off means compacting only once the limit is reached, as before.` | `When the context is nearly full, the older part is summarized in the background, so the switch happens without a wait when compaction is due. Off means compacting only once the limit is reached.` |
| `config/items/log.level/description` | en | `How much the runtime log records. Set debug when chasing a problem. MIYU_LOG, when set, wins for that launch.` | `How much the runtime log records. Choose "Detailed, for debugging" when chasing a problem. MIYU_LOG, when set, wins for that launch.` |
| `config/items/ui.language/description` | en | `The language terminals, the web page and the command line use for you. auto follows the terminal or browser.` | `The language terminals, the web page and the command line use for you. "Follow the system" uses the language of the terminal or browser.` |
| `config/items/usage.currency/name` | en | `Currency shown first` | `Display currency` |
| `config/items/ui.head/description` | en | `The interface plain miyu opens.` | `The interface that opens when you run miyu by itself.` |
| `config/items/skills.external.dirs/description` | en | `Folders read while "Read skills from other folders" is on, in order; on a name clash the earlier one wins. Write ~/… or an absolute path.` | `Folders read while "Read skills from other folders" is on, in order; on a name clash the earlier one wins. Enter ~/… or an absolute path.` |
| `config/items/models.chat/description` | en | `The model new sessions use, written as provider/model, for example deepseek/deepseek-flash, or @pool.` | `The model new sessions use.` |
| `config/items/pools.<id>.models/description` | en | `A group of models, each written as provider/model.` | `A group of models.` |
| `config/items/pools.<id>.strategy/description` | en | `Keep a session on one member, take the next member for every request, or start from the first member every time (a main model with backups used only on errors). Left out, a pool whose members are all billed per request rotates and the others pin.` | `Keep a session on one member, take the next member for every request, or start from the first member every time (a main model with backups used only on errors). When unset, a pool whose members are all billed per request rotates and the others pin.` |
| `config/items/pools.<id>.description/description` | en | `One line the AI sees after the pool name when it starts a subagent. Write it in English.` | `One line the AI sees after the pool name when it starts a subagent. Enter it in English.` |
| `config/items/providers.<id>.driver/description` | en | `How to talk to this provider. Known providers can leave it out.` | `How to talk to this provider. Not needed for providers Miyu recognizes.` |
| `config/items/providers.<id>.base_url/description` | en | `The address of this provider's API; the driver adds the path. Known providers can leave it out.` | `The address of this provider's API; the driver adds the path. Not needed for providers Miyu recognizes.` |
| `config/items/providers.<id>.key/description` | en | `Write { secret = "name" } (saved with miyu provider login) or { env = "VARIABLE" }. One per provider; to use several quotas together, set up several providers and put them in a pool. Without one, no auth header is sent.` | `One key per provider; to use several quotas together, set up several providers and put them in a pool. Without one, no auth header is sent.` |
| `config/items/providers.<id>.name/description` | en | `The name people see. The id is only for referring to models; without one, the model data's name for this provider is used, then the id.` | `The name people see. The id is only for referring to models; without one, the model catalog's name for this provider is used, then the id.` |
| `config/items/providers.<id>.catalog/description` | en | `Which provider in the model data this one is, for example deepseek for a relay that only forwards DeepSeek.` | `Which provider in the model catalog this one is, for example deepseek for a relay that only forwards DeepSeek.` |
| `config/items/providers.<id>.price_multiplier/description` | en | `Prices of this provider are multiplied by it. 1 if left out. A multiplier on the model wins.` | `Prices of this provider are multiplied by it. 1 when unset. A multiplier set on the model wins.` |
| `config/items/providers.<id>.local/description` | en | `A model service on this machine, counted as free. Left out, it follows the address: on this machine means yes.` | `A model service on this machine, counted as free. When unset, it follows the address: an address on this machine counts as local.` |
| `config/items/providers.<id>.cache/description` | en | `How this provider bills its cache. For now it only decides how a pool without a strategy splits. Left out, it follows the driver.` | `How this provider bills its cache. For now it only decides how a pool without a strategy splits. When unset, it follows the driver.` |
| `config/items/providers.<id>.models.<model>.catalog/description` | en | `Which models.dev entry to use: <provider>/<model>. Left out, it is found by name.` | `Which models.dev entry to use: <provider>/<model>. When unset, it is found by name.` |
| `config/items/providers.<id>.models.<model>.effort/description` | en | `The thinking effort this model uses by default: one of its levels, for example high, or off where thinking can be turned off. Left out, the provider decides.` | `The thinking effort this model uses by default: one of its levels, for example high, or off where thinking can be turned off. When unset, the provider decides.` |
| `config/items/providers.<id>.models.<model>.temperature/description` | en | `The temperature this model uses by default, from 0 to 2; higher gives looser answers. Left out, the provider decides. Not sent to models that do not take one.` | `The temperature this model uses by default, from 0 to 2; higher gives looser answers. When unset, the provider decides. Not sent to models that do not take one.` |
| `config/items/providers.<id>.models.<model>.price.input/description` | en | `Price per million input tokens. A written price is used whole.` | `Price per million input tokens. Once any price is set, all of this model's prices come from the values you set.` |
| `config/items/providers.<id>.models.<model>.price.currency/description` | en | `Currency of the price, three capital letters such as USD or CNY. USD if left out.` | `Currency of the price, three capital letters such as USD or CNY. USD when unset.` |
| `config/items/external.bindings.<external>/description` | en | `Which platform identities are you. The key is the platform identity (for example qq:10001), the value is owner. In private chats that identity is you.` | `Which platform identities are you. In private chats that identity is you.` |
| `config/items/compaction.prepare/description` | ja | `コンテキストがいっぱいに近づくと、古い部分を裏で先に要約しておき、圧縮が必要になった時点で待たずに差し替えます。オフにすると、従来どおり上限に達してから圧縮します。` | `コンテキストがいっぱいに近づくと、古い部分を裏で先に要約しておき、圧縮が必要になった時点で待たずに差し替えます。オフにすると、上限に達してから圧縮します。` |
| `config/items/log.level/description` | ja | `実行ログにどこまで記録するかです。問題を調べるときは debug にします。環境変数 MIYU_LOG があれば、その起動ではそちらが優先されます。` | `実行ログにどこまで記録するかです。問題を調べるときは「詳細（調査用）」を選びます。環境変数 MIYU_LOG があれば、その起動ではそちらが優先されます。` |
| `config/items/ui.language/description` | ja | `端末、ウェブ、コマンドラインで表示する言語です。auto は端末やブラウザの言語に合わせます。` | `端末、ウェブ、コマンドラインで表示する言語です。「システムに合わせる」を選ぶと、端末やブラウザの言語に合わせます。` |
| `config/items/usage.currency/name` | ja | `先に表示する通貨` | `表示する通貨` |
| `config/items/skills.external.dirs/description` | ja | `「ほかのフォルダのスキルを読む」がオンのときに読むフォルダです。順番どおりに読み、同じ名前なら先のものが優先されます。~/… か絶対パスで書きます。` | `「ほかのフォルダのスキルを読む」がオンのときに読むフォルダです。順番どおりに読み、同じ名前なら先のものが優先されます。~/… か絶対パスで入力します。` |
| `config/items/models.chat/description` | ja | `新しいセッションが使うモデル。プロバイダー/モデル の形で書きます。例：deepseek/deepseek-flash。@プール でもかまいません。` | `新しいセッションが使うモデル。` |
| `config/items/pools.<id>.models/description` | ja | `いくつかのモデルをひとまとめにします。それぞれ プロバイダー/モデル の形で書きます。` | `いくつかのモデルをひとまとめにします。` |
| `config/items/pools.<id>.strategy/description` | ja | `ひとつのセッションでずっと同じメンバーを使うか、リクエストごとに次のメンバーに替えるか、毎回先頭のメンバーから使うか（主力と予備。エラーのときだけ後ろを使う）。書かなければ、メンバーがすべてリクエストごとの課金なら順番に替え、ほかは固定します。` | `ひとつのセッションでずっと同じメンバーを使うか、リクエストごとに次のメンバーに替えるか、毎回先頭のメンバーから使うか（主力と予備。エラーのときだけ後ろを使う）。設定しない場合は、メンバーがすべてリクエストごとの課金なら順番に替え、ほかは固定します。` |
| `config/items/pools.<id>.description/description` | ja | `サブエージェントを出すときに、プール名のあとに見える一行です。英語で書きます。` | `サブエージェントを出すときに、プール名のあとに見える一行です。英語で入力します。` |
| `config/items/providers.<id>.driver/description` | ja | `このプロバイダーとの話し方。知っているプロバイダーなら書かなくてもかまいません。` | `このプロバイダーとの話し方。知っているプロバイダーなら設定しなくてもかまいません。` |
| `config/items/providers.<id>.base_url/description` | ja | `このプロバイダーの API のアドレス。パスはドライバーが付けます。知っているプロバイダーなら書かなくてもかまいません。` | `このプロバイダーの API のアドレス。パスはドライバーが付けます。知っているプロバイダーなら設定しなくてもかまいません。` |
| `config/items/providers.<id>.key/description` | ja | `{ secret = "名前" }（miyu provider login で保存）か { env = "環境変数" } を書きます。プロバイダーごとに一つ。複数の利用枠をまとめて使うなら、プロバイダーを分けてプールに入れます。書かなければ認証ヘッダーを付けません。` | `キーはプロバイダーごとに一つ。複数の利用枠をまとめて使うなら、プロバイダーを分けてプールに入れます。設定しない場合は認証ヘッダーを付けません。` |
| `config/items/providers.<id>.name/description` | ja | `画面に出す名前。ID はモデルの参照にだけ使います。書かなければモデル資料のプロバイダー名、それもなければ ID を使います。` | `画面に出す名前。ID はモデルの参照にだけ使います。設定しない場合はモデルカタログのプロバイダー名、それもなければ ID を使います。` |
| `config/items/providers.<id>.catalog/description` | ja | `モデル資料のどのプロバイダーに当たるか。DeepSeek だけを中継するなら deepseek と書きます。` | `モデルカタログのどのプロバイダーに当たるか。DeepSeek だけを中継するなら deepseek と入力します。` |
| `config/items/providers.<id>.price_multiplier/description` | ja | `このプロバイダーの価格に掛ける数。書かなければ 1。モデルに書いたほうが優先されます。` | `このプロバイダーの価格に掛ける数。設定しない場合は 1。モデルに設定したほうが優先されます。` |
| `config/items/providers.<id>.local/description` | ja | `このマシンで動くモデルのサービスで、価格は 0 として数えます。書かなければアドレスで決まります。` | `このマシンで動くモデルのサービスで、価格は 0 として数えます。設定しない場合はアドレスで決まります。` |
| `config/items/providers.<id>.cache/description` | ja | `このプロバイダーのキャッシュの課金のしかた。いまは分け方を書いていないプールの分け方を決めるだけです。書かなければドライバーの既定に従います。` | `このプロバイダーのキャッシュの課金のしかた。いまは分け方を設定していないプールの分け方を決めるだけです。設定しない場合はドライバーの既定に従います。` |
| `config/items/providers.<id>.models.<model>.catalog/description` | ja | `models.dev のどの項目に合わせるか：<プロバイダー>/<モデル>。書かなければ名前で探します。` | `models.dev のどの項目に合わせるか：<プロバイダー>/<モデル>。設定しない場合は名前で探します。` |
| `config/items/providers.<id>.models.<model>.effort/description` | ja | `このモデルが既定で使う思考の程度。段階のひとつを書きます（例：high）。思考を切れるモデルは off。書かなければプロバイダーの既定に従います。` | `このモデルが既定で使う思考の程度。段階のひとつを入力します（例：high）。思考を切れるモデルは off。設定しない場合はプロバイダーの既定に従います。` |
| `config/items/providers.<id>.models.<model>.temperature/description` | ja | `このモデルが既定で使う温度。0 から 2 で、高いほど答えが自由になります。書かなければプロバイダーの既定に従います。温度を受け付けないモデルには送りません。` | `このモデルが既定で使う温度。0 から 2 で、高いほど答えが自由になります。設定しない場合はプロバイダーの既定に従います。温度を受け付けないモデルには送りません。` |
| `config/items/providers.<id>.models.<model>.price_multiplier/description` | ja | `このモデルの価格に掛ける数。プロバイダーに書いたものより優先されます。` | `このモデルの価格に掛ける数。プロバイダーに設定したものより優先されます。` |
| `config/items/providers.<id>.models.<model>.price.input/description` | ja | `入力 100 万トークンあたりの価格。書いた価格はまるごと使われます。` | `入力 100 万トークンあたりの価格。価格を設定すると、このモデルの価格はすべて設定した値を使います。` |
| `config/items/providers.<id>.models.<model>.price.currency/description` | ja | `価格の通貨。USD、CNY のような大文字 3 文字。書かなければ USD。` | `価格の通貨。USD、CNY のような大文字 3 文字。設定しない場合は USD。` |
| `config/items/external.bindings.<external>/description` | ja | `通信プラットフォームのどの番号があなた本人かです。キーにプラットフォーム上の ID（例：qq:10001）、値に owner を書きます。個人チャットでは、その番号があなたです。` | `通信プラットフォームのどの番号があなた本人かです。個人チャットでは、その番号があなたです。` |

#### 其余回执（`said/guard`、`tool-results`、`permissions`、`extensions`、`package-layers`、`view`、`mcp`、`context`、`model`、`loading`）

共 48 句。

| 键 | 语言 | 原来 | 现在 |
|---|---|---|---|
| `said/loading/loaded` | zh | `加载了 {tools}` | `已加载 {tools}` |
| `said/loading/bad-args` | zh | `没有写明要加载哪几件工具` | `未指定要加载的工具` |
| `said/tool-results/restarted` | zh | `重启中断` | `因重启而中断` |
| `said/permissions/unresolvable` | zh | `说不清它指向哪里：{reason}` | `无法确定路径指向的位置：{reason}` |
| `said/guard/denied` | zh | `{package} 拦下了` | `已被 {package} 阻止` |
| `said/guard/denied-with-reason` | zh | `{package} 拦下了：{reason}` | `已被 {package} 阻止：{reason}` |
| `said/guard/timed-out` | zh | `{package} 没及时答，已拦下` | `{package} 未及时响应，已阻止本次工具调用` |
| `said/guard/not-running` | zh | `{package} 没有在运行，已拦下` | `{package} 未运行，已阻止本次工具调用` |
| `said/guard/failed` | zh | `{package} 出错了，已拦下` | `{package} 返回错误，已阻止本次工具调用` |
| `said/guard/malformed` | zh | `{package} 答得不对，已拦下` | `{package} 的响应格式不正确，已阻止本次工具调用` |
| `said/extensions/capabilities/tools/summary` | zh | `给 AI 加能调用的工具` | `给 AI 添加可以调用的工具` |
| `said/extensions/capabilities/tools.commands/summary` | zh | `它提供的工具会运行命令，命令里能以当前会话的身份调用 AI 的工具` | `它提供的工具会运行命令，这些命令可以用当前会话的身份调用 AI 的工具` |
| `said/extensions/capabilities/commands/summary` | zh | `加斜杠命令` | `添加斜杠命令` |
| `said/extensions/capabilities/context.inject/summary` | zh | `每一轮开始时往 AI 看到的内容里加东西` | `在每一轮开始时往 AI 看到的内容里添加内容` |
| `said/extensions/capabilities/context.system/summary` | zh | `在 AI 每一轮都看到的系统提示词最后放一段` | `在 AI 每一轮都会看到的系统提示词末尾放一段内容` |
| `said/extensions/capabilities/tool.guard/summary` | zh | `工具执行前放行、拒绝，或者先问你` | `工具执行前放行、拒绝，或者先向你确认` |
| `said/extensions/capabilities/events.read/summary` | zh | `读会话里的全部记录` | `读取会话里的全部记录` |
| `said/extensions/capabilities/model.driver/summary` | zh | `用它的模型时，发给模型的全部对话都经过它` | `使用它提供的模型时，发给模型的全部对话都会经过它` |
| `said/extensions/capabilities/act_for_external/summary` | zh | `替聊天平台上的人发消息、命令` | `替聊天平台上的人发消息、发命令` |
| `said/extensions/capabilities/login.page/summary` | zh | `它能让本机打开的页面不用密码登进来` | `它可以让本机打开的页面不用密码就登录进来` |
| `said/extensions/capabilities/fs.read/summary` | zh | `读指定的文件和目录` | `读取指定的文件和目录` |
| `said/extensions/capabilities/fs.write/summary` | zh | `写指定的文件和目录` | `写入指定的文件和目录` |
| `said/extensions/capabilities/audio.record/summary` | zh | `录本机麦克风的声音` | `录制本机麦克风的声音` |
| `said/extensions/capabilities/audio.play` | zh | `放出声音` | `播放声音` |
| `said/extensions/capabilities/audio.play/summary` | zh | `从本机的音箱、耳机放声音` | `通过本机的音箱、耳机播放声音` |
| `said/extensions/capabilities/memory.write/summary` | zh | `替 AI 记住、改、忘掉记忆，往知识库里加、改、删` | `替 AI 记住、修改、忘掉记忆，创建、修改、删除知识库和里面的内容` |
| `said/package-layers/distro` | zh | `系统包管理器装` | `系统包管理器安装` |
| `said/package-layers/home` | zh | `装上` | `你安装` |
| `said/view/summary/remembered/one` | zh | `记住了 {count} 条` | `记住了 {count} 条记忆` |
| `said/view/summary/remembered` | zh | `记住了 {count} 条` | `记住了 {count} 条记忆` |
| `said/view/summary/used` | zh | `用了 {count} 个工具` | `使用了 {count} 个工具` |
| `said/view/summary/errors` | zh | `{count} 个出错` | `{count} 个错误` |
| `said/mcp/login-done` | zh | `登录好了，可以关掉这一页，回到 Miyu。` | `登录成功，可以关闭这个页面，回到 Miyu。` |
| `said/mcp/login-failed` | zh | `登录没成，回到 Miyu 看看原因。` | `登录失败，回到 Miyu 查看原因。` |
| `said/context/part/system` | zh | `系统提示` | `系统提示词` |
| `said/model/side/core/title/summary` | zh | `会话说完第一轮以后起一个短标题。` | `会话的第一轮结束后，为它起一个简短的标题。` |
| `said/model/side/memory/organize/summary` | zh | `抽取、合并、整理记忆，要读懂一整段对话。` | `抽取、合并、整理记忆，需要理解一整段对话。` |
| `said/context/part/messages.user` | zh | `你说的` | `你的消息` |
| `said/context/part/messages.assistant` | zh | `AI 说的` | `AI 的回复` |
| `said/context/part/messages.knowledge` | zh | `知识库想起的` | `知识库召回的内容` |
| `said/tool-results/crashed` | en | `Internal error, may be partly done` | `Internal error; may have partially run` |
| `said/permissions/unresolvable` | en | `can't tell where it points: {reason}` | `Cannot determine where the path points: {reason}` |
| `said/guard/timed-out` | en | `Blocked: {package} did not answer in time` | `Blocked: {package} did not respond in time` |
| `said/guard/malformed` | en | `Blocked: {package} answered in a wrong format` | `Blocked: {package} returned a malformed response` |
| `said/extensions/capabilities/memory.write/summary` | en | `Remember, change and forget memories; add, change and remove knowledge bases` | `Remember, change and forget memories; add, change and remove knowledge bases and their content` |
| `said/package-layers/home` | en | `installed` | `user-installed` |
| `said/mcp/login-failed` | en | `Login didn't work. Go back to Miyu to see why.` | `Login failed. Go back to Miyu to see why.` |
| `said/extensions/capabilities/memory.write/summary` | ja | `記憶を覚える・変える・忘れる、ナレッジベースを追加・変更・削除する` | `記憶を覚える・変える・忘れる、ナレッジベースとその内容を追加・変更・削除する` |

#### 斜杠命令的回执和用法（`said/commands`）

共 33 句。

| 键 | 语言 | 原来 | 现在 |
|---|---|---|---|
| `said/commands/remembered` | zh | `记下了：{text}` | `已添加记忆：{text}` |
| `said/commands/dreamed-nothing` | zh | `无需整理，用时 {took}。` | `不需要整理，用时 {took}。` |
| `said/commands/dreamed/unchanged` | zh | `没有要改的` | `不需要修改` |
| `said/commands/bad-usage` | zh | `参数不对。用法：` | `参数错误。用法：` |
| `said/commands/cleared-memory` | zh | `清掉了这个会话记下的 {count} 条记忆。` | `已清除这个会话添加的 {count} 条记忆。` |
| `said/commands/cleared-memory-none` | zh | `这个会话没有记下的记忆。` | `这个会话没有添加过记忆。` |
| `said/commands/summary/stop` | zh | `停止此会话所有处理中的东西` | `停止这个会话中所有正在运行的任务` |
| `said/commands/summary/clear-memory` | zh | `清掉这个会话的记忆` | `清除这个会话添加的记忆` |
| `said/commands/loop-dynamic` | zh | `已开始循环：由 AI 定下一次 · {text}` | `已开始循环：由 AI 决定下一次运行的时间 · {text}` |
| `said/commands/loop-auto` | zh | `已开始循环：由 AI 自己定做什么、下一次什么时候` | `已开始循环：由 AI 决定执行的内容和下一次运行的时间` |
| `said/commands/loop-fixed-held` | zh | `已开始循环：每 {every} · {text}。有一处撤回还没处理，循环先搁着：发一句话或 /restore 以后开始` | `已开始循环：每 {every} · {text}。有一次撤回还没有处理，循环暂不运行；发送一条消息或运行 /restore 后开始` |
| `said/commands/loop-dynamic-held` | zh | `已开始循环：由 AI 定下一次 · {text}。有一处撤回还没处理，循环先搁着：发一句话或 /restore 以后开始` | `已开始循环：由 AI 决定下一次运行的时间 · {text}。有一次撤回还没有处理，循环暂不运行；发送一条消息或运行 /restore 后开始` |
| `said/commands/loop-auto-held` | zh | `已开始循环：由 AI 自己定做什么、下一次什么时候。有一处撤回还没处理，循环先搁着：发一句话或 /restore 以后开始` | `已开始循环：由 AI 决定执行的内容和下一次运行的时间。有一次撤回还没有处理，循环暂不运行；发送一条消息或运行 /restore 后开始` |
| `said/commands/loop-off` | zh | `已停止此会话的循环。` | `已停止这个会话的循环。` |
| `said/commands/loop-none` | zh | `此会话没有循环。` | `这个会话没有循环。` |
| `said/commands/usage/todo/show` | zh | `看待办` | `查看待办` |
| `said/commands/usage/goal/show` | zh | `看进度` | `查看进度` |
| `said/commands/usage/goal/set` | zh | `设定长期目标；已经有一个就改它` | `设定长期目标；已有目标时修改当前目标` |
| `said/commands/usage/goal/rounds` | zh | `设定或修改时指定最多运行几轮；0 表示不限` | `设定或修改时指定最多运行的轮数；0 表示不限` |
| `said/commands/argument/loop` | zh | `[[<间隔>] <要说的话> \| off]` | `[[<间隔>] <内容> \| off]` |
| `said/commands/placeholder/loop-fixed` | zh | `<间隔> <要说的话>` | `<间隔> <内容>` |
| `said/commands/placeholder/loop-dynamic` | zh | `<要说的话>` | `<内容>` |
| `said/commands/usage/loop/auto` | zh | `不给具体要求，由 AI 自己定做什么、什么时候` | `不指定内容：由 AI 决定执行的内容和时间` |
| `said/commands/usage/loop/fixed` | zh | `每隔一段时间说一次，例如 5m、2h` | `按固定间隔发送，例如 5m、2h` |
| `said/commands/usage/loop/dynamic` | zh | `由 AI 自己定下一次什么时候` | `由 AI 决定下一次运行的时间` |
| `said/commands/usage/loop/off` | zh | `停止此会话的循环（也可以写 clear、stop）` | `停止这个会话的循环（也可以使用 clear、stop）` |
| `said/commands/usage/context/show` | zh | `看上下文上限和压缩线` | `查看上下文上限和压缩线` |
| `said/commands/usage/context/set` | zh | `设上下文上限，例如 128k、1m` | `设置上下文上限，例如 128k、1m` |
| `said/commands/usage/context/off` | zh | `去掉上下文上限` | `取消上下文上限` |
| `said/commands/context-set` | zh | `上下文上限设为约 {context}` | `上下文上限已设为约 {context}` |
| `said/commands/context-off` | zh | `已去掉上下文上限` | `已取消上下文上限` |
| `said/commands/context-none` | zh | `未设上下文上限` | `未设置上下文上限` |
| `said/commands/failed/no-reply` | zh | `/{name} 没有给出回复` | `/{name} 未返回回复` |

#### 软件包、人格、预设的报错（`said/package-problems`、`persona-problems`、`preset-problems`）

共 228 句。

| 键 | 语言 | 原来 | 现在 |
|---|---|---|---|
| `said/persona-problems/unknown_table` | zh | `不认识的表 [{detail}]：persona.toml 里只能有 [persona]、[memory]、[appearance]` | `未知的表 [{detail}]：persona.toml 中只能包含 [persona]、[memory]、[appearance]` |
| `said/persona-problems/not_a_table` | zh | `{detail} 要写成一张表 [{detail}]` | `{detail} 必须是表 [{detail}]` |
| `said/persona-problems/unknown_key` | zh | `不认识的键 {detail}` | `未知的键：{detail}` |
| `said/persona-problems/not_phrases` | zh | `{detail} 要写一句字` | `{detail} 必须是文本` |
| `said/persona-problems/empty_phrase` | zh | `{detail} 要写一句不空的话` | `{detail} 不能为空` |
| `said/persona-problems/bad_knowledge_bases` | zh | `{detail} 要写成一列知识库的编号，例如 ["notes"]` | `{detail} 必须是知识库编号的列表，例如 ["notes"]` |
| `said/persona-problems/bad_seed` | zh | `{detail} 要写成 #rrggbb 这样的颜色` | `{detail} 必须是 #rrggbb 格式的颜色` |
| `said/persona-problems/bad_subagent` | zh | `{detail} 要写 true 或 false` | `{detail} 只能是 true 或 false` |
| `said/persona-problems/bad_background_opacity` | zh | `{detail} 要写成 0 到 100 的整数` | `{detail} 必须是 0 到 100 之间的整数` |
| `said/persona-problems/bad_background_focus` | zh | `{detail} 要写成 [x, y]，两个都是 0 到 100 的整数` | `{detail} 必须是 [x, y] 格式，x 和 y 都是 0 到 100 之间的整数` |
| `said/persona-problems/bad_background_blur` | zh | `{detail} 要写成 1 到 100 的整数` | `{detail} 必须是 1 到 100 之间的整数` |
| `said/persona-problems/first_line` | zh | `第一行要以 user: 开头` | `第一行必须以 user: 开头` |
| `said/persona-problems/last_line` | zh | `最后一句要是 assistant 的` | `最后一句必须是 assistant 的回答` |
| `said/preset-problems/unknown_table` | zh | `不认识的表 [{detail}]：预设文件里只能有 [preset]、[features]、[software]、[tools]、[limits]` | `未知的表 [{detail}]：预设文件中只能包含 [preset]、[features]、[software]、[tools]、[limits]` |
| `said/preset-problems/not_a_table` | zh | `{detail} 要写成一张表 [{detail}]` | `{detail} 必须是表 [{detail}]` |
| `said/preset-problems/unknown_key` | zh | `不认识的键 {detail}` | `未知的键：{detail}` |
| `said/preset-problems/not_phrases` | zh | `{detail} 要写一句字` | `{detail} 必须是文本` |
| `said/preset-problems/empty_phrase` | zh | `{detail} 要写一句不空的话` | `{detail} 不能为空` |
| `said/preset-problems/bad_software` | zh | `{detail}：软件包的编号小写字母开头，只有小写字母、数字、-、_` | `{detail}：软件包编号必须以小写字母开头，只能包含小写字母、数字、- 和 _` |
| `said/preset-problems/bad_tool` | zh | `{detail}：工具名只有字母、数字、-、_，最多 64 个` | `{detail}：工具名只能包含字母、数字、- 和 _，最多 64 个字符` |
| `said/preset-problems/not_false` | zh | `{detail} 只能写 false：预设里只能关掉单件工具` | `{detail} 只能是 false：预设只能单独关闭某个工具` |
| `said/preset-problems/bad_feature` | zh | `功能编号的写法不对：{detail}` | `功能编号格式不正确：{detail}` |
| `said/preset-problems/bad_icon` | zh | `{detail} 要写 Lucide 的图标名：小写字母开头，只有小写字母、数字、-` | `{detail} 必须是 Lucide 图标名：以小写字母开头，只能包含小写字母、数字和 -` |
| `said/preset-problems/bad_limit` | zh | `{detail} 要写正整数` | `{detail} 必须是正整数` |
| `said/package-problems/unknown_table` | zh | `不认识的表 [{detail}]：清单里只能有 [package]、[ui]、[process]、[builtin]、[worker]、[mascot]、[theme]、[command]、[check]、[settings]、[features]、[connection]、[depends]、[recommends]、[page]` | `未知的表 [{detail}]：清单中只能包含 [package]、[ui]、[process]、[builtin]、[worker]、[mascot]、[theme]、[command]、[check]、[settings]、[features]、[connection]、[depends]、[recommends]、[page]` |
| `said/package-problems/not_a_table` | zh | `{detail} 要写成一张表 [{detail}]` | `{detail} 必须是表 [{detail}]` |
| `said/package-problems/unknown_key` | zh | `不认识的键 {detail}` | `未知的键：{detail}` |
| `said/package-problems/missing_key` | zh | `少了 {detail}` | `缺少 {detail}` |
| `said/package-problems/wrong_kind` | zh | `这种包不能写 {detail}` | `这种类型的软件包不能包含 {detail}` |
| `said/package-problems/needs_command` | zh | `[{detail}] 要有 [command] 说明是哪个程序` | `[{detail}] 必须带有 [command]，用来指定程序` |
| `said/package-problems/two_programs` | zh | `一个包只带一个程序：{detail} 只能留一张` | `一个软件包只能包含一个程序：{detail} 只能保留其中一个` |
| `said/package-problems/bad_protocol` | zh | `{detail} 要写两个不小于 0 的整数 [最低, 最高]，最低不大于最高` | `{detail} 必须是两个非负整数 [最低, 最高]，且最低不大于最高` |
| `said/package-problems/not_text` | zh | `{detail} 要写成一段字` | `{detail} 必须是文本` |
| `said/package-problems/not_texts` | zh | `{detail} 要写成字的数组` | `{detail} 必须是文本数组` |
| `said/package-problems/not_phrases` | zh | `{detail} 要写成语言到一句话的表，语言是 zh、en、ja` | `{detail} 必须是从语言到文本的表，语言只能是 zh、en、ja` |
| `said/package-problems/empty_phrase` | zh | `{detail} 要写一句不空的话` | `{detail} 不能为空` |
| `said/package-problems/bad_command_name` | zh | `{detail} 要小写字母开头，只用小写字母、数字和 -` | `{detail} 必须以小写字母开头，只能包含小写字母、数字和 -` |
| `said/package-problems/bad_program` | zh | `{detail} 只写程序名，不带路径` | `{detail} 只能是程序名，不能带路径` |
| `said/package-problems/bad_page` | zh | `{detail} 里的页名只用小写字母、数字、- 和 _` | `{detail} 中的页名只能包含小写字母、数字、- 和 _` |
| `said/package-problems/bad_pages_dir` | zh | `{detail} 要写资源目录里的相对目录，不能有 .. 和开头的 /` | `{detail} 必须是包目录中的相对目录，不能包含 ..，也不能以 / 开头` |
| `said/package-problems/command_taken` | zh | `子命令 {detail} 已经被其他软件包占用，这一份不生效` | `子命令 {detail} 已被其他软件包占用，在这个软件包中不生效` |
| `said/package-problems/shadowed` | zh | `被{by}的那一份覆盖了（这一份 {version}，使用的是 {over}），这一份不加载` | `已被{by}的版本覆盖（这个版本是 {version}，正在使用的是 {over}），不加载这个版本` |
| `said/package-problems/shadowed_newer` | zh | `这一份比用的那份新：要换回这一份，运行 'miyu pkg remove {id}'。` | `这个版本比正在使用的版本新；如果要改用这个版本，运行 'miyu pkg remove {id}'。` |
| `said/package-problems/shadowed_newer_distro` | zh | `这一份比使用的那份新：要换回这一份，用系统的包管理器卸载 {id}。` | `这个版本比正在使用的版本新；如果要改用这个版本，使用系统的包管理器卸载 {id}。` |
| `said/package-problems/reserved_id` | zh | `编号 {detail} 是数据根里保留的名字，换一个编号` | `编号 {detail} 是数据根中的保留名称，需要更换编号` |
| `said/package-problems/protocol_mismatch` | zh | `这个包说的协议版本是 {detail}，核心只说 1：把它们升级到同一个版本` | `这个软件包支持的协议版本是 {detail}，核心支持的是 1；需要把它们升级到同一个版本` |
| `said/package-problems/bad_setting_name` | zh | `{detail}：配置项的名字要小写字母开头，只用小写字母、数字和 _` | `{detail}：配置项名称必须以小写字母开头，只能包含小写字母、数字和 _` |
| `said/package-problems/bad_title` | zh | `{detail} 要写这一组里一个 text、name 或 option 字段的名字` | `{detail} 必须是这一组中某个 text、name 或 option 字段的名字` |
| `said/package-problems/bad_shown_when` | zh | `{detail} 要写这个包自己的一个选项（不在这一组里）和它的一个或几个选项值` | `{detail} 必须指定这个软件包自己的一个选项（不在这一组中），以及它的一个或多个选项值` |
| `said/package-problems/bad_choices_from` | zh | `{detail} 要写这个包自己登记的一个方法的名字，只能写在普通的 text 项上（不是几行字、列表、一组字段里的字段）` | `{detail} 必须是这个软件包自己注册的一个方法的名字，并且只能用于普通的 text 项（不能用于多行文本、列表或一组字段中的字段）` |
| `said/package-problems/bad_default` | zh | `{detail} 不合这一项的类型，或者不在选项里；密钥、密钥的列表不能写默认值` | `{detail} 与这一项的类型不符，或者不在选项中；密钥和密钥列表不能设置默认值` |
| `said/package-problems/bad_choices` | zh | `{detail} 要列至少两个不重复的字` | `{detail} 必须列出至少两个不重复的文本` |
| `said/package-problems/bad_range` | zh | `{detail} 要写整数，最小不大于最大` | `{detail} 必须是整数，且最小值不大于最大值` |
| `said/package-problems/bad_layers` | zh | `{detail} 只能写 system` | `{detail} 只能包含 system` |
| `said/package-problems/settings_taken` | zh | `前缀 {detail} 和核心自己的配置撞了，它的配置项一项都不收：换一个前缀` | `前缀 {detail} 与核心的配置模块重名，这个软件包的配置项全部不生效；需要更换前缀` |
| `said/package-problems/bad_capability` | zh | `不认识的扩展能力，或者写了两遍：{detail}` | `未知或重复的扩展能力：{detail}` |
| `said/package-problems/bad_feature` | zh | `功能编号的写法不对：{detail}` | `功能编号格式不正确：{detail}` |
| `said/package-problems/bad_tool` | zh | `工具名写法不对，或者在这个包里列了两次：{detail}` | `工具名格式不正确，或者在这个软件包中重复：{detail}` |
| `said/package-problems/bad_platform` | zh | `{detail} 的写法不对` | `{detail} 格式不正确` |
| `said/package-problems/bad_dependency` | zh | `依赖的包编号写法不对，或者写了两遍：{detail}` | `依赖的软件包编号格式不正确，或者重复：{detail}` |
| `said/package-problems/feature_taken` | zh | `功能 {detail} 已经被其他软件包占用，这一份不生效` | `功能 {detail} 已被其他软件包占用，在这个软件包中不生效` |
| `said/package-problems/not_built_in` | zh | `这一份核心没有内置包 {detail} 的代码` | `这个版本的核心没有内置软件包 {detail} 的代码` |
| `said/package-problems/bad_icon` | zh | `{detail} 要写 Lucide 的图标名：小写字母开头，只有小写字母、数字、-` | `{detail} 必须是 Lucide 图标名：以小写字母开头，只能包含小写字母、数字和 -` |
| `said/package-problems/bad_logo` | zh | `{detail} 要写包目录里一张 .png 或 .webp 的相对路径` | `{detail} 必须是包目录中 .png 或 .webp 文件的相对路径` |
| `said/package-problems/bad_page_dir` | zh | `{detail} 要写包目录里的相对目录，不能有 .. 和开头的 /` | `{detail} 必须是包目录中的相对目录，不能包含 ..，也不能以 / 开头` |
| `said/package-problems/bad_page_source` | zh | `{detail}：dir、server 只写一样，server 只能写 true，只有扩展能写 server` | `{detail}：dir 和 server 只能写一个，server 只能是 true，并且只有扩展可以使用 server` |
| `said/package-problems/bad_mascot_model` | zh | `{detail} 要写包目录里的相对路径，不能有 .. 和开头的 /` | `{detail} 必须是包目录中的相对路径，不能包含 ..，也不能以 / 开头` |
| `said/package-problems/bad_theme_file` | zh | `{detail} 要写包目录里的相对路径，不能有 .. 和开头的 /` | `{detail} 必须是包目录中的相对路径，不能包含 ..，也不能以 / 开头` |
| `said/package-problems/bad_theme_dir` | zh | `{detail} 要写包目录里的相对目录，不能有 .. 和开头的 /` | `{detail} 必须是包目录中的相对目录，不能包含 ..，也不能以 / 开头` |
| `said/package-problems/bad_export` | zh | `[export] 里的 {detail} 写法不对：名字写进 provides 或 plugs，方法写成 <名字>.<方法>；都是小写字母开头，不能重复` | `[export] 中的 {detail} 格式不正确：名字必须写在 provides 或 plugs 中，方法写成 <名字>.<方法>；都必须以小写字母开头，且不能重复` |
| `said/package-problems/bad_mcp` | zh | `MCP 服务的写法不对：{detail}` | `MCP 服务格式不正确：{detail}` |
| `said/package-problems/bad_slash` | zh | `斜杠命令的写法不对：{detail}` | `斜杠命令格式不正确：{detail}` |
| `said/package-problems/bad_search` | zh | `搜索来源的写法不对：{detail}` | `搜索来源格式不正确：{detail}` |
| `said/package-problems/bad_guard` | zh | `守卫的写法不对：{detail}` | `守卫格式不正确：{detail}` |
| `said/package-problems/bad_driver` | zh | `模型驱动的写法不对：{detail}` | `模型驱动格式不正确：{detail}` |
| `said/package-problems/bad_provider_key` | zh | `需要哪一家供应商的密钥写法不对：{detail}` | `所需供应商密钥的声明格式不正确：{detail}` |
| `said/package-problems/driver_unapproved` | zh | `供应商 {provider} 用的模型驱动 {driver}：那个包还没批「提供模型驱动」，这一家在批了以前用不了` | `供应商 {provider} 使用的模型驱动 {driver} 所在的软件包还没有获准「提供模型驱动」，获准之前无法使用这个供应商` |
| `said/package-problems/driver_key_missing` | zh | `供应商 {provider} 用的模型驱动 {driver} 需要密钥，这家供应商没有配置` | `供应商 {provider} 使用的模型驱动 {driver} 需要密钥，但这个供应商没有配置密钥` |
| `said/package-problems/driver_key_unused` | zh | `供应商 {provider} 用的模型驱动 {driver} 不需要密钥：配置的密钥不会交给它` | `供应商 {provider} 使用的模型驱动 {driver} 不需要密钥，配置的密钥不会传给它` |
| `said/package-problems/provider_shadowed` | zh | `软件包 {package} 带的供应商 {provider} 没有加上：配置里已经有你自己的 {provider}。要使用软件包带的供应商，删掉配置里的 [providers.{provider}]` | `软件包 {package} 提供的供应商 {provider} 未添加：配置中已经有你自己的 {provider}。如果要使用软件包提供的供应商，删除配置中的 [providers.{provider}]` |
| `said/package-problems/provider_taken` | zh | `软件包 {package} 带的供应商 {provider} 没有加上：软件包 {by} 已经带了同编号的供应商` | `软件包 {package} 提供的供应商 {provider} 未添加：软件包 {by} 已经提供了相同编号的供应商` |
| `said/package-problems/provider_foreign` | zh | `软件包 {package} 带的供应商 {provider} 没有加上：驱动属于其他软件包` | `软件包 {package} 提供的供应商 {provider} 未添加：它的驱动属于其他软件包` |
| `said/package-problems/provider_invalid` | zh | `软件包 {package} 带的供应商 {provider} 有几项不对，已忽略：{keys}` | `软件包 {package} 提供的供应商 {provider} 中以下几项无效，已忽略：{keys}` |
| `said/package-problems/bad_after` | zh | `工具调用后通知的写法不对：{detail}` | `工具调用后通知格式不正确：{detail}` |
| `said/package-problems/bad_side` | zh | `旁路请求的写法不对：{detail}` | `旁路请求格式不正确：{detail}` |
| `said/package-problems/bad_purpose` | zh | `用量用途的写法不对：{detail}` | `用量用途格式不正确：{detail}` |
| `said/package-problems/bad_tray` | zh | `托盘入口的写法不对：{detail}` | `托盘入口格式不正确：{detail}` |
| `said/package-problems/bad_author` | zh | `作者的写法不对：{detail}` | `作者格式不正确：{detail}` |
| `said/package-problems/bad_upstream` | zh | `上游仓库的网址不对：{detail}` | `上游仓库地址无效：{detail}` |
| `said/package-problems/bad_license` | zh | `许可证的写法不对：{detail}` | `许可证格式不正确：{detail}` |
| `said/package-problems/bad_widget` | zh | `界面块的写法不对：{detail}` | `界面块格式不正确：{detail}` |
| `said/package-problems/bad_content` | zh | `给其他软件包带的内容写法不对：{detail}` | `为其他软件包提供的内容格式不正确：{detail}` |
| `said/package-problems/bad_needs` | zh | `要求的写法不对，现在只认 images：{detail}` | `要求无效或重复：{detail}，目前只支持 images` |
| `said/package-problems/bad_group` | zh | `组名写法不对、重复了，或者出厂没有这个组：{detail}` | `组名格式不正确、重复，或者不是出厂提供的组：{detail}` |
| `said/package-problems/bad_prefix` | zh | `前缀的写法不像包编号：{detail}` | `前缀不符合软件包编号的格式：{detail}` |
| `said/package-problems/prefix_taken` | zh | `前缀 {detail} 已经被其他软件包占用，这一份不生效` | `前缀 {detail} 已被其他软件包占用，在这个软件包中不生效` |
| `said/package-problems/bad_targets` | zh | `系统的写法不对，只认 linux-x86_64、linux-aarch64、macos-aarch64、windows-x86_64，不能重复：{detail}` | `目标平台无效或重复，目前只支持 linux-x86_64、linux-aarch64、macos-aarch64、windows-x86_64：{detail}` |
| `said/package-problems/bad_systems` | zh | `系统的写法不对，只认 linux、macos、windows，不能重复，targets 里每一种的系统都要写上：{detail}` | `系统无效或重复，目前只支持 linux、macos、windows，并且 targets 中每个目标的系统都必须列出：{detail}` |
| `said/package-problems/bad_provider` | zh | `自带的供应商写法不对：{detail}` | `自带的供应商格式不正确：{detail}` |
| `said/package-problems/bad_miyu` | zh | `{detail} 要写成三段数字的版本，例如 2026.11.0` | `{detail} 必须是三段数字的版本号，例如 2026.11.0` |
| `said/package-problems/needs_newer_miyu` | zh | `这个包要 Miyu {detail} 以上，先更新 Miyu` | `这个软件包要求 Miyu {detail} 或更高版本，需要先更新 Miyu` |
| `said/package-problems/tool_not_portable` | zh | `工具说明 {detail} 把 enum 写在了不是字符串的类型上，有的模型供应商会拒绝整个请求` | `工具说明 {detail} 在非字符串类型上使用了 enum，部分模型供应商会拒绝整个请求` |
| `said/package-problems/check_failed` | zh | `这个软件包自己的检查没有运行完：{detail}` | `软件包自带的检查未完成：{detail}` |
| `said/package-problems/check_unavailable` | zh | `没找到 {detail}，这个软件包自己的检查没有运行` | `未找到 {detail}，软件包自带的检查没有运行` |
| `said/package-problems/check_output` | zh | `这个软件包自己的检查输出了 {detail} 行无法识别的内容，已跳过` | `软件包自带的检查输出了 {detail} 行无法识别的内容，已跳过` |
| `said/package-problems/files_missing` | zh | `安装时带的文件少了 {detail} 个` | `缺少 {detail} 个安装时的文件` |
| `said/package-problems/prompt_override_bad` | zh | `这个提示词的覆盖没有生效，改用下一层或者出厂的：{detail}` | `这个提示词覆盖未生效，改用下一层或出厂的内容：{detail}` |
| `said/package-problems/override_bad` | zh | `这个工具说明的覆盖没有生效，改用出厂的：{detail}` | `这个工具说明覆盖未生效，改用出厂的内容：{detail}` |
| `said/package-problems/script_skipped` | zh | `跳过了这个脚本：{detail}` | `已跳过这个脚本：{detail}` |
| `said/package-problems/script_link_outside` | zh | `这个脚本是链接，指到数据根里的 {detail}，不在你放东西的那几个文件夹里，跳过了` | `已跳过这个脚本：它是指向数据根中 {detail} 的链接，这个位置不在存放你自己文件的目录中` |
| `said/package-problems/script_state_left` | zh | `这个脚本的状态目录（{detail}）还在，但脚本已经不存在；不需要的话可以删除这个目录` | `脚本 {detail} 的状态目录仍然存在，但这个脚本已不存在；如果不再需要，可以删除这个目录` |
| `said/package-problems/schedule_unreadable` | zh | `这个定时任务无法读取，到时间不会运行，其他定时任务不受影响（{detail}）；不需要的话可以按编号删掉` | `无法读取这个定时任务，到时间也不会运行，其他定时任务不受影响（{detail}）；如果不再需要，可以按编号删除` |
| `said/package-problems/script_clashed` | zh | `脚本 {detail} 和其他工具重名，没有登记上` | `脚本 {detail} 与其他工具重名，未注册` |
| `said/package-problems/script_not_made` | zh | `这个脚本没登记上：{detail}` | `脚本未注册：{detail}` |
| `said/package-problems/script_not_loaded` | zh | `核心还没读到脚本 {detail}；过一会儿再查，一直这样就重启核心` | `核心还没有加载脚本 {detail}；稍后再检查，如果一直这样，重启核心` |
| `said/package-problems/mcp_tool_skipped` | zh | `MCP 服务列出来的工具、提示词有几件没登记上：{detail}（提示词写成 /名字）。name_taken 是名字被占了（截短、清理以后撞了，或者其他工具已经叫这个），name 是名字不合写法，parameters 是参数格式不对` | `MCP 服务列出的部分工具和提示词未注册：{detail}（提示词显示为 /名字）。name_taken 表示名字已被占用（截短或清理后重名，或者已有其他工具使用这个名字），name 表示名字不符合格式要求，parameters 表示参数格式不正确` |
| `said/package-problems/skill_skipped` | zh | `跳过了这个技能：{detail}` | `已跳过这个技能：{detail}` |
| `said/package-problems/skill_link_outside` | zh | `这个技能文件夹是链接，指到数据根里的 {detail}，不在你放东西的那几个文件夹里，跳过了` | `已跳过这个技能文件夹：它是指向数据根中 {detail} 的链接，这个位置不在存放你自己文件的目录中` |
| `said/package-problems/skill_name_mismatch` | zh | `写的名字和文件夹不同名，使用写的名字 {detail}` | `技能名与文件夹名不一致，使用 name 中写的名字 {detail}` |
| `said/package-problems/skill_name_from_folder` | zh | `名字没写或者不合写法（1 到 64 个小写字母、数字、单个连字符），使用文件夹的名字 {detail}` | `名字缺失或不符合格式（1 到 64 个小写字母、数字或单个连字符），改用文件夹名 {detail}` |
| `said/package-problems/skill_yaml_lenient` | zh | `开头不是合法的 YAML：把没加引号的值当成一整句文字重新读取才成功，值里有冒号时请加上引号` | `开头的元数据不是合法的 YAML：把未加引号的值当作纯文本重新读取后才成功；值中含有冒号时，需要给它加上引号` |
| `said/package-problems/skill_description_cut` | zh | `说明超过 1024 个字，后面截掉了` | `说明超过 1024 个字符，超出部分已截断` |
| `said/package-problems/skill_compatibility` | zh | `compatibility 应是不超过 500 个字的一句话` | `compatibility 应为不超过 500 个字符的文本` |
| `said/package-problems/skill_metadata` | zh | `metadata 应是字到字的表（值不能是数字、列表或者嵌套的表）` | `metadata 应为文本到文本的映射（值不能是数字、列表或嵌套的表）` |
| `said/package-problems/script_covered` | zh | `和 {detail} 同名，被它覆盖了，这一份不使用` | `已被同名的 {detail} 覆盖，不会使用` |
| `said/package-problems/mcp_covered` | zh | `和 {detail} 同名，被它覆盖了，这一份不使用` | `已被同名的 {detail} 覆盖，不会使用` |
| `said/package-problems/skill_covered` | zh | `和 {detail} 同名，被它覆盖了，这一份不使用` | `已被同名的 {detail} 覆盖，不会使用` |
| `said/package-problems/persona_covered` | zh | `和 {detail} 同名，被它覆盖了，这一份不使用` | `已被同名的 {detail} 覆盖，不会使用` |
| `said/package-problems/preset_covered` | zh | `和 {detail} 同名，被它覆盖了，这一份不使用` | `已被同名的 {detail} 覆盖，不会使用` |
| `said/package-problems/persona_left` | zh | `软件包 {detail} 已经卸载，这一份是它留下的改动` | `软件包 {detail} 已卸载，这是它留下的改动` |
| `said/package-problems/preset_left` | zh | `软件包 {detail} 已经卸载，这一份是它留下的改动` | `软件包 {detail} 已卸载，这是它留下的改动` |
| `said/package-problems/slash_skipped` | zh | `跳过了这个斜杠命令：{detail}` | `已跳过这个斜杠命令：{detail}` |
| `said/package-problems/slash_link_outside` | zh | `这个命令模板是链接，指到数据根里的 {detail}，不在你放东西的那几个文件夹里，跳过了` | `已跳过这个命令模板：它是指向数据根中 {detail} 的链接，这个位置不在存放你自己文件的目录中` |
| `said/package-problems/slash_covered` | zh | `和 {detail} 同名，被它覆盖了，这一份不使用` | `已被同名的 {detail} 覆盖，不会使用` |
| `said/package-problems/slash_builtin` | zh | `和 Miyu 自带的 /{detail} 同名，这一份不用` | `与 Miyu 内置的 /{detail} 同名，不会使用` |
| `said/persona-problems/unknown_table` | en | `Unknown table [{detail}]: persona.toml only takes [persona], [memory] and [appearance]` | `Unknown table [{detail}]: persona.toml can only contain [persona], [memory] and [appearance]` |
| `said/persona-problems/unknown_key` | en | `Unknown key {detail}` | `Unknown key: {detail}` |
| `said/persona-problems/empty_phrase` | en | `{detail} must be text that is not empty` | `{detail} must be non-empty text` |
| `said/persona-problems/bad_background_opacity` | en | `{detail} must be a whole number from 0 to 100` | `{detail} must be an integer from 0 to 100` |
| `said/persona-problems/bad_background_focus` | en | `{detail} must be [x, y], each a whole number from 0 to 100` | `{detail} must be [x, y], each an integer from 0 to 100` |
| `said/persona-problems/bad_background_blur` | en | `{detail} must be a whole number from 1 to 100` | `{detail} must be an integer from 1 to 100` |
| `said/preset-problems/unknown_table` | en | `Unknown table [{detail}]: a preset only takes [preset], [features], [software], [tools] and [limits]` | `Unknown table [{detail}]: a preset can only contain [preset], [features], [software], [tools] and [limits]` |
| `said/preset-problems/unknown_key` | en | `Unknown key {detail}` | `Unknown key: {detail}` |
| `said/preset-problems/bad_software` | en | `{detail}: a package id starts with a lowercase letter and has only lowercase letters, digits, - and _` | `{detail}: a package id must start with a lowercase letter and contain only lowercase letters, digits, - and _` |
| `said/preset-problems/bad_tool` | en | `{detail}: a tool name has only letters, digits, - and _, at most 64` | `{detail}: a tool name can contain only letters, digits, - and _, up to 64 characters` |
| `said/preset-problems/not_false` | en | `{detail} can only be false: a preset can only turn single tools off` | `{detail} can only be false: a preset can only turn individual tools off` |
| `said/preset-problems/bad_feature` | en | `Badly written feature id: {detail}` | `Invalid feature id: {detail}` |
| `said/preset-problems/bad_icon` | en | `{detail} must be a Lucide icon name: a lowercase letter, then lowercase letters, digits and -` | `{detail} must be a Lucide icon name: it starts with a lowercase letter and contains only lowercase letters, digits and -` |
| `said/preset-problems/bad_limit` | en | `{detail} must be a positive whole number` | `{detail} must be a positive integer` |
| `said/package-problems/unknown_table` | en | `Unknown table [{detail}]: a manifest only takes [package], [ui], [process], [builtin], [worker], [mascot], [theme], [command], [check], [settings], [features], [connection], [depends], [recommends] and [page]` | `Unknown table [{detail}]: a manifest can only contain [package], [ui], [process], [builtin], [worker], [mascot], [theme], [command], [check], [settings], [features], [connection], [depends], [recommends] and [page]` |
| `said/package-problems/unknown_key` | en | `Unknown key {detail}` | `Unknown key: {detail}` |
| `said/package-problems/needs_command` | en | `[{detail}] needs a [command] to name the program` | `[{detail}] needs a [command] that names the program` |
| `said/package-problems/two_programs` | en | `A package carries one program: keep only one of {detail}` | `A package can contain only one program: keep only one of {detail}` |
| `said/package-problems/not_texts` | en | `{detail} must be a list of text` | `{detail} must be a list of strings` |
| `said/package-problems/bad_pages_dir` | en | `{detail} must be a relative directory inside the resource directory, without .. or a leading /` | `{detail} must be a relative directory inside the package directory, without .. or a leading /` |
| `said/package-problems/command_taken` | en | `The command {detail} is taken by another package; this one does not count` | `The subcommand {detail} is already taken by another package; it does not take effect for this package` |
| `said/package-problems/shadowed` | en | `Covered by the {by} copy (this one {version}, in use {over}); this one is not loaded` | `Covered by the {by} copy (this copy is {version}, the one in use is {over}); this copy is not loaded` |
| `said/package-problems/protocol_mismatch` | en | `This package speaks protocol versions {detail}, and the core speaks 1; upgrade them to the same release` | `This package supports protocol versions {detail}, but the core supports 1; upgrade them to the same release` |
| `said/package-problems/bad_range` | en | `{detail} must be integers, the lowest not above the highest` | `{detail} must be integers, with the minimum not above the maximum` |
| `said/package-problems/bad_layers` | en | `{detail} must list system` | `{detail} can only contain system` |
| `said/package-problems/settings_taken` | en | `The prefix {detail} is also a core settings section, so none of its settings count; give it another prefix` | `The prefix {detail} is also a core settings section, so none of this package's settings are used; use another prefix` |
| `said/package-problems/bad_feature` | en | `Badly written feature id: {detail}` | `Invalid feature id: {detail}` |
| `said/package-problems/bad_tool` | en | `Badly written tool name, or listed twice in this package: {detail}` | `Invalid tool name, or listed twice in this package: {detail}` |
| `said/package-problems/bad_platform` | en | `{detail} is badly written` | `{detail} is invalid` |
| `said/package-problems/bad_dependency` | en | `Badly written or repeated package id: {detail}` | `Invalid or repeated package id: {detail}` |
| `said/package-problems/feature_taken` | en | `The feature {detail} is taken by another package; this one does not count` | `The feature {detail} is already taken by another package; it does not take effect for this package` |
| `said/package-problems/bad_icon` | en | `{detail} must be a Lucide icon name: a lowercase letter, then lowercase letters, digits and -` | `{detail} must be a Lucide icon name: it starts with a lowercase letter and contains only lowercase letters, digits and -` |
| `said/package-problems/bad_page_source` | en | `{detail}: write either dir or server, server can only be true, and only extensions can write server` | `{detail}: set either dir or server; server can only be true, and only extensions can use server` |
| `said/package-problems/bad_mcp` | en | `Badly written MCP server: {detail}` | `Invalid MCP server: {detail}` |
| `said/package-problems/bad_slash` | en | `Badly written slash command: {detail}` | `Invalid slash command: {detail}` |
| `said/package-problems/bad_search` | en | `Badly written search source: {detail}` | `Invalid search source: {detail}` |
| `said/package-problems/bad_guard` | en | `Badly written guard: {detail}` | `Invalid guard: {detail}` |
| `said/package-problems/bad_driver` | en | `A model driver is written wrong: {detail}` | `Invalid model driver: {detail}` |
| `said/package-problems/bad_provider_key` | en | `The provider key request is written wrong: {detail}` | `Invalid provider key request: {detail}` |
| `said/package-problems/bad_after` | en | `Badly written tool.after notice: {detail}` | `Invalid tool.after notice: {detail}` |
| `said/package-problems/bad_side` | en | `Badly written side request: {detail}` | `Invalid side request: {detail}` |
| `said/package-problems/bad_purpose` | en | `Badly written usage purpose: {detail}` | `Invalid usage purpose: {detail}` |
| `said/package-problems/bad_tray` | en | `Badly written tray entry: {detail}` | `Invalid tray entry: {detail}` |
| `said/package-problems/bad_author` | en | `Badly written author: {detail}` | `Invalid author: {detail}` |
| `said/package-problems/bad_upstream` | en | `Bad upstream address: {detail}` | `Invalid upstream address: {detail}` |
| `said/package-problems/bad_license` | en | `Badly written license: {detail}` | `Invalid license: {detail}` |
| `said/package-problems/bad_widget` | en | `Badly written widget: {detail}` | `Invalid widget: {detail}` |
| `said/package-problems/bad_content` | en | `Badly written content for another package: {detail}` | `Invalid content for another package: {detail}` |
| `said/package-problems/bad_needs` | en | `Unknown or repeated need, only images is known: {detail}` | `Unknown or repeated need: {detail}; only images is supported` |
| `said/package-problems/bad_prefix` | en | `Prefix is not written like a package id: {detail}` | `Prefix is not a valid package id: {detail}` |
| `said/package-problems/prefix_taken` | en | `The prefix {detail} is taken by another package; this one does not count` | `The prefix {detail} is already taken by another package; it does not take effect for this package` |
| `said/package-problems/bad_targets` | en | `Unknown or repeated target, known are linux-x86_64, linux-aarch64, macos-aarch64, windows-x86_64: {detail}` | `Unknown or repeated target; supported targets are linux-x86_64, linux-aarch64, macos-aarch64 and windows-x86_64: {detail}` |
| `said/package-problems/bad_systems` | en | `Unknown or repeated system, known are linux, macos, windows, and every system in targets must be listed: {detail}` | `Unknown or repeated system; supported systems are linux, macos and windows, and every system in targets must be listed: {detail}` |
| `said/package-problems/bad_provider` | en | `A bundled provider is written wrong: {detail}` | `Invalid bundled provider: {detail}` |
| `said/package-problems/tool_not_portable` | en | `the tool description {detail} puts enum on a type that is not a string, and some model providers reject the whole request` | `The tool description {detail} puts enum on a type that is not a string, and some model providers reject the whole request` |
| `said/package-problems/check_output` | en | `The package's own check printed {detail} lines that are not problems; they were skipped` | `The package's own check output {detail} lines that could not be read; they were skipped` |
| `said/package-problems/script_link_outside` | en | `This script is a link to {detail} inside the data root, outside the folders you keep things in; skipped` | `This script is a link to {detail} inside the data root, outside the folders that hold your own files; skipped` |
| `said/package-problems/script_not_made` | en | `This script was not registered: {detail}` | `Script not registered: {detail}` |
| `said/package-problems/skill_link_outside` | en | `This skill folder is a link to {detail} inside the data root, outside the folders you keep things in; skipped` | `This skill folder is a link to {detail} inside the data root, outside the folders that hold your own files; skipped` |
| `said/package-problems/script_covered` | en | `Same name as {detail}, which covers it; this one is not used` | `Covered by {detail}, which has the same name; this one is not used` |
| `said/package-problems/mcp_covered` | en | `Same name as {detail}, which covers it; this one is not used` | `Covered by {detail}, which has the same name; this one is not used` |
| `said/package-problems/skill_covered` | en | `Same name as {detail}, which covers it; this one is not used` | `Covered by {detail}, which has the same name; this one is not used` |
| `said/package-problems/persona_covered` | en | `Same name as {detail}, which covers it; this one is not used` | `Covered by {detail}, which has the same name; this one is not used` |
| `said/package-problems/preset_covered` | en | `Same name as {detail}, which covers it; this one is not used` | `Covered by {detail}, which has the same name; this one is not used` |
| `said/package-problems/slash_link_outside` | en | `This command template is a link to {detail} inside the data root, outside the folders you keep things in; skipped` | `This command template is a link to {detail} inside the data root, outside the folders that hold your own files; skipped` |
| `said/package-problems/slash_covered` | en | `Same name as {detail}, which covers it; this one is not used` | `Covered by {detail}, which has the same name; this one is not used` |
| `said/persona-problems/unknown_table` | ja | `知らない表 [{detail}]：persona.toml に書けるのは [persona]、[memory]、[appearance] だけです` | `不明な表 [{detail}]：persona.toml に書けるのは [persona]、[memory]、[appearance] だけです` |
| `said/persona-problems/not_a_table` | ja | `{detail} は表 [{detail}] として書きます` | `{detail} は表 [{detail}] でなければなりません` |
| `said/persona-problems/unknown_key` | ja | `知らないキー {detail}` | `不明なキー：{detail}` |
| `said/persona-problems/not_phrases` | ja | `{detail} には一文を書いてください` | `{detail} は文字列でなければなりません` |
| `said/persona-problems/empty_phrase` | ja | `{detail} には空でない一文を書きます` | `{detail} は空でない文字列にしてください` |
| `said/persona-problems/first_line` | ja | `最初の行は user: で始めます` | `最初の行は user: で始まる必要があります` |
| `said/preset-problems/unknown_table` | ja | `知らない表 [{detail}]：プリセットに書けるのは [preset]、[features]、[software]、[tools]、[limits] だけです` | `不明な表 [{detail}]：プリセットに書けるのは [preset]、[features]、[software]、[tools]、[limits] だけです` |
| `said/preset-problems/not_a_table` | ja | `{detail} は表 [{detail}] で書いてください` | `{detail} は表 [{detail}] でなければなりません` |
| `said/preset-problems/unknown_key` | ja | `知らないキー {detail}` | `不明なキー：{detail}` |
| `said/preset-problems/not_phrases` | ja | `{detail} には一文を書いてください` | `{detail} は文字列でなければなりません` |
| `said/preset-problems/empty_phrase` | ja | `{detail} には空でない文を書いてください` | `{detail} は空でない文字列にしてください` |
| `said/preset-problems/bad_feature` | ja | `機能 ID の書き方が正しくありません: {detail}` | `機能 ID の形式が正しくありません: {detail}` |
| `said/package-problems/unknown_table` | ja | `知らない表 [{detail}]：マニフェストに書けるのは [package]、[ui]、[process]、[builtin]、[worker]、[mascot]、[theme]、[command]、[check]、[settings]、[features]、[connection]、[depends]、[recommends]、[page] だけです` | `不明な表 [{detail}]：マニフェストに書けるのは [package]、[ui]、[process]、[builtin]、[worker]、[mascot]、[theme]、[command]、[check]、[settings]、[features]、[connection]、[depends]、[recommends]、[page] だけです` |
| `said/package-problems/not_a_table` | ja | `{detail} は表 [{detail}] で書いてください` | `{detail} は表 [{detail}] でなければなりません` |
| `said/package-problems/unknown_key` | ja | `知らないキー {detail}` | `不明なキー：{detail}` |
| `said/package-problems/two_programs` | ja | `1 つのパッケージに入れられるプログラムは 1 つだけ：{detail} のどちらか 1 つにする` | `1 つのパッケージに入れられるプログラムは 1 つだけです：{detail} のどちらか 1 つにしてください` |
| `said/package-problems/empty_phrase` | ja | `{detail} は空でない一文を書いてください` | `{detail} は空でない文字列にしてください` |
| `said/package-problems/bad_pages_dir` | ja | `{detail} はリソースディレクトリ内の相対ディレクトリで、.. や先頭の / は使えません` | `{detail} はパッケージディレクトリ内の相対ディレクトリで、.. や先頭の / は使えません` |
| `said/package-problems/command_taken` | ja | `サブコマンド {detail} はほかのパッケージが使っています。こちらは数えません` | `サブコマンド {detail} はほかのパッケージが使っているため、このパッケージでは無効です` |
| `said/package-problems/bad_feature` | ja | `機能 ID の書き方が正しくありません: {detail}` | `機能 ID の形式が正しくありません: {detail}` |
| `said/package-problems/feature_taken` | ja | `機能 {detail} はほかのパッケージが使っています。こちらは数えません` | `機能 {detail} はほかのパッケージが使っているため、このパッケージでは無効です` |
| `said/package-problems/prefix_taken` | ja | `プレフィックス {detail} はほかのパッケージが使っています。こちらは数えません` | `プレフィックス {detail} はほかのパッケージが使っているため、このパッケージでは無効です` |
| `said/package-problems/check_output` | ja | `パッケージ自身のチェックが読めない行を {detail} 行出したので、飛ばしました` | `パッケージ自身のチェックが読み取れない行を {detail} 行出力したため、スキップしました` |
| `said/package-problems/script_link_outside` | ja | `このスクリプトはデータルート内の {detail} へのリンクで、あなたがものを置くフォルダーの外なので、スキップしました` | `このスクリプトはデータルート内の {detail} へのリンクで、自分のファイルを置くフォルダーの外にあるため、スキップしました` |
| `said/package-problems/script_not_made` | ja | `このスクリプトは登録されませんでした：{detail}` | `スクリプトは登録されませんでした：{detail}` |
| `said/package-problems/skill_link_outside` | ja | `このスキルフォルダーはデータルート内の {detail} へのリンクで、あなたがものを置くフォルダーの外なので、スキップしました` | `このスキルフォルダーはデータルート内の {detail} へのリンクで、自分のファイルを置くフォルダーの外にあるため、スキップしました` |
| `said/package-problems/slash_link_outside` | ja | `このコマンドテンプレートはデータルート内の {detail} へのリンクで、あなたがものを置くフォルダーの外なので、スキップしました` | `このコマンドテンプレートはデータルート内の {detail} へのリンクで、自分のファイルを置くフォルダーの外にあるため、スキップしました` |

#### 配置的报错和参考文件（`said/config`）

共 80 句。

| 键 | 语言 | 原来 | 现在 |
|---|---|---|---|
| `said/config/reference-header` | zh | `Miyu 的全部配置项和默认值。这份文件是自动生成的，修改它不会生效。` | `Miyu 的全部配置项和默认值。这个文件是自动生成的，修改它不会生效。` |
| `said/config/reference-where` | zh | `配置写在数据根的 config.toml。` | `配置文件是数据根下的 config.toml。` |
| `said/config/facts` | zh | `能写：{values}。只能写在{layers}里。{applies}。` | `取值：{values}。只能在{layers}中设置。{applies}。` |
| `said/config/secrets-header` | zh | `Miyu 的密钥：只经 Miyu 写入、替换、删除。不要把这份文件发给其他人。` | `Miyu 的密钥文件：只能通过 Miyu 写入、替换和删除。不要把这个文件发给其他人。` |
| `said/config/unknown-secret` | zh | `{key} 引用的密钥 {name} 还没有设置` | `{key} 引用的密钥 {name} 未设置` |
| `said/config/env-not-set` | zh | `{key} 引用的环境变量 {name} 在核心启动时没有设置` | `{key} 引用的环境变量 {name} 在核心启动时未设置` |
| `said/config/no-provider` | zh | `{key} 指向的供应商 {name} 没有配置` | `{key} 指向的供应商 {name} 未配置` |
| `said/config/no-pool` | zh | `{key} 指向的池 {name} 没有配置` | `{key} 指向的池 {name} 未配置` |
| `said/config/unknown-effort` | zh | `{key} 写的 {name} 不是这个模型现在支持的一档：请求按没有填写发送` | `{key} 的值 {name} 不是当前模型支持的档位，发送请求时按未设置处理` |
| `said/config/temperature-unsupported` | zh | `{key}：这个模型不接受温度参数，请求按没有填写发送` | `{key}：当前模型不支持温度参数，发送请求时按未设置处理` |
| `said/config/temperature-too-high` | zh | `{key} 写的 {got} 超过了这个模型的上限 {max}：请求按没有填写发送` | `{key} 的值 {got} 超过了当前模型的上限 {max}，发送请求时按未设置处理` |
| `said/config/bad-secret-name` | zh | `{key} 不能当密钥的名字：小写字母开头，只有小写字母、数字、-、_，最长 64 个字符` | `{key} 不是有效的密钥名：必须以小写字母开头，只能包含小写字母、数字、- 和 _，最长 64 个字符` |
| `said/config/bad-secret-value` | zh | `{key} 的值要写成带引号的字，不能是空的` | `{key} 的值必须是带引号的非空字符串` |
| `said/config/trust-header` | zh | `Miyu 记着的项目配置的信任：哪个仓库、哪一份内容、信不信任。` | `Miyu 记录的项目配置信任情况：仓库、配置内容和是否信任。` |
| `said/config/unreadable` | zh | `读不了这份文件：{why}` | `无法读取这个文件：{why}` |
| `said/config/too-big` | zh | `这份文件超过 1 MiB，不读` | `这个文件超过 1 MiB，未读取` |
| `said/config/settings-left` | zh | `有 {count} 项是已经卸载的 {package} 留下的设置：重装 {package} 后它们会原样生效；不需要的话用 miyu pkg remove {package} --purge 删掉` | `有 {count} 项设置来自已卸载的 {package}：重新安装 {package} 后这些设置会继续生效；如果不需要，可以运行 miyu pkg remove {package} --purge 删除` |
| `said/config/not-utf8` | zh | `这份文件不是 UTF-8` | `这个文件不是 UTF-8 编码` |
| `said/config/syntax` | zh | `TOML 写法不对：{why}` | `TOML 语法错误：{why}` |
| `said/config/unknown-key` | zh | `没有 {key} 这一项。是不是想写 {suggest}？` | `配置项 {key} 不存在。是否应为 {suggest}？` |
| `said/config/unknown-key-plain` | zh | `没有 {key} 这一项` | `配置项 {key} 不存在` |
| `said/config/wrong-type` | zh | `{key} 要写 {expected}，写的是 {got}` | `{key} 必须是 {expected}，当前为 {got}` |
| `said/config/not-an-option` | zh | `{key} 只能是 {options}，写的是 {got}` | `{key} 只能是 {options}，当前为 {got}` |
| `said/config/wrong-layer` | zh | `{key} 只能写在{layers}里，写在{layer}里不生效` | `{key} 只能在{layers}中设置，在{layer}中设置不会生效` |
| `said/config/not-tightening` | zh | `项目配置只能让限制更严。{key} 现在是 {current}，这里写的 {got} 更宽，不生效` | `项目配置只能让限制更严格。{key} 当前为 {current}，这里的 {got} 更宽松，不会生效` |
| `said/config/bad-dir-path` | zh | `{got} 写法不对：要写成 ~/… 或绝对路径` | `{got} 格式不正确：必须是 ~/… 或绝对路径` |
| `said/config/source-incomplete` | zh | `软件源 {name} 要同时写 url 和 key 才能用` | `软件源 {name} 必须同时设置 url 和 key` |
| `said/config/source-key` | zh | `软件源 {name} 的 key 无法读取：要写 base64 的 Ed25519 公钥` | `软件源 {name} 的 key 无法解析：必须是 base64 编码的 Ed25519 公钥` |
| `said/config/fix-example` | zh | `改成其中一个，例如 {example}` | `可以改为其中一个，例如 {example}` |
| `said/config/fix-write` | zh | `改成 {example}` | `可以改为 {example}` |
| `said/config/fix-move` | zh | `挪到{layers}里去` | `改为在{layers}中设置` |
| `said/config/kept` | zh | `已忽略此行` | `已忽略这一行` |
| `said/config/using-nothing` | zh | `已忽略此文件` | `已忽略这个文件` |
| `said/config/got/text` | zh | `{count} 个字符的字` | `{count} 个字符的字符串` |
| `said/config/got/text/one` | zh | `{count} 个字符的字` | `{count} 个字符的字符串` |
| `said/config/expected/text` | zh | `最多 {max} 个字的文字` | `不超过 {max} 个字符的文本` |
| `said/config/expected/lines` | zh | `最多 {max} 个字的文字，可以分行` | `不超过 {max} 个字符的文本，可以换行` |
| `said/config/expected/english` | zh | `最多 {max} 个字的一行英文` | `不超过 {max} 个字符的单行英文` |
| `said/config/expected/duration` | zh | `{min} 到 {max} 之间的时长，写成 30s、10m、1h 这样` | `{min} 到 {max} 之间的时长，格式如 30s、10m、1h` |
| `said/config/expected/name` | zh | `小写字母开头的名字，只有小写字母、数字、-、_，最长 64 个字符` | `以小写字母开头、只包含小写字母、数字、- 和 _ 的名字，最长 64 个字符` |
| `said/config/unknown-persona` | zh | `{key} 写的人格 {name} 现在不存在：创建这个人格之后就可以使用` | `{key} 引用的人格 {name} 不存在，创建这个人格后就可以使用` |
| `said/config/unknown-knowledge` | zh | `{key} 写的知识库 {name} 现在不存在：创建这个知识库之后就可以使用` | `{key} 引用的知识库 {name} 不存在，创建这个知识库后就可以使用` |
| `said/config/unknown-preset` | zh | `{key} 写的预设 {name} 现在不存在：创建这个预设之后就可以使用` | `{key} 引用的预设 {name} 不存在，创建这个预设后就可以使用` |
| `said/config/unknown-driver` | zh | `{key} 写的模型驱动 {name} 现在没有：安装提供它的软件包并批准之后才能使用，在那之前这家供应商不可用` | `{key} 引用的模型驱动 {name} 不存在：需要安装提供它的软件包并批准，在这之前这个供应商无法使用` |
| `said/config/missing-field` | zh | `{key} 没有填写：这个字段没有默认值，必须填写` | `缺少 {key}：这个字段没有默认值，必须填写` |
| `said/config/expected/id` | zh | `小写字母开头，只有小写字母、数字、-、_，最长 32 个字符` | `必须以小写字母开头，只能包含小写字母、数字、- 和 _，最长 32 个字符` |
| `said/config/expected/model-name` | zh | `1 到 128 个字节，没有控制字符` | `必须为 1 到 128 个字节，不能包含控制字符` |
| `said/config/expected/long-name` | zh | `1 到 128 个字符，没有 . 和控制字符` | `必须为 1 到 128 个字符，不能包含 . 和控制字符` |
| `said/config/bad-format` | zh | `{key} 要写 {expected}，写的是 {got}` | `{key} 必须是 {expected}，当前为 {got}` |
| `said/config/out-of-range` | zh | `{key} 要在 {min} 到 {max} 之间，写的是 {got}` | `{key} 必须在 {min} 到 {max} 之间，当前为 {got}` |
| `said/config/bad-segment` | zh | `{key} 里的 {name} 不能当名字：{expected}` | `{key} 中的 {name} 不是有效的名称：{expected}` |
| `said/config/reference-where` | en | `Write the config in config.toml in the data root.` | `The config file is config.toml in the data root.` |
| `said/config/facts` | en | `Allowed: {values}. Only in {layers}. {applies}.` | `Allowed values: {values}. Can be set only in {layers}. {applies}.` |
| `said/config/option-cooling` | en | `Resting until {time}` | `Cooling down until {time}` |
| `said/config/unknown-effort` | en | `{key} is {name}, which is not one of this model's levels now. Requests go out as if it were not set` | `{key} is {name}, which this model does not support; requests are sent as if it were not set` |
| `said/config/temperature-unsupported` | en | `{key}: this model does not take a temperature. Requests go out as if it were not set` | `{key}: this model does not support temperature; requests are sent as if it were not set` |
| `said/config/temperature-too-high` | en | `{key} is {got}, above this model's limit of {max}. Requests go out as if it were not set` | `{key} is {got}, above this model's limit of {max}; requests are sent as if it were not set` |
| `said/config/bad-secret-name` | en | `{key} is not a valid secret name: start with a lowercase letter and use only lowercase letters, digits, - and _, up to 64 characters` | `{key} is not a valid secret name: it must start with a lowercase letter and contain only lowercase letters, digits, - and _, up to 64 characters` |
| `said/config/bad-secret-value` | en | `The value of {key} must be quoted text, not empty` | `The value of {key} must be a non-empty quoted string` |
| `said/config/too-big` | en | `The file is over 1 MiB and is not read` | `The file is over 1 MiB and was not read` |
| `said/config/settings-left` | en | `{count} settings were left by the removed package {package}: reinstalling {package} uses them as they are; to delete them, run miyu pkg remove {package} --purge` | `{count} settings were left by the removed package {package}: they take effect again if {package} is reinstalled; to delete them, run miyu pkg remove {package} --purge` |
| `said/config/settings-left/one` | en | `{count} setting was left by the removed package {package}: reinstalling {package} uses it as it is; to delete it, run miyu pkg remove {package} --purge` | `{count} setting was left by the removed package {package}: it takes effect again if {package} is reinstalled; to delete it, run miyu pkg remove {package} --purge` |
| `said/config/unknown-key` | en | `There is no {key}. Did you mean {suggest}?` | `Unknown setting: {key}. Did you mean {suggest}?` |
| `said/config/unknown-key-plain` | en | `There is no {key}` | `Unknown setting: {key}` |
| `said/config/wrong-type` | en | `{key} needs {expected}, not {got}` | `{key} must be {expected}, not {got}` |
| `said/config/wrong-layer` | en | `{key} belongs in {layers}. It does not count in {layer}` | `{key} can only be set in {layers}; it has no effect in {layer}` |
| `said/config/not-tightening` | en | `A project config can only make limits stricter. {key} is {current}, and {got} here is looser, so it does not count` | `A project config can only make limits stricter. {key} is {current}, and {got} here is looser, so it has no effect` |
| `said/config/bad-dir-path` | en | `{got} isn't written right: use ~/… or an absolute path` | `{got} is not a valid path: use ~/… or an absolute path` |
| `said/config/source-reserved` | en | `official is the name of the official package source; other sources can't use it` | `official is the name of the official package source and cannot be used by other sources` |
| `said/config/source-incomplete` | en | `package source {name} needs both url and key` | `Package source {name} needs both url and key` |
| `said/config/source-key` | en | `the key of package source {name} isn't readable: write a base64 Ed25519 public key` | `The key of package source {name} cannot be read: it must be a base64 Ed25519 public key` |
| `said/config/missing-field` | en | `{key} is missing: this field has no default and must be written` | `{key} is missing: this field has no default and must be set` |
| `said/config/unknown-field` | en | `{key} is not a field of this entry` | `{key} is not a valid field of this entry` |
| `said/config/expected/id` | en | `start with a lowercase letter and use only lowercase letters, digits, - and _, up to 32 characters` | `it must start with a lowercase letter and use only lowercase letters, digits, - and _, up to 32 characters` |
| `said/config/expected/model-name` | en | `1 to 128 bytes without control characters` | `it must be 1 to 128 bytes without control characters` |
| `said/config/expected/long-name` | en | `1 to 128 characters without . or control characters` | `it must be 1 to 128 characters without . or control characters` |
| `said/config/bad-format` | en | `{key} needs {expected}, not {got}` | `{key} must be {expected}, not {got}` |
| `said/config/bad-segment` | en | `{name} in {key} cannot be a name: {expected}` | `{name} in {key} is not a valid name: {expected}` |
| `said/config/syntax` | ja | `TOML の書き方が正しくありません：{why}` | `TOML の構文エラー：{why}` |
| `said/config/applies/next_turn` | ja | `次のターンから反映されます` | `次のターンから反映` |

#### 启动和连接核心时的报错（`said/start`）

共 23 句。

| 键 | 语言 | 原来 | 现在 |
|---|---|---|---|
| `said/start/data_root_not_ours` | zh | `{path} 里有其他内容，无法确认它是 Miyu 的数据根，没有修改它。如果它是 0.7 版的数据目录，先 mv {path} {path}-0.7，再 miyu pkg install migrate、miyu migrate {path}-0.7；如果不是，把 MIYU_HOME 设成一个空目录` | `{path} 里有其他内容，无法确认它是 Miyu 的数据根，没有修改它。如果它是 0.7 版的数据目录，先运行 mv {path} {path}-0.7，再运行 miyu pkg install migrate、miyu migrate {path}-0.7；如果不是，把 MIYU_HOME 设成一个空目录` |
| `said/start/data_root_old_layout` | zh | `{path} 是旧版 Miyu 的数据根，新版不能使用，没有修改它。把 MIYU_HOME 设成一个空目录` | `{path} 是旧版 Miyu 的数据根，当前版本无法使用，没有修改它。把 MIYU_HOME 设成一个空目录` |
| `said/start/data_root_other_layout` | zh | `{path} 的布局不是这一版的，没有修改它` | `{path} 的目录布局与当前版本不符，没有修改它` |
| `said/start/miyu_home_relative` | zh | `MIYU_HOME 要写绝对路径，写的是 {path}` | `MIYU_HOME 必须是绝对路径，当前为 {path}` |
| `said/start/not_running` | zh | `核心没有在运行` | `核心未运行` |
| `said/start/not_private_socket` | zh | `{path} 不属于当前用户，或者其他用户也能访问：不连接` | `{path} 不属于当前用户，或者其他用户也能访问，因此没有连接` |
| `said/start/not_private_pipe` | zh | `命名管道 {path} 另一头不是当前用户的进程：不连接` | `命名管道 {path} 的另一端不是当前用户的进程，因此没有连接` |
| `said/start/silent` | zh | `核心没有启动，也没有给出原因：查看数据根的 {log}` | `核心没有启动，也没有给出原因，可以查看数据根中的 {log}` |
| `said/start/lock_failed` | zh | `核心无法启动：拿不到数据根的锁：{reason}` | `核心无法启动：无法获取数据根的锁：{reason}` |
| `said/start/resources_relative` | zh | `核心无法启动：MIYU_RESOURCES 要写绝对路径，现在是 {reason}` | `核心无法启动：MIYU_RESOURCES 必须是绝对路径，当前为 {reason}` |
| `said/start/resources_not_found` | zh | `核心无法启动：找不到资源目录，找过 {reason}。开发时设 MIYU_RESOURCES 指到源码树的 resources/` | `核心无法启动：找不到资源目录，已查找 {reason}。开发时可以把 MIYU_RESOURCES 设为源码树中的 resources/` |
| `said/start/resources_unknown` | zh | `核心无法启动：找不到资源目录：不知道程序在哪。开发时设 MIYU_RESOURCES 指到源码树的 resources/` | `核心无法启动：无法确定程序的位置，找不到资源目录。开发时可以把 MIYU_RESOURCES 设为源码树中的 resources/` |
| `said/start/socket_too_long` | zh | `核心无法启动：套接字的路径 {reason} 太长，临时目录下也放不下` | `核心无法启动：套接字路径 {reason} 过长，改放到临时目录下也超出长度限制` |
| `said/start/socket_not_private` | zh | `核心无法启动：{reason} 不属于当前用户，或者其他用户也能访问：套接字不放在这里` | `核心无法启动：{reason} 不属于当前用户，或者其他用户也能访问，因此不在这里创建套接字` |
| `said/start/socket_occupied` | zh | `核心无法启动：{reason} 上有其他内容：不是套接字的文件，或者其他程序正在监听。没有修改它` | `核心无法启动：{reason} 已被占用，可能是一个不是套接字的文件，或者其他程序正在监听。没有修改它` |
| `said/start/not_private_socket` | en | `{path} isn't yours or others can enter it, so it wasn't connected to` | `{path} is not owned by the current user or is accessible to other users, so it was not connected to` |
| `said/start/not_private_pipe` | en | `The other end of named pipe {path} isn't your process, so it wasn't connected to` | `The other end of named pipe {path} is not a process of the current user, so it was not connected to` |
| `said/start/lock_failed` | en | `The core couldn't start: cannot take the data root's lock: {reason}` | `The core couldn't start: cannot acquire the data root lock: {reason}` |
| `said/start/socket_not_private` | en | `The core couldn't start: {reason} isn't yours or others can enter it, so the socket isn't put there` | `The core couldn't start: {reason} is not owned by the current user or is accessible to other users, so the socket was not created there` |
| `said/start/miyu_home_relative` | ja | `MIYU_HOME は絶対パスで書いてください（{path} と書かれています）` | `MIYU_HOME は絶対パスである必要があります（現在の値：{path}）` |
| `said/start/not_private_socket` | ja | `{path} は自分のものではないか、ほかの人も入れるので、接続しませんでした` | `{path} は現在のユーザーのものではないか、他のユーザーもアクセスできるため、接続しませんでした` |
| `said/start/resources_relative` | ja | `コアを起動できません：MIYU_RESOURCES は絶対パスで書いてください（{reason} と書かれています）` | `コアを起動できません：MIYU_RESOURCES は絶対パスである必要があります（現在の値：{reason}）` |
| `said/start/socket_not_private` | ja | `コアを起動できません：{reason} は自分のものではないか、ほかの人も入れるので、ソケットを置きません` | `コアを起動できません：{reason} は現在のユーザーのものではないか、他のユーザーもアクセスできるため、ソケットを作成しません` |

#### 会话导出（`said/export`）

共 40 句。

| 键 | 语言 | 原来 | 现在 |
|---|---|---|---|
| `said/export/brief` | zh | `交代` | `任务说明` |
| `said/export/state/denied` | zh | `被拒` | `已拒绝` |
| `said/export/state/cancelled` | zh | `打断了` | `已中断` |
| `said/export/state/skipped` | zh | `跳过了` | `已跳过` |
| `said/export/compacted` | zh | `上下文压缩了` | `上下文已压缩` |
| `said/export/cleared` | zh | `上下文清空了` | `上下文已清空` |
| `said/export/paused` | zh | `自动压缩暂停了` | `自动压缩已暂停` |
| `said/export/job/done` | zh | `后台命令运行完了：{title}` | `后台命令已完成：{title}` |
| `said/export/job/failed` | zh | `后台命令失败了：{title}` | `后台命令失败：{title}` |
| `said/export/job/stopped` | zh | `后台命令停了：{title}` | `后台命令已停止：{title}` |
| `said/export/agent/stopped` | zh | `子代理没做完就停了` | `子代理未完成就已停止` |
| `said/export/answered` | zh | `回答了提问` | `已回答提问` |
| `said/export/model` | zh | `换了模型：{model}` | `模型已切换为 {model}` |
| `said/export/persona` | zh | `换了人格：{to}` | `人格已切换为 {to}` |
| `said/export/preset` | zh | `换了预设：{to}` | `预设已切换为 {to}` |
| `said/export/workspace` | zh | `工作目录换成了 {cwd}` | `工作目录已切换为 {cwd}` |
| `said/export/end/error` | zh | `这一轮出错了：{why}` | `本轮出错：{why}` |
| `said/export/end/interrupted` | zh | `这一轮被打断了` | `本轮已中断` |
| `said/export/end/step-limit` | zh | `这一轮走到了步数上限` | `本轮已达到步数上限` |
| `said/export/end/aborted` | zh | `核心停了，这一轮没走完` | `核心已停止，本轮未完成` |
| `said/export/shell/running` | zh | `还在运行` | `运行中` |
| `said/export/shell/timed-out` | zh | `到时被停了` | `超时已停止` |
| `said/export/shell/interrupted` | zh | `被打断了` | `已中断` |
| `said/export/shell/aborted` | zh | `核心停了，没有运行完` | `核心已停止，命令未完成` |
| `said/export/omitted` | zh | `前面省掉了 {count} 轮` | `已省略前 {count} 轮` |
| `said/export/approval` | zh | `要确认（{access}）：{decision}` | `权限确认（{access}）：{decision}` |
| `said/export/approval/nested` | zh | `#{n} {tool} 要确认（{access}）：{decision}` | `#{n} {tool} 的权限确认（{access}）：{decision}` |
| `said/export/access/write` | zh | `写文件` | `写入文件` |
| `said/export/access/outbound` | zh | `对外发消息` | `对外发送消息` |
| `said/export/access/venue` | zh | `场所里的操作` | `场所内的操作` |
| `said/export/decision/once` | zh | `允许这一次` | `仅允许本次` |
| `said/export/decision/session` | zh | `本会话都允许` | `在本会话内允许` |
| `said/export/decision/workspace` | zh | `这个工作区以后都允许` | `在这个工作区内始终允许` |
| `said/export/decision/none` | zh | `没等到决定` | `未作出决定` |
| `said/export/state/needs-approval` | zh | `要确认` | `待确认` |
| `said/export/state/running` | zh | `还在运行` | `运行中` |
| `said/export/nested` | zh | `脚本里调了 {count} 个工具` | `脚本调用了 {count} 个工具` |
| `said/export/answered/line` | zh | `回答了提问：{answers}` | `已回答提问：{answers}` |
| `said/export/end/aborted` | ja | `Miyu が停止し、このターンは完了しませんでした` | `コアが停止し、このターンは完了しませんでした` |
| `said/export/shell/aborted` | ja | `Miyu が停止し、完了しませんでした` | `コアが停止し、完了しませんでした` |
