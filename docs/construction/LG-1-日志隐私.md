## 施工单 LG-1：运行日志不漏内容

状态：上在做；下排在上后面（主会话 2026-10-22 审过）。项目主人 2026-10-22 定：运行日志里正文、工具参数、路径不进日志，只记编号、长度、类别。设计 06 U2「管理员看不到成员的会话」要靠它：运行日志管理员看得到（`state/logs/core.log`，`log.level` 只在系统层，管理员能开到 `debug`）。地基排期 10 号；设计 06 第八再补里写作 L-1，那个编号已经是「工具登记带 `need_load`」，改叫 LG-1（同设计 28 运行日志的 LG 前缀）。

### 目的

规矩早有（设计 28 LG3、`miyu-log` 的文档、蓝图 `log.md` 第七节），没有门禁守着：照 origin/main 59f1c40f 盘了 600 处日志，直接写对话正文的没有，漏的都在旁边：路径（会话的工作目录、人的仓库、附件、知识库文档、技能、包文件、脚本交的图）、会话里来的网址、外部身份（QQ 号）、错误原话（`JoinError` 带 panic 的内容、会话日志读坏了的带目录和那一行的字、扩展工具的 `detail` 读不成的原因转述了结果）、人起的名字（记忆的 `room`、人格、预设、知识库、密钥）。加一道门禁扫，先改要紧的，剩下的列单子，下清空。

### 写什么、不写什么（主会话 2026-10-22 定）

| 可以写 | 不写 |
|---|---|
| 编号：会话（短号）、调用、任务、blob、包的编号；账号的编号 | 路径：一律不写。数据根里的写成哪一层、哪一类（`layer=`、`kind=`），不写账号目录 |
| 数、长度、用时、状态码；原因码、种类、枚举的名字 | 网址：会话里来的不写，主机名也不写；模型供应商这种管理员配的只写主机 |
| 工具名、MCP 服务名（和它交的提示词名）、包名：装上的东西照写 | 工具的参数、结果、转述结果的原因 |
| 错误的种类（`io::ErrorKind`、错误的枚举名） | 外部身份；人起的名字（人格、预设名不写，`room` 写种类加短号）；错误原话里带路径、带输入的、panic 的内容（只写 `panic=true`） |

### 门禁（`xtask` 多一项「日志」，`cargo xtask logs` 单跑）

逐个包扫 `src/`（测试不扫）的 `tracing::{trace,debug,info,warn,error}!`、`*_span!`（span 的第一个字符串是名字）：格名不许是路径、正文、名字这一类；值里不许 `.display()`、`to_string_lossy()`、`as_os_str()`、`format!(`；消息不许插值；「panicked」「task failed」的不许带 `error`，`match` 里 `Ok(Ok(…))`、`Ok(Err(…))` 后面那支 `Err(e)` 记了 `%e`、`?e` 的也不许（等后台任务出的错）。写明了为什么可以的在 `ALLOWED`、`FIXED_MESSAGES`（插的是写死的字），还没改的在 `PENDING`。

### 上：做什么

1. 门禁，接进 `cargo xtask check`（九项）。
2. 26 处 `JoinError`：`error = %error` 改成 `panic = error.is_panic()`（原来查出 23 处，加了 `match` 那一条又找出 3 处：`knowledge failed`、`memory failed`、`link image not stored`）。
3. 要紧的改掉：`files index failed` 不写目录、不写原话；`project trust` 不写仓库路径；`kept attachment path not usable` 写 blob 编号；`knowledge document skipped` 不写文档路径、抽字程序说的（`kb` 留给下）；`package file not unpacked` 不写文件；`skill skipped` 写 `kind`（`covered`、`outside`、`other`）；`script image dropped` 不写文件名，原因只写写死的几句；`link preview failed` 不写主机，`link site fallback` 写 `api`；`binding to an unknown account ignored` 不写外部身份；`mcp not started` 不写命令路径、原话只写种类；`tool detail dropped` 不写原因。
4. 会话日志的错（`miyu_store::log::OpenError`）照 Display 写的那一句改成不带目录、不带怎么坏的：`no session log`、`<段的文件名> line <行> is broken`；套着它的 `LoadError`、`ForkError`、`Backlog::read` 跟着变。
5. 蓝图 `log.md` 第七节第 6 条、第八节的表，`protocol.md`、`net.md`、`knowledge.md`、`config.md`、`mcp.md`、`core.md`、`store.md`、`protocol/undo.md` 里的那几行；作者文档 `scripts-skills.md` 中英两份。

### 上：验收

- `xtask/src/logs/tests.rs`：编号、数、种类过；不许的格名、值、插值、panic 原话各报；span 的名字不算消息；`JoinError` 那一支认得出、`panic = …` 的过、普通的 `Err(e)` 不报；字符串里的 `//`、括号、宏名、测试模块不误认；`ALLOWED`、`FIXED_MESSAGES` 都写了为什么。
- `miyu-store` 的 `a_broken_log_says_where_in_english`（目录带账号、原因带对话的字，都不在那一句里）、没有日志的那一句；`miyu-session` 的 `loading_says_it_in_english`。
- 断言日志格的测试跟着改（`project trust` 这些）。变异全抓住；`cargo xtask check` 九项、Windows 版 clippy；碰路径、子进程：推 step 分支过三台。

### 下：做什么

`PENDING` 清空、门禁不再认它：记忆的 `room` 写种类加短号；人格、预设、知识库、密钥的名字写短编号或者不写；数据根里的 `file=`、`dir=`、`root=`、`socket=`、`helper=` 写成 `layer=`、`kind=`；配置写错的原文、读不懂的值不写；软件源只写主机。套着带路径的错误的（读人格、预设、配置那几处的 `error`）只写种类。

### 不做什么

- 不改日志的格式、级别、轮换；不改账号日志（`trust.changed` 这些是给本人看的，照旧带路径）。
- 门禁照文本找，防手滑，不防存心绕过；不追错误类型里的每一个 `Display`（下把「套着带路径的错误」那几处收掉）。
- 成员自己装的脚本名以后算成员的（Y-8），那时再收。
