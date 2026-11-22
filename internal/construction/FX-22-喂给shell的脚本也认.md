## 施工单 FX-22：危险命令识别补上喂给 shell 的脚本和几种读错的写法

状态：做完了，等合并（分支 `step/FX-22-喂给shell的脚本也认`，从 main 的 088e099ac 开；主会话 2026-11-22 自己做，主人同日定「发现的问题就自己改掉」）。照 LiteMiyu LM-37 的两个提交（379c249、e56f288，lm-kernel 的 `policy/hazard` 是从这里搬过去的），它的施工记录写着这两条「Miyu 没有」。

### 改了什么

- 喂给 shell 跑的脚本里面的也认（照 379c249）：heredoc、here-string 喂给不带 `-c`、不带脚本文件的 shell；管道里 shell 前面紧挨着 `echo`、`printf` 印的字，或者 `cat` 读进来的 heredoc；`env -S`（`--split-string`）的值；`su -c`（`--command`）的值。
- 几种读错的（照 e56f288）：`$"…"` 照双引号读（原来只问）；`~名字` 照说不清（原来当成工作目录下的文件）；`find` 起点前面的 `-H`、`-L`、`-P`、`-D`、`-O` 不再让起点一个都读不到；`-ok rm`、`-okdir rm` 认；`rsync --delete` 清空的去处照删去处判；`setsid`、`pkexec`、`run0`、`busybox`、`ionice` 当外壳剥。
- `rules.rs` 加完超过 500 行：喂给 shell 的几个函数和原来「先下载再交给 shell」的几个一起拆进 `hazard/fed.rs`。
- 和 LiteMiyu 不一样的一处：数据根整个删掉、挪走照 Miyu 的 RO-4 问人（LiteMiyu 是直接拒），这一步不动。
- 用户手册 `guide/permissions.md` 中英补上新认的外壳和写法。

### 验收

- `hazard::tests::fed::scripts_fed_to_a_shell_are_looked_into`、`hazard::tests::audit::audit_reads_that_fx22_fixed`（照 LiteMiyu 的搬，数据目录换成 `~/.miyu`）：改之前都红，改之后绿。测试里的命令只是交给识别器的字，一条都不执行。
- `miyu-policy` 258 条测试过、clippy 过；本机门禁 12 项全过。
