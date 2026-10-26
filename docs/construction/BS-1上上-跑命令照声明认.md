## BS-1 上上：跑命令照工具自己的声明认

状态：主会话做。设计 44（基础系统拆包）第三节第 3 条，项目主人 2026-10-26 指出：不装出厂的 `shell` 时，核心里不该有针对它的特判；shell 专属的放在工具里，核心只留通用的规矩。

### 一、为什么

拆包以后 `shell` 是一个能卸、能换的包。核心里原来有五处照名字 `shell` 认：
1. 前台会话去掉 `run_in_background`（`miyu-session` 的 `agents.rs`）；
2. `miyu call` 那一段用法接在哪件的说明后面（`agents/clicall.rs` 的 `describe`）；
3. system 里那一句带不带（同上，`tells`）；
4. 嵌套调用的通道开给哪件的调用（`tools.rs` 的 `nested`）；
5. 管理员放行时 `miyu check` 提醒 `sandbox_shell`（`miyu-endpoint` 的 `check/unsandboxed.rs`）。

别人的 `shell` 照名字也能拿到这些，出厂的卸了以后核心里的这几处还在。

### 二、做了什么

- `Tool` 多两个方法，默认都没有：`runs_commands()`（跑命令的工具）、`background_parameter()`（只在有后台时给的参数）。出厂的 `shell` 实现这两个：是、`run_in_background`。
- 上面五处都照这两个方法判，不看名字。工具面的 `ToolEntry` 多一格 `runs_commands`（组工具面时照目录写进来；没有的不写，以前的快照读成不是）；重拼 system 之前照这一刻的目录补上（`clicall::stamped`），以前的会话拼出来一字不差。
- 测试的假工具多 `Fake::commanding`、`commanding_with`：要「像 shell」的测试明着声明，原来照名字拿到通道的十几条测试改用它。
- 蓝图 `tools/interface.md`、`tools/shell.md`、`session/guard.md`、`policy.md`、`prompts.md`，设计 44 第三节跟着改。

### 三、不做什么

- 扩展经 `provide` 交的工具还不能声明这两样（规格里没有这两格）。以后有人要，规格加同名的两格。
- 拆包本身（十七份清单、组）在 BS-1 上下。

### 四、怎么验

1. `agents/tests.rs` 的 `the_background_only_parameter_follows_what_the_tool_declares`：声明了的那件（名字叫 `bash`）在后台关着的会话里去掉它声明的参数，名字叫 `shell` 但没声明的不动；后台开着的照旧都在。
2. `agents/clicall/tests.rs` 的 `whatever_declares_running_commands_gets_the_section_and_the_name_does_not_matter`（叫 `bash` 的声明了照样接上、要 system 那一句，叫 `not_a_shell` 没声明的不接）、`an_old_snapshot_face_is_stamped_from_the_catalog`（以前的快照补上以后和新造的一样）。
3. 两件都实现了那一段时只接在声明了跑命令的那件后面（同上一条测试里）；会话测试 `nested_calls` 的 `only_a_tool_that_runs_commands_gets_the_port`：声明了的（叫 `bash`）拿得到嵌套调用的端口，叫 `shell` 没声明的拿不到。
4. 端点 `check_unsandboxed`：放行了声明跑命令的那件照旧提醒 `sandbox_shell`。
5. 手写变异 7 个全抓到：工具面不写这一格、前台不去掉声明的参数、有没有通道不看声明、说明接到每一件后面、补的时候不补、端口开给每一件、`miyu check` 不提醒。
6. 整个工作区的测试、门禁全过。
