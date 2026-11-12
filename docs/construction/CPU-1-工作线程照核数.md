## 施工单 CPU-1：工作线程照核数

状态：做完了（2026-11-12）。基准测试 bm2 报告（2026-11-11）第 3 节查到的；项目主人 2026-11-12 定做法，细节 core1 2026-11-12 定。

### 为什么

核心的 tokio 运行时原来写死 2 个工作线程，注释写的是「都是等 I/O，两个够用，占用也低」，没有实测过。整机 CPU 被一千多个线程挤满时，Linux 大体按线程分 CPU。这 2 个线程约 75% 的时间在排队等 CPU，10 分钟里各只拿到约 7 秒：长会话一轮从 0.65 秒掉到 10 秒，线程多的别家 0.5 秒（报告 §3.2「只压 CPU+内存」、§3.3）。

项目主人 2026-11-12 定：工作线程数默认等于 CPU 核数，配置里可以改，最少 2 个。

### 做了什么

1. 默认照 `std::thread::available_parallelism()`，取不到的照 2，夹到 2 到 64（`crates/miyu-core/src/workers.rs`）。
2. 配置项 `core.workers`（`serve.rs` 的 `CoreSettings`）：整数 0 到 64，默认 0，0 表示照核数，写 1 的在代码里照 2（「0 照核数」和「最少 2」放不进同一个范围）。超过 64 的照仓库的规矩报 `out_of_range`，这一项不收，照核数。只在系统一层，「高级」页「核心」组。
   - 中文：「核心的工作线程数」「核心用几个线程处理连接、会话和请求。0 表示照 CPU 的核数（默认）；写 1 的照 2 个。」
   - 英文：「Core worker threads」「How many threads the core uses for connections, sessions and requests. 0 follows the CPU core count (the default); 1 counts as 2.」
   - 日文：「コアのワーカースレッド数」「コアが接続、セッション、リクエストの処理に使うスレッドの数です。0 は CPU のコア数に合わせます（既定）。1 は 2 として扱います。」
3. 生效时机多一种 `restart`（`miyu_config::Applies::Restart`），这是第一项用它的配置。字照蓝图早就定好的写：「重启核心后生效」「Takes effect after the core restarts」「コア再起動後に反映」（`config/applies/restart`）。命令行早就认得 `restart`。包清单的配置项也能写 `applies = "restart"`（读「几层」「什么时候生效」的两个函数从 `package/settings.rs` 挪进 `package/settings/scope.rs`：那一份过了 500 行）。核心空闲时自己重启另起一步，没排（core1 2026-11-12 定）。
4. 早读：运行时要先建，配置在运行时里读。不过读配置本身是同步的，所以 `main` 在建运行时以前单独读一次系统配置，只认这一项（`miyu_store::config_file::read` 加 `miyu_config::parse::parse`，清单只给 `CoreSettings`）。读不到、写坏了、超范围的照核数，不报，后面正式读配置时照旧报。启动的先后不变，多读一次小文件（core1 2026-11-12 定，没选「把建工作区、读清单、读配置挪到建运行时以前」：那样要改先后）。运行日志在 `DEBUG` 记一条 `runtime workers=<个数>`。
5. 不动的：
   - tokio 的阻塞线程池、自己开的线程。
   - glibc 的 arena（`miyu_heap::ARENAS`，8 个）：它不照工作线程数算，对的是读日志分块的那几块（`miyu-store` 的 `MOST`，8 个）；`tune` 要在起任何线程以前调，那时还没读配置。工作线程多了以后共用 arena 抢不抢锁，等 bm 复测量了再定。
6. 蓝图：`core.md`「起来的先后」第 7 条和文件表，`config.md`（文件表、生效时机那张表、第八条第 1 条、`miyu config set` 那一行、「还没有的」，里面嵌着的参考文件样本），`heap.md` 的 `ARENAS`，`packages.md` 的 `applies`。参考：`docs/authors/manifest.md` 中英两份的 `applies`。样本 `docs/designs/samples/config/` 重新生成。

### 空闲占用前后

本机 12 核（Linux）。数据根是新建的测试沙箱，不连头、不联网（`MIYU_CATALOG_UPDATE=false`），读到 `ready` 以后等 10 秒，读 `/proc/<pid>/status` 的 `Threads`、`VmRSS` 和 `smaps_rollup` 的 `Pss`。「前」用同一个程序、配置写 `core.workers = 2`：和原来写死的 2 一样，只差多读的那一项。每种起三次、交替着起，内存取中位，括号里是三次的范围。

| 构建 | 工作线程 | 线程数 | VmRSS | Pss |
|---|---|---|---|---|
| release | 2（前） | 12 | 25.5 MiB（25.0～25.5） | 22.6 MiB（22.2～22.8） |
| release | 12（后，照核数） | 22 | 25.8 MiB（25.7～26.2） | 23.0 MiB（22.9～23.4） |
| debug | 2（前） | 12 | 80.3 MiB（77.6～80.7） | 77.5 MiB（74.8～77.9） |
| debug | 12（后，照核数） | 22 | 79.8 MiB（79.5～81.6） | 77.0 MiB（76.7～78.8） |

多 10 个线程，release 空闲多约 0.3～0.4 MiB（一个线程约 40 KiB：栈只碰到的几页、线程自己的那一份），三次之间的抖动和它差不多大；debug 里看不出差别。

### 留给 bm 复测的

照 bm2 报告 §3.2 的 `run-ioab.sh`：长会话 10 分钟，同样 50 个后台会话，「只压 CPU+内存」「三样一起压」两种，Miyu 用这一版核心。对照：同一个程序配置 `core.workers = 2`。看三样：

- 轮数和一轮的中位；
- 工作线程处在 R 状态的比例；
- 工作线程多了以后有没有抢 arena 的锁：`perf` 看 `__lll_lock_wait`、`arena_get2` 一带。

### 验收

- `crates/miyu-core/src/workers/tests.rs`：
  - 默认照核数并夹住（1 核照 2、256 核照 64、取不到照 2）；
  - 写 0、1、3、64 各自的结果（0 照核数、1 照 2）；
  - 超范围、负数、字、小数、整份写坏的照核数；
  - 没有配置文件、不是 UTF-8 的照核数；
  - 有 BOM、别的项、不认识的键都不碍事。
- `crates/miyu-core/tests/workers.rs`：
  - 核心登记的清单里有 `core.workers`，默认 0，只在系统一层，`applies` 是 `restart`；
  - 连上核心问 `config.schema`：整数 0 到 64，`applies` 是 `restart`，「高级」页「核心」组；
  - `config.set` 写 3 的回应说 `restart`，写 65 的不收。
- `crates/miyu-config/src/package/settings/tests.rs`：包清单写 `applies = "restart"` 读成 `Applies::Restart`。
- `crates/miyu-core/tests/settings.rs`：登记的先后、三种语言的名字和说明、生成的文件和样本逐字节一样。
- 命令：
  - `cargo test -p miyu-core -p miyu-config -p miyu-endpoint --no-fail-fast`
  - `cargo clippy --workspace --all-targets -- -D warnings`，同上加 `--target x86_64-pc-windows-gnu`
  - `cargo fmt --check`
  - `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
  - `cargo xtask prompts` 无差异
