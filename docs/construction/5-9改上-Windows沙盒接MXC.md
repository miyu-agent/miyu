## 施工单 5-9 改上：Windows 沙盒接 MXC

状态：合了（2026-10-21，cd1fc1df；先过了三台 CI）。2026-10-20 项目主人定：Windows 沙盒改用 MXC，「要做的」。原来的 5-9（`5-9-Windows：跑.md`）停在单独的桌面上；这一张换路子。下半是 `5-9改下-Windows沙盒收尾.md`。

### 目的

Windows 上沙盒里的命令照规格管住读写，和 Linux（bubblewrap、Landlock）、macOS（Seatbelt）一样。现在 Windows 的助手（`crates/miyu-sandbox/src/bin/miyu-sandbox/windows.rs`）什么都不收紧，探测报没有可用的手段，工作区这一级每条命令都问人。

改用微软的 MXC（Microsoft eXecution Container，MIT，Rust 库 `mxc-sdk` 1.0.1，2026-10 正式发布）：Windows 上是 ProcessContainer（AppContainer、BaseContainer），不用建沙盒用户、不要管理员权限（设计 11 A3 的重开条件）。

### 试验（2026-10-20，虚拟机 Windows 11 25H2，仓库外的小程序）

| 设计 11 的要求 | 结果 |
|---|---|
| 工作区能写 | 能 |
| 工作区外写、私密目录读 | 拒绝访问（`denied_paths` 在这台机器上生效） |
| 别处只读 | 能读 |
| 退出码 | 原样交回 |
| PowerShell、git | 在登录的桌面会话里能跑；SSH 这类没有桌面的会话里起不来（0xC0000142：AppContainer 把桌面写死 `winsta0\default`） |
| 每条命令多花的时间 | cmd 几十到一百多毫秒，git 642 毫秒，PowerShell 1.4 秒（多半是它自己启动慢） |

界面策略要放开窗口（`UiPolicy { disable: false }`），全关会挡 Win32k，PowerShell、git 起不来；剪贴板、模拟输入照旧不给。

### 定了的

1. **用 MXC**（2026-10-20 项目主人）。
2. **没有桌面会话的不跑**（2026-10-20 项目主人照推荐定）：核心跑在 SSH、服务这类没有桌面的会话里时，探测报这一条手段用不了，沙盒里的命令拒绝，照实说「没有桌面会话，沙盒里起不了这个程序；到桌面上用，或者自己关掉沙盒」，不自动放宽。
3. **网络放行**：设计 11 A5「沙盒不管网络」，MXC 的出站、入站都照放行写。

### 第 0 步：先量读的范围怎么给

设计 11 A4 是「读整盘放开」。AppContainer 默认读得到系统目录，读不到你家目录里的文件，要在只读路径里写上。9 月的实验里给家目录加一条可读，15.3 万个文件花了 27.5 秒。开工先量：

- 只读路径写家目录、写盘根，各自每条命令多花多少；MXC 是每次加了再撤（`clear_policy_on_exit` 默认是真），还是走过滤驱动（源码里的 `filesystem_bfs`）。
- 量出来每条命令多花的超过一秒的，停下报告，带上几种做法的数（例如只给常用的几个目录、装的时候加一次长期的）。

### 第 0 步量出来的（2026-10-21，同一台虚拟机，桌面会话里，每种 5 次）

| 只读路径 | `cmd /c echo` 中位 | 家目录里的文件 | 禁止路径（私密目录） | `git --version` |
|---|---|---|---|---|
| 只给试验目录 | 45 ms | 读不到 | 挡住 | 152 ms |
| 家目录 | 44 ms | 读得到 | 挡住 | 62 ms |
| `C:\` 整盘 | 47 ms | 读得到 | 挡住 | 61 ms |

- MXC 在 Windows 上走 BFS（Brokered File System）：`bfscfg.exe` 给这个 AppContainer 照路径登记策略，一条路径一次，不往下走、不改每个文件的 ACL，跑完清掉（`mxc-sdk` 的 `process_container/common/filesystem_bfs.rs`）。所以只读给多大都不多花，不用退到「只给常用的几个目录」。
- 整盘只读时禁止路径照样挡住：私密目录、数据根照写进禁止路径。
- 定：读整盘就写各固定盘的盘根进只读路径（设计 11 A4）。

### 怎么做

- 助手：`windows.rs` 的 `run` 改成照规格拼 MXC 的请求，起命令，标准输入、输出、错误照原样转，退出码照原样交回；助手被杀时容器里的整棵进程树跟着停。
- 规格翻成策略：`write` 进读写路径，`hidden` 进禁止路径，`read`（外部身份的会话）进只读路径、别处读不到，`reveal` 进只读路径，读整盘照第 0 步定的做法。`sockets`（脚本调工具的通道）在 Windows 上照 M-3 下的做法放行那一根管道，和做 M-3 下的会话对好。
- 探测：Windows 11 24H2 以上、MXC 能起 ProcessContainer、这一刻有桌面会话的，`mechanisms()` 报 `processcontainer`；别的照实报用不了、为什么。
- 依赖：`mxc-sdk` 只在 `cfg(windows)` 下进 `miyu-sandbox`；许可证 MIT，照门禁登记。

### 不做什么

- 5-8 装的沙盒用户撤不撤：下半。
- 单独的桌面：MXC 自己管（AppContainer 的桌面），不另做。

### 验收

1. 先写会红的测试（Windows）：工作区写得进、外面写不进、数据根读不到、私密目录读不到、只读路径读得到、退出码交回、标准输入输出转得对、杀助手连着停；没有桌面会话时探测报用不了。CI 的 Windows 机器是 Windows Server 2025（26100），先看 ProcessContainer 起不起得来，起不来的这几条只在虚拟机上跑、施工单里写明。
2. 虚拟机真机走一遍：Miyu 在桌面会话里，她在沙盒里跑 PowerShell、git、cargo，读你的家目录、写不了工作区外面。
3. `cargo xtask check` 全过。

### 风险

- 读整盘的代价（第 0 步）。
- `mxc-sdk` 体积、编译时间：只在 Windows 上编进去，Linux、macOS 不受影响。
- MXC 的仓库自己写着策略不该当安全边界：我们的定位同设计 11（控制读写，不是完全隔离），数据根照旧靠禁止路径和本机令牌两层（A6）。

### 第二轮试验（2026-10-21，同一台虚拟机）

- `v1::probe` 报的档是 `base-container`，不用补访问控制（`needs_dacl_augmentation: false`），禁止路径基础容器自己就支持。
- `v1::spawn` 给三根管道：喂进去的读得回来，退出码照原样（`Exited(3)`），76 ms。
- 跑完、跑到一半把起容器的进程杀掉：工作区、禁止路径上一条访问控制项都没多；容器里的 `ping` 跟着停，不留。
- MXC 在基础容器起不来时会退到「AppContainer 加访问控制」那几档（`dispatcher.rs`）：加的项靠管理器 `Drop` 时撤，助手被杀撤不回去。所以只认 `base-container`。

### 做了什么

1. `mxc-sdk = "=1.0.1"` 只在 `cfg(windows)` 下进 `miyu-sandbox`；新拉进来的 17 个包许可证都是 MIT、Apache-2.0 一类。
2. 助手的 `windows.rs` 拆成几块（`request.rs`、`pump.rs`、`resolve.rs`、`desktop.rs`、`drives.rs`），程序、参数拼成一行的 `cmdline.rs` 放进库里（纯算字，哪个系统都测）。怎么走写在新的蓝图页 `sandbox/windows-run.md`。
3. 和施工单原文不一样的：
   - **没有桌面会话不是探测的事**：探测只看基础容器，跑的时候没有桌面会话的每条命令照实拒绝、说怎么办（这一句就是命令的输出）。原文写「探测报这一条手段用不了」：那样核心照沙盒用不了办，工作区一级会转去每条问人，不是「拒绝并说清楚」。
   - **读整盘写盘根**（第 0 步定的）；`reveal` 进只读路径。
   - **`read`、`sockets` 不在这一步**：见蓝图「怎么走」第 4、5 条。
4. `main.rs`：`cannot_run` 各平台都用（Windows 上先认程序）；只为它导入的几样改成写全路径。

### 验收（做下来的）

- Linux：`cargo xtask check` 八项；`cmdline` 的单元测试。
- 虚拟机 SSH 会话（收不住）：`cargo test -p miyu-sandbox` 全过，`windows.rs`、`run.rs` 照拒绝断言；探测报 `processcontainer`，跑命令退 125、说没有桌面会话。
- 虚拟机桌面会话（收得住，临时计划任务以登录的身份跑）：`cargo test -p miyu-sandbox` 全过，`windows.rs` 真进了容器（0.6 秒）；另手跑：工作区写得进、外面拒绝访问、私密目录读不到、家目录的文件读得到、退出码 7、标准输入、`git --version`、`curl https://example.com` 200、带空格的参数；规格路径带 `\\?\` 前缀的照样认。
- 三台 CI：先推 step 分支过了再合（Windows 上收不住，只测拒绝）。第一趟 Windows 红在 `miyu-cli` 的 `steps`：GitHub 的 Windows 机器有桌面会话、MXC 只能退到 `appcontainer-dacl`，助手照规矩拒绝；会话、命令行的测试场地不看探测、直接塞编出来的助手。加 `testkit` 里的直通（蓝图「守着它的」末段），正式的助手没有。
