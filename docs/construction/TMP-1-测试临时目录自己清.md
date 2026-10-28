## 施工单 TMP-1：测试的临时目录自己清

状态：写好了（2026-10-28）。主会话 2026-10-28 派：本机 `/tmp` 里攒了十二万个 `miyu-*` 目录，把用户配额顶满，Bash 和终端里的 `!` 命令都跑不了。先量，再修夹具，再加门禁守着。`miyu-core` 的状态、配置那一处写的、`miyu-endpoint` 的测试夹具和 config 一片等 SU-3 上合了才动（那一步在改同一片），变基到 SU-3 上以后做完。图纸 `docs/blueprint/store.md`「怎么走」第四条。

### 量出来的

门禁那样跑一遍全工作区的测试，`TMPDIR` 指到一个新的空目录，跑完数：565 个目录、44 MB。照名字分：

1. **夹具删了数据根，后台还在写的又一层层把它建回来**（大头）：会话的回忆库、用量库、汇率库、`state/log-salt`、`journal.jsonl`、记忆模块的 `*.jsonl`、blob、MCP 的缓存。夹具 `Drop` 时删了整个临时目录，后台任务晚几毫秒写，`create_dir_all` 把数据根连同一路的上级建回来。光 `miyu-session` 的就两百多个。
2. **夹具只删自己的目录，不删旁边派生的**：沙盒给命令的临时目录 `<目录>-sandbox-tmp`、`pkg_pack` 的 `<目录>.pack-alone` 这类。
3. **测试里的命令照系统的临时目录建**：`mktemp -d`、chromium 自己的。
4. **本来就活得比进程长的**：`miyu-call-<进程号>`（核心的回调目录，下一个核心进程起来清）、`miyu-scripts`（没有数据根时脚本的缓存，名字固定）。不算漏。

### 做了什么

1. `miyu-store`：建目录（`durable::create_dir`）只在数据根里建：往上找不到 `.miyu-root` 的报 `NotFound`，不建。SQLite 开库、会话日志、配置日志、blob、任务输出都经它，数据根没了就开不成、写不成，不再建回来。别的 crate 在数据根里建目录用 `root::create_inside`（扩展、MCP 的标准错误文件和工作目录）。建数据根本身、配置文件（项目目录的 `.miyu/`）不在数据根里，用 crate 里的 `create_dir_anywhere`。
   - 生成的文件分两种写法：`generated::write` 哪里都建（模型目录的缓存在机器的缓存目录 `~/.cache/miyu/models` 里，`miyu-client` 也把它转给头写自己的文件）；`write_inside` 只在数据根里建，配置清单（`state/config/`）、MCP 和扩展登记的缓存、`state/models/` 换成它。头一版把 `write` 整个收紧了：测试的缓存目录碰巧在数据根里，全绿，实际用起来模型目录的缓存会再也写不进。逐个查调用方时查出来的。
2. 夹具：
   - `miyu-store` 的 `Scratch::new()` 建好目录、写上标记；`Scratch::bare()` 是还没建、没标记的（测数据根本身、测「不在数据根里」）。
   - 其余 crate 自己造数据根的单元测试（会话日志、blob、任务输出）写上标记。
   - `miyu-endpoint` 单元测试的 `temp_root` 交回带 `Drop` 的目录（原来交回路径、测完不删，写成 `(root, _)` 的当场就丢了）。
   - `miyu-cli`、`miyu`、`miyu-endpoint` 的 `Home` 删目录时连 `<目录>-sandbox-tmp`（`miyu`：还有 `<目录>.<后缀>`）一起删。
   - `miyu` 的 `Home` 删之前等拉起的核心走（最多五秒）：核心晚一步写模型目录的摘要（测试把缓存目录指在数据根里），会把删了的建回来。摘要写在机器的缓存目录里，不归数据根管，修在测试这一头。
   - 临时目录删的时候重试（`miyu-store` 新的 `testkit` 特性的 `remove_tree`，五个 crate 的夹具共用）：后台还在写的（回忆库、会话索引、任务输出）删到一半往还在的目录里添了文件，整个删失败。
   - 量尺（`miyu-perf`）测追加的那一格写上标记、日志放在下面一层：原来直接在 `target/perf/append` 建会话日志，收紧以后建不了（三平台 CI 的量尺查出来的）。
   - 沙盒里跑 chromium 的测试把 `TMPDIR` 指到一个跟着测试删的临时目录（不放进 `target/tmp` 下的工作目录：太深，chromium 的单例套接字路径超长就崩）。
3. 门禁：`cargo xtask check` 跑测试时 `TMPDIR`、`TMP`、`TEMP` 指到系统临时目录下一个新的空目录 `m<进程号>`（名字短：macOS 上 Unix 套接字路径有 104 字节的上限），跑完照名字的前缀数剩下的，报在「临时目录」一项，然后整个删掉。`miyu-call-`、`miyu-scripts` 不算（上面第 4 种）。先加门禁、见红（26 种）再修。只在 Linux 上拦：攒满配额的是开发机（Linux）的 `/tmp`；Windows 上别处还开着的文件删不掉（后台任务、核心进程拿着库的句柄，CI 上一次剩一千多个），要做到零得把每个夹具改成先关掉所有句柄再删，CI 的机器又是一次一扔的，不值；macOS 的几个（`Library`、任务输出）同样照数、照删、只报出来。
4. 图纸 `store.md`。

### 不在这一步

- `miyu-call-<进程号>` 改成进程退出时删：核心被杀掉时照样留，下一个进程清的那一套已经在。

### 验收

1. `miyu-store`：`sqlite/tests.rs`（数据根里开得成；删了的不建回来、报错写明；已经有的目录不要标记）；`generated/tests.rs`（`write` 在数据根外照建，`write_inside` 删了的不建回来）；原有测试照旧全过。
2. `xtask`：`temp.rs` 的前缀分组；不算的两种不报、整个目录照样删掉。
3. 门禁「临时目录」为 0。
4. 手写变异 8 个：不查标记、认错标记、已有的目录也要标记、`write` 也收紧、`write_inside` 不收紧、`create_inside` 不收紧、白名单不认、跑完不删。头一轮 `create_inside` 不收紧活了（没有测试直接测它），补了 `root/tests.rs` 的 `create_inside_does_not_bring_a_deleted_root_back` 才抓住。
5. `cargo xtask check`（「临时目录」为 0）、Windows 版 clippy；三平台 CI。
