## 施工单 MEM-1：内存不够不开新会话

状态：施工中（2026-11-12）。

### 为什么

2026-11-11 的基准测试（`miyu-基准测试报告-2026-11-11.md` §2.5、§4 的 M3）一直开新会话：核心的内存约 1.75 MB 一个会话、照直线涨，36 分钟 10333 个会话 18.25 GB 时撞到 cgroup 的上限（`MemoryMax=18G`）被系统杀掉，所有会话一起没。核心从不拒绝新会话，撑到被杀为止。项目主人 2026-11-12 定做法 A：可用内存低于 5% 时，新开会话（和把没在内存里的会话打开到内存里）当场拒绝，给清楚的报错；已经开着的会话照常。

### 改了什么

1. **读还剩多少**：`miyu-heap` 新加 `room`（蓝图 `heap.md`「还剩多少内存」）。放在 `miyu-heap`：它本来就管「进程和内存」，在第 3 层，会话表所在的 `miyu-endpoint`（第 4 层）往下用它，不在高层写低层要用的东西。
   - Linux：`/proc/meminfo` 的 `MemAvailable`、`MemTotal`；`/proc/self/cgroup` 的 v2 那一层起往上到根，每层 `memory.max` 不是 `max` 的算一份，可用的是上限减去在用的，在用的扣掉 `memory.stat` 的 `inactive_file`（回收得了的文件缓存：不扣的话核心读过一阵日志，上限还远就当作满了）。几份里照比例最低的。
   - macOS：`kern.memorystatus_level`（内核给的可用百分比）乘 `hw.memsize`。
   - Windows：`GlobalMemoryStatusEx` 的 `ullAvailPhys`、`ullTotalPhys`（`windows-sys`，工作区里本来就有）。
   - 读不到的不算进来，一样都没有的是不知道。
2. **挡在哪**（`miyu-endpoint` 的 `sessions/room.rs`，蓝图 `protocol.md`「会话表」第 10 条）：`session.create` 真要造新的时候（重发的照旧交回原来那一个）、从磁盘载入会话（`Open::found`：说话、订阅这些找会话的都走它）、`session.fork` 写下支线之前、造子代理、fork 子代理、顾问会话之前。低于 `LOW_PERCENT`（5，常量）的拒 `low_memory`，`data.available_mib`、`data.available_percent`（都往下取整），记一行 `WARN low memory, session not opened`；子会话那几处的原因是英文的一句，只进运行日志。不知道的照常开。读数留 1 秒，读在阻塞线程里。
3. **给人看的字**：中文、英文各一句，照剩多少说（`refusal/room.rs`）；`ja` 照英文，同别的拒绝（`protocol.md` 握手 `language` 那一格）。
4. **文档**：蓝图 `heap.md`、`protocol.md`（会话表第 10 条、`session.create` 第 6 条、`session.fork` 的拒绝、在哪、守着它的）；参考 `errors.md` 中英两份。不加给模型看的字。

### 怎么测

- `crates/miyu-heap/src/room/tests.rs`：`meminfo` 读成字节，少了 `MemAvailable`、总量是 0 的是不知道；只认 v2 那一行；一层是 `max`、读不懂的不算，上限减在用的、扣 `inactive_file`、超过上限的是 0；往上走取比例最紧的那一层，cgroup 比系统紧的取 cgroup（bm 的样子：18 GiB 用掉 17.5 GiB 是 2%）；什么都读不到的不知道；百分比往下取整、总量 0 不除；正好 5% 不算低于；macOS 的百分比算法；真机上三个平台都读得到。读文件的全喂假的，不靠本机。
- `crates/miyu-endpoint/src/sessions/room/tests.rs`：门槛两边、不知道的放过、读数留着时只读一次、读的那一下崩了当不知道、子会话的英文那一句。
- `crates/miyu-endpoint/tests/low_memory.rs`（照 `Core::with_memory_probe` 注入读数，测试态开关）：只剩 3% 时 `session.create` 拒 `low_memory`、带 3072 MiB 和 3%、中文那一句、什么都不造，重发造过的编号照旧交回，开着的会话照常说话；重启的核心载入磁盘上的会话拒、日志不多一条，改成不知道以后照常说话；`session.fork` 拒、磁盘上不留支线；不知道的照常开。
- 先红：去掉 `Sessions::create`、`Open::found`、`forking::fork` 里过的那一道，对应的集成测试报红。

### 已知局限

- 只挡新开的，不卸载已经开着的会话：开着的会话越聊越大、别的程序吃内存，照样可能撑到上限。卸载空闲会话另做。
- 只认 cgroup v2；只有 v1 的机器只看系统的。
- 门槛照比例：总量很大的机器 5% 也有好几 GB，总量很小的 5% 可能不够一个大会话载入。
- 读数留 1 秒：一秒里连着开的一串会话照同一个读数判。

### 风险

- cgroup 里文件缓存算在用的：扣了 `inactive_file`，活跃的文件缓存（`active_file`）仍算在用的，cgroup 贴着上限跑、缓存又热的机器上可能早拒；拒了的照报错里的数看得出来。
- 读不到当不知道：读法坏了的平台不拒，等于没有这一道，和以前一样。
