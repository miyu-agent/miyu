## 施工单 SB-1（修）：firefox 截图写相对路径时告诉她写绝对路径

状态：合了（2026-10-23）。项目主人验收 SB-1 没过（验收树 02684f665，工作区级别，Linux），主会话转来要修。

### 验收撞见的

1. firefox 用相对路径静默不写：`firefox --headless --screenshot out.png <网址>` 退出码 0，工作区里没有 `out.png`；写成 `"$PWD/out.png"` 能截成。
2. chromium 原样跑报 `Failed to create a unique user data directory for headless.`，退出码 1；她换了能写的 HOME、再加 `--no-sandbox --user-data-dir` 才跑通。要复测的是只照末尾提示改、不动 HOME 能不能一次成。

### 查下来的（照真会话的沙盒，带沙盒缓存，HOME 是真的家目录、只读）

- firefox：起来先 `chdir("/")`（strace 看到的），相对路径落到 `/`，写不进，也不报错。沙盒外一模一样（同样的资料目录、`--profile`、`./out.png` 都丢），是 firefox 自己的行为，Miyu 这边修不了。绝对路径一次成。
- chromium：原样跑那一句在 `CHROMIUM_SIGNS` 里，末尾提示出现了；照提示只加 `--no-sandbox --user-data-dir=$(mktemp -d)`、不改 HOME，一次截成，图在工作区。crashpad、dbus 刷一串 `ERROR`（往只读的 `~/.config/chromium/Crash Reports` 写不进），不影响截图。
- 所以「改一次就成」不用动 `XDG_CONFIG_HOME`、HOME，SB-1 定的第 1 条照旧。

### 蓝图改哪几节

- `tools/shell.md` 第 9 条：firefox 截图写了相对路径，末尾接 `firefox-screenshot.txt`；表里加一行。
- `26-提示词.md` 登记簿加一行（`7c5d6079`），`prompts.md` 重新生成。

### 做了什么

- `shell` 跑完了、命令里调了 firefox、`--screenshot` 的路径是相对的（或者没写路径）的，不管跑成没有，结果末尾接一句：`Firefox switches to / before it writes --screenshot, so a relative path is dropped without an error; give it an absolute path, such as "$PWD/shot.png".`
- chromium 不动：提示原来就对，复测一次成。

### 不做什么

- 不改 HOME、`XDG_CONFIG_HOME`，不替她改命令。
- 不进 `shell` 的说明：只有撞上才花 token。

### 验收

- `miyu-basesystem` 的 `shell/tests.rs` 的 `firefox_s_relative_screenshot_paths_get_a_hint`：相对路径、`=` 写法、带引号的 `./`、没写路径、`firefox-esr`、全路径的 firefox 都接；`"$PWD/…"`、绝对路径、`~/`、chromium、不截图的不接；`tests/shell.rs` 的 `a_relative_firefox_screenshot_gets_told_to_use_an_absolute_path`（真跑一次 `shell`，接得上）。
- 真沙盒复测（会话沙盒的规格，带沙盒缓存）：
  - firefox 相对路径：末尾有那句提示，照它改成 `"$PWD/out.png"`，一次截成。
  - chromium 原样跑：有 `Failed to create…`、末尾有原来的提示，照它加两个参数，一次截成。
- 手写变异 5 个都红（另有一个活下来，查出是多余的判断：下一个词是网址时照同一条规矩判结论一样，删了）；`cargo xtask check`、Windows 版 clippy；推 step 分支过三台。

### 风险

- 认命令是照词认的：变量里放了 firefox、脚本里调的认不出，那时退回到原来的样子（退出码 0、没有图），她自己查得到。
