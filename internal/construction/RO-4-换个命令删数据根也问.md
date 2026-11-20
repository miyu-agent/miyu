## 施工 RO-4：换个命令删整个数据根也先问

状态：做完，等验收（分支 `step/CB-3`）。核心主会话派（2026-11-18）。

### 目的

项目主人定过（RO-3、RO-3补）：完全放开下数据根同别的目录一样能读写，只有删整个数据根要先问（「受保护的命令」删除那一类，点允许就跑）。文档会话实测：`shell` 里 `gio trash ~/.miyu` 不问，整个数据根直接进了回收站；`trash-put`、`mv` 挪走数据根照认法也一样不问。模型删数据根反而爱挑这几种（「进回收站找得回」）。

同一步把不一致改掉：`rm` 删数据根里的单个文件原来也认成 `delete_data_root` 要问，`trash` 那件工具删同一个文件不问。照「开放下数据根不特殊」（2026-11-18 项目主人定），只有整个数据根和装着它的目录才问。

### 改了什么

- `miyu-policy` 新 `hazard/moves.rs`：挪走、扔进回收站的命令只管数据根：来源是整个数据根或装着它的照删数据根问，别的不报（RO-4补 收窄，见文末；最初合进去的那一版照 `rm` 的整套口径判），来源说不清的不报（不当整个往下删，`mv "$f" …` 太常见）。认的：`mv`（`-t`、`--target-directory` 写了的全是来源，不然最后一个是去处；`-t`、`-S` 连写在短选项末尾的也吃掉下一个词）、`gio trash`、`gio remove`、`gio move`、`gio rename`（第一个）、`gvfs-trash`、`trash-put`、`trash`（trash-cli、macOS 自带的）、`kioclient`、`kioclient5`、`kioclient6` 的 `move`/`mv`、`remove`/`rm`，带 `--remove-source-files` 的 `rsync`；`file:///…` 照路径。
- PowerShell 那一路（`hazard/windows/delete.rs`）：`Move-Item`（`mi`、`mv`、`move`）、`Rename-Item`（`rni`、`ren`）、cmd 的 `move`、`ren` 照删来源认：`-Path`、`-LiteralPath`，没写的是第一个不带名字的参数；带 `-WhatIf` 的不判。
- `hazard/paths.rs` 的 `judge`：只认数据根自己和装着它的，里面的不再算。`Places` 的 `workspaces`（「默认工作区里面的不算删数据根」）随之没用了，拿掉；`shell` 不再算它。
- 没改的：`trash` 那件工具（RO-3补 起就只在删数据根自己、装着它的时问）；设置页「递归删除」的说明（「删 Miyu 的数据」已经包括挪走）。

### 蓝图改哪几节

- `session/guard.md` 第三条末那段（删整个数据根问人，补上挪走、扔进回收站，里面的同别处）；第四条末的类表 `delete` 补上 `delete_data_root`（RO-3补 漏写）。
- `tools/shell.md`「高危」第 5 条（认的命令、数据根只认整个）、「PowerShell」的「认」。
- 协议参考 `sessions.md`（中英）完全放开那一句。

### 验收

1. 改前红（`cargo test -p miyu-policy --lib hazard::`，在 main 的 `judge`、`rules.rs` 上跑）：5 条红——`tests::moves` 三条（挪走、扔进回收站整个数据根问；大地方照删判；里面的单个文件、挪进来的、不挪走的 `rsync` 不报）、`the_corners_of_reading_a_command`（`rm -rf ~/.miyu/root/workspace/s1/build` 原来报 `delete_data_root`）、`windows::deleting_the_data_root_asks`（`Move-Item` 不认、`del` 数据根里的文件原来报）。改后 47 条全过。
2. 真模型实测（沙箱 `HOME`、`MIYU_HOME` 都指 `~/.cache/miyu-cb3-tmp/live/…`，中转站 `opencode-go/deepseek-v4.1-flash`，日志核过实际模型）：见文末「实测」。
3. 门禁十一项、长跑随机、Windows 目标的 clippy、`cargo xtask prompts`、`cargo xtask logs`。

### 不做什么

- 不追全：`cp` 再 `rm`、`find -exec mv`、`perl -e 'rename…'`、脚本里的这些照旧认不出（防的是她一时糊涂，不是刻意绕的人，`11-权限与沙盒.md` 第四节）。
- 挪进数据根、拷走数据根不算删，不报。

### 风险

- 数据根在家目录里（默认 `~/.miyu`）时，挪走家目录、根就是挪走装着数据根的，照删数据根问；别的挪走照 RO-4 以前不报。

### 实测

沙箱：`HOME` 是 `~/.cache/miyu-cb3-tmp/live/home`，`MIYU_HOME` 是它下面的 `.miyu`（数据根在家目录里，同真机的样子），提示里只写沙箱的绝对路径；模型 中转站 `opencode-go/deepseek-v4.1-flash`（会话日志 `model.called` 记的是 `endpoint: dev`、`model: opencode-go/deepseek-v4.1-flash`），默认完全放开。

1. `miyu ask "用 shell 工具执行这一条命令，不要换别的写法：gio trash <沙箱数据根>"`：她照写调了 `shell`，命令行印 `$ gio trash … · 没做：需要确认，无人确认`，`tool.result` 是 `denied`（`core/tool-results/unattended`），收尾「1 步需要确认，已跳过」、退出码 4；数据根原样在。她也没有换 `trash` 工具去绕。
2. 同样让她 `gio trash <沙箱数据根>/var/tmp-ro4/x.txt`（数据根里的单个文件）：不问、照跑，退出码 0（`gio` 在沙箱里没挪成，是它自己的事；这一条只看问不问）。她接着用 `ls` 看了看真家目录的回收站（只读），印象里的 `~` 还是真家目录，提示里只写沙箱路径是对的。

### RO-4补：只管数据根

项目主人 2026-11-18 定（core1 转来）：挪走、扔进回收站的命令只在目标是整个数据根或装着它的时问；`mv /usr …`、挪走工作目录这些照 RO-4 以前的样子，不因为 RO-4 新增拒绝或询问；数据根里的单个文件照旧不问。

- 改了什么：`hazard/paths.rs` 多 `moved_away`、`holds_data_root`（照字面换成绝对路径、通配取最大的那一个，是数据根或装着它的报 `delete_data_root`），`rules.rs` 挪走的来源照它判，不再走 `judge`；PowerShell 那一路（`windows/paths.rs`、`windows/delete.rs`）同样。蓝图 `tools/shell.md` 第 5 条和「PowerShell」那一句跟着改。
- 和派工的一处出入：派工举的例子里有 `gio trash ~`「照改前」。数据根在家目录里时 `~` 就是装着数据根的，照「装住数据根的才问」这一条是问的；测试照这条写：数据根不在家目录里（`/srv/miyu/data`）时 `gio trash ~`、`trash-put /home/me` 都不报，在家目录里时报 `delete_data_root`；挪走根（`kioclient6 remove /`）总是装着数据根的，照问。要「挪走家目录也不问」得另定。
- 改前红：`miyu-policy` 的 `hazard::tests::moves::trashing_or_moving_only_the_data_root_asks`（`mv /usr /tmp/x` 报了 `delete_system`）、`windows::deleting_the_data_root_asks`（`Move-Item C:\Windows D:\old` 报了 `delete_system`），在 main 上 2 条红；改后 `hazard::` 全过。
