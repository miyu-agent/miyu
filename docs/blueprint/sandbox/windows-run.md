## 沙盒：Windows 跑

### 是什么

Windows 上每条命令怎么关进沙盒（施工 5-9 改上，`11-权限与沙盒.md` A3 的 2026-10-20 改：用微软的 MXC）。助手（`miyu-sandbox.exe`）照规格拼一个 MXC 的请求（`mxc-sdk` 1.0.1，ProcessContainer），把命令关进基础容器（BaseContainer）里起、等它，标准输入、输出、错误原样转，照它的退出码退出。不用沙盒用户、不要管理员（5-8 的沙盒用户撤不撤在 5-9 改下）。

和 Linux、macOS 一样：整盘能读（A4）、只能写规格给的、数据根和私密目录读写都不行、网络不管（A5）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-sandbox/src/bin/miyu-sandbox/windows.rs` | 探测报什么、跑一条命令的先后：先认程序，再看有没有桌面会话、有没有基础容器，再起 |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/windows/request.rs` | 规格翻成请求 |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/windows/pump.rs` | 三根管道原样转，照退出码退出 |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/windows/resolve.rs` | 先认程序：找不到的 127、不是程序的 126 |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/windows/desktop.rs` | 有没有桌面会话（窗口站是不是 `WinSta0`） |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/windows/drives.rs` | 固定盘的盘根 |
| `crates/miyu-sandbox/src/cmdline.rs` | 程序、参数照 `CommandLineToArgvW` 的规矩拼成一行（MXC 收一整行字）；纯算字，哪个系统都测 |

### 怎么走

1. **探测**（`miyu-sandbox probe`）：MXC 探得出基础容器（`v1::probe` 报的档是 `base-container`、不用补访问控制）的报 `processcontainer`；别的档不报：MXC 起不来基础容器时会退到给目录加访问控制项的那几档，助手被杀时那些项撤不回去，留在盘上（2026-10-21 定）。有没有桌面会话不看：那是跑的时候的事。
2. **跑**（`miyu-sandbox run --spec … -- <程序> …`）：
   1. 先认程序：照 `CreateProcess` 的先后找（当前目录，再 `PATH`，没写扩展名的照 `PATHEXT` 试）。找不到的印 `cannot run <程序>: …`、退 127；`.exe`、`.com` 开头不是 `MZ` 的退 126。MXC 起不来的分不出是哪一种，所以先认。
   2. 没有桌面会话（核心跑在 SSH、服务里）的：印 `cannot confine: no desktop session, programs in the sandbox cannot start here; use Miyu from the desktop, or turn the sandbox off`、退 125，不跑（2026-10-20 项目主人定「拒绝并说清楚」：要窗口的程序在容器里起不来，0xC0000142；不放宽、不转去问人）。这一句就是这条命令的输出，她和你都看得到。
   3. 再看一眼基础容器还在：不在的说为什么、退 125。
   4. 拼请求：命令行照 `cmdline.rs` 拼；`write` 进读写路径；`hidden` 进禁止路径（禁止压过只读；能写的落在禁止的里面照样能写，2026-10-30 桌面端在虚拟机上量过：工作区在数据根里写得进）。正被别的进程独占打开的文件不交（施工 SW-1，`miyu_sandbox::deniable`、`busy`）：系统建安全环境时要打开每一条禁止路径，用户登录着时的 `NTUSER.DAT`、`UsrClass.dat` 打不开，报 `0x80070020` 共享冲突，整条命令跑不了（10-30 桌面端在 Win11 虚拟机上撞到：出厂私密清单里有这两个，会话里每条命令都 125）；这样的文件谁都读不到，跳过不少挡什么。出厂私密清单在 Windows 上不列 `~/AppData/Local/Packages`（施工 SW-2，2026-10-30 项目主人定）：容器自己的目录 `Packages\sandbox.{编号}`（临时目录 `AC\Temp` 在内，MXC 在容器里强行把 `TEMP`、`TMP` 指到它，外面设的不算）就在它下面、编号每次不一样，禁了整个它，Windows PowerShell 5.1 写不进临时目录做语言模式的自查，退到 ConstrainedLanguage，开头那句设编码就失败、命令一条条报错（桌面端在虚拟机上对照证实）；代价是沙盒里的命令读得到商店应用的私有数据（写照旧不行）；每个固定盘的盘根、`reveal` 进只读路径（A4 整盘能读：MXC 照路径给容器登记，一条路径一次，不往下走、不改每个文件的访问控制，给多大都不多花，2026-10-21 量过约 45 ms 一条）；网络进出都放行；窗口放开（全关会挡 Win32k，PowerShell、git 起不来），剪贴板、模拟输入照 MXC 的默认不给；环境照助手自己的原样交（执行器已经拣好了），不叠 MXC 的默认；工作目录是助手的；不限时（时限由执行器管）。
   5. 起：`v1::spawn`，三根管道各一根线程搬，每搬一段就送出去；命令退了，输出搬完再照退出码退。助手被杀，容器里整棵进程树跟着停（2026-10-21 量过：杀了起容器的进程，`ping` 不留）。
3. 规格里的路径带不带 `\\?\` 的前缀都认（核心换成真实位置时带）。
4. `read`（外部身份的会话）照旧不收：`CONFINES_READS` 在 Windows 上照旧是假的，助手照 `sandbox.md` 拒。基础容器能做到（只读路径只给名单里的），放 5-9 改下。
5. `sockets`（脚本调工具的通道）：助手不看这一格。Windows 上的通道是命名管道，容器里连得上靠管道自己的访问控制（施工 M-3 下：多给 `ALL APPLICATION PACKAGES` 连的权限，`tools/shell.md`「通道」第 2 条）；容器里的进程在助手所在的作业对象里（MXC 的作业对象嵌在里面），通道认得出。虚拟机的桌面会话里量过。

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-sandbox/src/busy/tests.rs` | 禁止路径去掉被独占着的、先后不变；Windows 上真的独占打开一个文件算被占着，放开了、目录、不在的不算（施工 SW-1，CI 的 Windows 机器上跑） |
| `crates/miyu-sandbox/src/cmdline/tests.rs` | 照写的原样、要引号的、引号里的双引号和反斜杠、结尾的反斜杠、空的参数 |
| `crates/miyu-sandbox/tests/windows.rs` | 收得住的机器上（虚拟机的桌面会话）：能写的写得进、别处写不了、藏起来的读不到、别处读得到、退出码、标准输入照原样；收不住的（CI、SSH 里）照实拒绝，说 `cannot confine: …` |
| `crates/miyu-sandbox/tests/run.rs` | 命令照直接跑的样子跑、成功的只印自己的：收不住的机器上照拒绝断言；找不到的 127、不是程序的 126，哪种机器上都一样 |

CI 的 Windows 机器上收不住（GitHub 的 Windows 机器有桌面会话，MXC 却只能退到 `appcontainer-dacl` 那一档），只测拒绝；收得住的那几条在虚拟机的桌面会话里跑过（施工单记着）。

会话、命令行的测试经 `miyu_sandbox::testkit::built_helper` 拿助手，测的是工具和命令行，不是收紧：它在 Windows 上设环境变量 `SANDBOX_TESTKIT_PASSTHROUGH`，带 `testkit` 编的助手收不住时照 5-9 以前直接起命令（`windows.rs` 的 `passthrough`）。正式的助手不带 `testkit`，没有这一段；名字不带 `MIYU_`，因为执行器起命令时去掉 `MIYU_` 开头的。助手自己的测试不设，照旧严格断言拒绝（AGENTS.md 8.2：下层的测试态开关一律 `testkit`）。

### 出处

- 设计：`11-权限与沙盒.md` A3 的 2026-10-20 改、第四节。
- MXC：`mxc-sdk` 1.0.1（MIT），`process_container/common/dispatcher.rs`（档位、访问控制的管理器）、`filesystem_bfs.rs`、`lib.rs` 的 `v1`。
- 试验：施工单 `5-9改上-Windows沙盒接MXC.md` 的「试验」「第 0 步」。

**WMI**（施工 SW-3，2026-10-30 项目主人定）：MXC 的容器挡住 WMI/CIM（`Get-CimInstance` 报连不上 CIM 资源，`systeminfo`、`Get-ComputerInfo` 底下也走它）。不放宽：看过 Codex（MXC、沙盒用户两条路都没为 WMI 开口子，被挡就申请到沙盒外跑）、Claude Code（Windows 上不进沙盒，只读名单特意不收 `Get-CimInstance`、`Get-WmiObject`：查询语句任意、能连远程主机）。沙盒里用到 WMI 的命令（不看退出码：`Get-CimInstance` 连不上是不终止的错、照样退 0，施工 SW-4），`shell` 的结果末尾接 `no-wmi.txt` 一句，她申请在沙盒外跑这一条，人点允许。

### 还没有的

- 外部身份的会话的 `read`、撤不撤 5-8 的沙盒用户、git 的 `safe.directory`：5-9 改下。
- 用着用着基础容器没了（系统更新）：那一条照实拒绝，不重探；要重探得重开核心。
