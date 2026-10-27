## 施工单 ED-2：write 没读过的文件也能写

状态：已完成（2026-10-27）。经过：2026-10-27 拆 pi 提示词的对照实验（`~/Projects/miyu-remake-benchmark/runs/2026-10-27/`）里，用 miyu 原来的 `write` 说明的 21 次有 4 次整份重写 `cli.py`，其中 3 次撞上「没读过，先读」：她用 shell 的 `cat` 看过了，可那不算 `read`，只好再读一遍、再整份写一遍，这几次都是最贵的那一头；项目主人当天定照 ED-1 去掉 `write` 的「先读」。设计 `10-自带软件.md` 第三节「`write` 的细则」、第五节「她看过的」；图纸 `tools/write.md`「是什么」「怎么走」第 4 条、出错表、「给人看的字」「守着它的」。

### 目的

覆盖一个已经在了的文件不用先 `read`：她多半已经知道内容（shell 里看过、任务里给过），先读一遍只是为了过这道检查，多花一轮、多读进一份内容。读过、之后文件又被改了的照旧拦，免得照着旧内容盖掉人或者别的程序刚做的改动。

### 做法

1. `write.rs` 改用 `Common::stale`（ED-1 加的，只拦看过、现在的内容又变了的）。`Common::unseen` 和 `common/not-read.txt` 没人用了，删掉；登记簿去掉那一行，`prompts.md` 重新生成。给人看的 `common/not-read` 那两句留着：以前的日志里有这个说法，翻出来照样显示。
2. `tools/write.json` 的说明去掉「A file that already exists must be read first.」，登记簿换指纹、`prompts.md` 重新生成。「To change part of a file, use `edit`.」留着：改一部分用 `edit` 省输出，和先读无关。
3. 会话里拿「没读过被拒」来看「她看过的」记没记对的几条测试（重新载入、删了、撤销、恢复），改成拿「读过以后又被改了」来看：记着的会拦，忘了的会写。

### 不做

- 「读过以后又被改了」照旧拦，理由同 ED-1。
- shell 里 `cat` 过的不算看过：前台命令只记它改了的文件（施工 E-D 补），看了哪些认不出来。

### 验收

- `crates/miyu-basesystem/tests/write.rs` 的 `an_existing_file_is_written_unless_it_changed_since_seen`：没读过的照写；读过以后被改了的不写、文件不动；看的是现在的样子就写；自己刚写过的接着写。改代码之前它是红的。
- `crates/miyu-session/tests/write.rs`、`undo_files.rs`：重新载入以后读过的照样算、删了的不再算看过、撤掉的那一轮读过的不算、恢复以后又算，都拿「又被改了」看出来。
- 手写变异八个全杀：write 不核对、比反、没看过的照旧拦、删了的不拿掉、读过的不记、写过的不记、撤着的回合照算、恢复不认。
- 本机门禁全过，Windows 版 clippy 过，三平台 CI 过。
