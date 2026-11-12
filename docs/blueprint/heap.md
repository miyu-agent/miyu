## 进程的堆 `miyu-heap`

### 是什么

核心用系统的分配器（`23-性能预算.md` F3）。glibc 释放的内存多半留在自己手里、不还给系统：读完模型目录剩下的、关掉的大会话放下的，核心的内存一直不回落（施工 V-2 中量过：一万条事件的会话关掉以后核心还占 48 MB，空闲时 27 MB）。这个 crate 做两件：glibc 上调两个参数（arena 的个数、大块的门槛），把堆里空着的还给系统。别的平台（macOS、Windows、musl）的分配器自己会还，两件都什么都不做。另交出堆的统计、进程这时占了多少内存，核心照它自己记账（F4，施工 V-4，下面「自己记账」）；还交出这台机器还剩多少可用内存，核心照它决定开不开会话（施工 MEM-1，下面「还剩多少内存」）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-heap/src/lib.rs` | `tune`、`trim`、`stats`、`huge_pages_off`；glibc 上声明、调 `mallopt`、`malloc_trim`、`mallinfo2`，Linux 上关透明大页（施工 YC-12），别的平台是空的 |
| `crates/miyu-heap/src/usage.rs` | 进程这时占了多少内存：Linux 上读 `/proc/self/smaps_rollup`、`/proc/self/status`，两份的读法是纯的（施工 V-4）；macOS 上问内核 `proc_pid_rusage`（施工 V-4 补） |
| `crates/miyu-heap/src/room.rs`、`room/macos.rs`、`room/windows.rs`（施工 MEM-1） | 还剩多少可用内存：Linux 上读 `/proc/meminfo` 和 cgroup v2 一路往上的几层，读法是纯的、读文件的照传进来的函数；macOS 上问 sysctl，Windows 上问 `GlobalMemoryStatusEx` |
| `crates/miyu-endpoint/src/sessions/room.rs`（施工 MEM-1） | 开会话之前照它看一眼，低于 5% 的不开（`protocol.md`「会话表」第 10 条） |
| `crates/miyu-core/src/memlog.rs` | 核心自己记账：什么时候采、一行的样子、采样的任务（施工 V-4） |
| `crates/miyu-core/src/lib.rs` | 起来时最先调好（`core.md`「起来的先后」第 0 步） |
| `crates/miyu-core/src/models.rs` | 读完模型目录 trim 一次 |
| `crates/miyu-session/src/open/load.rs` | 载入一个会话以后 trim 一次（`session/actor.md` 第 2 条） |
| `crates/miyu-session/src/actor/life.rs` | 会话的 actor 结束、它占的放掉以后 trim 一次（`session/actor.md`）：停下、删掉、闲够了退下（`protocol.md`「会话表」第 9 条）都算 |
| `crates/miyu-core/src/settle.rs` | 一阵请求过去、安静 2 秒以后 trim 一次（施工 V-5 中），再看内存涨得快不快（施工 V-6 下）；连接每读到一个请求、连上、断开都叫一声，信号是 `miyu-endpoint` 的 `Activity`（`activity.rs`，几处各拿一份都叫得醒） |

### 对外的样子

- `ARENAS`：8。glibc 默认是核数的 8 倍，多开的只多攒碎片；读日志分块在几个线程里一起解（`store.md` 第 6 条，最多 8 块），少于它就抢锁（限到 2 个时重开一万条事件的会话，读日志从约 10 ms 慢到 16 ms）。不照核心的工作线程数算（施工 CPU-1 把工作线程改成照核数以后照旧 8 个）：它对的是读日志的那几块；`tune` 要在起任何线程以前调，那时还没读配置。工作线程多了以后共用 arena 抢不抢锁，等压测复测量了再定。
- `MMAP_FROM`：128 KiB，多大的一块直接向系统要、放掉当场还。写明了 glibc 就不再自己往上调：默认放掉一块大的以后把门槛抬到它那么大（最多 32 MB），之后每一轮拼的请求（几百 KB）都落进堆里，和长住的穿插着放，空出来的碎成小片、`malloc_trim` 也还不回去。
- `tune() -> bool`：起别的线程之前调一次，glibc 上照 `M_ARENA_MAX`、`M_MMAP_THRESHOLD` 设成这两个，交回都设成了没有；别的平台交回假。Linux 上（不管是不是 glibc）还给这个进程关掉透明大页（`PR_SET_THP_DISABLE`，施工 YC-12，下面「怎么走」第 4 条），关没关上不在交回的里面。
- `huge_pages_off() -> bool`（施工 YC-12）：这个进程的透明大页关着没有（`PR_GET_THP_DISABLE`），核心记进 `DEBUG heap tuned` 的 `huge_pages_off`；别的平台交回假。
- `trim() -> bool`：`malloc_trim(0)`，交回还了没有；别的平台交回假。要走一遍堆，在阻塞线程里调。
- `stats() -> Option<Stats>`（施工 V-4）：glibc 的 `mallinfo2`，各个 arena 加起来：`arenas`（各 arena 从系统要来的）、`mmapped`（单独 mmap 的大块）、`used`（arena 里在用的）、`free`（arena 里空着的），字节；别的平台没有。
- `usage::usage() -> Option<Usage>`、`usage::resident() -> Option<u64>`（施工 V-4）：字节。`rss` 哪个平台都有，别的几样只有量得出的平台有。Linux 上前者读 `smaps_rollup`，多 `pss`、`private`（`Private_Clean` 加 `Private_Dirty`）、`anon`、`swap`；后者只读 `status` 的 `VmRSS`，便宜，看涨没涨用。macOS 上两个都问内核 `proc_pid_rusage`（`RUSAGE_INFO_V2`，一次系统调用），前者多 `footprint`（`phys_footprint`：算在这个进程头上的，活动监视器「内存」那一列，压缩了的也算；施工 V-4 补）。Windows 没有（`23` 第五节「后续再定」）。

### 怎么走

1. 工作区不许 `unsafe`（`forbid`）；这个 crate 照 `miyu-sandbox` 的办法改成 `deny`，只在声明、调用 glibc 的三处单独放开，每处写明为什么安全：`mallopt` 只改分配器自己的参数，`malloc_trim` 只把分配器手里空着的页还回去，已经分出去的不动、glibc 自己加锁。
2. 什么时候 trim：读完模型目录（原文、解析的半成品放掉了）；载入一个会话以后（读、解的整份日志只留最近一次压缩以后的，`session/actor.md` 第 2 条）；一个会话的 actor 结束以后（它的状态机、事件放掉了：停下、删掉、闲够了退下都算）；一阵请求过去、安静 2 秒以后（施工 V-5 中，下一条；trim 之前先放掉这一阵里读好放着的给人看的字，施工 V-30，`store/resources.md` 第 3 节第 8 条）。都在阻塞线程里、不挡着谁。不定时 trim：空着的时候没有新的可还。
   - **安静下来还一次**（施工 V-5 中，`settle.rs`）：头一连上就是一阵请求（读配置、列模型、读人），几百 KB 的回应拼完就放掉，放掉的留在 glibc 手里。施工 V-5 中量过：连上终端以后核心匿名内存多 3.3 MB，活着的数据只多 40 KB。所以连接每读到一个请求叫一声（`Activity::touch`，没人等的记下一次）；盯着的任务等到叫声，再等到 2 秒（`QUIET`）里没有新的叫声，trim 一次，再回去等。没有请求的时候它一直睡着，不定时醒。trim 在阻塞线程里做：崩了的记 `ERROR heap settle panicked`、接着等；停核心时运行时在收尾、阻塞任务被取消的记一行 `DEBUG heap settle cancelled`、不再等（施工 BUG-1：以前不分取消和崩溃，停核心时总打一行误报的 `ERROR`）。
3. 量到的（施工 V-2 再补，`docs/perf/2026-10-10-linux-x86_64-v2再补.md`）：只 trim 的时候，大会话退下以后堆里还有 16 MB 空着的碎片、核心匿名内存比空闲时多 8.8 MB；写明大块的门槛以后只多 3.9 MB，重开大会话、说一句以后 92 MB 降到 49 MB。

4. **透明大页**（施工 YC-12，`23` F3）：系统设成 `always` 时，`khugepaged` 把还有一页在用的 2 MB 一段合成大页，trim 还掉的空洞又被填回去（2026-11-10 压测量过：高压过后 trim 以后 PSS 569 MB 降到 82 MB，五分钟里每分钟涨约 90 MB，回到约 476 MB，在用的堆只有 13 MB，`AnonHugePages` 约等于整个匿名内存）。所以 `tune` 照 `prctl(PR_SET_THP_DISABLE, 1)` 关掉，用 `rustix` 的安全封装，不写 `unsafe`；关不上（3.15 以前的内核）照旧起来，`huge_pages_off` 如实交回假。这个标记 `fork`、`exec` 都传下去：经助手跑的命令由助手换成命令之前恢复（`sandbox.md`「助手的命令行」第 4 步）；扩展、MCP 服务、不经助手的命令跟着关着，只是用不上大页，不碍事。

### 自己记账（施工 V-4，23 F4）

1. **记在哪**：`var/log/memory.jsonl`，一行一个 JSON；满 1 MiB 挪成 `memory.jsonl.1`（盖掉上一份），只留这一份（`miyu_log::RotatingFile::open_file`，和运行日志一个换法，Unix 上新建的 0600）。一分钟一行、一行三百来字节，一份够记两天多。核心是按需起的，空闲够久了就退，一般记不满。
2. **一行**：格照名字的字母先后，字节数都是整数：

   ```json
   {"anon":20123648,"at":"2026-10-11T05:32:08.270Z","heap":{"arenas":18874368,"free":3145728,"mmapped":2097152,"used":15728640},"pss":41230336,"private":37699584,"rss":49983488,"sessions":1,"swap":0,"why":"tick"}
   ```

   `why` 是 `start`（起来时）、`tick`（到点了）、`grew`（涨得快）；`sessions` 是这时载入着几个会话（`Core::loaded`）；不是 glibc 的没有 `heap`。这个平台量不出的那几样不写：macOS 上只有 `rss`、`footprint`（施工 V-4 补），例如 `{"at":"…","footprint":31457280,"rss":52428800,"sessions":0,"why":"start"}`。
3. **什么时候采**：写了 `ready`、拉起扩展以后记一行 `start`；之后每 60 秒记一行 `tick`。一阵请求过去、安静下来、还过内存以后（「还内存」，`settle.rs`）看一眼 RSS（`resident`），比上一次采的时候涨了 32 MiB 的记 `grew`（打开一个一万条事件的大会话约涨 20 MB，比它大的才算涨得快）。以前两次之间每 5 秒看一眼 RSS，空闲的核心因此每秒醒一次多，施工 V-6 下改成安静下来时看（`Ledger::settled`）：不定时看。读文件、写一行都在阻塞线程里。核心没了（`Core` 只剩弱引用）就停。
4. **哪些平台记**：Linux（`/proc/self/status` 读得到的）、macOS（施工 V-4 补）。Windows 怎么量还没定（23「后续再定」），不起，记一条 `DEBUG memory samples unsupported`、不建文件。
5. **开销**：见施工单 `V-4-核心自己记内存账.md`「量到的」。

### 还剩多少内存（施工 MEM-1）

一直开新会话时，核心的内存约 1.75 MB 一个会话照直线涨，撞到上限被系统杀掉，所有会话一起没（2026-11-11 基准测试 M3）。核心开会话之前照这里看一眼（`protocol.md`「会话表」第 10 条：低于 5% 的拒 `low_memory`）。这里只管读，门槛在核心。

1. **对外的样子**：`room::room() -> Option<Room>`；`Room { available, total }`，字节，`total` 不是 0。`percent()` 往下取整的百分比；`below(p)` 照整数乘开比、正好等于的不算少；`mib()` 往下取整的 MiB。读不到的是没有，调的一方当作不知道。
2. **Linux**：`/proc/meminfo` 的 `MemAvailable`、`MemTotal`（`kB`，少了哪一行、总量是 0 的不算）一份；`/proc/self/cgroup` 里 `0::<路径>` 那一行（只有 v1 的没有这一行，只看系统的）起，从这一层往上到根，每层 `/sys/fs/cgroup/<路径>/memory.max` 不是 `max` 的一份：可用的是上限减在用的（`memory.current`），在用的扣掉 `memory.stat` 的 `inactive_file`（回收得了的文件缓存：读过的日志都算在 cgroup 头上，不扣的话上限还远就当作满了），超过上限的是 0、扣多了的照上限。几份里照比例最低的那一份（`tightest`，一样低的取先来的）：上一层比自己那一层紧的，照上一层。一次读两个文件加每层三个，都在 `/proc`、`/sys` 里，不碰磁盘。读法照传进来的读文件函数（`linux_with`），测试喂假文件。
3. **macOS**（Apple 芯片）：`kern.memorystatus_level`（内核给的可用百分比）乘 `hw.memsize`；哪一个问不出、百分比超过 100 的没有。
4. **Windows**：`GlobalMemoryStatusEx` 的 `ullAvailPhys`、`ullTotalPhys`（`windows-sys` 的 `Win32_System_SystemInformation`）；问不出的没有。
5. 别的平台没有。两处 `unsafe`（sysctl、`GlobalMemoryStatusEx`）照「怎么走」第 1 条单独放开、写明为什么安全。

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-heap/src/tests.rs` | glibc 上调得成；放掉一堆小块以后还得回去；别的平台都交回假；glibc 上有堆的统计、分出去的算在用的里（施工 V-4）；Linux 上 `tune` 以后透明大页关着（施工 YC-12） |
| `crates/miyu-heap/src/usage/tests.rs`（施工 V-4） | 真机的 `smaps_rollup` 读成字节；不是它的读不出来；`VmRSS`；Linux 上量得到自己、有 PSS 这几样，macOS 上量得到自己、有 `footprint`、没有 Linux 那几样（施工 V-4 补，CI 的 macOS 上跑），Windows 没有 |
| `crates/miyu-heap/src/room/tests.rs`（施工 MEM-1） | `meminfo` 读成字节，少了一行、总量是 0 的没有；只认 v2 那一行；一层是 `max`、读不懂的不算，上限减在用的、扣 `inactive_file`、超了的是 0；往上走取比例最紧的那一层，比系统紧的照 cgroup；什么都读不到的没有；百分比往下取整、总量 0 不除；正好 5% 不算低于；macOS 的算法；三个平台的真机上读得到、可用的不比总量多 |
| `crates/miyu-core/src/memlog/tests.rs`、`crates/miyu-core/tests/memlog.rs`（施工 V-4） | 涨够了才算涨得快，落了的、没涨够的不算，不溢出；一行的格和先后，没有堆的不写 `heap`；真核心：起来记 `start`、到点记 `tick`、行里有 RSS、PSS、会话数、glibc 上有堆；安静下来时没涨够的不采、涨了 64 MiB 的记 `grew`，不安静下来不看（施工 V-6 下）；量不出来的平台不起、不写。`tests/memlog.rs` 自己一个测试程序：记的是整个进程，和别的测试并在一起，它们分配的也算进涨幅（V-6 下修） |
| `crates/miyu-log/src/rotate/tests.rs` 的 `a_file_named_as_given_rotates_under_its_own_name`（施工 V-4） | 照原样的文件名换下成 `<文件名>.1`、只留一份 |
| 量尺（`perf.md`） | 大会话没人订阅、闲够了退下以后核心的内存回落 |
