## 施工单 MO-6：macOS 的发行包改用链式修正（最低系统提到 12.0）

状态：做完，等验收（分支 `step/MO-6-macOS链式修正`）。2026-11-28 主会话派：优化会话核对 rc.5 的 macOS 发行包时查出来的，属于 LW 线（轻量化），接着 MO-1～MO-5。

### 目的

rc.5 的 macOS 程序照最低系统 11.0 链接，用的是老式的修正信息（`LC_DYLD_INFO_ONLY`）：启动时 dyld 把所有带指针的只读数据页改写一遍，这些页成了脏页，一直算进占用。2026-11-28 在一台 M1 MacBook（macOS 15.6.1）上量到：核心的 `__DATA_CONST` 1024 kB、终端界面的 592 kB，在空闲、说过一句、九次工具调用三个状态里都是全脏，合计约 1.6 MB。

照最低系统 12.0 链接，苹果的链接器就改用链式修正（`LC_DYLD_CHAINED_FIXUPS`）；macOS 13 起内核在某一页被读到时才修正它，修正过的页仍然是干净页，不算进占用。对照小程序上实测：每 1 MB 带指针的只读数据少约 1 MB 占用，和碰到多少页无关。

项目主人 2026-11-28 定可以做（macOS 上的用户多半用的是新系统）。最低系统定在 12.0 而不是 13.0：12.0 就能让链接器用链式修正，macOS 12 上程序照样能跑，只是不省；写 13.0 只会多排除 macOS 12 的人，没有多的好处（技术题，按推荐定）。

### 照的图纸

- 设计 `59-发版.md` 第三节第 1 条：macOS aarch64 的程序照最低系统 12.0 编、打包前和查包时核对（新加的一款）。
- 设计 `12-进程形态与分发.md` 第六节的平台表：macOS 一行写明 12 或更新的版本。
- `internal/blueprint/perf.md`「排函数」的 macOS 一款：量尺带同一个最低系统编。
- `docs/{zh,en}/guide/install.md`「选择安装方式」、`docs/{zh,en}/dev/contributing.md` 的 `cargo xtask dist`、`dist-check` 两行。

### 改了什么

1. **打发行包时照 12.0 编。** `xtask/src/dist.rs` 加常量 `MACOS_MINIMUM`（`"12.0"`，文档注释写明为什么是 12）和 `released`：给编要发出去的程序的 cargo 命令补环境，macOS 上带 `MACOSX_DEPLOYMENT_TARGET=12.0`，别的系统原样。它同时管 rustc 的链接和 `cc` 编的 C 依赖（SQLite、zstd、ring）。经它的有四处：核心的四个程序（三个包一起编那一条，带不带垫片都经它）、终端界面、网页，和探针那一轮编的量尺（`order/probe.rs`；量尺不发出去，但和核心共用 target 目录，不带的话共同的依赖要照 11.0 再编一遍）。Linux 上排函数的 `order.rs` 只在 Linux 上走，不经它。
2. **打包前自查。** `xtask/src/dist/check/macho.rs` 照 Mach-O 格式读每个程序的加载命令（64 位、小端，只看命令的种类和 `LC_BUILD_VERSION` 的最低系统）：要有 `LC_DYLD_CHAINED_FIXUPS`、不能有 `LC_DYLD_INFO_ONLY`（或 `LC_DYLD_INFO`），最低系统要正好是 12.0。`dist` 在搭好目录、打第一个压缩包以前对目录里的六个程序查一遍（`check::chained`），不对的报出是哪个程序、少了什么，不打包；`cargo xtask dist-check` 查 macOS 的包时在 `contents` 里同样逐个查（同 CRT-1 的导入表那一项）。读法自己写、不调 `otool`（技术题，按推荐定）：和 CRT-1 读 PE 导入表一样，同一份代码在哪台机器上都一样，Linux 上也测得到。
3. **排函数照旧生效。** 链接器垫片（`order/link.rs`）原样把 rustc 给的参数交给 `cc`，只多一个 `-order_file`，这一步不用改；`MACOSX_DEPLOYMENT_TARGET` 经环境到 rustc，rustc 自己把 `-mmacosx-version-min` 换成 12.0。
4. **macOS 11 上不装上起不来的程序。** 查下来的现状：
   - 安装脚本 `packaging/install.sh` 原来只照 `uname` 认 `macos-aarch64`，不看系统版本。现在在 `target` 里多一个 `macos`：`sw_vers -productVersion` 的主版本小于 12 的停下，提示 `需要 macOS 12 或更新的版本，这台是 macOS <版本>`，什么都不装；读不出版本的不拦，后面照样跑一次新的 `miyu --version`。卸载不经这一步。`packaging/test-install.sh` 开头加一段：`PATH` 前面放假的 `uname`、`sw_vers`，11.7.10 上要停下、说对话、不留东西，12.0 上不能停在系统版本上；Linux 的机器上也走得到这一段。
   - `miyu upgrade` 不改：升级时跑的是装着的旧程序，新加的判断管不到 macOS 11 上已经装着的旧版。旧程序升级前本来就会在暂存目录里跑一次新的 `miyu --version`（`crates/miyu-cli/src/upgrade/flow.rs`），macOS 11 的 dyld 拒绝加载最低系统 12.0 的程序，这一步失败，`miyu upgrade` 停下、装着的不动，只是报的是「跑不起来」，不是「要 macOS 12」。
5. **文档。** 安装页中英两份写明 macOS 12 或更新的版本和安装脚本的提示；开发者文档的 `dist`、`dist-check` 两行补上 macOS 的核对；设计 59、12 和蓝图 `perf.md` 各补一条。Homebrew 的配方和 cask（`packaging/homebrew/`）只写了 `depends_on :macos`、`depends_on arch: :arm64`，没有写最低系统，这一步不动。仓库里还没有 `next-release-note.md`（第一次正式发布以后才开始记），发版说明的一条写在交回里，由做发版的会话记。

### 怎么验

- `xtask/src/dist/check/macho/tests.rs`：现搭的 Mach-O 文件：12.0 加链式修正的通过；rc.5 那样的 11.0 加老式修正报三处；11.0 强制链式修正、13.0、没写最低系统的都不通过；截断、长度是 0、条数比实际多、32 位、多架构合在一起的、ELF 都报错；版本的两种写法互换；macOS 的包六个程序都查、只漏一个的只报它，Linux 的包不查。macOS 上另读这个测试程序自己的加载命令，证明读法对得上真的链接器写的程序。
- `xtask/src/dist/tests.rs`：`released` 只在 macOS 上带 `MACOSX_DEPLOYMENT_TARGET=12.0`。
- Linux 的门禁这一次没有在本地的测试机上预跑（排队太久，主会话定以三台 CI 为准，CI 的 Linux 一项跑的是同一套 `cargo xtask check`）；三台 CI 绿；GitHub 的 macOS 机器上照 `release.yml` 打一次包、查包（临时的 `measure/mo-6-*` 分支，不进 main），自查在 CI 的链接器版本下也通过。
- 一台 M1 MacBook（macOS 15.6.1、8 核、16 GB）上从头 `cargo xtask dist` 一次，用 `otool -l` 核对三个程序，再和 rc.5 的发行包交替量占用（结果见下）。

### 没有验的

- macOS 12、13 上的实际效果：手上没有这两个版本的机器。照机制推测，macOS 12 上 dyld 启动时仍然把所有页修正一遍，和 rc.5 一样脏；macOS 13 起和下面 macOS 15 上量的一样。
- macOS 11 上安装脚本的提示只在假的 `sw_vers` 下验过。

### 不做什么

- 不关 nano 区（`MallocNanoZone=0`）：实测空闲少 3.0 MB，但是启动那一段核心多执行 28% 的指令，是拿 CPU 换内存。
- 不把最低系统提到 13.0。
- 不动 Linux、Windows 的构建。
- 平时开发的 `cargo build`、测试不设这个变量（只管发行包）。

### 风险

- macOS 11 上的用户装新版以后起不来：安装脚本拦下；`miyu upgrade` 由旧程序跑，跑新程序那一步失败、不换（见第 4 条）。
- C 依赖照 12.0 编以后行为不变（只是允许用更新的系统接口），`ring` 的汇编、SQLite 的编译选项由三台 CI 和打包试跑过一遍。
- 链接器版本：CI 的 macOS 机器和那台 M1 的 Xcode 不同，链式修正的默认行为可能不一样；打包前的自查照编出来的程序查，不对就不打包。

### 量到的

**打包**（一台 M1 MacBook，macOS 15.6.1，Xcode 命令行工具的 ld-1167.5，2026-11-28）：从头 `cargo xtask dist` 一次，用了 16 分 31 秒，打出四个压缩包；打包前的自查通过。排函数照旧生效：垫片对上核心起点清单 10613 个名字里的 10358 个（97%）、终端界面 2344 个里的 2097 个（89%），核对时两个程序的清单都 100% 排在代码段开头。链接时原来那 42 行 `ld: warning`（zstd-sys 的目标文件比部署目标新）一行都没有了。

**`otool -l`**：

| 程序 | rc.5 | 这一步打的 |
|---|---|---|
| `miyu` | `LC_DYLD_INFO_ONLY`，`minos 11.0` | `LC_DYLD_CHAINED_FIXUPS`，`minos 12.0` |
| `miyu-tui` | `LC_DYLD_INFO_ONLY`，`minos 11.0` | `LC_DYLD_CHAINED_FIXUPS`，`minos 12.0` |
| `miyu-web` | `LC_DYLD_INFO_ONLY`，`minos 11.0` | `LC_DYLD_CHAINED_FIXUPS`，`minos 12.0` |
| `miyu-sandbox`、`miyu-extract`、`miyu-watchdog` | — | `LC_DYLD_CHAINED_FIXUPS`，`minos 12.0` |

**GitHub 的 macOS 机器上照 `release.yml` 试打**（临时的 `measure/mo-6-mac` 分支，macOS 26.7.2、Xcode 26.6、ld-1267）：打包前的自查通过，四个压缩包 `cargo xtask dist-check` 都没有问题，六个程序的 `otool -l` 都是 `LC_DYLD_CHAINED_FIXUPS`、`minos 12.0`。

**占用**（同一台 M1 MacBook；量法同优化会话核对 rc.5 时：沙箱数据根，本机 HTTPS 假模型，第三次启动，三个状态各起三遍、歇 70 秒再量，两份交替量四批：rc.5、新包、rc.5、新包，同一时刻只有一组进程。`phys_footprint` 是核心加终端界面两个进程各自三遍中位之和，单位 kB）：

| 状态 | rc.5 第一批 | 新包第一批 | rc.5 第二批 | 新包第二批 | 少了（两批） |
|---|---|---|---|---|---|
| 空闲 | 14546 | 13138 | 14770 | 13234 | 1408、1536 |
| 说过一句 | 15234 | 13586 | 15090 | 13682 | 1648、1408 |
| 九次工具调用 | 15458 | 14258 | 15826 | 14323 | 1200、1503 |

| 分项（空闲，每遍都一样） | rc.5 核心 | 新包核心 | rc.5 终端界面 | 新包终端界面 |
|---|---|---|---|---|
| 程序自己的 `__DATA_CONST` 脏页 | 1024 | 0 | 592 | 0 |
| 程序自己的 `__TEXT` 常驻 | 5680 | 5648 | 2656～2672 | 2640 |

三个状态、两批里 `__DATA_CONST` 的脏页都从 1024、592 kB 降到 0；两个进程合计少 1.2 到 1.6 MB，和预期的约 1.6 MB 相近。空闲时常驻的代码没有变多，排函数没有退。

**启动**（核心已经在跑时起 `miyu` 到出现输入框、再过 1.5 秒，每轮 10 遍，两份交替各两轮，取指令数的中位）：

| | rc.5 | 新包 |
|---|---|---|
| 终端界面执行的指令数 | 178.9 M、179.0 M | 171.0 M、171.2 M |
| 核心同一段时间多执行的指令数 | 317.1 M、317.6 M | 314.4 M、314.8 M |
| 起来到输入框的墙上时间中位 | 278、280 ms | 279、282 ms |

指令数不比 rc.5 多：终端界面少约 4%，核心少约 1%；墙上时间没有差别（这段时间大部分在等）。

> 新包是这一步的分支（main 加上 MO-6）打的，rc.5 以后 main 上别的改动也在里面，所以上面的差不全是这一步的。`__DATA_CONST` 那一行只和修正方式有关，可以单独归到这一步。
