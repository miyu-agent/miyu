## 施工单 SB-1：沙盒里能用无头浏览器

状态：合了（2026-10-23）。项目主人 2026-10-22 报的：沙盒里的无头 chromium 跑不起来（他查到的是 dconf 只读、没有 dbus）。10-23 看过 Claude Code、Codex、Gemini、Cursor 的做法以后定：对齐 Claude Code。

### 目的

她在沙盒里的命令里直接用 firefox、chromium 无头地开网页、截图。

### 查下来的根因（2026-10-22、23 照会话沙盒的规格经助手真跑，Arch、bubblewrap 0.13.1、chromium 154、firefox 158）

- dconf 只读、连不上 dbus：是真的，但只是刷屏，不致命。
- chromium：默认的资料目录在 `~/.config/chromium`，沙盒里家目录只读，无头模式建不起它的临时资料目录（`Failed to create a unique user data directory`）。给了能写的资料目录以后，它自己那层沙盒要新开用户命名空间、往 `/proc/self/uid_map` 写，我们的沙盒里 `/proc` 只读、Landlock 也不让写 → 退回 setuid 的辅助程序 → 我们设了 no_new_privs，不生效 → 它中止（只有 Landlock 的机器上是 `Zygote process exited prematurely`）。加 `--no-sandbox` 就成，没有环境变量能代替这个参数。
- firefox：资料目录在 `~/.mozilla`，沙盒里写不进；桌面开着 firefox 时还锁着（它卡到超时）。换到能写的资料目录以后，它自己那几层子进程沙盒同样起不来（子进程段错误、超时），关掉就成。
- playwright：默认就带 `--no-sandbox`、自己建临时资料目录，现在的沙盒里不改就能起。
- 叠加层（bwrap `--tmp-overlay` 叠 `~/.config`、`~/.mozilla`）试过可行，但写进去命令一结束就丢、不报错，比它省的那点更难受；chromium 照样要 `--no-sandbox`。没用。

### 定的（2026-10-23 项目主人定，对齐 Claude Code）

1. 家目录、`~/.config` 照旧只读：不上叠加层、不换 `HOME`、不换 `XDG_CONFIG_HOME`。
2. 沙盒命令的环境（工作区这一级，同工具链的缓存那一套）多设：`XDG_CACHE_HOME` 指沙盒的缓存，`XRE_PROFILE_PATH` 指沙盒的缓存里事先建好的一份 firefox 资料目录，`GSETTINGS_BACKEND=memory`，firefox 的 `MOZ_DISABLE_CONTENT_SANDBOX`、`MOZ_DISABLE_RDD_SANDBOX`、`MOZ_DISABLE_GMP_SANDBOX`、`MOZ_DISABLE_SOCKET_PROCESS_SANDBOX`、`MOZ_DISABLE_UTILITY_SANDBOX` 设 `1`（外面这层沙盒照旧关着它）。firefox 裸跑一条就成，给她的字一个没加。
3. chromium 不进 `shell` 的说明：结果里认出它自己那几句报错（`shell.rs` 的 `CHROMIUM_SIGNS`）时，末尾接一句英文（`chromium-sandbox.txt`，进登记簿），告诉她这里要加 `--no-sandbox --user-data-dir=<临时目录>`（外层沙盒还在，内层多余，同 Claude Code 的说法）。只有撞上才花 token。
4. 保底照旧：人批准出沙盒。
5. `miyu shot` 不做（项目主人：环境修好了她自己会用浏览器截，多一个命令只多说明的开销和维护）。
6. macOS：chromium 在 Seatbelt 下还缺 `mach-register`（社区验证 `(allow mach-register (global-name-regex "org.chromium."))`），这一步不加，先在 CI 的 Mac 上量现状（下面），要加另开一步。

### 做了什么

- `miyu-session` 的 `sandbox/caches.rs`：环境变量表多 `XDG_CACHE_HOME`（`<沙盒的缓存>/xdg`）、`XRE_PROFILE_PATH`（`<沙盒的缓存>/firefox`，备的时候建好），另一张表 `BROWSERS` 是那几个固定值。
- `miyu-basesystem` 的 `shell.rs`：`CHROMIUM_SIGNS` 四句原话（建不起资料目录、setuid 辅助程序不对、zygote 提前退出、没有能用的沙盒），失败（退出码不是 0、被信号杀掉）的结果里认出一句（截了的看头尾两段），末尾接 `chromium-sandbox.txt`。脚本工具共用。
- 蓝图 `session/tools.md` 第 1a 条第 5 步的表、`tools/shell.md` 的结果；登记簿 `26-提示词.md` 多一行。

### 验收

- `miyu-session` 的 `tests/sandbox.rs` 的 `the_workspace_level_gets_its_own_toolchain_caches`：多的几个变量、firefox 的资料目录事先建好。
- `miyu-basesystem` 的 `shell/tests.rs` 的 `chromium_s_own_sandbox_failures_get_a_hint_and_nothing_else_does`：四句原话各认得出、别的失败不接、跑成了的不接。
- 真跑（2026-10-23，照会话沙盒的挂法和这一步设的环境，经这一步编出来的助手）：`firefox --headless --screenshot out.png https://example.com` 一条截成；`chromium --headless --no-sandbox --user-data-dir=/tmp/x --screenshot=out.png https://example.com` 一条截成。
- 手写变异 5 个都红（不接那一句、只认一句、不设 firefox 的资料目录、不建它、少关一层 firefox 的沙盒）；`cargo xtask check`、Windows 版 clippy；碰沙盒、子进程：推 step 分支过三台。

### macOS 的现状（CI 的 Mac 上量的）

2026-10-23 在 CI 的 macOS 上经助手、Seatbelt 真跑一次（只测的分支，测完删了）：Chrome、Edge 无头截图，不加参数、加 `--user-data-dir`、再加 `--no-sandbox` 三种都直接中止（退出码 134，没有输出，一张图都没出）；firefox 不加环境、加这一步设的那几个环境变量两种也都中止（134）。CI 的 Mac 上没装 Chromium。所以 macOS 上沙盒里的浏览器现在一个都起不来，这一步设的环境变量在 macOS 上也照样设，但救不了；看样子是 Seatbelt 规则缺的东西（主会话说的 `mach-register` 那一类），要加另开一步给主会话审。

### 不做什么

- 不换 `HOME`、`XDG_CONFIG_HOME`，不上叠加层（见上）。不放开 dbus（会话总线能叫外面的程序干活）。
- 不让浏览器自己那层沙盒在我们的沙盒里起来（要给所有命令新开进程命名空间、`/proc` 可写，改动大）。
- macOS 的 Seatbelt 规则这一步不动。
