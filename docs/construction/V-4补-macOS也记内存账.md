## 施工单 V-4 补：macOS 也记内存账

状态：写好了，等门禁和三台 CI（2026-10-11 主会话）。V-4 只在 Linux 上记（23「后续再定」：macOS、Windows 怎么量还没定）；`AGENTS.md` 要 Linux、macOS（M 系列）都能用，这一步把 macOS 接上。

### 这一步做什么

1. `miyu-heap` 的 `Usage`：`rss` 哪个平台都有；Linux 专有的 `pss`、`private`、`anon`、`swap` 改成可以没有；多一样 `footprint`（macOS 的 `phys_footprint`）。
2. macOS 上 `usage`、`resident` 都问内核 `proc_pid_rusage`（`RUSAGE_INFO_V2`，`libc` 里有，工作区本来就依赖它）：`ri_resident_size` 是 RSS，`ri_phys_footprint` 是 `footprint`。`unsafe` 只在这一个函数里，两处各写明为什么安全。
3. `memlog` 的一行只写这个平台量得出的：Linux 上和 V-4 一字不差；macOS 上是 `at`、`footprint`、`rss`、`sessions`、`why`（没有 glibc，也没有 `heap`）。
4. 图纸 `heap.md`「对外的样子」「自己记账」第 2、4 条、守着它的；`core.md` 第 15 步；设计 23 F4 的状态、「后续再定」那一条。

### 定了的（主会话，技术题照推荐）

- macOS 记 `phys_footprint`：它是系统照这个进程算的账（内存压力、活动监视器、杀进程都看它），压缩了的也算，比 RSS 更像「占了多少」。PSS、匿名的这些 macOS 上没有现成的，不硬凑。
- 不加新依赖：`libc` 已经在工作区里（`miyu-fs`、`miyu-sandbox`），只在 macOS 上给 `miyu-heap` 带上。
- Windows 照旧不记：项目只考虑 Linux 和 macOS（M 系列）上跑，Windows 只在 CI 上编、测；要记的时候用 `GetProcessMemoryInfo`。

### 验收

- Linux：一行的格和 V-4 一字不差（单测的 Linux 那一行原样过）；macOS 那一行的样子单测守着。
- macOS（CI 的 macOS 上跑）：量得到自己、有 `footprint`、没有 Linux 那几样；真核心起来写 `start` 那一行；占 64 MiB 当场记 `grew`。本机照 `aarch64-apple-darwin` 做了类型检查和 clippy。
- `cargo xtask check` 八项；三台 CI。
