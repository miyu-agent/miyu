## 施工单 CS-6：等核心和停核心

状态：做完，等验收（分支 `step/CS-6-等核心和停核心`）。项目主人 2026-11-27 定了三条（下面「定了的」）。

### 目的

压力下等核心启动、停核心的四处毛病修掉：

1. 头等核心说「好了」超时以后退不了：读管道的线程堵着，放下运行时要等它，60 秒到了也要等核心写出那一行（量到 138 秒）；网页每超时一次漏一个线程。
2. 等核心照时间判：60 秒一到就说超时，冷启动慢的机器上起得来的核心被说成起不来。
3. 连上了没人回：核心第 8 步就写 `run/socket`，到第 16 步才开始接连接；收到停的信号时监听器要等收尾做完才放。这两段里头连得上，握手一直空等。
4. 停的信号懒装：起来那一段收到的被系统默认杀掉；第二次被吞掉；收尾没有总时限。

### 定了的

1. 核心活着、只是起得慢：过 3 秒先提示「核心还在启动…」，接着等，上限 10 分钟。核心退出、说了 `error`、人按 Ctrl+C 都当场结束（项目主人 2026-11-27）。
2. 收尾时收到第二次停止信号（SIGTERM、SIGINT）直接强退，第一次照常收尾（同上）。
3. 先开界面、后连核心定了要做，排在正式版以后，这次只记进图纸的「还没有的」（同上）。
4. 技术题照推荐定（主会话交来的）：
   - 读那一行：Unix 上把管道交给 tokio 异步地读，Windows 上另起一条不归运行时管的线程读、经通道交回（匿名管道在 Windows 上不能异步读）。
   - 慢了的接口：`Slow = &(dyn Fn() + Send + Sync)`，一次等最多叫一次；`miyu-ipc` 加带 `_waiting` 的五个函数，不带的照旧。
   - 判据先看状态：管道到头、拉起的进程退了（退了再听 1 秒）、锁放了。锁在别的核心手里又连不上的，等它连得上或者走了再拉。
   - 核心侧两头都做：开始接连接的前一刻才写 `run/socket`、说 `ready`；定下要停就关监听、删套接字和 `run/socket`，锁另外拿着到收尾做完。
   - 握手的上限照第 1 条：3 秒提示，10 分钟上限。
   - 收尾总时限 30 秒：比 `miyu core stop` 等核心退的 60 秒短，比 systemd 默认的 90 秒短。

### 蓝图改哪几节

- `ipc.md`：在哪、`run/` 下的文件、类型（`Listener::publish`、`take_lock`、`Patience`、`Slow`）、「核心起来」第 2、5 步、新的「等多久」、「连不上就拉起」第 2、4、6 步、出错表、说法、守着它的、还没有的（先开界面、拉第三个）。
- `core.md`：在哪（`stops.rs`）、「起来的先后」第 5、8、13、16 步、「停下」第 4、5、8 条（第二次强退、总时限、不再接新连接）、运行日志、守着它的。
- `client.md`：门面的 `connect`、`open::Core`。
- `cli/main.md`：直接运行 `miyu` 时等核心的两句。`cli/acp.md`：标准错误上的两句、「还没有的」（先回 `initialize`）。
- 设计 `12-进程形态与分发.md` 第二节「拉起时的握手」、「按需运行」。
- 用户手册 `troubleshooting.md`（中英）：等多久、几句提示、核心停止得很慢。

### 给头的接口

| 用哪个 | 什么时候叫 `slow` |
|---|---|
| `connect::connect_or_start_waiting`、`reconnect_or_start_waiting`、`connect_or_start_bare_waiting`、`reconnect_or_start_bare_waiting` | 从开始连起，过了 3 秒还没连上，叫一次 |
| `connect::spawn_detached_waiting` | 拉起的程序过了 3 秒还没写来那一行，叫一次 |
| `open::Core::connect_waiting`、`Core::reconnect_waiting` | 连核心、等握手的回应两段各最多一次 |
| `connect::patiently(future, Patience, slow)` | 等任何一个 future：过了 `slow` 叫一次，过了 `limit` 交回 `None` |

不带 `_waiting` 的照旧、不叫，等的上限一样是 10 分钟。`Core` 的握手原来 30 秒没回是 `no_answer`，现在照 10 分钟。

### 不做什么

- 先开界面、后连核心：排在正式版以后（定了的第 3 条）。
- `miyu acp` 先回 `initialize` 再连核心：看过，没做，写进 `cli/acp.md`「还没有的」。
- 头（网页、终端、托盘）换成带 `_waiting` 的接口、画「还在启动」：各自的会话做。

### 验收

1. 改前红（`origin/main` 编出来的程序跑新写的端到端测试，`miyu` 的 `waiting.rs`、`stopping.rs`，Unix）：九条里七条红，等着的核心被杀、按 Ctrl+C 两条本来就当场结束（当护栏）。
2. 单元测试改前红（把改动退回去跑）：读管道退回阻塞线程池，`giving_up_does_not_hold_the_runtime` 红（10 秒）；收尾时不关监听，`closing.rs` 红；`miyu core stop` 不等起来、收尾中的核心，`a_core_between_listening_and_answering_is_waited_for` 红；拉起的进程退了不再听 1 秒，`a_process_that_left_is_not_waited_for_through_its_children` 红。
3. 门禁、Windows 目标的 clippy、`cargo doc -D warnings`、三平台 CI。
4. 沙箱实测：系统配置换成命名管道，`miyu core start` 3 秒后印 `miyu: 核心还在启动…`（英文 `miyu: the core is still starting…`）；核心 `SIGSTOP` 时 `miyu config get ui.language` 3 秒后印 `miyu: 核心还没有回应，继续等待…`。

### 风险

- 头看锁在不在用的是「试拿一次、拿到当场放」：正好同一刻有别的程序（服务管理器）起核心，那一个可能拿不到锁、说 `running` 走掉。只在拉起锁里、确实连不上时看，碰上要几样同时发生。
- Windows 上读那一行的线程在放下以后还堵着，到核心写出那一行或者退出才走；只是不归运行时管，进程退出不等它。
