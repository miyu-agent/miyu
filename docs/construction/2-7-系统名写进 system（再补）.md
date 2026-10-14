## 施工单 2-7 再补：系统名写进 system

状态：已合（2026-10-14，755c1cdf，CI run 38524965227）。项目主人 2026-10-14 问环境里有没有带系统信息（Arch Linux、Windows 11、macOS），想避免她在 Windows 上跑 Linux 命令。查下来她只知道是哪种 shell（`shell` 的说明里写着 bash、zsh 或哪一种 PowerShell），不知道发行版和系统版本。项目主人定放进 system：系统名一个会话里不会变，环境那一块带着「此刻几点」，每跨一个小时重发一次，不放那里。

### 做了什么

1. `miyu_store::env::os_name()`（新文件 `crates/miyu-store/src/env/os.rs`）：核心头一次要时认一次，之后照它。
   - Linux 照 `/etc/os-release`（没有的看 `/usr/lib/os-release`）的 `NAME` 加 `VERSION_ID`，得到 `Arch Linux`、`Ubuntu 24.04` 这样的名字。
   - macOS 照 `SystemVersion.plist` 写大版本，如 `macOS 15`。
   - Windows 照内部版本号分 10、11，服务器版写 `Windows Server`。版本号用微软的 `windows-version`（只在 Windows 上编，和已有的 `windows-link` 0.2 共用）。
   - 认不出的照平台写。只写名字和大版本：小版本常变，变了新会话的 system 和前一天的不同，跨会话的缓存接不上。
   - 只留一行，控制字符换空格，最长 64 个字。
2. 新资源 `core/machine.txt`：`The local machine runs {os}.`，是核心的几行的第三行，接在本机文件的路径那一句后面。
   - 工具面是空的会话不带，和权限那一句一样。
   - `Snapshot::with_core_lines` 多一个参数 `os`，拼快照时传 `os_name()`。
   - 以前造的快照照旧发，前缀一字不变。光是程序升级、人格和预设没改的不换快照，老会话也不变。
3. 登记簿 26 第十节加一行（6 token），`prompts.md` 重新生成；`policy.md`、`kernel/request.md`、`store.md`、`store/resources.md` 跟着改。
4. `compose.rs` 的测试挪进 `compose/tests.rs`，守 500 行。

### 验收

- 单元测试：
  - os-release、plist、Windows 版本号的解析，以及恶意的长名字、控制字符。
  - 本机认得出。
  - 核心的几行三句的顺序；没有工具的不带系统那一句。
  - 会话、端点测试照真资源比 system。
- 变异 8 个全被测试拦下：
  - 去掉系统那一句。
  - 没有工具也带。
  - 22000 改成 `>`。
  - `NAME` 改读 `PRETTY_NAME`。
  - 不截长。
  - macOS 不取大版本。
  - 读错文件。
  - 拼快照不传系统名。
- 真模型 A/B（开发端点的 `deepseek-v4.1-flash`，`miyu ask` 问装 ripgrep 敲什么命令，只说、不装，各 4 遍）：
  - 不加：4 遍都先跑 `cat /etc/os-release`，一共多调 5 次工具。
  - 加了：4 遍都直接答 `sudo pacman -S ripgrep`，不调工具。
- token：带和不带各问四次 `hi` 取平均，差 6 个。
- 三台 CI 过。
