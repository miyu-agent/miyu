## 施工单 SB-1（修二）：chromium 的 crash 上报在沙盒里起不来时也教她

状态：合了（2026-10-23）。主人重验 SB-1（验收树 a47b6409c，工作区级别）：1、2、3、5 条过了，第 4 条没过；主会话转来，根因主人那边的 AI 用 strace 查出来了，修法主会话定。

### 撞见的

照 SB-1 的提示加了 `--no-sandbox --user-data-dir=$(mktemp -d)`，还是没图，退出码 133（SIGTRAP）。输出里有 `chrome_crashpad_handler: --database is required`、`recvmsg: 连接被对方重置 (104)`。末尾没有提示：这种崩不在 `CHROMIUM_SIGNS` 里。

### 查下来的

- `~/.config/chromium` 在出厂的私密目录清单里（`miyu-fs` 的 `private/shipped.rs`），沙盒把它藏成只读的空目录。crashpad 照 `XDG_CONFIG_HOME` 在 `chromium/Crash Reports` 建库，建不了，handler 秒退，主进程跟着 SIGTRAP。`--crash-dumps-dir`、`--disable-breakpad`、`--no-zygote`、`--single-process` 都压不住（主人那边试的）。
- 在这一条命令前加 `XDG_CONFIG_HOME=$(mktemp -d)` 就过：主人连跑 3 次都出图，图都是 42581 字节；我这边照同样的情形复测也是 42581 字节。
- 我上次复测为什么没崩：测试场地的会话拿的是场地的假家目录，私密目录照它算，藏的是假家目录下的 `.config/chromium`；chromium 用的却是进程环境里真的 `HOME`，那里的 `~/.config/chromium` 没被藏（它本来就有、只读，crashpad 只报打不开 `settings.dat`，不崩）。这一步的测试改成在假家目录里建好 `.config/chromium`、命令里把 `HOME` 指过去，和真机一样被藏。
- 崩的说法不止一种：主人那边是 `--database is required`；我这边是 `mkdir …/Crash Reports/new: Permission denied` 加 `recvmsg` 那句，后半句随系统语言变。

### 定的（主会话）

只改提示那一句：让她在这一条命令前加 `XDG_CONFIG_HOME=<临时目录>`，照旧加 `--no-sandbox --user-data-dir=<临时目录>`。不动挂载，不改全局的 `XDG_CONFIG_HOME`，主人 10-23 定的「`~/.config` 只读」守住。`--headless=old` 不教（旧 headless 在被拆）。

### 蓝图改哪几节

- `tools/shell.md` 第 9 条：`CHROMIUM_SIGNS` 多两句，表里的原文换掉。
- `26-提示词.md` 登记簿那一行换指纹（`c20feeb3`）、约 50 token（估的），`prompts.md` 重新生成。

### 做了什么

- `CHROMIUM_SIGNS` 多认两句：`chrome_crashpad_handler: --database is required`、`Crash Reports`（只在失败的结果里认，跑成了的照旧不接）。
- `chromium-sandbox.txt` 换成：`Chromium can't start its own sandbox, profile or crash reporter inside this sandbox, which already contains it; run it as XDG_CONFIG_HOME=$(mktemp -d) chromium --no-sandbox --user-data-dir=$(mktemp -d) <your other arguments>.`

### 不做什么

- 不把 `~/.config/chromium/Crash Reports` 挂成可写，不改全局 `XDG_CONFIG_HOME`、HOME。

### 验收

- `miyu-basesystem` 的 `shell/tests.rs`：crashpad 的两种说法都接提示，提示里有 `XDG_CONFIG_HOME`、`--no-sandbox`、`--user-data-dir`。
- `miyu-session` 的 `tests/sandbox_browser.rs`（装了 chromium、有收紧手段的机器才跑，CI 上跳过）：假家目录的 `.config/chromium` 被藏，照旧提示的命令崩、结果末尾接上新提示；照新提示加 `XDG_CONFIG_HOME` 就出图。去掉新认的两句、换回旧提示，各红一次。
- `cargo xtask check`、Windows 版 clippy；推 step 分支过三台。

### 风险

- `Crash Reports` 这一句比较宽：别的失败输出里碰巧带这个词，也会接上这句提示。只在失败时接，多花约 50 token，不影响结果。
