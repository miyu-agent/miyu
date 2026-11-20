## XG-1下 扩展工具声明跑命令、能力 tools.commands、通道挪到共用层

状态：施工完，待 core1 审（2026-11-06）。设计 `63-扩展工具的效果和声明.md`（core1 2026-11-05 定五条：1A、2A、3A、4A、5A），这一步是
5A 拆出来的下半：`runs_commands`、能力 `tools.commands`、`tool.call` 的 `channel`、说明后面接 `miyu call` 那一段、通道挪出来和两种
认人、人跑命令只认内置。顺手按 core1 2026-11-06 的话给设计 63 第四节第 3 条补半句「回应 `error` 为真时丢的 `todo.written` 不算写错、
不记」。蓝图 `providers.md`（规格一格、`tool.call` 的 `channel`、「会跑命令的」、出错、守着它的、还没有的）、`extensions.md`（能力、
还没有的）、`tools/shell.md`（「在哪」、脚本调工具第 5、7、8、10 条）、`tools/interface.md`（`serves_person`）、`session/tools.md`
第 7 条、`protocol.md`（`shell_unavailable`、`bad_tool`、`needs_approval`）、`ipc.md`（`miyu-pipe` 问父进程）、`cli/call.md`。

### 目的

扩展换掉 `shell`（在容器里、远程机器上跑命令）以后，她在它跑的命令里用不了 `miyu call`：通道只有内置 `shell` 开，说明后面那一段
也只接在 `shell` 后面。设计 00 第四节「内置功能不开后门」：扩展照内置 `shell` 的几处接上，一处不按名字；开的这条路（扩展的子进程
这一次调用期间以会话身份调她的工具）是扩展原来没有的，要人批一项新能力（3A）。

### 做了什么

- **能力 `tools.commands`**（`miyu-config/src/package/capability.rs`）：表里第二项，十八项。名字、一句说明进 `resources/core/human`
  三份（`extensions/capabilities/tools.commands`、`…/summary`），装包、打开时照能力列给人看（设计 61）。作者文档 `manifest`（中英）的
  能力表加一行（`every_capability_is_in_the_authors_docs` 守着）。
- **规格的 `runs_commands`**（`miyu-endpoint/src/provide.rs`）：布尔，不写是假，别的 `bad_tool`（`problem` 是 `runs_commands`）。
  写了真的、包没批 `tools.commands` 的（清单没声明的也是）整份不收，`needs_approval`，`data.capabilities` 是 `["tools.commands"]`；
  查在 `register` 里，`provide` 和读登记缓存都过（缓存照它登记不上的记 `WARN`，等扩展自己登记）。不写的规格、缓存逐字节不变。
- **`RemoteTool`**（`provide/remote.rs`、新的 `provide/commands.rs`）：`runs_commands` 照规格交；`clicall_section` 交核心的那一段
  （`miyu_call::Section`，例子照这台机器挑）；`run` 拿到了端口（`Call::nested`）、知道扩展进程号的，开一个认「这个扩展进程的子孙」的
  端点，`tool.call` 带 `channel: {sock, token, bin}`。端点跟着这个 future：答了、超时、被掐都丢掉、关上。
- **扩展进程号**（`provide.rs` 的 `Provided`）：提供者表多记一格进程号，`provide` 那一刻照扩展的状态取（`Starting`、`Running` 的
  `pid`，拉起后就记下了）。`extensions/supervise.rs` 没动。
- **通道挪到新的一层 `miyu-call`**（第 3 层，`01-架构.md` 第九节登记）：`miyu-basesystem/src/shell/channel/` 的 `serve`、`reply`、
  `unix`、`windows`、`other`、`leftovers` 和 `shell/section.rs` 整个挪过去（测试跟着），基础系统只留 `shell/channel.rs` 的 `prepare`
  （开端点、环境变量、沙盒规格）。认人抽成 `kin.rs` 的 `Kin`：`Command`（`shell` 的，进程组、作业对象，原样，`Binder` 记一次）、
  `Descendant`（扩展的，照父进程链认子孙：Linux 读 `/proc/<pid>/stat`，macOS `miyu_pipe::parent_process`，Windows
  `miyu_pipe::process_parents` 拍一张快照）。交给扩展的那一格 `Handed`（`serve.rs`）。运行日志的来处从 `miyu::shell` 改成
  `miyu::call`（`shell` 自己开不成的那一行照旧 `miyu::shell`）。
- **`miyu-pipe` 问父进程**（`src/parent.rs`）：Windows `process_parents`（`CreateToolhelp32Snapshot`），macOS `parent_process`
  （`proc_pidinfo` 的 `PROC_PIDTBSDINFO`）。仓库里只有这个 crate 放开 `unsafe`，两处都放在这里；macOS 上多带 `libc`（工作区里本来有）。
- **字挪成核心的**：`resources/software/basesystem/shell/call/` 整个挪到 `resources/core/call/`，字节不变（`shell`、扩展、命令行都读
  这一份）；登记簿（设计 26 第十节）的路径跟着改、`describe.txt`、`approval.txt` 两行的「进到哪」写上扩展，`prompts.md` 重新生成。
- **人跑命令只认内置**：`Tool` 多一个 `serves_person()`（默认否，`shell` 是）；`session.send {shell: true}` 的检查（端点
  `methods/session.rs`）和会话找那一件（`miyu-session/src/tools/person.rs`）都照它，只有扩展的跑命令工具的会话照旧 `shell_unavailable`。
- **测试扩展**（`src/bin/miyu-test-extension.rs`、`miyu-test-extension/run.rs`）：多一步 `hold:<路径>`；`serve` 答 `ext_run` 时照
  `channel` 起自己（`client` 模式）当 `miyu call` 调参数 `call` 写的工具，等那个路径有了再答。
- **文档**：见开头的蓝图；协议参考 `extensions`（中英）：能力表、`provide` 的 `runs_commands`、出错、`tool.call` 的 `channel` 和
  「跑命令」一节；`errors`（中英）的 `needs_approval`；作者文档 `extensions`（中英）：规格一格和「跑命令的工具」一节；设计 63 状态
  和第四节第 3 条半句；设计 44 第三节标做了；设计 24 接受的风险记通道这一条。

### 定的技术题

1. **共用的那一层是新 crate `miyu-call`**，不放 `miyu-ipc`：`miyu-ipc` 是头的门面 `miyu-client` 也依赖的，通道要 `miyu-tool`、
   `miyu-sandbox`、`miyu-kernel`，放进去头就跟着拖进一大串。也不放 `miyu-tool`：工具的接口和目录不该带套接字、管道的服务。
2. **问父进程的两处 `unsafe` 放进 `miyu-pipe`**（2026-09-27 项目主人定的「单开一个小 crate 放开 unsafe」），不在 `miyu-call` 里另放开；
   `miyu-pipe` 从此在 macOS 上也有一样东西，说明改了。Linux 读 `/proc` 不要 `unsafe`，放 `miyu-call`。
3. **扩展进程自己不算子孙**（设计写的是「子孙」）：它本来就拿着令牌，放不放它安全上没差别，照字面收紧；作者文档写明要起子进程。
4. **进程号在 `provide` 那一刻照扩展状态记**，不改 `supervise.rs`：拉起后马上记成 `Starting { pid }`，能 `provide` 的连接一定是核心
   亲手拉起的那一个；扩展重启会重新 `provide`，跟着换。不知道进程号的（测试里的假连接）不开通道，记 `DEBUG`。
5. **能力在 `register` 里查**：`provide`、读缓存一处；拒的写法照 `connection/rights.rs` 挡方法的（`data.capabilities` 一项）。
6. **扩展工具那一段的例子照这台机器的默认挑**：Windows 上 PowerShell 的，别处 `jq` → Python → 不带（扩展用哪个 shell 核心不知道）。
7. **`channel` 只给拿到端口、开成了的调用**：`bin` 建不成的不写；端点路径不是 UTF-8 的（写不进 JSON）不开这一次。
8. **认子孙在阻塞线程里做**：Windows 拍一张快照要几毫秒，不占跑异步任务的线程；链最多往上找 256 层，到 0 号、绕回来的停。
9. **整个 `call/` 一起挪**（连命令行那头编进程序的几句）：原来就说「和端点那头的放在一起」，拆两处反而对不上。
10. **Windows 不过继父进程**：快照里的父进程号可能是被重用的旧编号，链上会误认；还要令牌对得上才进得来，先不另比进程起的时刻（写在
    `kin.rs`、`miyu_pipe::process_parents` 的说明里）。

### 不做

- 开发包 `miyu-sdk`（另一个仓库）的 `runs_commands`、`Call.channel`、给子进程的环境：XG-1sdk 另做。
- 给模型看的字：没加。`core/clicall.txt`（system 里那一句）写的是「see the shell tool」，只有扩展的跑命令工具时指错了；core1 2026-11-06 定不改（前缀稳定第一），记在设计 63 第九节「已知边界」。
- Windows 父进程号被重用：现在不比进程起来的先后，靠令牌兜着（core1 2026-11-06 接受）；以后加「父进程起得比子进程早」的比较。
- 扩展起不了会话的后台任务：没有 `shell` 那种「端点交给任务表、跟着任务走」。

### 怎么验

- `crates/miyu-call`：挪过来的 26 个测试照过（端点、回答、说明那一段、留下的目录）；`kin/tests.rs` 子孙照假表找（子、孙是，自己、
  别人家的、问不出的、绕回来的、太长的不是）、`/proc` 那一格、真的子进程、`shell` 那种照组认；`unix/tests.rs`、`windows/tests.rs`
  扩展的端点答它父进程的子孙（测试进程）、拒另一个进程的，交给扩展的那一格和设进命令的一样。
- `crates/miyu-pipe/src/parent/tests.rs`（Windows、macOS）：子进程的父进程是测试进程。
- `provide/tests/declared_tests.rs`：`runs_commands` 是布尔、只有真的跑命令、人跑命令不交给它、缓存原样存。
- `tests/provide_commands.rs`：真核心拉起测试扩展，扩展的子进程经通道调 `probe` 成了、记 `tool.nested` 挂在那次调用下面；核心进程
  拿着令牌连上来退出码 2；答完连不上；没批能力的 `needs_approval`、不是布尔的 `bad_tool`；说明后面接那一段；`!命令` 只有扩展的跑命令
  工具时 `shell_unavailable`；放行出沙盒的扩展跑命令工具 `miyu check` 报 `sandbox_shell`。
- 受影响的 crate（`miyu-call`、`miyu-pipe`、`miyu-basesystem`、`miyu-endpoint`、`miyu-session`、`miyu-config`、`miyu-tool`、
  `miyu-cli`、`miyu`）测试全过；本机另用 `x86_64-pc-windows-gnu` 跑过 `miyu-call`、`miyu-pipe`、`miyu-basesystem` 的 clippy，
  `aarch64-apple-darwin` 跑过 `miyu-pipe` 的；门禁 11 项（`xg1b`）；三平台 CI。

### 变异

| 改坏 | 抓住的 |
|---|---|
| `provide` 不查能力 `tools.commands` | `provide_commands` 的 `runs_commands_needs_tools_commands_approved_and_a_boolean` |
| 通道不认令牌 | `unix/tests.rs` 的 `a_wrong_token_or_version_is_refused_without_calling` |
| 非子孙也放行 | `kin` 真子进程那条、`unix` 扩展端点那条、`provide_commands` 核心进程连上来那一步 |
| 扩展进程自己算子孙 | `kin` 假表那条、真子进程那条 |
| 问不出父进程的当是 | `kin` 假表那条 |
| `/proc` 读错一格 | `kin` 解析那条、真子进程那条、`unix` 扩展端点那条 |
| 通道调用期外还能用（端点不关） | `provide_commands` 答完连不上那一步 |
| `tool.call` 不带 `channel` | `provide_commands` 第一条 |
| 人跑命令照 `runs_commands` 认 | `provide_commands` 的 `a_person_s_command_is_not_handed_to_an_extension` |
| 扩展工具的说明不接那一段 | `provide_commands` 的 `the_miyu_call_section_follows_a_tool_that_runs_commands` |
