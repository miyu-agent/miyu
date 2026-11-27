## 本机传输

### 是什么

核心在本机的套接字上等连接，头连过去：Linux、macOS 上是 Unix 域套接字，Windows 上是命名管道。只有本人连得上；连上以后头出示本机令牌（`protocol.md` 握手）。一个数据根只跑一个核心，靠 `run/core.lock` 这把锁。核心没在跑，头把它拉起来，等它说一声好了再连。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-ipc/src/lib.rs` | `open`、`open_locked`：核心起来；`connect`：头连核心 |
| `crates/miyu-ipc/src/place.rs` | 套接字放哪：`Dirs`、数据根的指纹、路径的上限 |
| `crates/miyu-ipc/src/lock.rs` | 单实例锁 `run/core.lock` |
| `crates/miyu-ipc/src/files.rs` | `run/token`、`run/socket` |
| `crates/miyu-ipc/src/listener.rs` | 等连接的一头 `Listener`、连上的一个连接 `Connection` |
| `crates/miyu-ipc/src/unix.rs` | Unix 域套接字：只有自己能进的目录、旧套接字、跟终端脱开（开新会话，施工 CS-10） |
| `crates/miyu-ipc/src/windows.rs` | 命名管道：等连接、连过去、核对另一头、跟终端脱开 |
| `crates/miyu-ipc/src/ready.rs` | 核心说「好了」的那一行 |
| `crates/miyu-ipc/src/start.rs` | `connect_or_start`：连不上就拉起核心；`connect_or_start_bare` 同样拉起、不读本机令牌（施工 W-8）；`reconnect_or_start`、`reconnect_or_start_bare`：头自己重连用，人停的不拉（施工 CS-5）；带 `_waiting` 的四个同样，慢了叫调用方给的 `Slow`（施工 CS-6）；等拉起锁、等锁在别的核心手里却连不上的那一个 |
| `crates/miyu-ipc/src/start/spawn.rs`（施工 CS-6） | `spawn_detached`、`spawn_detached_waiting`：拉起、跟终端脱开、照管道和进程在不在等那一行 |
| `crates/miyu-ipc/src/patience.rs`（施工 CS-6） | `Patience`：等多久（3 秒叫一声、最多 10 分钟）；`Slow`：慢了叫谁；`patiently`：照它等一个 future（头等握手的回应） |
| `crates/miyu-ipc/src/heard.rs`（施工 CS-6） | 读拉起的程序写来的第一行，能当场放下：Unix 上异步读管道，Windows 上另起一条不归运行时管的线程 |
| `crates/miyu-ipc/src/stopped.rs`（施工 CS-5） | 人停的记号 `run/stopped`：留下（`mark_stopped`）、看在不在、核心起来时删掉 |
| `crates/miyu-ipc/src/unreachable.rs` | `running_unreachable`：照 `run/socket` 连不上时，核心是不是还在、只是连不上（施工 CS-3） |
| `crates/miyu-ipc/src/error.rs` | 起不来、连不上、拉不起的几种情形和它们的话 |
| `crates/miyu-pipe/src/lib.rs`、`windows.rs`、`process.rs`、`parent.rs` | Windows 的安全接口：只对本人开放的管道实例，当前用户和管道另一头的 SID；一个进程还在不在跑（`process_running`，脚本调工具的通道清留下的目录用，施工 WC-1，`tools/shell.md`）；问父进程（施工 XG-1 下，脚本调工具的通道认扩展的子孙，`providers.md`「会跑命令的」）：Windows 拍进程快照（`process_parents`），macOS 问 `proc_pidinfo`（`parent_process`，这个 crate 在 macOS 上唯一的一样）；Windows 上一个进程什么时候起的（`process_created`，`GetProcessTimes`，施工 XG-1补2，认出父进程链上被重用的编号）。整个仓库只有这个 crate 放开了 `unsafe`，每个 `unsafe` 块都写着为什么安全 |

### 对外的样子

**`run/` 下的文件**

| 文件 | 内容 | 谁写 |
|---|---|---|
| `run/core.lock` | 空文件，锁在它上面 | 核心 |
| `run/token` | 本机令牌：64 位小写十六进制，加 `\n`；Unix 上权限 0600 | 核心，每次起来 |
| `run/socket` | 一行：套接字的实际位置，路径的原样字节，加 `\n`；Unix 上权限 0600 | 核心，每次起来、开始接连接的前一刻（施工 CS-6） |
| `run/token.tmp`、`run/socket.tmp` | 写的时候先写它们，再改名 | 核心 |
| `run/spawn.lock` | 拉起锁，空文件 | 头 |

核心走了，`run/token` 留着，下一个核心起来重写。`run/socket` 在核心不再接连接时删掉（施工 CS-6：定下要停的那一刻、丢掉监听器时）；崩了留下的，下一个核心拿到锁就先删，开始接连接时再写。所以 `run/socket` 在，就是有一个核心在接连接（崩了的那一刻到下一个起来之间除外：那时照它连是被拒绝，`NotRunning`）。

**套接字放哪**

| 平台 | 什么时候 | 位置 |
|---|---|---|
| Linux | 设了能用的 `$XDG_RUNTIME_DIR` | `$XDG_RUNTIME_DIR/miyu-<指纹>/core.sock` |
| Linux（没有能用的 `$XDG_RUNTIME_DIR`）、macOS | | `<数据根>/run/core.sock` |
| Linux、macOS | 上面那个超过上限 | `$TMPDIR/miyu-<uid>/<指纹>.sock` |
| Windows | | 命名管道 `\\.\pipe\miyu-<指纹>` |

- **指纹**：数据根路径的 SHA-256 的前 4 个字节，写成 8 位小写十六进制。照数据根写的路径算，不追链接：同一个数据根换个写法算出来不一样也不要紧，头照 `run/socket` 去连。
- **上限**按字节算：Linux 107，macOS 103（系统的上限 108、104 都算上了结尾的零）。临时目录下也放不下，或者没有用户编号：起不来。Windows 上没有上限。
- **能用的 `$XDG_RUNTIME_DIR`**：是绝对路径、顺着链接是一个目录、属主是自己（有效用户编号）、组和别人一点权限都没有。没设的、空的当没设；设了却不能用的也当没设，记一条 `WARN XDG_RUNTIME_DIR not usable, using run/ dir=…`。macOS、Windows 上放套接字不看它；但 macOS 上核对、记 WARN 这一步照样做（`Dirs::current` 在所有 Unix 平台上都调它），只是算出来的位置不用；Windows 上 `runtime_dir` 直接是 `None`，不核对也不记。
- `$TMPDIR` 没设的，是系统默认的临时目录。`<uid>` 是有效用户编号。
- 别的类 Unix 系统照 Linux 的走。

**类型**

| 类型 | 是什么 |
|---|---|
| `Dirs` | 找套接字放哪要看的快照：`platform`、`runtime_dir`（核对过的 `$XDG_RUNTIME_DIR`）、`temp_dir`、`uid`（Unix 上是有效用户编号，Windows 上没有）。`Dirs::current` 从进程里读一次；测试喂一份快照，不改进程的环境变量 |
| `Opened` | 核心起来了：`listener` 等连接，`token` 这一次的本机令牌。打出来时不带令牌 |
| `Listener` | 等连接：`accept` 等下一个，`path` 套接字在哪；`publish` 写 `run/socket`、记 `INFO listening`（施工 CS-6，核心在开始接连接的前一刻调）；`take_lock` 把锁拿走另外拿着（施工 CS-6：停的时候先丢监听器、不放锁）。丢掉它：先删套接字文件、`run/socket`（`DEBUG socket location not removed error=…`），放在 `$XDG_RUNTIME_DIR/miyu-<指纹>/` 里的连这一层目录一起删，再放锁（锁拿走了的不放） |
| `Patience`（施工 CS-6） | 等多久：`slow` 过了这么久叫一声（3 秒），`limit` 最多等这么久（10 分钟）。`Patience::current` 是这一次用的：出厂的那一份；打开 `testkit` 的构建（`cargo test` 编出来的主程序）另认测试口子 `MIYU_TEST_WAIT_SLOW_MS`、`MIYU_TEST_WAIT_LIMIT_MS`（毫秒），发出去的程序不读 |
| `Slow`（施工 CS-6） | `&(dyn Fn() + Send + Sync)`：慢了叫谁。一次等（一次 `connect_or_start*`、一次 `spawn_detached*`、一次 `patiently`）最多叫一次 |
| `Connection` | 连上的一个连接：异步的字节流，能读能写，交给协议端点（`protocol.md`） |
| `Lock` | 拿着的单实例锁，丢掉就放开；`held` 交出锁文件在哪、是哪一个（`Held`，施工 TMP-2） |
| `Held` | `here`：数据根里的锁文件还是拿着的这一个（Unix 上照设备号和 inode 比，拿锁时记下打开的文件的，看时重新查路径；Windows 上没有文件的身份，照路径另开一次试拿：拿得到的就没人拿着、不是这一把，施工 CS-4；以前只看在不在，删了 `run\` 以后另一个核心重建了锁文件又起不来，这一个就永远不走）。数据根被删了、删了又建了一个新的，不是。核心照它定时看，不是了就走（`core.md`「停下」第 9 条）。`Listener::held` 交出同一份 |
| `Ready` | 核心写的那一行：`Ready`、`Running`、`Failed(原因)` |

### 怎么走

**核心起来**（`open` 先拿锁、做完下面六步；`open_locked` 接着已经拿到的锁往下走，第 5 步留给调用的一方，核心进程用它，`core.md`）

1. 拿锁：打开 `run/core.lock`（没有就建，不清空），标准库的文件锁，不等。拿不到：`Running`。一直拿到核心退出；进程没了，系统替它放开，崩了也不会留下死锁；同一个进程里打开两次也是两把，第二把拿不到。拿到了记下锁文件是哪一个（施工 TMP-2）：数据根被删了，锁还在核心打开的那个文件上，路径上已经没有了，下一个核心在新建的数据根上照样拿得到一把，核心照它看出来就走。
2. 删掉上一个核心留下的 `run/socket`（施工 CS-6）：没有的不算错，删不掉的起不来（`OpenError::Io`）。算出套接字放哪（上表）。
3. 换本机令牌：32 个系统给的随机字节，写成 64 位小写十六进制。先删掉上次留下的 `run/token.tmp`，新建它（Unix 上 0600，已经有的算错），写进令牌加 `\n`，再改名成 `run/token`。旧的就作废了。不同步：断电丢了，下次起来再写。
4. 在套接字上等连接：Unix、Windows 各见下。
5. 记下实际位置（`Listener::publish`）：同样先写 `run/socket.tmp`，再改名成 `run/socket`。记一条 `INFO listening socket=<位置>`。核心进程在开始接连接的前一刻才做这一步（施工 CS-6，`core.md`「起来的先后」第 16 步）：原来第 4 步一绑好就写，核心读配置、造路由那几步里头连得上却没人回，握手一直空等（2026-11-27 量过）。
6. 令牌在套接字能连之前就换好了：连上的头读到的一定是新的。

**Unix 域套接字**

1. 套接字所在的目录没有的，建成 0700：只建这一层，上一层要已经在。
2. 核对这个目录只有自己能进：是目录、不是链接，属主是自己，组和别人一点权限都没有。不是的：`NotPrivate`，不用它，也不去改它。放在 `run/core.sock` 的，核对的就是 `run/` 本身。
3. 看套接字的位置上有什么：
   - 什么都没有：绑。
   - 套接字文件，试着连一下被拒绝的：上一个核心崩了留下的旧套接字，删掉，记一条 `INFO stale socket removed socket=…`，再绑。
   - 不是套接字的文件，或者连得上的套接字（别的程序正在听，例如指纹撞了的另一个数据根的核心）：`Occupied`，不动它。
4. 套接字文件本身的权限不另外设：靠所在的目录挡住别人。
5. 核心走的时候删套接字文件（`DEBUG socket file not removed error=…`）。放在 `$XDG_RUNTIME_DIR/miyu-<指纹>/` 里的，这一层目录空了一起删（`DEBUG runtime dir not removed error=…`）；数据根的 `run/`、临时目录下几个数据根共用的 `miyu-<uid>/` 不删（施工 5-11 补：以前留着这一层，一个数据根留一个）。

**命名管道**（Windows）

1. 管道的每个实例都带安全描述符 `D:P(A;;GA;;;<当前用户的 SID>)`：受保护、不继承上一层，只有一条，允许当前用户全部权限。
2. 核心建第一个实例时声明「第一个实例」。名字已经被别的程序占了（拒绝访问）：`Occupied`，不动它。
3. 等连接：连上一个，先建好下一个实例等着，再把连上的交出去。等的时候出了错的实例也换成新建的，不然以后每次接都卡在同一个错上。建下一个实例这一步本身失败的，交回这个错；连上的那个实例没换出来，还留着等：下一次接时对它再等一次连接（已经连着的，系统当场说连上了），建成了下一个实例才把它交出去。少见。
4. 核心走的时候没有文件要删：句柄都关了，管道就没了。
5. `run/` 下的文件靠用户目录本身的访问控制，不另设。

**头连核心**（`connect`）

1. 读 `run/socket`：没有这个文件：`NotRunning`。去掉末尾的一个 `\n`；Unix 上照原样的字节，Windows 上要是 UTF-8；不是绝对路径的：出错（`run/socket 写的不是绝对路径：<路径>`）。
2. Unix：先核对套接字所在的目录只有自己能进（同上）：目录没了：`NotRunning`；不是只有自己能进：`NotPrivate`，不连，免得把令牌交给冒充核心的人。再连：没有套接字文件、被拒绝：`NotRunning`。
3. Windows：连管道。管道忙（核心刚接走一个、下一个实例还没建好），歇 50 毫秒再连，最多再连 100 次，一共 5 秒，还忙的出错；没有这个管道：`NotRunning`。连上以后问出管道服务端的进程，取它的用户 SID，和自己的比：不一样：`NotPrivate`，不交令牌。
4. 连上以后才读 `run/token`，去掉末尾的空白：核心刚换过令牌的，读到的是新的。
5. 交回连接和令牌；令牌在握手时出示（`protocol.md`）。

**连不上的核心还在不在**（`running_unreachable`，施工 CS-3）：头照 `run/socket` 连得 `NotRunning` 以后，要说「没在跑」的（`miyu core stop`，`core.md`）再看两样，有一样就是核心还在、只是连不上（跑着时 `run/` 被删了，令牌、位置都没了）：

1. 锁被拿着：打开 `run/core.lock`（没有的算没拿着），试拿一次，拿到当场放。同一个进程里另开一次也是另一把，拿着锁的进程自己问也是「拿着」。
2. 照这台机器的 `Dirs::current` 算出来的位置上有自己的核心在听：同「头连核心」第 2、3 步连一下就断开；`NotRunning`、`NotPrivate` 的算没有。放不下（路径太长）的算没有。

套接字本来就在 `run/` 里的（macOS，Linux 上没有能用的 `$XDG_RUNTIME_DIR`）跟着删了，外面找不到；核心自己照 `Held::here` 定时看到锁文件没了会走（施工 TMP-2）。

**等多久**（施工 CS-6，2026-11-27 项目主人定）：拉起核心、等拉起锁、等锁在别的核心手里却连不上的那一个、等握手的回应，都照 `Patience`：

1. 先看状态，不只看时间：管道里来了一行、管道到头了、拉起的进程退了、锁放了，都当场往下走。
2. 过了 3 秒（`slow`）还没好：叫一次调用方给的 `Slow`（命令行在标准错误上说「核心还在启动…」，网页把页面换成「还在启动」），接着等。
3. 最多等 10 分钟（`limit`）：活着却一直不动的（卡死、被 `SIGSTOP`）兜在这里。
4. 人按了 Ctrl+C：头照系统的默认当场退（等的时候没有装 Ctrl+C 的处理）；拉起的核心另开了一个会话（Unix，施工 CS-10），不跟着停。
5. 读管道的那一段能当场放下（`heard.rs`）：原来在阻塞线程池里读、外面套时限，时限到了读的线程还堵着，头放下运行时要等它，60 秒到了也要等核心写出那一行才退得了（2026-11-27 量到 138 秒），网页每超时一次漏一个线程。现在 Unix 上把管道交给 tokio 异步地读，放下就关；Windows 上匿名管道不能异步读，另起一条不归运行时管的线程读，放下了只是不再听，那条线程读到那一行、或者拉起的程序退了就走，放下运行时、进程退出都不等它。
6. 以前（施工 CS-1）照时间判：等那一行、等拉起锁各 60 秒，冷启动慢的机器上起得来的核心被说成起不来。

**拉起别的程序、等那一行**（`spawn_detached`、`spawn_detached_waiting`，施工 W-9；施工 CS-6 起照「等多久」）：「连不上就拉起」第 4、5 步单独开放出来：拉起一条命令、跟终端脱开、工作目录照给的、等它写来的那一行（`Ready`）。核心照它拉起，网页软件的 `open` 也照它拉起 `serve`（`web-ui.md`）。Windows 上拉起之前，头把自己的三个标准句柄标成不能继承（施工 CS-2，`miyu_pipe::uninherit_std_handles`）：标准库拉起子进程时本进程所有能继承的句柄都跟着进去，`miyu ask` 的标准输入是重定向来的文件时，核心不然一直开着它，文件删不掉（2026-11-02 桌面端撞到）。

**不读本机令牌地连**（`connect_bare`、`connect_or_start_bare`，施工 W-8）：同上，只是第 4 步不读 `run/token`，只交回连接。网页软件转发浏览器的连接用它：浏览器的凭据由页面在握手时自己出示，网页软件的代码里拿不到本机令牌（`web-module.md`「起草时定的」第 4 条）。核对目录、核对管道另一头照旧；拉起照「连不上就拉起」。

**连不上就拉起**（`connect_or_start`）

1. 连。连上了就用。连不上、又不是因为没在跑的（`NotPrivate`、读写出错）：出错，不拉起。
2. 没在跑：拿拉起锁 `run/spawn.lock`（没有就建）。别的头正在拉，每 50 毫秒试一次，每次顺手连一次，连上了就用、不要锁了；照「等多久」，到了上限还拿不到：`Busy`。
3. 拿到了，再连一次：别的头可能刚拉起来。
4. 还连不上，看核心的锁 `run/core.lock`（施工 CS-6）：被拿着的，是一个核心正在起来（绑好了套接字、还没写 `run/socket`）或者正在收尾（不再接连接了）。每 50 毫秒连一次、看一次锁：连上了就用；锁放了（它走了）接着第 5 步拉一个新的；照「等多久」，到了上限：`Timeout`。原来不看锁当场拉，拉起的那一个说 `running` 又去连收尾中的那一个，等它走了也连不上。
5. 还连不上，拉起核心：
   1. 命令由调用的一方给：主程序给的是自己加上 `core`（`cli/main.md`）。环境照头的，不另改：没设 key 的 shell 里拉起的核心就没有模型，之后换一个设了 key 的头来连，连上的还是它，一样没有可用的模型，要等它空闲退出（`core.md`）。
   2. 工作目录是数据根，不占着头的当前目录。
   3. 标准输入、标准错误接空；标准输出是一根管道的写端。
   4. 跟终端脱开：Unix 上开一个新会话（`setsid()`，施工 CS-10，`miyu_pipe::new_session`），不带控制终端，进程组号就是它的进程号；Windows 上 `DETACHED_PROCESS`、`CREATE_NEW_PROCESS_GROUP`，不带控制台窗口。终端里按 Ctrl+C 打不到它。Unix 上原来只自成一个进程组，核心还在头的会话里、拿着头的控制终端，它起的 `zsh -i` 这类命令打开 `/dev/tty`、`tcsetpgrp` 抢走头那个终端的前台（`bash -i` 则 `SIGTTIN` 停住自己）：2026-11-27 网页终端会话实测撞到。现在核心起的命令、扩展、看门的都在核心的会话里，打不开 `/dev/tty`。开新会话和设进程组不能同时做（标准库先 `setpgid`、后跑 `pre_exec`，先当了组长 `setsid()` 回 `EPERM`），所以只做前一样；标准库开新会话的开关没稳定，`pre_exec` 要 `unsafe`，放在 `miyu-pipe`。
   5. 头不等它退出：另起一个线程替它收尸。
6. 等它写来一行，照「等多久」，最多读 4096 字节：
   - `ready`：连它；连不上的照连不上说。
   - `running`：已经有一个核心在跑（别的程序抢先起了一个，例如服务管理器）。同第 4 步等它能连上；它也走了的：`NotRunning`。
   - `error <原因>`：`Refused`，原因照原样。认不出的一行也当起不来，原因就是这一行。带代码的（`error <代码>: <原因>`，施工 LC-1；施工 EN-2 起核心每一步都带）`Refused` 多带 `code`，头照 `core/start/<代码>` 取给人看的字，原因换进 `{reason}`（数据根认不出的那几种原因是数据根的位置，字里是 `{path}`；`StartError::said` 两个字段都给），照自己的语言说；没有代码的照原因说。取不到字的 `Display` 是「核心无法启动（<代码>）：<原因>」。
   - 什么都没写就退了：`Silent`。管道到头了是；拉起的进程退了、管道却还没到头的（它起的子进程拿着写端），再听 1 秒，还是没有的也是（施工 CS-6：看进程在不在，不只看管道）。
   - 到了上限没等到：`Timeout`。
7. 拉起锁一直拿到交回为止（连上了，或者出了错）。
8. 真正保证一个数据根只跑一个核心的是核心自己拿的 `run/core.lock`；拉起锁只是免得几个头同时白拉。清旧套接字的是核心，头不碰。

**人停的不自己拉起**（施工 CS-5，2026-11-17 项目主人定「停了就停，卡片带启动」：网页开着时 `miyu core stop`，核心断一瞬又被网页拉起来）：

1. 记号是 `run/stopped`，一个空文件：核心收到 `core.stop`（人停的：`miyu core stop`、头上的「停止」、托盘的「退出」）回应以前写（`mark_stopped`，写不进的记一行 `WARN stop mark not written`、照停）。崩了、空闲退出、收到停的信号的不写。
2. 核心起来拿到锁以后、建套接字以前删掉它（`open_locked`），不管是谁、怎么拉起来的。删不掉的起不来（`OpenError::Io`）。
3. 拉起分两种：人要用的（命令行的命令、头刚打开、头上的「启动」）照上面「连不上就拉起」（`connect_or_start`、`connect_or_start_bare`）；头断线以后自己重连的照 `reconnect_or_start`、`reconnect_or_start_bare`：第 1 步没在跑的，看一眼记号，在就交回 `StoppedByUser`（说法 `core/start/stopped_by_user`「核心已停止」），不拿拉起锁、不拉；拿到拉起锁、再连还是没在跑的再看一眼（等锁的时候别的头可能刚停了它）。
4. 记号读不了的当没有：照旧拉起，和以前一样。

**那一行**

| 核心写的 | 读成 |
|---|---|
| `ready\n` | `Ready`：好了 |
| `running\n` | `Running`：已经有一个核心在跑，这一个走了 |
| `error <原因>\n` | `Failed { code: None, reason }`：写的时候，原因里的 `\r`、`\n` 换成空格，只有一行 |
| `error <代码>: <原因>\n` | `Failed { code: Some(代码), reason }`（施工 LC-1）：代码是小写字母开头、只有小写字母、数字、`_` 的一个词；冒号前不合这个写法的整句当原因（以前的核心写的照旧读） |
| 别的 | `Failed(这一行)` |

读的时候先去掉行尾的 `\r`、`\n`。

### 出错

| 情形 | 什么时候 | 说的话 |
|---|---|---|
| `OpenError::Running` | 已经有一个核心拿着锁 | 这个数据根上已经有一个核心在运行 |
| `OpenError::TooLong` | 套接字哪里都放不下 | 套接字的路径太长，放不下：<本该放的位置>；临时目录下也放不下 |
| `OpenError::NotPrivate` | 套接字要放的目录不是自己的，或者别人也能进 | <目录> 不属于当前用户，或者其他用户也能访问：套接字不放在这里 |
| `OpenError::Occupied` | 位置上有别的东西；Windows 上管道名被占了 | <位置> 上有其他内容：不是套接字的文件，或者其他程序正在监听。没有修改它 |
| `OpenError::Io` | 读写出错 | 系统的原话 |
| `ConnectError::NotRunning` | 核心没在跑 | 核心未运行 |
| `ConnectError::NotPrivate` | 套接字的目录别人也能进；Windows 上管道另一头不是自己的进程 | <目录或管道> 不属于当前用户，或者其他用户也能访问：不连接 |
| `ConnectError::Io` | 读写出错，`run/socket` 写的不是绝对路径 | 系统的原话；`run/socket 写的不是绝对路径：<路径>` |
| `StartError::Refused` | 核心说了起不来 | 核心无法启动：<原因> |
| `StartError::Silent` | 核心什么都没写就退了 | 核心没有启动，也没有给出原因：查看数据根的 var/log/core.log |
| `StartError::Timeout` | 等过了上限（10 分钟）还没等到那一行，或者锁在别的核心手里一直连不上（施工 CS-6） | 等待核心启动超时 |
| `StartError::Busy` | 等过了上限还没拿到拉起锁 | 其他程序正在启动核心，等待超时 |
| `StartError::Connect` | 连不上 | 照 `ConnectError` 的话 |
| `StartError::Io` | 读写出错，例如找不到程序、拉不起来 | 无法启动核心：<系统的原话> |

- 上面是原话（`Display`，中文），日志和取不到字的时候用。`ConnectError`、`StartError` 另交说法 `said()`（施工 EN-1）：内核 `human/` 里三种语言的 `start/not_running`、`start/not_private_socket`、`start/not_private_pipe`（`{path}`）、`start/refused`（`{reason}`，核心的原话；带代码的头另可照 `start/<代码>` 说）、`start/silent`（`{log}`，数据根里的相对位置）、`start/timeout`、`start/busy`、`start/stopped_by_user`（施工 CS-5）、`start/spawn_failed`（`{error}`）；`Io` 的没有，照系统的原话。另有三句不是错（施工 CS-6，头自己取）：`start/slow`「核心还在启动…」、`start/answer_slow`「核心还没有回应，继续等待…」、`start/no_answer`「核心一直没有回应，已停止等待」。命令行照握手以前的界面语言取字（`miyu-cli` 的 `startup.rs`），印在标准错误上，退出码 1；`miyu ask` 没有 key、核心又没在跑的另说，退出码 5（`cli/ask.md`）。
- 运行日志（目标 `miyu::ipc`）：核心里记 `INFO listening socket=…`、`INFO stale socket removed socket=…`、`WARN XDG_RUNTIME_DIR not usable, using run/ dir=…`、`DEBUG socket file not removed error=…`（删套接字文件没删成）。头里发的 `DEBUG core not reaped error=…`（替拉起的核心收尸没收成）记不下来：头不装运行日志。

### 给人看的字

就是「出错」表里的那几句，只有中文。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-ipc/src/place/tests.rs` | 指纹是前 8 位十六进制；Linux 放 `$XDG_RUNTIME_DIR` 下；没有的放 `run/`；macOS 不看它；太长的放临时目录；上限按字节、不算结尾的零；哪里都放不下；Windows 是命名管道 |
| `crates/miyu-ipc/src/lock/tests.rs` | 第二把锁等第一把放开；两个数据根不共用锁；锁文件删了的、删了又建了的不是拿着的那一个（只在 Unix 上，施工 TMP-2）；`held` 分得清拿着的和留下的锁文件（施工 CS-3） |
| `crates/miyu-ipc/src/unreachable/tests.rs`（施工 CS-3） | 锁拿着、令牌和位置删了的算在跑；锁文件也删了、算出来的位置还在听的算在跑；从没起过的、走了只留下文件的不算 |
| `crates/miyu-ipc/src/files/tests.rs` | 令牌是 64 位小写十六进制、读回来一样、每次换新的、只有自己能读（上次留下的临时文件不沿用）；位置是一行、读回来一样、相对路径不认、不是 UTF-8 的照原样 |
| `crates/miyu-ipc/src/ready/tests.rs` | 三种写了读回来一样；原因只有一行；认不出的是起不来 |
| `crates/miyu-ipc/src/start/tests.rs` | 听得到每一种；没说话就退了的；等太久的；等过了上限放下运行时不用等那个一直不说话的程序（施工 CS-6，原来要等它说话）；起得慢的叫一次「慢了」、接着等到好了；说了起不来的当场结束；拉起的进程退了、子进程还拿着管道的不等子进程（Unix）；别的头一直拿着拉起锁是 `Busy`、叫一次；核心的锁在别人手里又连不上的等它放了再拉、一直不放的是 `Timeout` |
| `crates/miyu-ipc/src/patience/tests.rs`（施工 CS-6） | 出厂 3 秒、10 分钟；一直不好的叫一次、到上限交回没有；快的不叫；几段共用一个钟只叫一次 |
| `crates/miyu-ipc/tests/published.rs`（施工 CS-6） | 绑好了、还没 `publish` 的连是 `NotRunning`，上一个留下的 `run/socket` 起来时删掉；写了就连得上；锁拿走以后丢掉监听器删掉位置、不放锁 |
| `crates/miyu-ipc/src/unix/tests.rs` | `$XDG_RUNTIME_DIR` 要是绝对路径、只有自己能进；只有自己的真目录才算；别人的不算；新建的 0700，已经有的不动 |
| `crates/miyu-ipc/tests/socket.rs` | 照 `run/socket` 连上、拿到令牌；Linux 在 `$XDG_RUNTIME_DIR` 下；太长的在临时目录下；一个数据根一个核心、走了删套接字、每次换令牌；旧套接字清掉；别的东西不动；别人进得来的目录不用、链接不用；临时目录也要只有自己能进；头不从别人进得来的目录连；没有核心时连不上 |
| `crates/miyu-ipc/tests/pipe.rs` | Windows：照管道连上、拿到令牌，接走一个以后下一个在等；连上就走的不堵管道；一个数据根一个核心；名字被占了不动；没有核心时连不上 |
| `crates/miyu-pipe/src/tests.rs` | 安全描述符的写法 |
| `crates/miyu-pipe/src/session/tests.rs`（施工 CS-10，Unix） | 开了新会话的子进程会话号、进程组号都是它自己的进程号，不在测试进程的会话里；同时设了 `process_group` 的起不来（`EPERM`） |
| `crates/miyu-pipe/src/parent/tests.rs`（施工 XG-1 下，Windows、macOS） | 起一个子进程，问出来的父进程是测试进程自己；macOS 上测试进程的父进程和标准库说的一样，没有的进程问不出 |
| `crates/miyu-pipe/src/windows/tests.rs` | Windows：管道只对本人开放；一个名字只有一个第一个实例；头看到另一头是自己；当前用户是一个 SID |
| `crates/miyu/tests/core.rs` | 真的主程序：拉起、再连不拉起、两个头同时只拉起一个、起不来的说原因、没说话就退了的、已经在跑还没开始等连接的等着连上它、核心的工作目录是数据根 |
| `crates/miyu/tests/terminal.rs`（施工 CS-10，Unix） | 真的伪终端，`python3` 当头、拿它当控制终端敲 `miyu core start`：核心的会话是它自己开的（会话号是它的进程号）、不是头的、没有控制终端；人用 `!` 跑的抢终端的小程序、`sh -c 'exec </dev/tty'`、`bash --norc -i`、`zsh -i`、`zsh -i -c 'echo hi'`（没有 bash、zsh 的跳过那几条）30 秒内都跑完、打不开 `/dev/tty`、印出 hi，终端的前台一直是头的组 |
| `crates/miyu/tests/waiting.rs`（施工 CS-6，Unix） | 真的主程序、真的核心，照测试口子缩短时限：核心卡在读配置（系统配置换成命名管道）时 `miyu core start` 过了一会儿说「核心还在启动」、到了上限说超时当场退；等的时候核心被杀当场说没起来；按 Ctrl+C 当场退；绑好套接字、还没开始接连接的连是 `NotRunning`；核心被 `SIGSTOP` 时 `miyu config get` 说「核心还没有回应」、到了上限说没回应当场退 |

### 出处

- `04-核心协议.md` 第二节「各平台的坑」：路径上限、0700 的目录和核对属主、只对本人开放的管道和核对另一头；P2。第四节：连接即身份，本机令牌。
- `07-存储.md` 第二节：套接字放哪、指纹、核对、旧套接字、`run/socket`、`run/token`；第十节：单实例与锁。
- `12-进程形态与分发.md` 第二节「拉起时的握手」。
- `11-权限与沙盒.md` 第五节：本机令牌只有本人读得到。

### 还没有的

- 浏览器、远程用 WebSocket，核心自己查 Origin 头；远程要登录（`04-核心协议.md` 第二节、第四节，P5、P6）。
- 核心拉起的扩展、桥经标准输入输出连（`04-核心协议.md` 第二节）。
- `miyu stdio`：替别的程序连上核心、出示令牌，之后原样转发（`22-命令行.md` 第四节）。
- 沙盒里的命令连不上核心、读不到令牌（`11-权限与沙盒.md` 第五节，A6）：沙盒还没做（M5）。
- 先开界面、后连核心（2026-11-27 项目主人定做，排在正式版以后）：直接运行 `miyu` 时现在先连上核心、问 `config.get` 拿 `ui.head` 才拉起界面（`cli/main.md`），核心起得慢就先对着空终端等；以后先照本机的配置开界面，界面自己连核心、显示「还在启动」。
- 拉起时还在收尾的核心走了以后再拉一个是在拉起锁里做的；拉起的那一个说 `running`、接着那一个也走了的，只报 `NotRunning`，不再拉第三个（拉起的命令只给一次，施工 CS-6）。
