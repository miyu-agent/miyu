## 施工单 M-8：脚本调工具在 Windows 上实测出的三处

状态：施工中。经过：主人 2026-10-23 在 Windows 11 虚拟机里（Windows PowerShell 5.1，核心 `ae2f8cb8a^` 交叉编译）让她实测 `miyu call`，shellhook 会话转来三条；根因查过，核心主会话同一天照推荐通过、排在 X-45 补之后。设计 `39-脚本调工具.md`；图纸 `cli/call.md`、`session/tools.md`「1g」。

### 三条和根因

1. **PS 5.1 下 `--json '{"query":"niri"}'` 用不了**：5.1 把参数交给原生程序时不转义里面的双引号，Windows 的命令行解析当引号吃掉，`miyu` 收到 `{query:niri}`，报「--json takes one JSON object.」；`'{\"query\":\"niri\"}'`、`--%` 也退化成用法不对。只有走标准输入（`'{...}' | miyu call x --json -`）能过。PS 7.3 起修了，5.1 没有。给她的 PowerShell 例子写的是具名参数（`--query niri`），本身没事；但 `shell` 说明那一段、`--help` 都提到内联写法。
2. **`miyu call packages` 回「Packages cannot be looked at here.」**：bug。施工 M-2 补把 `packages` 放进脚本里能调的名单，嵌套调用造 `Call` 时软件包的端口却是空的（`tools/nested.rs` 的 `run_nested`）。
3. **`miyu call session_usage` 报 0**：同一个根因，用量的端口是空的。直接调用由 actor 现算派它那一刻的上下文、用时（`usage::asked`），嵌套那条路没算。

### 做法

1. 命令行那头：直接写在命令行上的 `--json` 读不懂、有花括号没有双引号的，`bad-json.txt` 后面空一格接 `quotes.txt`：`Its double quotes were lost on the way, as Windows PowerShell 5.1 does. Pipe the JSON instead: '<json>' | miyu call <tool> --json -`。从标准输入读的、有双引号的、没花括号的照旧只说那一句。不放宽解析：`{query:niri}` 照猜容易猜错。`--help` 中英写明 Windows PowerShell 5.1 用管道交。
2. 嵌套调用照直接调用给软件包端口（`self.packages`）。装卸的 `package` 脚本里调不了（`clicall` 关、内核拦），给这个端口不多开写的口子。
3. 嵌套调用照直接调用给用量：actor 派 `Nested(Run)` 时照 `usage::asked` 算一份（只有 `session_usage` 有），交给 `run_nested`；账照这个会话算。

### 测试

| 测试 | 管什么 |
|---|---|
| `crates/miyu-cli/src/call/tests.rs` 的 `json_that_lost_its_quotes_says_to_pipe_it` | 引号被吃掉的报错接那一句、说 `--json -`；有双引号的、没花括号的照旧 |
| `crates/miyu-endpoint/tests/packages_tool.rs` 的 `a_script_sees_the_same_packages_as_a_direct_call` | 真核心：脚本里调的 `packages` 和直接调的看到同一份 |
| `crates/miyu-session/tests/session_usage.rs` 的 `a_script_reads_this_sessions_usage_too` | 脚本里调的 `session_usage` 照这个会话的账，和直接调的同一个说法 |

变异 4 个全红：引号的判断改成恒假、不记「直接写在命令行上」、软件包端口改回空、用量改回空。

### 给她看的字

`quotes.txt` 新登记，37 个 token，只在出错时出现、不进前缀。
