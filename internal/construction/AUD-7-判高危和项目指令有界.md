## 施工单 AUD-7：判高危时换变量有上限，项目指令只读普通文件、有界地读

状态：做完了，等合并（分支 `step/AUD-7-判高危和项目文档有界`，从 main 开）。

### 为什么

2026-11-28 审计 LiteMiyu（终端那一轮）时读出两处缺陷，LiteMiyu 的这两段代码是从 Miyu 搬过去的，Miyu 核心是同样的写法，core1 派到核心上核实，有就修。两条在核心上都成立：

1. **判高危时换变量呈指数增长**（审计笔记 c3 的 F4）。`crates/miyu-policy/src/hazard/vars.rs` 的 `assign` 把单独成句的赋值换好存回，`expand` 换的时候不限长度。`x=$x$x$x$x$x$x$x$x$x$x` 这类赋值每一句都把值放大十倍，两百来字的命令照字面换要十几 GB。判高危在命令跑起来之前、在核心进程里做，模型发一条这样的命令，判这一步就能把核心吃爆，所有会话一起没了。
2. **项目指令先整份读进来再截**（审计笔记 c1 的 F2）。`crates/miyu-session/src/project_docs.rs` 的 `read` 先 `read_to_string`、读完才看 `is_file`，32 KiB 的上限也是读完才截。仓库里的 `AGENTS.md`、`CLAUDE.md` 可以是 git 里的符号链接：指向 `/dev/zero` 的每一轮开头都往内存里读到吃光；指向 `/dev/tty`、FIFO 的一直等着读，这一轮永远开不了头。子目录里的那一份在她读、改那个子目录的文件时也走同一个 `read`。

### 改成什么样

1. **换变量有上限。** 一个词换出来最多 64 KiB（`vars::EXPANDED`）。超了的不换，交回原样（照说不清），赋值的那个名字忘掉；同时记下这条命令看不透，`rules::look` 最后补一条 `Kind::TooDeep`。看不透照现有那一路走：问人，没人确认的地方直接拒；不归哪一类，开关关不掉。平常的路径、拼起来的参数离 64 KiB 很远，判法不变。没有碰 SH-3 刚改过的 `fed.rs`、`lex.rs`。
2. **项目指令只读普通文件、有界地读。**
   - 先 `metadata`（跟着链接走）看是不是普通文件，不是的当没有、记一行 debug（`project doc not a regular file, skipped`，照 LG-1 不带路径），这一层照旧接着找 `CLAUDE.md`；指向普通文件的链接照读（`CLAUDE.md -> AGENTS.md` 是常见的写法）。
   - 打开以后对打开的那一份再看一次是不是普通文件（查过以后被换掉的）。
   - 最多读 `LIMIT + 4` 个字节（再多读一个字节，知道后面还有没有）。后面还有、读到的结尾切开了一个字的，丢掉那半个字：剩下的仍然比总上限长，截不截、截在哪、截断那一句和整份读完再截一模一样。

### 和原来不一样的地方（要拍板）

只在比 `LIMIT + 4`（32 772）字节还长的文件上有两处不同，进请求的字在正常文件下一个字节都没变：

- **前 32 772 字节以后才出现不是 UTF-8 的字节的：** 原来整份读完才看得出，当没有（这一层改找 `CLAUDE.md`）；现在看不到后面，照截了交。推荐接受：截掉的部分她本来就看不到。
- **子目录那份「交过没有」的指纹：** 执行器记的是读回来的字的指纹，现在只算开头那一段。改动只发生在 32 KiB 以后的，原来会再交一份（截出来的字一样），现在不再重交。推荐接受：重交的那一份和交过的一字不差。

### 验收

- 修之前测试是红的，修之后是绿的。
- 请求形状探针：正常文件下改前改后请求逐字节一样。
- 门禁、Windows 目标的 clippy、三个平台的 CI 全部通过。

### 做下来的

新测试都套 `MemoryMax=2G`、不用 swap 的 cgroup 跑，测试自己吃爆也只杀这一个 scope。

- **改前红**（本机，改动退回 main 的写法）：
  - `hazard::tests::sizes::doubling_assignments_are_too_deep_to_read`：1 秒内内存到 2G 上限，被内核的 OOM 机制杀掉（journal：`Failed with result 'oom-kill'`，`2G memory peak`）。
  - `hazard::tests::sizes::a_word_expanding_past_the_limit_is_too_deep_to_read`：一个词换出来 100 KiB 照常换，判下来没有看不透这一条。
  - `project_docs::tests::a_doc_linked_to_dev_zero_is_skipped`：同样被 OOM 杀掉。
  - `project_docs::tests::a_doc_linked_to_a_fifo_is_skipped`：十秒没交回，「fifo: 卡住了」。
  - 守着不变的两条（`a_doc_linked_to_a_regular_file_is_read`、`a_bounded_read_cuts_where_reading_it_all_would`）改前改后都绿。
- **改后绿**（.91）：上面六条都过，`miyu-policy` 263 条全过，`miyu-session` 的单元测试 411 条全过。
- **请求形状探针** `request_shape_probe_project_docs`：根上 20 001 字节、`sub/CLAUDE.md` 是指向普通文件的链接且在剩下的上限处切开一个 emoji、子目录 `deep/AGENTS.md` 比上限长且在上限处切开 `é`，回合开始一块、读文件后补一块，两块都截、都说截断那一句。改前（本机）改后（.91）两份 JSON 比下来，只有两处不同：`<env time=…>` 的时段和 `<session id=…>`，都是每次跑都会变的；项目指令的几块逐字节一样。
