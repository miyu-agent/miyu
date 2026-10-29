## 施工单 TMP-1 补3：核心停下等记忆的读写做完

状态：写好了（2026-10-29）。O-61 的 Linux CI 门禁「临时目录」头一回剩一个 `miyu-core-*`，里面是 `modules/memory/_none/000000000001.jsonl`、`index/recall/turns-_none.db-wal`、`blobs/…` 等 9 个；重跑过了。主会话定（2026-10-29）：照 TMP-1 补2 的做法，先在本机做出能复现的测试，再找根因，修在产品上，不在夹具上补。

### 根因

回合开始问记忆的挂接点（常驻那一块、人卡、联想）在阻塞线程里读，各自限时 300 毫秒（`actor/model.rs` 的 `HOOKS`）；过了会话就不等了，那次读照样读完。头一回读一间还要建记忆日志、开库，CI 上慢，常常超时。`crates/miyu-core/tests/serve.rs` 里两个「收到停的信号」的测试，会话停了、核心交回了、测试删了数据根，那次读还在读：本机插桩打出来的先后是「删数据根」在前、「摘要读完」在后。删的时候它正往里建，`remove_dir_all` 删不干净，剩下的就是门禁报的那几样。

回合索引的后台（`memory/lane.rs`，一个核心一个线程）、补齐旧会话的线程（`memory/backfill.rs`）也一样：核心停下时没人等，`Memory::settle` 原来只有删会话、搜以前的对话和测试用。

### 做了什么

1. 回合开始问记忆的那几次读登记下来（`crates/miyu-session/src/memory/reads.rs`）：派到阻塞线程以前登记，读完放下。`Memory::finish` 先等登记着的都读完，再走 `Memory::settle`（回合索引的后台、补齐）。读在前：联想头一回开回合库会起补齐。
2. 核心停下时等它（`crates/miyu-endpoint/src/lib.rs` 的 `Core::finish_writes`，在阻塞线程里等）：`Core::stop_sessions` 停完会话就等；`serve_then` 空闲退出的没停会话，停下以后、收尾（`closing`）以前等。那时锁还在手里。等的线程 panic 了记 `ERROR memory writes wait panicked`。
3. 测试用的口子：`miyu-session` 的 `testkit` 里 `Memory::slow_reads`，只让这一份记忆的读先慢下来，别的测试不受影响；`miyu-endpoint` 的 `testkit` 交出 `Core::memory`（只有测试的构建有），`miyu-core` 的测试经 dev-dependencies 打开它。
4. 蓝图：`core.md`「在哪」「停下」第 4、6 条、出错、守着它的；`memory.md`「在哪」、第三条第 4 款、守着它的、施工步子。

### 不做什么

- 不给夹具加「删完再看一眼」。
- 记忆抽取在路上的那一次（`actor/extract.rs` 派出去的 `Job`）、照意思找补向量（`memory/vectors.rs`）、在跑的工具里的阻塞写：这一步不收。它们要真模型、语义模型或者工具在跑，核心测试里没有；之后再撞上照报出来的文件另修。
- 知识库的自动想起（`knowledge/recall.rs`）同样限时不等，可它只读、核心测试里没有知识库，这一步不收。

### 验收

1. `crates/miyu-core/tests/serve.rs`：记忆的读慢 1.5 秒（比限时长得多），收到停的信号、空闲退出各一条，交回时那次读已经读完（头一回读建的记忆日志在了）；修之前两条都红。
2. `crates/miyu-session/src/memory/reads/tests.rs`：登记着的都读完了才交回；没有登记着的当场交回。
3. `cargo xtask check`；Windows 版 clippy；三平台 CI。
