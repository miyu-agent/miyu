## 施工单 DOC-3：手册跟上 WN-1（Windows 对齐）

状态：待验收（2026-11-16）。

### 目的

DOC-1 写手册时 Windows 上有几处和 Linux、macOS 不一样，手册照当时的样子写了（危险命令的规则在 Windows 上不查、`~\` 有几处不认、编号不拦设备名等）。WN-1 把这几处修了，手册中英两份跟着改回来。

### 改了什么

中文页和英文页同一处一起改，改完每页中英行数、标题、表格、代码块个数照旧相等。

- `guide/permissions`：「Windows 上不认」一节换成「Windows 上的写法」：一张表列 Windows 上每一类认哪些写法（PowerShell、cmd），两条说明（外壳里的照认、哪些不认）；开头那张表和下面一段里「Windows 上没有」的话去掉；`sandbox.private.add` 认 `~\`；沙盒不可用那张表里 Windows 的一句照新的原文。
- `guide/security`：「危险命令」一行去掉「只在 Linux、macOS 上」；「不挡的」表里「Windows 上的危险命令」一行换成「Windows 上认不出的危险命令」（编过码的命令、往磁盘设备直接写、`Set-ExecutionPolicy`、删注册表）。
- `guide/tools`：危险命令一条；到时照杀的 `sleep` 补 `Start-Sleep`。
- `guide/config-keys`：五个开关三个平台都管用；`sandbox.private.add`、`skills.external.dirs` 认 `~\`。
- `guide/concepts`：去掉「Windows 上不认危险命令」。
- `guide/extending`：`skills.external.dirs`、MCP 的 `command` 认 `~\`。
- `guide/personas-presets`、`guide/knowledge`、`guide/cli`：编号不能是 `con`、`nul`、`com1` 这类 Windows 保留名。
- `guide/troubleshooting`：命名管道的两句、沙盒不可用的一句、没有发行包的提示，照新的原文和办法。
- `guide/cli`：`miyu upgrade` 没有发行包时有提示。
- `dev/architecture`：「三个平台」表里危险命令的识别一行。

### 没写的

- `Set-ExecutionPolicy`、删注册表 WN-1 里等拍板、还不认：手册写「不认」。定了、做了以后再改。
- Windows 上在家目录开会话，终端、网页不提示：WN-1 没改，手册照旧。

### 蓝图改哪几节

不改。只改文档。

### 验收

1. `cargo xtask check` 全过。
2. 改过的每页中英行数、标题层级、表格行列、代码块个数相等；英文页的站内链接和锚点都指得到。
3. 手册里列的 Windows 上认的写法，每一条在 `crates/miyu-policy/src/hazard/tests/windows.rs` 或 WN-1 的施工单里有。

### 风险

- Windows 上的规则照代码和测试写，没有在 Windows 真机上逐条试。
