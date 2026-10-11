## 施工单 X-5：MCP 真服务走一遍

状态：合了（1c29ec64，三台 CI run 1833；2026-10-11、12 主会话）。图纸 `mcp.md`（「守着它的」后面那一段、「起草时定的」施工 X-3 上那一条）；作者说明 `docs/authors/README.md` 第三节第 8 条。

### 这一步做什么

1. 拿官方的两个参考服务真走一遍：`@modelcontextprotocol/server-everything`（`stdio`）、`@modelcontextprotocol/server-filesystem`（放开一个目录），写成一个只带服务的包，经 `miyu pkg install` 装、`package.enable` 打开，开发用的那个模型在 `miyu ask` 里调。
2. Windows 上 `npx`、`uvx` 这类是 `.cmd`：命令第一项只有名字的，Windows 上照 `.exe`、`.cmd`、`.bat` 依次在包的 `bin/`、`PATH` 里找，找到的照完整路径起（标准库经 `cmd.exe` 转、参数照它的规矩转义）；别的平台照旧把 `PATH` 交给 `Command`（施工 X-3 上留到这一步）。
3. 给第三方作者的说明补上怎么带 MCP 服务（第三节第 8 条），第一节第 2 条、开头的状态、第九节第 4 条跟着改。
4. 图纸 `mcp.md`：状态写成 MCP 这一段做完，走一遍的结果写进「守着它的」后面，「还没有的」不动。

### 走一遍的结果（2026-10-11，沙箱数据根，开发用的模型）

- 两个服务都说旧时代（`2025-11-25`）：先探 `server/discover`，回来的是「没有这个方法」，当场退到 `initialize`，不等 5 秒。起来到能用约 1.5 秒（多半是 `npx`）。各列出 13、14 件工具，只读注解都读到了（`everything` 9 件、`filesystem` 10 件只读）。
- `everything` 起来就说一次工具变了：照常重列，工具一样，缓存不写、不影响前缀。
- 模型一轮里并着调 `mcp__files__read_text_file`、`mcp__everything__get-sum`，读到了文件里的暗号、算对了 17 加 25；只读的两件都没问人。
- 第二轮：`get-tiny-image` 回图片块，`get-structured-content` 只有结构化的照紧凑的 JSON 写成字，`get-resource-links` 写成一行一个链接；`mcp__files__write_file` 没标只读，照联网的一类要人确认，`miyu ask` 里确认不了，交回「要你确认，miyu ask 里确认不了」，模型照实转述、改用自带的写入。
- 运行日志没有跳过的工具、没有警告；核心停下时两个服务一起退出。
- 没发现要修的毛病。

### 照着自己走一遍

1. 工作区根目录 `cargo build -p miyu -p miyu-sandbox`；在一个新的数据根上照开发的规矩 `cargo xtask dev-home <数据根>`（地址、模型、窗口照开发变量）。
2. 写一个包目录，`package.toml` 照作者说明第三节第 8 条写两段 `[[mcp]]`；`MIYU_HOME=<数据根> miyu pkg install <包目录> --yes`。
3. 经协议发 `package.enable {package}`（网页的「软件包」页、或者自己写的客户端），`package.list` 里那一项的 `mcp` 两格变成 `running`、带进程号。
4. 停掉装包时拉起的核心，在带着开发用 key 的终端里 `MIYU_HOME=<数据根> miyu ask "用 files 服务读一下……"`。

### 验收

- `miyu-endpoint` 的 `mcp/launch/tests.rs` 多一条：Windows 的找法（`.exe`、`.cmd`、`.bat` 的先后，包里的在 `PATH` 前面，都没有的交回名字）照文件名找，哪个平台都跑。
- `tests/mcp_status.rs` 多一条只在 Windows 上跑的：包的 `bin/` 里放一个 `.cmd` 转给测试用的服务，命令只写名字，起得来、握得上手。
- `cargo xtask check` 八项；三台 CI。
