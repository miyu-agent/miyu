## BS-1 中上：装卸包不上工具面，通道多一种 `op: "pkg"`

状态：主会话做。设计 44（基础系统拆包）第三节第 4 条，项目主人 2026-10-26 定：装卸包不再是她的工具，会话里她跑 `miyu pkg`，经 `miyu call` 的通道带着会话身份，照原来的权限表问人。这一步做核心这一头；命令行那一头（会话里的 `miyu pkg` 换成调用、不认 `--yes`）和写技能的说明是 BS-1 中下。

### 一、做了什么

1. **工具声明**：`Tool` 多 `command_only`（默认不是），出厂的 `packages`、`package` 是。核心照声明认，不看名字。
2. **快照和策略**：`ToolEntry` 多 `command_only`（组工具面时照目录写，没有的不写：以前造的会话照旧带着这两件，字节不变）；拆成组装器的工具面时跳过它（tools 数组里没有），内核的规则照样有、带着 `ToolRule::command_only`。
3. **内核**：
   - 她直接调的当不认识（`core/tool-results/unknown`），不交给执行器。
   - 嵌套调用多一格 `NestedInput::Call::command`：`miyu call` 调的（`false`）当不认识；核心自带的命令调的（`true`）只认不上工具面的那几件，别的答「不准」；认了的不查访问类别那道硬规矩、不查 `clicall`，照常过链、照常问人、只读照常拒。
   - `Session::has_command_tools`：执行器照它开通道；`clicall_tools` 不列不上工具面的。
4. **执行器**：直接调的跑命令的工具，脚本里能调的、核心自带的命令能调的有一样就开通道（`Dispatch::commands`）；端口多 `NestedPort::command`（默认当不认识）。嵌套调用判出要问人的，工具要看一眼的照直接调用先看（`Tools::previewed`）：会话里 `miyu pkg` 装包，卡片照旧是看一眼的那一份。
5. **通道**：请求的 `op` 多一种 `pkg`，走端口的 `command`；这个会话没有它们（`packages` 包关着）的答退出码 2、`software/basesystem/shell/call/pkg-off.txt`，不列 `miyu call` 能调的。
6. **字**：`packages.json` 去掉 `"clicall": true`（`miyu call` 调不了它，那一段的名单里也不再有）；登记簿（设计 26 第十节）`packages.json`、`package.json` 改成不发给模型、`pkg-off.txt` 新登记。

### 二、不做什么

- 命令行那一头（`MIYU_CALL_SOCK` 在的走通道、子命令换成调用、会话里不认 `--yes`、`files` 这些说用不了）和 `authoring` 里写上 `miyu pkg`，是 BS-1 中下。
- `packages` 包关着、通道没开的会话里，`miyu pkg` 照旧走协议、借本人的身份：同设计 39 MC10、MC11 记着的那一条（人批过在沙盒外跑的命令读得到本机令牌），设计 44 不加新口子；BS-1 中下照设计说明。

### 三、怎么验

1. 内核：`scenario/nested_command.rs`（`miyu call` 调它当不认识、`miyu pkg` 调别的不准、调它照常过链和跑；她直接调当不认识、不交给执行器；`has_command_tools`、`clicall_tools` 不列它）。
2. 策略：`tools/tests.rs` 的 `a_tool_off_the_face_is_ruled_but_not_sent`（写 `"command_only":true`、以前的快照字节不变、规则有 tools 数组没有）。
3. 通道：`reply/tests.rs`（关着的说用不了、收下的照平常写）、`unix/tests.rs` 的 `miyu_pkg_goes_the_command_way`（真套接字上 `op: "pkg"` 走 `command`）；`clicall/tests.rs` 的 `tools_off_the_face_are_left_out`。
4. 端点：`package_tool*`、`packages_tool` 改成她跑一条命令、命令里的 `miyu pkg` 经通道调（`support/pkg.rs`，假的 `shell` 照参数调几次）：工作区那一级问人带着看一眼的那一份和序号、完全放开不问、成员装卸开都拒、预设照开；`miyu call` 调 `packages` 当不认识；关了脚本调工具、开着 `packages` 的会话通道照开。
5. 跟着改：出厂工具面少两件（`read.rs`、`sessions.rs`），脚本里能调的出厂工具八件（`miyu-core` 的 `tools.rs`）。
6. 手写变异 16 个全抓到：`miyu call` 能调到它、`miyu pkg` 能调别的、`miyu pkg` 照查类别硬规矩、她直接调能调到、不开通道（内核、执行器两处）、`clicall_tools` 列它（内核、`shell` 那一段两处）、tools 数组不跳过、规则不带 `command_only`、组工具面不写、`op: "pkg"` 不认、嵌套问人不先看一眼、端口走 `call`、`package` 上工具面、关着的不说用不了。
7. 整个工作区测试、本机门禁。
