## 进程的堆 `miyu-heap`

### 是什么

核心用系统的分配器（`23-性能预算.md` F3）。glibc 释放的内存多半留在自己手里、不还给系统：读完模型目录剩下的、关掉的大会话放下的，核心的内存一直不回落（施工 V-2 中量过：一万条事件的会话关掉以后核心还占 48 MB，空闲时 27 MB）。这个 crate 做两件：glibc 上调几个参数（arena 的个数、大块的门槛、顶上留多少），把堆里空着的还给系统。macOS 的分配器同样把放掉的块留在手里，系统内存紧了才还，所以「还给系统」这一件在 macOS 上也做（施工 LW-18，下面「怎么走」第 6 条）；调参数只在 glibc 上做。别的平台（Windows、musl）的分配器自己会还，两件都什么都不做。另交出堆的统计、进程这时占了多少内存，核心照它自己记账（F4，施工 V-4，下面「自己记账」）；还交出这台机器、核心所在 cgroup 还剩多少可用内存，核心照它和自己的常驻判内存紧不紧，紧的把闲着的会话卸出内存（施工 MEM-1 读法、MEM-2 卸会话，下面「还剩多少内存」）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-heap/src/lib.rs` | `tune`、`trim`、`stats`、`huge_pages_off`；glibc 上声明、调 `mallopt`（四个参数，顶上那两个是施工 LW-25 加的）、`malloc_trim`、`mallinfo2`，Linux 上关透明大页（施工 YC-12）；macOS 上声明、调 `malloc_zone_pressure_relief`（施工 LW-18）；别的平台是空的 |
| `crates/miyu-heap/src/launch.rs`（施工 LW-12） | 拉起核心时带的环境变量：线程缓存每种大小留几块（`TCACHE`）、拼出要设的两项（`vars`）、核心起的命令恢复成原来的值（`restored`）；下面「线程缓存」 |
| `crates/miyu/src/tuned.rs`（施工 LW-12） | 主程序拉起核心的命令带上这两项；前台跑核心的进程没带着时，带上以后重新执行自己（下面「线程缓存」第 2、3 条） |
| `crates/miyu-heap/src/usage.rs` | 进程这时占了多少内存：Linux 上读 `/proc/self/smaps_rollup`、`/proc/self/status`，两份的读法是纯的（施工 V-4）；macOS 上问内核 `proc_pid_rusage`（施工 V-4 补） |
| `crates/miyu-heap/src/room.rs`、`room/macos.rs`、`room/windows.rs`（施工 MEM-1，MEM-2 分开交） | 还剩多少可用内存：Linux 上读 `/proc/meminfo` 和 cgroup v2 一路往上的几层，读法是纯的、读文件的照传进来的函数；macOS 上问 sysctl，Windows 上问 `GlobalMemoryStatusEx` |
| `crates/miyu-endpoint/src/sessions/pressure.rs`（施工 MEM-2，MEM-4 改门槛、近线快查） | 隔 10 秒（近线时隔 1 秒）、开会话时照它和 `usage::resident` 判紧不紧（门槛离总量还有 10%、最多 2 GiB、不低于 5%），紧的把闲着的会话卸出内存（`protocol.md`「会话表」第 10 条） |
| `crates/miyu-core/src/memlog.rs` | 核心自己记账：什么时候采、一行的样子、采样的任务（施工 V-4） |
| `crates/miyu-core/src/lib.rs` | 起来时最先调好（`core.md`「起来的先后」第 0 步） |
| `crates/miyu-core/src/models.rs` | 读完模型目录 trim 一次 |
| `crates/miyu-core/src/start.rs`（施工 LW-5） | 生成参考文件放在一条做完就退出的线程里（下面「怎么走」第 5 条） |
| `crates/miyu-log/src/zone.rs`（施工 LW-5） | 本机时区：运行日志的时刻、会话的环境里的偏移照它算，不打开 jiff 的时区库（下面「本机时区」） |
| `crates/miyu-session/src/heap.rs`（施工 MEM-3） | `trim_soon`：会话这一层要 trim 的几处攒一阵还一次（下面「怎么走」第 2 条） |
| `crates/miyu-session/src/open/load.rs` | 载入一个会话以后 trim 一次（`session/actor.md` 第 2 条；经 `trim_soon`） |
| `crates/miyu-session/src/actor/life.rs` | 会话的 actor 结束、它占的放掉以后 trim 一次（`session/actor.md`；经 `trim_soon`）：停下、删掉、闲够了退下（`protocol.md`「会话表」第 9 条）、内存紧时退下（第 10 条）都算 |
| `crates/miyu-session/src/route/kept.rs`（施工 LW-3） | 会话没有主请求满 3 分钟、放掉编好的历史以后 trim 一次（`session/actor.md`「8. 经路由请求」第 5 款第 2 条；经 `trim_soon`） |
| `crates/miyu-endpoint/src/sessions/pressure.rs`（施工 MEM-3） | 内存紧时卸完一批、从表里拿掉以后 trim 一次（经 `trim_soon`） |
| `crates/miyu-core/src/wal.rs`（施工 LW-15） | 库闲下来合回写前日志、放掉页缓存以后 trim 一次（`store.md`「怎么走」第 14 条第 7 款） |
| `crates/miyu-core/src/settle.rs` | 一阵请求过去、安静 2 秒以后 trim 一次（施工 V-5 中），再看内存涨得快不快（施工 V-6 下）；连接每读到一个请求、连上、断开都叫一声，信号是 `miyu-endpoint` 的 `Activity`（`activity.rs`，几处各拿一份都叫得醒） |

### 对外的样子

- `ARENAS`：8。glibc 默认是核数的 8 倍，多开的只多攒碎片；读日志分块在几个线程里一起解（`store.md` 第 6 条，最多 8 块），少于它就抢锁（限到 2 个时重开一万条事件的会话，读日志从约 10 ms 慢到 16 ms）。不照核心的工作线程数算（施工 CPU-1 把工作线程改成照核数以后照旧 8 个）：它对的是读日志的那几块；`tune` 要在起任何线程以前调，那时还没读配置。工作线程多了以后共用 arena 抢不抢锁，等压测复测量了再定。
- `MMAP_FROM`：128 KiB，多大的一块直接向系统要、放掉当场还。写明了 glibc 就不再自己往上调：默认放掉一块大的以后把门槛抬到它那么大（最多 32 MB），之后每一轮拼的请求（几百 KB）都落进堆里，和长住的穿插着放，空出来的碎成小片、`malloc_trim` 也还不回去。
- `TOP_PAD`：0；`TRIM_FROM`：4 KiB（施工 LW-25）。一个 arena 顶上那一块空着的（top）往上要时多要多少、往回还时留多少，攒到多大才往回还；glibc 默认都是 128 KiB。下面「怎么走」第 7 条。
- `tune() -> bool`：起别的线程之前调一次，glibc 上照 `M_ARENA_MAX`、`M_MMAP_THRESHOLD`、`M_TOP_PAD`、`M_TRIM_THRESHOLD` 设成上面这四个，交回都设成了没有；别的平台交回假。Linux 上（不管是不是 glibc）还给这个进程关掉透明大页（`PR_SET_THP_DISABLE`，施工 YC-12，下面「怎么走」第 4 条），关没关上不在交回的里面。
- `huge_pages_off() -> bool`（施工 YC-12）：这个进程的透明大页关着没有（`PR_GET_THP_DISABLE`），核心记进 `DEBUG heap tuned` 的 `huge_pages_off`；别的平台交回假。
- `launch::TCACHE`（施工 LW-12）：4。glibc 每条线程的缓存每种大小最多留几块，默认是 16（下面「线程缓存」）。
- `launch::vars(拉起的一方自己的 GLIBC_TUNABLES) -> 要另外设的环境变量`（施工 LW-12）：glibc 上交回两项，`GLIBC_TUNABLES`（原来的值后面接上 `glibc.malloc.tcache_count=4`）和 `MIYU_GLIBC_TUNABLES`（原来的值，没有设置时是空的）；其他平台、原来的值里已经写了 `glibc.malloc.tcache_count` 时不交回任何变量。
- `launch::restored(核心环境里的 MIYU_GLIBC_TUNABLES, 核心自己的 GLIBC_TUNABLES) -> 命令拿到的 GLIBC_TUNABLES`（施工 LW-12）：有前者时使用前者，前者是空的时不传；没有前者时使用后者。
- `launch::tunables() -> String`（施工 LW-12）：这个进程带着的 `GLIBC_TUNABLES`，核心记进 `DEBUG heap tuned` 的 `tunables`；没有设置时是空的。
- `trim() -> bool`：glibc 上是 `malloc_trim(0)`，macOS 上是 `malloc_zone_pressure_relief`（施工 LW-18），交回还了没有；别的平台交回假。要走一遍堆，在阻塞线程里调。
- `stats() -> Option<Stats>`（施工 V-4）：glibc 的 `mallinfo2`，各个 arena 加起来：`arenas`（各 arena 从系统要来的）、`mmapped`（单独 mmap 的大块）、`used`（arena 里在用的）、`free`（arena 里空着的），字节；别的平台没有。
- `usage::usage() -> Option<Usage>`、`usage::resident() -> Option<u64>`（施工 V-4）：字节。`rss` 哪个平台都有，别的几样只有量得出的平台有。Linux 上前者读 `smaps_rollup`，多 `pss`、`private`（`Private_Clean` 加 `Private_Dirty`）、`anon`、`swap`；后者只读 `status` 的 `VmRSS`，便宜，看涨没涨用。macOS 上两个都问内核 `proc_pid_rusage`（`RUSAGE_INFO_V2`，一次系统调用），前者多 `footprint`（`phys_footprint`：算在这个进程头上的，活动监视器「内存」那一列，压缩了的也算；施工 V-4 补）。Windows 没有（`23` 第五节「后续再定」）。

### 怎么走

1. 工作区不许 `unsafe`（`forbid`）；这个 crate 照 `miyu-sandbox` 的办法改成 `deny`，只在声明、调用 glibc 的三处单独放开，每处写明为什么安全：`mallopt` 只改分配器自己的参数，`malloc_trim` 只把分配器手里空着的页还回去，已经分出去的不动、glibc 自己加锁。
2. 什么时候 trim：读完模型目录（原文、解析的半成品放掉了）；载入一个会话以后（读、解的整份日志只留最近一次压缩以后的，`session/actor.md` 第 2 条）；一个会话的 actor 结束以后（它的状态机、事件放掉了：停下、删掉、闲够了退下、内存紧时退下都算）；内存紧时卸完一批、从会话表拿掉以后（表里那几份在会话退下以后才放，施工 MEM-3）；一个会话放掉编好的历史以后（没有主请求满 3 分钟，施工 LW-3，`session/actor.md`「8. 经路由请求」第 5 款第 2 条）；一个会话的有效历史收成槽以后（歇着满 3 分钟，施工 LW-10 下，`session/actor.md`「闲下来收成槽」：收的时候拿出来的、放回时读来的那一份都交给放完就退的线程放，放完再还）；一阵请求过去、安静 2 秒以后（施工 V-5 中，下一条；trim 之前先放掉这一阵里读好放着的给人看的字，施工 V-30，`store/resources.md` 第 3 节第 8 条）。库闲下来、合回写前日志并放掉页缓存以后（施工 LW-15，在合回的那条线程 `miyu-wal` 上，只在有库合过的时候做）。都在阻塞线程里、不挡着谁。不定时 trim：空着的时候没有新的可还。
   - **会话这一层攒一阵还一次**（施工 MEM-3，`miyu_session::trim_soon`）：载入、actor 结束、内存紧时卸完一批、会话放掉编好的历史（施工 LW-3）、有效历史收成槽和放回以后（施工 LW-10 下）这几处不当场 trim，头一次要的时候等 1 秒（`QUIET`）再 trim 一次，这一秒里又要的都算在这一次里；开始 trim 的那一刻放下记号，之后要的另攒一次（任何一刻最多一次在等、一次在走；等的时候运行时停了的也放下记号）。原来每一处当场排一个：内存紧时一下子退下上千个会话就是上千次 trim 一个接一个，trim 走堆时拿着 arena 的锁，工作线程上的分配、释放都等它，会话表一个个问退不退跟着慢（2026-11-13 量过，量尺 `crates/miyu-endpoint/tests/memory_pressure_rss.rs`，见施工单 `MEM-3-卸会话卸得少.md`）。
   - **安静下来还一次**（施工 V-5 中，`settle.rs`）：头一连上就是一阵请求（读配置、列模型、读人），几百 KB 的回应拼完就放掉，放掉的留在 glibc 手里。施工 V-5 中量过：连上终端以后核心匿名内存多 3.3 MB，活着的数据只多 40 KB。所以连接每读到一个请求叫一声（`Activity::touch`，没人等的记下一次）；盯着的任务等到叫声，再等到 2 秒（`QUIET`）里没有新的叫声，trim 一次，再回去等。没有请求的时候它一直睡着，不定时醒。trim 在阻塞线程里做：崩了的记 `ERROR heap settle panicked`、接着等；停核心时运行时在收尾、阻塞任务被取消的记一行 `DEBUG heap settle cancelled`、不再等（施工 BUG-1：以前不分取消和崩溃，停核心时总打一行误报的 `ERROR`）。
3. 量到的（施工 V-2 再补）：只 trim 的时候，大会话退下以后堆里还有 16 MB 空着的碎片、核心匿名内存比空闲时多 8.8 MB；写明大块的门槛以后只多 3.9 MB，重开大会话、说一句以后 92 MB 降到 49 MB。

4. **透明大页**（施工 YC-12，`23` F3）：系统设成 `always` 时，`khugepaged` 把还有一页在用的 2 MB 一段合成大页，trim 还掉的空洞又被填回去（2026-11-10 压测量过：高压过后 trim 以后 PSS 569 MB 降到 82 MB，五分钟里每分钟涨约 90 MB，回到约 476 MB，在用的堆只有 13 MB，`AnonHugePages` 约等于整个匿名内存）。所以 `tune` 照 `prctl(PR_SET_THP_DISABLE, 1)` 关掉，用 `rustix` 的安全封装，不写 `unsafe`；关不上（3.15 以前的内核）照旧起来，`huge_pages_off` 如实交回假。这个标记 `fork`、`exec` 都传下去：经助手跑的命令由助手换成命令之前恢复（`sandbox.md`「助手的命令行」第 4 步）；扩展、MCP 服务、不经助手的命令跟着关着，只是用不上大页，不碍事。

5. **一次性的大活不放在长住的线程上**（施工 LW-5）：一条线程分配、放掉的块先进它自己的缓存（glibc 的 tcache），线程退出时才交回 arena；长住的线程上做完一件分配得多、做完全放掉的活，放掉的块和长住的块穿插在同一个 arena 里，`malloc_trim` 只能还整页，夹在中间的还不回去。所以起来时读模型目录在 `catalog` 线程里（施工 V-2 下补），生成参考文件在 `generate` 线程里（`core.md`「起来的先后」第 6 步），都是做完就退出。2026-11-22 量过：核心连着一个头空闲时，各 arena 合计从 4.3 MB 降到 4.0 MB（连同下面「本机时区」），活着的堆只有 1.7 MB；glibc 2.44 每条线程的缓存每种大小留 16 块，核心 28 条线程的缓存里攒着约 0.8 MB（用 `GLIBC_TUNABLES=glibc.malloc.tcache_count=0` 关掉时合计降到 2.5 MB），这一部分在进程里调不了（`mallopt` 没有这一项），施工 LW-5 没有做；施工 LW-12 改成在拉起核心时用环境变量设置（下面「线程缓存」）。

6. **macOS 上也还**（施工 LW-18）：macOS 的分配器把放掉的块留在手里等着复用，系统内存紧了才还。2026-11-22 在一台 M1 上量的（核心加终端界面，占用照 `phys_footprint`，就是活动监视器「内存」那一列）：核心空闲时堆里写过 7.3 MB，在用的只有 1.7 MB；终端界面写过 5.8 MB，在用 1.4 MB。`malloc_zone_pressure_relief` 是内存紧了时系统替每个进程调的那一步，`trim` 在 macOS 上调它（分配区给空指针是每个区都还，目标给 0 是能还多少还多少），只还空着的，已经分出去的不动。调的时机和 glibc 上一样，就是上面第 2 条的那几处，不另加。这个函数交回的字节数靠不住（放掉 32 MB 小块、占用确实落回去了，它交回 0），所以 `trim` 交回的「还了没有」另看这个进程的占用前后落了没有，两样里有一样说还了就算还了。实测核心的占用：空闲 10.2 → 8.2 MB，说过一句 12.3 → 9.1 MB，九次工具调用以后 12.3 → 9.7 MB。分配器的参数（arena 的个数、大块的门槛）和线程缓存那一节仍然只在 glibc 上做。32 KiB 以上的块直接向系统要的做法没有做：用环境变量 `MallocMediumZone=0` 对照过，核心加终端界面只再少 0.1 到 0.7 MB。

7. **线程分配区的顶上不多留**（施工 LW-25）：每个 arena 顶上那一块空着的（top）往上要的时候多要 128 KiB、往回还的时候也留 128 KiB，攒满 128 KiB 才还（glibc 的 `M_TOP_PAD`、`M_TRIM_THRESHOLD` 默认值）。`malloc_trim` 只还主分配区的顶，线程分配区的顶它不管，只在放掉一块并进顶、合起来不小于 64 KiB 时由 glibc 自己收，收的时候照样留 128 KiB。核心空闲时 7 个线程分配区里有 0.2 到 0.4 MB 就是这样留下的。`tune` 把留多少设成 0（`TOP_PAD`）、攒多大才还设成 4 KiB（`TRIM_FROM`）：放掉的并进顶以后当场还到只剩不满一页。终端界面经 `miyu-client` 调同一个 `tune`，也跟着这样做。
   - 量到的（2026-11-23，本机，沙箱数据根）：三批交替对照（两批用环境变量 `GLIBC_TUNABLES=glibc.malloc.top_pad=0:glibc.malloc.trim_threshold=4096`，一批用改了的程序和同样编法的改前程序；`pss.py` 三个状态各三次取中位），核心连着终端界面空闲、说过一句、九次工具调用以后，核心的堆和匿名内存少约 0.15 到 0.25、0.2 到 0.35、0.2 MB，三个状态两个进程合计每一批都少约 0.55 MB，终端界面没有变化；长回答说到第 10、30 轮（`grow.py`，用环境变量对照，各两遍）核心的堆和匿名内存少约 0.31、0.42 MB。这几样每一遍之间上下差 0.2 到 0.3 MB，所以只能说少了这么多左右。gdb 走核心的分配区：空闲时线程分配区驻留 1360 kB → 1112 kB，顶上驻留的 196 kB → 8 kB。
   - 快慢：量尺（`miyu-perf`，不排函数，五批交错）的冷启动、重开大会话（关着、开着记忆）、大会话上一轮、追加并同步、流式一轮的 CPU 都在基线的波动以内。只有从头投影（重开一个一万条事件的大会话以后头一次说，日志从头投影成请求）慢了一点：几批的比法不同（p50、不是 0 的平均），慢 6% 到 19%，就是每次多 0.15 到 0.5 ms，方向每一批都一样。这一步只在重开大会话以后的头一句做一次，预算是 p95 50 ms，每一轮都走的几项都没有变慢，所以算无关紧要、仍然采用。终端界面起来到出现输入框、说一句、九次工具调用那一轮（各五遍）都没有变化。
   - 没选的：顶上留 64 KiB（`TOP_PAD` 取 64 KiB、门槛 4 KiB）从头投影不慢，但是三个状态合计只少约 0.29 MB，在波动以内，gdb 走下来顶上驻留的几乎没变（196 → 172 kB）：基线时每个线程分配区的顶平均只有约 28 kB，留 64 KiB 就收不到什么。攒到 64 KiB 才还（`TRIM_FROM` 取 64 KiB）比 4 KiB 少收约 0.2 MB，从头投影一样慢；只设 `TOP_PAD` 为 0、门槛照默认 128 KiB 少得更少。施工 LW-5 量空闲时只试过 `TOP_PAD`，没有同时降门槛，所以当时量不出差别。

### 线程缓存（施工 LW-12）

glibc 的每条线程有自己的缓存（tcache）：线程释放的小块先进入缓存，不回到 arena，每种大小默认最多留 16 块。核心有十几条常驻的线程，每一条都留着一份，所以空闲时这些缓存里的块白白占着内存。这个上限在进程里无法修改（`mallopt` 没有这一项），glibc 只在进程启动时读取环境变量 `GLIBC_TUNABLES`，因此由拉起核心的一方把它设在核心的环境里。

1. 设成多少：每种大小留 4 块（`launch::TCACHE`）。2026-11-22 实测（施工单 `LW-12-线程缓存少留.md`「量到的」）：留 4 块时，核心连着一个头空闲时的堆和匿名内存合计少约 0.7 MB，说过一句以后少约 0.7 MB，量尺的各项时延没有退步。一块都不留时只再少 0.3 MB，但是重开大会话、追加事件、流式一轮都慢一成左右，所以没有选它。
2. `miyu` 自己拉起核心时（打开界面时顺带拉起、`miyu core start`），把 `launch::vars` 交回的两项设在命令上（`crates/miyu/src/tuned.rs` 的 `on`），核心启动时就已经调好。
3. 用其他方式启动的核心（systemd 等托管方式、直接运行 `miyu core`）没有带着这两项。此时主程序在进入核心之前，带上这两项、用原来的参数重新执行自己一次（`tuned.rs` 的 `relaunch`，Unix 上的 `exec`）。重新执行不改变进程号，标准输入输出保持不变，所以托管的一方看到的仍然是同一个进程；带上以后不会再次重新执行。如果重新执行失败（找不到程序自己的位置、系统不允许），核心仍然正常启动，只是线程缓存使用 glibc 的默认值。因为打包目录里的托管文件有六种（systemd 用户服务和系统服务、dinit、runit、OpenRC、XDG 自启动），所以没有在每一份文件里写环境变量，而是由程序自己补上。
4. 人自己设置的 `GLIBC_TUNABLES` 保留：新的一项接在原来的值后面。如果原来的值里已经写了 `glibc.malloc.tcache_count`，核心不做任何修改，也不重新执行。
5. 核心运行的命令不带核心自己的这一项：拉起的一方把原来的 `GLIBC_TUNABLES` 记在 `MIYU_GLIBC_TUNABLES` 里，核心为命令挑选环境变量时用它恢复（`tools/shell.md`「环境变量」）。这样，AI 运行的命令和你在自己的终端里运行同一条命令时，分配器的行为相同。扩展、MCP 服务这些由核心直接启动的进程会继承核心的值。
6. 核心启动时在 `DEBUG heap tuned` 那一行记下 `tunables`（这个进程带着的 `GLIBC_TUNABLES`），可以用它确认这一项有没有生效。
7. 量尺拉起核心时也带上这两项（`crates/miyu-perf/src/sandbox.rs`），所以量尺量的是用户实际使用的状态；在模拟器里录排函数的清单时，核心也不会因为补环境变量而重新执行自己、离开模拟器。
8. 其他平台（macOS、Windows、musl）不使用 glibc，`launch::vars` 不交回任何变量，以上各条都不发生。
9. **施工 LW-25 重量过、仍然留 4 块**：顶上不多留以后，核心空闲时线程分配区里剩下最多的是各线程缓存里的小块（gdb 数过：540 kB 的页上只压着缓存里的块）。同一天在顶上不多留的基础上对照了几种更少留的（`GLIBC_TUNABLES`，只给核心）：每种大小留 2 块，三个状态合计再少约 0.6 MB；留 1 块再少约 1.5 MB；不留再少约 2.1 MB；进缓存的块最大 256 字节（`tcache_max`）再少约 1.1 MB。这几种都让量尺的从头投影（重开大会话后头一次说）慢约四成（合并所有样本的 p50 从 2.65 ms 到 3.69–3.81 ms，几遍量下来每一遍都慢），不留的还让重开大会话慢约一成，所以都没有选。

### 本机时区（施工 LW-5）

运行日志每一行的时刻、核心起来那一行的 `tz`（`log.md`）、会话的环境里的时区偏移（`miyu-endpoint` 的 `sessions.rs`）照 `miyu_log::zone::local()` 算，不直接用 `jiff::Zoned::now()`：jiff 认本机时区时每次都先打开它的 zoneinfo 库，库第一次打开时把 `/usr/share/zoneinfo` 整个目录走一遍，一千多个时区名连同完整路径留在堆上不放（2026-11-21 量过：核心活着的堆里 111 kB）。

1. 没设 `TZ`、`TZDIR` 的 Unix 上，直接读 `/etc/localtime`：jiff 照它的链接认出名字以后在库里读的是同一份文件，链接认不出名字的 jiff 也是直接读它，所以规则一样，写出来的时刻和偏移一样。时区名照链接里 `zoneinfo/` 后面的那一段（和 jiff 认的一样），不是链接的叫 `Local`；这几处都不印时区名。
2. 别的情况照旧交给 jiff（`TimeZone::system()`）：设了 `TZ` 的、设了 `TZDIR` 的、`/etc/localtime` 读不出来或者不是时区文件的、不是 Unix 的。
3. 读到的留 5 分钟（和 jiff 自己留的一样久），过了重读一次：核心开着时改了本机时区，照旧 5 分钟以内跟上。
4. 定时任务照名字记时区、照名字找（`schedules.md`），名字要和 jiff 库里的对得上，仍然用 jiff；回收站记删除时间（`miyu-fs`）也仍然用 jiff。这两处用到时才打开库。

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

### 还剩多少内存（施工 MEM-1，MEM-2 改）

一直开新会话时，核心的内存约 1.75 MB 一个会话照直线涨，撞到上限被系统杀掉，所有会话一起没（2026-11-11 基准测试 M3）。核心隔一阵、开会话时照这里看一眼，紧的把闲着的会话卸出内存（`protocol.md`「会话表」第 10 条，施工 MEM-2；MEM-1 原来低于 5% 拒开会话，2026-11-12 撤了）。这里只管读，判紧不紧在核心（门槛、隔多久看都在 `pressure.rs`，施工 MEM-4 起是 max(5%, min(10%, 2 GiB))）：整台机器紧的还要看核心自己占多少（`usage::resident`），cgroup 紧的不看，所以两份分开交。

1. **对外的样子**：`room::room() -> Rooms`；`Rooms { system, cgroup }`，都是 `Option<Room>`：`system` 整台机器的，`cgroup` 核心所在 cgroup 有上限的几层里最紧的那一层（没有上限的、只有 v1 的、不是 Linux 的没有）。`Room { available, total }`，字节，`total` 不是 0。`percent()` 往下取整的百分比；`below(p)` 照整数乘开比、正好等于的不算少；`mib()` 往下取整的 MiB。读不到的是没有，调的一方当作不知道。
2. **Linux**：`system` 是 `/proc/meminfo` 的 `MemAvailable`、`MemTotal`（`kB`，少了哪一行、总量是 0 的不算）；`cgroup` 从 `/proc/self/cgroup` 里 `0::<路径>` 那一行（只有 v1 的没有这一行，没有 `cgroup`）起，从这一层往上到根，每层 `/sys/fs/cgroup/<路径>/memory.max` 不是 `max` 的一份：可用的是上限减在用的（`memory.current`），在用的扣掉 `memory.stat` 的 `inactive_file`（回收得了的文件缓存：读过的日志都算在 cgroup 头上，不扣的话上限还远就当作满了），超过上限的是 0、扣多了的照上限。几层里照比例最低的那一份（`tightest`，一样低的取先来的）：上一层比自己那一层紧的，照上一层。一次读两个文件加每层三个，都在 `/proc`、`/sys` 里，不碰磁盘。读法照传进来的读文件函数（`linux_with`），测试喂假文件。
3. **macOS**（Apple 芯片）：`system` 是 `kern.memorystatus_level`（内核给的可用百分比）乘 `hw.memsize`；哪一个问不出、百分比超过 100 的没有。没有 `cgroup`。
4. **Windows**：`system` 是 `GlobalMemoryStatusEx` 的 `ullAvailPhys`、`ullTotalPhys`（`windows-sys` 的 `Win32_System_SystemInformation`）；问不出的没有。没有 `cgroup`。
5. 别的平台没有。两处 `unsafe`（sysctl、`GlobalMemoryStatusEx`）照「怎么走」第 1 条单独放开、写明为什么安全。

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-log/src/zone/tests.rs`（施工 LW-5） | 本机时区新旧对照：一年多里每隔 22 小时一个时刻，运行日志的时刻、和 UTC 差多少写出来和 jiff 照系统认的一样；读不出来的、不是时区文件的交给 jiff；照抄的一份照它认；链到 zoneinfo 里的名字和 jiff 认的一样 |
| `crates/miyu-heap/src/launch.rs` 里的测试（施工 LW-12） | 没有设置时设上线程缓存那一项、原来的值记成空的；原来有其他项时保留在前面并记下；人自己写了留几块时不设；只有 glibc 上交回变量；命令拿回拉起的一方原来的值 |
| `crates/miyu/tests/tuned.rs`（施工 LW-12） | 真的拉起核心、读它的环境：直接运行的核心也带上了那两项；人自己设置的其他项保留并记下；人自己写了留几块时保持原样；其他平台什么都不带 |
| `crates/miyu-heap/src/tests.rs` | glibc 上调得成；放掉一堆小块以后还得回去；调好以后一条新线程写过 4 MB 中块、放掉，各 arena 从系统要来的合计回到原样、差不到 64 KiB（施工 LW-25：顶上不多留）；macOS 上放掉 32 MB 小块、还一次以后进程的占用落回去 16 MB 以上（施工 LW-18，CI 的 macOS 上跑）；别的平台都交回假；glibc 上有堆的统计、分出去的算在用的里（施工 V-4）；Linux 上 `tune` 以后透明大页关着（施工 YC-12） |
| `crates/miyu-heap/src/usage/tests.rs`（施工 V-4） | 真机的 `smaps_rollup` 读成字节；不是它的读不出来；`VmRSS`；Linux 上量得到自己、有 PSS 这几样，macOS 上量得到自己、有 `footprint`、没有 Linux 那几样（施工 V-4 补，CI 的 macOS 上跑），Windows 没有 |
| `crates/miyu-heap/src/room/tests.rs`（施工 MEM-1，MEM-2 改） | `meminfo` 读成字节，少了一行、总量是 0 的没有；只认 v2 那一行；一层是 `max`、读不懂的不算，上限减在用的、扣 `inactive_file`、超了的是 0；往上走取比例最紧的那一层，系统的另交；没有上限的没有 `cgroup`；什么都读不到的两份都没有；百分比往下取整、总量 0 不除；正好 5% 不算低于；macOS 的算法；三个平台的真机上读得到整台机器的、可用的不比总量多，不是 Linux 的没有 `cgroup` |
| `crates/miyu-core/src/memlog/tests.rs`、`crates/miyu-core/tests/memlog.rs`（施工 V-4） | 涨够了才算涨得快，落了的、没涨够的不算，不溢出；一行的格和先后，没有堆的不写 `heap`；真核心：起来记 `start`、到点记 `tick`、行里有 RSS、PSS、会话数、glibc 上有堆；安静下来时没涨够的不采、涨了 64 MiB 的记 `grew`，不安静下来不看（施工 V-6 下）；量不出来的平台不起、不写。`tests/memlog.rs` 自己一个测试程序：记的是整个进程，和别的测试并在一起，它们分配的也算进涨幅（V-6 下修） |
| `crates/miyu-log/src/rotate/tests.rs` 的 `a_file_named_as_given_rotates_under_its_own_name`（施工 V-4） | 照原样的文件名换下成 `<文件名>.1`、只留一份 |
| 量尺（`perf.md`） | 大会话没人订阅、闲够了退下以后核心的内存回落 |
| 量尺 `crates/miyu-endpoint/tests/memory_pressure_rss.rs`（施工 MEM-3，`#[ignore]`，自己一个测试程序） | 照核心调好分配器、多线程的运行时，载入 N 个会话（`MIYU_RSS_SESSIONS`，默认 1000）、判成紧全卸：卸那一批花了多久、卸的期间另一个连接每 10 毫秒列一次会话（`session.list`）最久等了多久（施工 MEM-5），卸完 1、2、5、12 秒和再 trim 一次的常驻、匿名、堆里在用的和空着的、线程数 |
