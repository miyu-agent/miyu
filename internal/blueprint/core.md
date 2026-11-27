## 核心进程 `miyu core`

### 是什么

一个数据根只跑一个的核心进程：由头拉起（`ipc.md`），平时不用人敲。起来时建骨架、拿单实例锁、装运行日志、建默认工作区、找资源目录、在本机的套接字上等连接、读供应商的档案造会话的路由（用哪家、哪个模型全照配置，施工 8-6）、登记工具，然后往标准输出写一行 `ready`，在后台清一次回收处；之后一个个接连接，照协议说话（`protocol.md`）。默认一直开着（施工 Z-1 上，`12-进程形态与分发.md` R1）；系统配置写了 `core.idle_exit` 的，没有连接、没有在跑的回合、也没有在跑的后台命令，空闲够久了自己退出；收到停的信号，先让在跑的会话有计划地停下（后台命令先记 `restarted`、再整组杀）再退出。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `core`、`--idle-seconds` |
| `crates/miyu-core/src/lib.rs` | `main`：起来的先后；工具目录；写那一行 |
| `crates/miyu-core/src/start.rs`、`owner.rs` | 起来的后半段：开套接字、找主人，没有的当场照系统用户名建一个（施工 SU-1），照他读清单和配置 |
| `crates/miyu-core/src/models.rs`、`models/` | 读资源目录里的供应商档案（TOML 读成 JSON 交给 `miyu-models`）和模型资料，造会话的路由（施工 8-6，`models.md`） |
| `crates/miyu-core/src/serve.rs` | 接连接，空闲退出（配置项 `core.idle_exit`，施工 Z-1 上），头叫停（施工 DT-4 下，`core.stop`），数据根没了就走（施工 TMP-2）；定下要停就不再接新连接（施工 CS-6）；停下以后放锁之前的收尾（`serve_then`，施工 TMP-1 补2） |
| `crates/miyu-core/src/stops.rs`（施工 CS-6） | 停的信号：起来时就装；说 `ready` 以前来的当场退（施工 CS-8）；第二次当场强退；收尾的总时限 30 秒 |
| `crates/miyu-core/src/workers.rs` | 运行时开几个工作线程：照核数和系统配置的 `core.workers`，建运行时以前单独读这一项（「起来的先后」第 7 条，施工 CPU-1） |
| `crates/miyu-endpoint/src/closing.rs` 的 `Core::finish_writes`、`Core::close_databases`（施工 CS-9 从 `lib.rs` 挪出来） | 等记忆在后台还没做完的：回合开始没等完的读、回合索引的后台、补齐旧会话（施工 TMP-1 补3，下面「停下」第 6 条；`memory.md` 第三条第 4 款）；停会话（`stop_sessions`）时也等。放锁以前关上数据根里的库（施工 CS-9，下面「停下」第 8 条） |
| `crates/miyu-core/src/pending.rs` | 另起线程写盘的照门起、登记下来，核心放锁之前关门、等它们写完（施工 TMP-1 补2、CS-8，下面「停下」第 6 条）；还在读目录的记个数，只有测试的夹具等它们（施工 TMP-3、CS-8） |
| `crates/miyu-core/src/sandbox.rs` | 起来时找沙盒的助手、探一次，记日志（施工 5-1）；探到了手段的，交回助手（施工 5-4 上） |
| `crates/miyu-core/src/packages.rs`、`packages/net.rs` | 照编进来的可选软件包（cargo 开关）往查询表里登记，交给 `Core`（施工 W-4，`mermaid.md`；`net` 的 `link.preview` 在后台答，卡片的图存进 blob，施工 W-7，`net.md`）；起来时清掉分块上传留下的暂存（施工 W-5） |
| `crates/miyu-core/src/leftovers.rs` | 起来时收拾上一次崩了留下的：升级到一半的包（施工 AE-7）、写配置和密钥剩下的临时文件（施工 AS-11） |
| `crates/miyu-core/src/trash.rs` | 起来时清一次回收处；删了的会话留多久 `KEEP`（施工 3-8 三补）；删了的人格同样清（施工 P-3 下） |
| `crates/miyu-core/src/settle.rs` | 一阵请求过去、安静 2 秒以后还内存（施工 V-5 中，`heap.md` 第 2 条） |
| `crates/miyu-core/src/wal.rs` | 库最后一次提交过去 2 秒放掉页缓存，过去 30 秒把写前日志合回主库、截短（施工 V-28、BO-3，`store.md`「怎么走」第 14 条） |
| `crates/miyu-core/src/memlog.rs` | 自己记内存账：什么时候采、一行的样子、采样的任务（施工 V-4，`heap.md`「自己记账」） |
| `crates/miyu-core/src/settings.rs` | 配置清单：登记各模块的几项；起来时读配置（施工 8-5 起连同密钥文件，环境照进程的）、照 `log.level` 换运行日志的级别（施工 8-2），生成两份 JSON Schema 和参考文件（施工 8-1，`config.md`「怎么走」第一、二条）；运行中跟着配置换级别、重写这三份（`settings/follow.rs`，施工 8-4） |
| `crates/miyu-sandbox/src/lifeline.rs`、`lifeline/` | 核心没了，它起的命令跟着没（施工 7-8，下面「子进程随核心退出」）：Unix 上每条命令的组里一个看门的（照主程序旁边的 `miyu-watchdog` 起，退回 `/bin/sh`，施工 LW-28），Windows 上核心进作业对象；扩展、MCP 服务、语义模型小程序照 `keep` 起在共用的那一个看门的组里（施工 XH-1，`lifeline/unix/shared.rs` 是施工 LW-28） |
| `crates/miyu-watchdog` | 看门的小程序 `miyu-watchdog`（施工 LW-28）：读到标准输入的结尾就杀掉自己所在的整个组；只在 Unix 上看门 |
| `crates/miyu-ipc` | 单实例锁、套接字、本机令牌、那一行的写法（`ipc.md`） |
| `crates/miyu-endpoint` | 协议端点：核心的家底 `Core`（里面有执行器的任务表，施工 7-3）、接连接、空不空闲（`protocol.md`） |
| `crates/miyu-basesystem`、`crates/miyu-tool` | 基础系统的七件工具、工具目录（`tools/interface.md`） |
| `crates/miyu-log` | 运行日志（`log.md`） |
| `crates/miyu-store` | 数据根、资源目录（`store.md`） |
| `crates/miyu-sandbox` | 沙盒：找助手、探测（`sandbox.md`） |

### 对外的样子

命令：`miyu core [--idle-seconds <秒>]`：跑核心本身，由头拉起，不用人敲，这个写法和它的参数不写进帮助。`miyu core start | status | stop` 管在后台跑的那一个（施工 CL-8，2026-11-18 项目主人定：这三条只管核心），写进主程序帮助页的「系统」一组，`miyu core -h` 和三条子命令的 `-h` 印同一页（`cli/main.md`「帮助页」）。

`miyu core start`（施工 CL-8，`crates/miyu-cli/src/core_start.rs`）：建好数据根的骨架，照「连不上就拉起」（`miyu_ipc::connect_or_start`，`ipc.md`）连一次，连上了就放开、不握手。成了什么都不印，退出码 0；已经在跑的也是。只拉核心：不开界面、不开网页、不开浏览器。人停过的（`run/stopped`，施工 CS-5）照样拉——这就是人要它起来，核心起来时自己删掉记号。起不来的（它说了原因的、没说就退了的、等太久的）照设计 47 说在标准错误上，退出码 1。拉起来的核心同头拉起的一样照 `core.idle_exit` 走（出厂一直开着）。

`miyu core status [--format <text|json>]`（施工 CL-8，`core_status.rs`）：只看，不拉起（`miyu_ipc::connect`）。

1. 在跑：握手，版本照核心报的（`hello` 回应的 `core.version`；升级以后没重启的和这个程序的不一样）。标准输出三行「核心正在运行」「版本：<版本>」「数据根：<绝对路径>」（英文 `The core is running`、`version: …`、`data root: …`；照握手回的语言），退出码 0。
2. 没在跑（先照 `miyu core stop` 那样确认，施工 CS-3）：标准输出三行「核心没有在运行」「数据根：<绝对路径>」「提示：运行 'miyu core start' 启动」（英文 `The core is not running`、`data root: …`、`hint: run 'miyu core start' to start it`），退出码 3（同 `systemctl status`；网页包的 `miyu web status` 照同一套退出码）。
3. 说不准：核心还在跑却连不上（同 `stop` 的那一句）、握手被拒、连不上，照设计 47 说在标准错误上，退出码 1。
4. `--format json`：标准输出一行 `{"running": bool, "version": "<版本>" | null, "data_root": "<绝对路径>"}`，没在跑的 `version` 是 `null`；退出码照上面，没在跑也是这一行加退出码 3。

`miyu core stop`（施工 CL-4，桌面端的「退出」用）：连已经在跑的核心（不拉起），握手、发 `core.stop`（下面「停下」），再阻塞地拿一次 `run/core.lock` 等它真的退了才回（`miyu_ipc::wait_released`，不轮询：进程没了操作系统放锁）。成了什么都不印，退出码 0；没在跑的什么都不说，退出码 0。「没在跑」要先确认（施工 CS-3）：照 `run/socket` 连不上时再看锁被拿着没有、照这台机器算出来的套接字位置上有没有自己的核心在听（`miyu_ipc::running_unreachable`），有的是核心跑着时 `run/` 被删了、令牌和位置都没了，说「核心还在跑，但连不上它」、提示照运行日志 `starting` 那一行的进程号结束它，退出码 1，不另想办法结束它；套接字本来就在 `run/` 里的（macOS、Linux 上没设 `$XDG_RUNTIME_DIR`）跟着删了、外面找不到，它自己定时看到锁文件没了会走（施工 TMP-2）。被拒绝的照核心的原话、连不上的照原因、等了 60 秒还没退的说「核心在 60 秒内没有退出」，都照设计 47 印在标准错误上，退出码 1。

| 参数 | 做什么 |
|---|---|
| `--idle-seconds <秒>` | 空闲多少秒以后退出，非负整数，盖过配置项 `core.idle_exit`，开发、测试用；不写的照配置（施工 Z-1 上：默认不退，原来不写是 600 秒） |

用到的环境变量（拉起的核心照拉起它的头的环境，`ipc.md`）：

| 变量 | 做什么 |
|---|---|
| `MIYU_HOME` | 数据根；不设是家目录的 `.miyu`（`store.md`） |
| `MIYU_RESOURCES` | 资源目录，开发时指到源码树的 `resources/`（`store.md`） |
| `MIYU_LOG` | 运行日志记到哪一级，压过配置项 `log.level`（`log.md`、`config.md`） |
| `MIYU_CATALOG_UPDATE` | 后台更新 models.dev 的目录不更新，`true`、`false`，压过配置项 `models.catalog.update`（施工 8-7，2026-10-01 主会话定：离线的机器、测试拉起的核心用它） |
| `XDG_CACHE_HOME`（Linux）、家目录（macOS）、`LOCALAPPDATA`（Windows） | 缓存目录在哪（`store.md` 第 3 条）：后台拉的目录 |
| 配置里 `{ env = "…" }` 写的那几个 | 供应商的 key（`config.md` 第九条第 5 条）：核心照自己起来时的环境取，回合开始时取一次 |
| `XDG_RUNTIME_DIR`（Linux）、`TMPDIR` | 套接字放哪（`ipc.md`） |
| `HTTPS_PROXY`、`HTTP_PROXY`、`NO_PROXY` 这些 | 请求模型走不走代理（`http.md`） |

- 标准输出上只写一行（下面「那一行」）。标准输入、标准错误不用。
- 家目录（`std::env::home_dir`）交给协议端点：权限策略照它换 `~`，工作目录太宽照它判（`protocol.md`）。

写到数据根里的：

| 文件 | 是什么 |
|---|---|
| `.miyu-layout`、`etc/`、`etc/private/`、`run/`、`tmp/` | 骨架（`store.md`） |
| `workspace/` | 默认工作区 |
| `trash/sessions/` | 回收处：删掉的会话，留 7 天（`store.md` 第 12 条，施工 3-8 三补）。起来时清掉满了 7 天的 |
| `trash/personas/` | 删掉的人格，`<编号>.<删的时刻，毫秒>/`，同样留 7 天、起来时清（施工 P-3 下） |
| `var/log/core.log` | 运行日志，满 10 MiB 换一份，留 `core.log.1` 到 `core.log.5`（`log.md`） |
| `var/cache/provide/<包>.json` | 扩展上一次登记的工具（`provide` 的参数原样），起来时照它先登记（`providers.md`「登记缓存」，施工 O-2 中） |
| `run/core.lock`、`run/token`、`run/socket` | 单实例锁、本机令牌、套接字的位置（`ipc.md`） |
| `run/core.sock` | 套接字本身，放在 `run/` 的时候（`ipc.md`「套接字放哪」） |

### 怎么走

**起来的先后**

进核心之前（施工 LW-8，2026-11-22）：主程序先看参数是不是前台跑核心的三种写法之一：`core`、`core --idle-seconds <秒>`、`core --idle-seconds=<秒>`，秒数照 `u64` 解析得出（同 clap 解析这个参数用的 `str::parse`）。认得出的直接进核心，不搭整棵命令行树、不读界面语言（`crates/miyu/src/main.rs` 的 `running_core`）；认不出的（`start`、`status`、`stop`，要帮助的、写错的、不是 UTF-8 的）照原来整棵树解析，报错的字由 clap 照原样说。原因：核心进程只认这一种写法，原来搭整棵树、解析一遍要执行 266 kB 代码，执行过的函数都留在核心的内存里（`perf.md`「排函数」）。`main.rs` 的测试拿一组参数对照快路和整棵树的结果。

进核心之前还有一步（施工 LW-12，`heap.md`「线程缓存」）：如果这个进程没有带着「线程缓存少留」的环境变量（不是由 `miyu` 拉起的核心，例如 systemd 托管的、直接运行 `miyu core` 的），主程序带上它、用原来的参数重新执行自己一次，进程号不变；已经带着时直接往下运行。只有 glibc 上有这一步。

0. 进程的堆（施工 V-2 再补，`heap.md`）：起别的线程之前，glibc 上把 arena 限到 8 个、大块的门槛定在 128 KiB；记一行 `DEBUG heap tuned tuned=<真假> arenas=8 mmap_from=131072 huge_pages_off=<真假> tunables=<这个进程带着的 GLIBC_TUNABLES>`（装了日志以后补记）。别的平台什么都不做。
1. 读一次环境的快照（`MIYU_HOME`、`MIYU_RESOURCES`、家目录、程序的真实位置这些），数据根、资源目录、沙盒的助手照它找；`MIYU_LOG`、放套接字的目录到用的那一步才读；配置里 `{ env }` 引用的变量到用的时候才查（施工 8-6 起核心不再读开发用的环境变量、`DEEPSEEK_API_KEY`）。
2. 找数据根，建骨架（`store.md`）。
3. 拿单实例锁 `run/core.lock`，不等。拿不到：已经有一个核心在跑，写 `running`，退出码 0，运行日志一个字都不写。先拿锁、再装日志：两个核心不写同一份日志。
4. 装运行日志 `var/log/core.log`，级别照 `MIYU_LOG`，带上第 1 步的家目录（`log.md`）。记一条 `INFO starting version=<版本> pid=<进程号> root=<数据根> tz=<和 UTC 差多少>`，数据根里的家目录写成 `~`，例如 `root=~/.miyu tz=+09:00`。接着让它起的子进程随它结束（下面「子进程随核心退出」，施工 7-8）：Windows 上进作业对象，进不去记一条 `WARN children not bound error=…`，照样起来；别的平台这一步什么都不做。
5. 找资源目录、开始读目录（同下面第 6、9 条）；起运行时（第 7 条），在运行时里先装停的信号（下面「停下」第 4 条，施工 CS-6），再算出套接字放哪、换本机令牌、绑好套接字（第 8 条）。接着建默认工作区 `workspace/` 和敏感的几处顶层目录 `trash/`、`blobs/`、`modules/`、`index/`、`kb/`（没有就建，Unix 上 0700；后几个施工 R3-1 补：布局不变，只是不再用到才建，Windows 的沙盒只禁命令起来那一刻顶层已有的，命令跑着时第一次建的整棵读得到），建不了的起不来（`stage=home`）。接着收拾上一次崩了留下的（`leftovers.rs`）：`packages/` 里升级到一半的包放回去（施工 AE-7，`packages.md`「装卸」第 1 条）；数据根里写配置、密钥时剩下的临时文件删掉（施工 AS-11，`store.md`「在哪」的 `config_file.rs`；施工 R3-1 起看 `run/` 和数据根这一层）。访问密码不在这里设：要从局域网连时用 `account.setup` 设（`protocol.md`）。
6. 找资源目录（`store.md`）。接着读两层清单、读配置（施工 8-2，`config.md`「怎么走」第二条）：配置、信任的记录、密钥文件（施工 8-5），读不进来的照空的，有问题的每份记一条 `WARN config problems`（密钥文件组、别人读得到的记一条 `WARN secrets readable by others`）；照 `log.level` 的最终值换运行日志的级别，记一条 `INFO log level`。再照配置清单生成两份 JSON Schema 和参考文件，放在 `var/cache/config`（施工 8-1，`config.md`「怎么走」第一条第 6 到 8 条）。这一步在一条做完就退出的线程（`generate`）里做，主线程等它写完再往下走（施工 LW-5，`heap.md`「怎么走」第 5 条）：字照 `ui.language` 的最终值，`auto` 的照系统的语言挑，一样的不重写；读好的配置交给协议端点（`Core::with_config`）；写不成的、字缺了的，一份记一条 `WARN config schema not written`（目标 `miyu::config`），不影响起不起得来。说「好了」之前开始监视配置文件（`Core::watch_config`，施工 8-4，`config.md` 第七条），起一个任务跟着配置换：`log.level` 变了换级别，`ui.language` 变了重写那三份（第八条）；监视起不来的记一条 `WARN config watch unavailable`，照样起来。 照清单拼出来的包的配置项、清单的副本和配置的值只在这一步里使用，生成完参考文件就放下，不留到核心停下（施工 LW-16：原来这几份是启动函数的局部变量，启动函数一直等到核心停下才交回，它们跟着留在堆里约 0.19 MB）；读好的清单在造家底时交进去（`Core::with_found`），家底不再自己读第二遍（`packages.md`「怎么走」第 1 条）。
7. 起运行时：多线程，接连接、会话、请求都在上面。工作线程数照 CPU 核数（`available_parallelism`，取不到的照 2），系统配置写了 `core.workers`（1 到 64）的照它；都夹到 2 到 64（施工 CPU-1：原来写死两个，整机被挤满时 Linux 大体按线程分 CPU，两个线程大半时间在排队）。运行时要先建、配置在它里面读，所以这一项在建运行时以前单独读一次系统配置（`workers.rs`）：读不到、写坏了、超范围的照核数，不报，第 6 条正式读配置时照旧报；改了重启核心生效（`config.md` 的 `restart`）。运行日志在 `DEBUG` 记一条 `runtime workers=<个数>`。tokio 的阻塞线程池、自己开的线程不跟着变；glibc 的 arena 照旧 8 个（`heap.md`）。
8. 删掉上一个留下的 `run/socket`，算出套接字放哪、换本机令牌、绑好套接字（`ipc.md`）。还不写 `run/socket`（施工 CS-6）：头照它找不到这一个，到第 16 步开始接连接的前一刻才写。原来这一步就写，到第 16 步之间（读配置、造路由、探沙盒这些，冷启动、硬盘忙时几十秒）头连得上却没人回，握手一直空等（2026-11-27 量过）。
9. 读供应商的档案、认原厂的表，造会话的路由和核心一份的模型资料（下面「模型」，施工 8-6、8-7）。目录这时还不读。
10. 找沙盒的助手、探一次（`sandbox.md`「怎么走」第 1、2 条）：记一行 `INFO` `sandbox` 或者 `WARN` `sandbox unavailable`（施工 5-1）。探到的结果（能不能用、为什么，`Availability`）交给协议端点：握手时报给头（施工 5-4 下）；能用的，造会话、载入时把助手交给会话，权限策略照它判执行命令，执行器照它带沙盒（施工 5-4 上）；用不了的，会话里当沙盒用不了。
    - 沙盒的缓存（缓存目录下的 `sandbox/`，施工 5-4 下）只给「工作区」一级的沙盒用，随它去掉了（施工 LV-1，`session/tools.md` 第 1a 条第 5 条）：核心起来时不再算它，也不记 `sandbox cache unavailable`。
    - 本机 embedding（施工 R-5 下；R-5 三补改成照包，`recall.md` 第四条第 1 款）：照起来时读到的包清单拼（`embed::setup`）：人格记忆装着、它推荐的小程序包 `embed` 也装着（读成了、种类是小程序），程序照小程序清单先找包文件夹的 `bin/`、再找主程序真实位置的旁边（施工 F-8 上再补），模型清单 `model.toml`、模型文件在包目录里；交给协议端点（`Core::with_vectors`，接在模型资料后面：远程的 embedding 照那一份查供应商、记账，施工 R-5 补）。这一步只读模型清单，不核对文件、不碰网络、不拉起小程序：都等第一次要向量。没装 `embed` 的记一行 `INFO embedder unavailable reason=no embed package`（出厂不装，常态）；程序不在、模型清单读不了的记一行 `WARN embedder unavailable reason=…`。本机的那一路没有时，`models.embedding` 写了远程的照样用远程的，不写的只照关键词找。
    - 这两样都不影响起不起得来。
11. 工具目录：登记基础系统，十一件：`edit`、`glob`、`grep`、`history`、`jobs`（施工 7-4）、`send_message`（施工 7-7）、`read`、`shell`、`subagent`（施工 7-5；7-5 再补从 `agent` 改名）、`trash`、`write`；工具的字从资源目录读，登记完就冻结（`tools/interface.md`）。
12. 核心的家底：数据根、资源目录、模型、工具目录、系统的家目录、本机令牌，会话表是空的，执行器的任务表是空的（施工 7-3，`protocol.md`）；人格记忆装没装照读到的清单设在核心一份的记忆上（`Memory::set_installed`，施工 R-10，`memory.md` 第十一条）；照编进来的可选软件包往查询表里登记（`packages.rs`，cargo 开关 `mermaid`、`net`，施工 W-4、W-7）：`mermaid.render`、`link.preview` 调得到，没编进来的核心起来时这张表里就没有这一行；两个包都等第一次调才读自己的资源（`net` 这时才读环境变量里的代理）。清一遍分块上传留下的暂存：`blobs/tmp/` 里的 `upload-*`，崩了、被杀留下的（`packages::clear_uploads`，施工 W-5，`store.md` 第九条第 6 款）；在这一步做，接连接以前：真要传的东西这时都还没开始。不载入任何会话；会话列表的索引 `index/sessions.db` 用到时才开，开了一直开着：没有的新建，读不了、坏了、版本不对的删掉换一份空的，列会话时照日志补；都不影响起不起得来（施工 3-8 七补，`store/index.md`「怎么走」第 1 条）。同样打开用量汇总 `var/cache/usage.db`，一直开着，交给会话（落盘时写）、模型资料（一次性入口记账，`ModelData::keep_ledger`）、协议（`usage.query`）；新建的记 `INFO usage index created`，读不了、坏了、版本不对的删掉重建、记 `WARN usage index rebuilt`，查之前照日志补（施工 8-15，`models.md`「怎么走」第九条第 4 条）。会话表造会话、载入时交给会话一份造子会话的端口（施工 7-5，`protocol.md`「会话表」第 7 条）。
13. 在后台读 models.dev 的目录、用出来的、供应商的列表，读完放行等着它的（造会话、载入、`model.list`），之后在后台更新目录（下面「模型」第 5、6 条，施工 8-7）。
14. 清一次回收处（施工 3-8 三补，`store.md` 第 12 条第 2 款）：回收处里删了满 7 天（`KEEP`，2026-09-30 项目主人定）的会话连目录删掉。删之前先留用量的底（施工 8-15，`models.md`「怎么走」第九条第 5 条）：照它的日志算好合计，往 `journal.jsonl` 追加一条 `usage.purged`，写进去了才删；写不进去的（连同日志读不了的）留着，记 `WARN trash entry kept`，下次再清。写了 `ready` 以后在阻塞线程里清，不耽误头连上来、第 15 步照常；核心放锁之前等它清完（下面「停下」第 6 条）。钟是这时系统的钟，读不出的当 1970 年（什么都不满时限，一个都不删）。删了的记一条 `INFO trash purged removed=<几个>`，一个都没删的不记；读不出删的时刻、删不掉的，一个一条 `WARN trash entry kept session=… error=…`；回收处读不了的记 `WARN trash not read error=…`。都不影响起不起得来。
15. 开始自己记内存账（施工 V-4，`heap.md`「自己记账」）：Linux、macOS（施工 V-4 补）上起一个任务，记一行 `start`，之后每分钟一行，涨得快的多一行，写进 `var/log/memory.jsonl`；量不出来的平台不起，记一条 `DEBUG memory samples unsupported`；文件打不开的记一条 `WARN memory samples not started`，照样起来。接着起合库的写前日志的线程（施工 V-28，`store.md`「怎么走」第 14 条）：库最后一次提交过去 2 秒，把 `-wal` 合回主库、截短；一个进程只起一个，起不来的记一条 `WARN wal checkpoints not started`，照样起来。
16. 写 `run/socket`（`Listener::publish`，记 `INFO listening`），往标准输出写一行 `ready`，接着一个个接连接，直到停下（下面「停下」）。这三件之间没有别的事：头读到 `ready`、照 `run/socket` 连上就有人回（施工 CS-6）。原来 `ready` 写在第 13 步，之后拉起扩展、补登记 blob 这些做完才开始接连接。写不进 `run/socket` 的起不来（`stage=socket`，`socket_failed`）。

第 2 到 11 步哪一步出了错（第 3 步拿不到锁的除外；第 10 步探沙盒只记日志，不会出错）：写 `error <代码>: <原因>`（施工 EN-2：每一步都带代码，`miyu-core` 的 `not_started.rs`；原因只写会变的那一段，系统的原话或路径），退出码 1；运行日志已经装上了的（第 5 步起），再记一条 `WARN not started stage=<哪一步>`：第 5 到第 9 步依次是 `home`、`resources`、`runtime`、`socket`、`models`，第 11 步是 `tools`。原因只交给头，不进运行日志（施工 4-9 再补四上：原来 `reason=<原因>` 整句写进去）。第 8 步以后出的错，走的时候照样删掉套接字文件、放开锁（`ipc.md`）；写那一行之前先关门、等已经起了的写盘线程写完，还在读目录的不等（下面「停下」第 6 条，施工 CS-8），调它的地方接着就放锁。

**那一行**

头拉起核心时，把一根管道的写端给它当标准输出，等它写来一行（`ipc.md`）：

| 写的 | 意思 | 退出码 |
|---|---|---|
| `ready` | 好了：在套接字上等连接了 | 之后照「停下」 |
| `running` | 已经有一个核心在跑：这一个走了，头去连那一个 | 0 |
| `error <原因>` | 起不来。原因是出错的原话，照原样给人看；里面的回车、换行换成空格 | 1 |
| `error <代码>: <原因>` | 起不来、认得出是哪一种（施工 LC-1；EN-2 起每一步都带）：数据根认不出（`data_root_not_ours`）、是旧版的（`data_root_old_layout`）、布局不是这一版的（`data_root_other_layout`），原因是数据根的位置；`MIYU_HOME` 相对、找不到家目录（`miyu_home_relative`、`no_home`、`no_local_app_data`），建骨架读写出错（`data_root_failed`），拿锁（`lock_failed`）、装运行日志（`log_failed`）、资源目录（`resources_relative`、`resources_missing`、`resources_not_found`、`resources_unknown`）、运行时（`runtime_failed`）、套接字（`socket_too_long`、`socket_not_private`、`socket_occupied`、`pipe_occupied`、`socket_failed`）、工作区（`home_failed`）、模型（`models_failed`）、工具（`tools_failed`），原因是系统的原话或路径。头照 `core/start/<代码>` 照自己的语言说（`resources/core/human/`） | 1 |

- 每一种都带一个 `\n`，写完马上送出去。写不出去的，记一条 `WARN ready line not written error=…`（运行日志装上了的话）。
- 原因是出错的原话：Miyu 自己写的多是中文，例如 `MIYU_HOME 要写绝对路径，写的是 …`、`找不到资源目录：… 都没有。开发时设 MIYU_RESOURCES 指到源码树的 resources/`（`store.md`、`ipc.md`）；工具的字读不出来的是 `<哪一份文件>: <为什么>`；工具登记时查不过的是英文，例如 `tool "read": another tool has the same name`（`tools/interface.md`）；系统和用到的库报的错（例如 HTTP 客户端造不出来的）照它们的写法。

**模型**（施工 8-6，`models.md`「怎么走」第一条、第四条）

1. 起来时读资源目录里的两份：供应商的档案 `models/profiles.toml`（TOML 读成 JSON，照 `miyu-models` 的样子读；`[npm]`、认得出的供应商的驱动、地址、`openai-chat` 的开关、一张图怎么算）和认原厂的表 `models/vendors.toml`（施工 8-7，同样读成 JSON）。读完记一条 `INFO model profiles loaded profiles=<几家>`。哪一份读不出来、写法不对：起不来，原因写明是哪一份。HTTP 客户端（请求模型的、GET 用的两个）造不出来（系统的证书读不了之类）：起不来。代理照环境变量（`http.md`）。
2. 造会话的路由（`miyu_session::Routes`，`session/actor.md` 第 8 条）交给协议端点：用哪家供应商、哪个模型、哪个 key，全照配置（`[providers.<id>]`、`models.chat`），每个会话照回合开始时冻结的那一份挑（`config.md` 第八条第 3 条）。空闲超时 180 秒（`http.md`）。核心不再读开发用的 `MIYU_DEV_*` 和 `DEEPSEEK_API_KEY`：开发自测改成 `cargo xtask dev-home <目录>`（`models.md` 第十条）。
3. 没配模型（`models.chat` 没配、配的解析不出、那一家用不了、key 一个都取不到）：照样起来；每次请求都当场说完、没发出去，分类 `no_model`，原话照 `models.md`「出错」（例如 `no model configured: set models.chat`），`model.called` 里没有端点和模型；内核不再来（`kernel/session.md`）。路由每次这样记一条 `WARN no model why=…`（目标 `miyu::session`）。
4. key 照配置里的引用取：`{ secret }` 取密钥文件里的，`{ env }` 取核心起来时的环境；换了密钥，下一个回合开始时用上，不用重启（`config.md` 第九条第 8 条）。key 只在内存里，不进日志。
5. 目录（施工 8-7，`models.md`「怎么走」第二条第 1、2 条）：找到资源目录就在一个线程里读（施工 V-2 下补；V-2 下以前写了 `ready` 以后才读，头一连上就要载入会话，白等约 30 ms；V-2 下是造好路由才读），和读清单、配置、生成文件这些起来的步子并着走、不挡 `ready`，造好路由以后等它读完、放行，安装包带的快照（`models/models-dev.json`）和缓存目录里的 `models/models-dev.json` 比旁边 `meta` 的 `fetched`，用新的；新的读不了用另一份；都读不了目录是空的，照样起来。同一次读 `var/cache/models` 下的用出来的、供应商的列表、池的指针（`pools.json`，施工 8-8），坏了的当没有。读完记 `INFO catalog loaded source=… fetched=… providers=… models=… ms=…`。要它的（造会话、载入、`model.list`）在这之前等着。2026-10-01 在开发机上量的读快照（release）见施工单 8-7「验收结果」。
6. 后台更新（`models.md`「怎么走」第二条第 3 条）：`models.catalog.update` 开着的（`MIYU_CATALOG_UPDATE` 压过），缓存的 `fetched` 旧过 `every` 的，GET `url`（是环境变量的引用的照核心的环境取，取不到的照失败算，施工 8-8：`models::refresh::Schedule`），带上次的 `ETag`；200 读得进、至少有一家的写进缓存目录、换上，304 只改 `meta`，别的一小时后再试。核心一直开着的每过 `every` 再查，配置改了当场照新的算。缓存目录算不出来的只读快照、不拉，记一行 `WARN catalog cache unavailable reason=…`。

**子进程随核心退出**（施工 7-8，`12-进程形态与分发.md` R5，`agents.md` 第八条）

有计划地停下时核心自己先记 `restarted`、再整组杀掉后台命令（下面「停下」第 4 条）。这一条管的是核心崩了、被 `SIGKILL`、被任务管理器结束的时候：没人来杀，命令得自己知道核心没了。三个平台各用自己最稳的现成做法（`crates/miyu-sandbox/src/lifeline.rs`）：

1. Unix（Linux、macOS 一样）：核心手里一根「生命线」管道，写端带 `O_CLOEXEC`、只在核心手里，核心怎么没的，内核都会关掉它。`shell` 起每条命令（前台、后台，经不经沙盒的助手都一样）之前，先起一个看门的（环境变量清空、工作目录是 `/`、标准输入是生命线的读端、标准输出和错误接空的，别的文件描述符一个都不带），自成一组；命令起在它的组里，整组杀照这个组号。看门的只做一件事：读到结尾（核心没了）就杀掉自己所在的整个组（`kill(0, SIGKILL)`），命令和它起的孙进程都在里面；读到的东西不管，读不了也当结尾。看门的是一个单独的小程序 `miyu-watchdog`（`crates/miyu-watchdog`，施工 LW-28），放在主程序真实位置的旁边（同沙盒的助手）：生命线第一次起看门的时照这个进程的真实位置找一次，之后都照它。它不带参数（带了参数的说一句用法、退出 125），不碰环境变量，起来只读、只杀；看门的做的事固定，所以不和核心对构建 id，也不等核心探沙盒（V-48 到 LW-28 之间照沙盒的助手起，`miyu-sandbox lifeline`，探成了、构建 id 对得上才用；2026-11-23 项目主人定拆出来：看门的不该叫 sandbox）。找不到的（测试程序、只拷了主程序的）、起不来的（执行不了）退回 `/bin/sh`，跑 `while read -r _; do :; done; kill -s KILL 0`，做的是同一件事。2026-11-23 量到每个看门的 CPU 0.24～0.26 ms，常驻 RSS 2.1 MB、匿名 112～120 KB（V-48 的助手看门是 0.24～0.27 ms、2.25 MB、120～124 KB；`/bin/sh` 在这台上是 bash，0.52～0.55 ms；起一个什么都不做的程序是 0.16～0.17 ms），施工单 LW-28。看门的只认自己的组，它在组里组号就不会被别人占，不会杀错；命令自己不是组长（组长是看门的），命令 `kill 0` 会连看门的一起打到。命令自己退出、被停、超时时，整组杀会连它一起杀掉；丢掉这条命令时收掉它，不留僵尸进程。看门的被别人杀了，只有这一条命令没人看着；`setsid` 出去、自己换了进程组的不在组里，核心没了也收不掉。看门的起不来的（`/bin/sh` 也没有、生命线建不起来），记一行 `WARN command lifeline not started error=…`（目标 `miyu::shell`），照旧自成一组：核心崩了它不跟着死，载入时照样补 `aborted`。
2. Windows：核心起来时把自己放进一个带 `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` 的作业对象（起来的先后第 4 步），句柄拿到进程结束。之后起的子进程、子进程再起的，都自动在里面；核心没了，系统关掉句柄，里面的全部结束。有计划地退出时后台命令已经杀了，剩下的（`taskkill`、跑到一半的前台命令）也跟着结束。
3. 没选的：Linux 的 `PR_SET_PDEATHSIG` 盯的是起子进程的那个线程，运行时线程池里的线程一退，命令就被误杀，而且只管直接的子进程，组里别的照样活；子进程收割者（`PR_SET_CHILD_SUBREAPER`）只管收养孤儿，核心没了它也没了；macOS 的 kqueue 盯父进程要另起一个进程盯着，和生命线一样，却只有一个平台能用；Windows 上一条命令一个作业对象要在起好以后才放进去，中间起的孙进程漏掉。命令共用一个看门的（一个核心一个，V-40 提的，施工 V-48 看过，2026-11-05 项目主人改批成换助手，每条命令仍然一个看门的；施工 LW-28 只让长住的子进程共用，见第 5 条）：命令得自己当组长，核心起好命令到登记进看门的之间有约 0.1～0.2 ms 的空窗，组号要靠留着僵尸占住，看门的被杀所有命令一起没人看，命令自己调 `setsid()` 会失败；墙钟本来就省不下（看门的启动和命令并着走），省的只有 CPU。
4. 再载入时内核照样给没有结束记录的后台命令补 `aborted`（`kernel/session.md`「载入和崩溃」第 9 条）：进程是真的没了。
5. 核心起的长住的子进程（施工 XH-1）：扩展（`extensions.md`「怎么走」第 1 条）、命令型 MCP 服务（`mcp.md` 第二条第 6 条）、语义模型小程序（`recall.md` 第四条第 4 款）照同一套跟着核心走（`lifeline::keep`）。Unix 上它们共用一个看门的、一个组（施工 LW-28，`lifeline/unix/shared.rs`；XH-1 到 LW-28 之间一个进程一个看门的，装了十个扩展加语义模型的机器上常驻十一个）：头一次拉起时在同一根生命线上起一个看门的（同第 1 条：照 `miyu-watchdog` 起、退回 `/bin/sh`，起法一样），自成一组；之后拉起的都起在这个组里，不再起看门的。核心怎么没的，看门的都杀整组，所有长住的子进程和它们起的、还在组里的子进程都在里面（原来扩展、MCP 服务、小程序在核心的组里，靠读到标准输入关了自己退：卡住的、不读标准输入的、它们起的子进程都留下来）。能共用是因为它们有计划地停时从不整组杀（下面这一款），不用各占一组；命令要整组杀，仍然一条一个组（第 1 条，第 3 条「没选的」）。Windows 上照继承在第 2 条的作业对象里，不另做。
   - 每个长住的子进程拿着这个组（`Kept`），拿到它退出、收掉以后才丢；丢掉一个时组里的都不杀、看门的也不收（别的还要它）。有计划地停照旧先关标准输入、等、再只杀进程本身（扩展、MCP 服务等 5 秒），核心放锁以前等它们停完，所以不会提前掐；它们正常退出、被重启以后组里剩下的子进程照旧留着（原来就留着），别的长住的子进程还在时，核心没了照样被看门的收掉；最后一个丢掉时一起杀掉（下一款）。
   - 最后一个拿着的丢了（最后一个长住的子进程退出、收掉了），先整组 `SIGKILL`（施工 CS-7：这时组里剩的只有它们起的、留在组里的子进程和看门的；看门的这一刻还没被收才杀，组号不会是别人的），看门的跟着杀掉、收掉，不留僵尸进程；之后再拉起的另起一个看门的、一个新组。原来只收看门的：扩展被杀以后留下的孙进程、MCP 服务起的子进程没人看着，核心停了、没了都不走，一直留着（门禁每趟留 5 个 `miyu-test-extension hang`）。核心有计划地退出时：扩展、MCP 服务先停完，丢掉的是最后一个的那一下整组杀、收掉看门的；没停完就退出的（语义模型小程序跟着会话，核心退出时还拿着的），核心一没，看门的读到结尾杀整组，它被系统收养、收掉，同核心崩了一样，也不留僵尸。
   - 共用的看门的被别人杀了：它是核心的子进程，在下一次拉起之前是一个僵尸进程。下一次拉起（开扩展、重启、服务退避重来、第一次要向量）时发现它没了，收掉它，再起一个 `setpgid` 进同一个组（组里还有没收掉的长住的子进程，组号就还是这一组的），组里原有的子进程在核心没了时照样被收掉；进不去的（组里已经一个进程都没有了，组号可能被别人占）另起一个新组。
   - 代价：所有长住的子进程在同一个组里，一个扩展自己 `kill(0, …)`（或者 `kill -<组号>`）会打到别的扩展、MCP 服务、小程序和看门的（原来只打到它自己的组）；共用的看门的没了到下一次拉起之间（没有下一次的就一直），所有长住的子进程都没人看着，核心这时崩了它们留下来（原来只有那一个）。拿着组的都还在、却都自己换了进程组（`setsid`、`setpgid`），组号又恰好被核心别的子进程占了组长的这一种，重起的看门的会进错组：要几样同时碰上，没有处理。
   - 看门的起不来的记一行 `WARN extension lifeline not started`、`mcp lifeline not started`、`embedder lifeline not started`（带 `error`，目标各是 `miyu::extensions`、`miyu::mcp`、`miyu::session`），照旧自成一组拉起。附带的：它们不在核心的进程组里了，核心在终端前台跑时按 Ctrl+C 只打到核心（核心照「停下」有计划地停它们），原来会一起打到；进程不是组长，自己调 `setsid()` 能成。拉起多花的时间：头一次起看门的多约 45 µs（p50，同 XH-1 量的一个一个起），之后的和不起看门的一样（施工单 XH-1、LW-28「量尺」）。

**停下**

1. **连着头的时候不看**（施工 V-6 下）：有连接就不算空闲，不定时醒，等连上、断开、来请求的信号（`miyu_endpoint::Activity`，`activity.rs`）再算。没有连接的时候每隔一会儿看一次空不空闲：空闲时限的四分之一，最少 100 毫秒，最多 30 秒；600 秒的是每 30 秒（回合、后台命令、扩展忙完没有，没有统一的信号）。没有空闲时限的（默认）也每 30 秒看一次，照这一刻的 `core.idle_exit` 判（施工 Z-1 上：配置当场生效）；间隔照起来时的那个时限定。起来时先等一个间隔再看第一次。以前连着头也每 30 秒看一次，空闲的核心每秒醒一次多（另一半是内存账，`heap.md`「自己记账」第 3 条），施工 V-6 下量过：连着一个头空闲时每秒醒 1.2 次，改完 0 次；没有连接、等着退出时 1.3 次，改完 0.13 次。
2. 空闲：没有连接（握没握手都算），没有忙着的会话，也没有在跑的后台命令（结束了、记录还没落盘的也算；`12-进程形态与分发.md` R1，施工 7-3；`protocol.md`「空闲和停下」）。
3. 看到空闲，从这一次看的时刻记起；哪一次看的时候还空闲、已经够了空闲时限，就停。中间哪一次看到不空闲，从头记。所以从真的空下来算，到退出，至少是空闲时限，最多再多两个间隔；600 秒的，在 600 到 660 秒之间：记起的时刻和后来看的时刻都是当时的钟，正好满 600 秒的那一次看可能差几微秒不够，要等下一次。`--idle-seconds 0` 的，第一次看到空闲就停。
4. 停的信号：Ctrl+C（SIGINT），Unix 上还有 SIGTERM。起来时就装好（「起来的先后」第 5 步，`stops.rs`，施工 CS-6）：原来到第 16 步才装，起来那一段收到的照系统的默认当场杀掉。说 `ready` 以前（第 16 步以前）收到的第一次：记一行 `INFO stopped reason=signal ready=false`、把运行日志落到盘上，退出码 0 当场退出（施工 CS-8）：还没有会话、没有连着的头，没有要收尾的；另起线程读目录、写盘的不等，读的只读，写的先写到旁边再换过去，进程没了最多留下旁边那一份。CS-6 到 CS-8 之间是记着、等开始接连接时才停：压力下起来要几分钟，基准测试的测具（和多数服务管理器一样）只发一次 SIGTERM、5 秒后 SIGKILL，核心一直没走（2026-11-27 基准测试 B1 第 1 次）。说了 `ready` 以后收到的，先有计划地停下全部在跑的会话（跑到一半的回合记成「重启了」，下次载入接着干；在跑的后台命令先记 `restarted`、落了盘再整组杀，R5，施工 7-3），等记忆在后台的读写做完（施工 TMP-1 补3，第 6 条），再停。停会话不等正在造的会话手里的会话表的锁（施工 CS-9，`protocol.md`「会话表」第 11 条）：原来造会话一直拿着锁到 `session.created` 落盘，盘慢时停会话跟着等几十秒（2026-11-27 复现：每次同步拖 4 秒，SIGTERM 以后 17.6 秒才记出会话停了）。拉起的核心自成一个进程组，终端里按 Ctrl+C 打不到它，要停得明着发。
5. 装不上的当它不会来：Unix 上 SIGTERM 装不上记一条 `WARN SIGTERM not watched error=…`，Ctrl+C（Unix 上是 SIGINT、Windows 上是控制台的 Ctrl+C）装不上记一条 `WARN Ctrl+C not watched error=…`，照样等装上了的；都装不上的一直等下去（施工 4-9 再补三上）。
   - **第二次就强退**（施工 CS-6，2026-11-27 项目主人定）：第一次照第 4 条有计划地停；之后再来一次（SIGTERM、SIGINT 哪一种都算），记一行 `WARN second stop signal, exiting now`、把运行日志落到盘上，退出码 1 当场退出，收尾剩下的不做。原来第二次被吞掉（tokio 装上信号以后不再交还系统的默认），收尾卡住的核心 SIGTERM 怎么发都不走。
   - **收尾有总时限**（施工 CS-6）：定下要停的那一刻（信号、头叫停、空闲、数据根没了都算）起表，30 秒还没走到第 8 条，记一行 `WARN stopping took too long, exiting now`，同样落盘、退出码 1 当场退出。30 秒：停扩展、MCP 服务各等 5 秒，记忆写完、清回收处一般几秒；比 `miyu core stop` 等核心退的 60 秒短，人敲的「停」等得到它；比 systemd 默认的 `TimeoutStopSec` 90 秒短，服务管理器不用自己去杀。打开 `testkit` 的构建另认测试口子 `MIYU_TEST_STOP_LIMIT_MS`（毫秒）。
   - 当场退出时：锁由系统放（进程没了就放），套接字文件留下、下一个核心起来时当旧的清掉，`run/socket` 留下、照它连是被拒绝（`NotRunning`）；长住的子进程、后台命令照「子进程随核心退出」由看门的收掉，再载入时补 `aborted`。
   - **定下要停就不再接新连接**（施工 CS-6）：第 2、4、9、10 条哪一种定下要停，先关掉监听、删掉套接字文件和 `run/socket`（端点记 `INFO listener closed`，目标 `miyu::endpoint`），头再连是「没在跑」，不会连上了却等不到回应；拉起核心的头看到锁还被拿着就等它走了再拉（`ipc.md`「连不上就拉起」第 4 步）。已经连着的照常说话，到第 8 条才断。锁另外拿着（`Listener::take_lock`），第 7 条记完才放。原来监听器跟着锁一起放在最后，收尾那几秒里新连上的头一直空等。
6. 收尾，在放锁之前做完（施工 TMP-1 补2，`serve_then`）：等第 14 步清完回收处、清完缓存（`cache.rs`），等另起线程写盘的写完（缓存目录里整理好的模型目录，`models.md`「怎么走」第二条；`pending.rs`）。写盘的线程照门起（`pending::write`，施工 CS-8）：收尾先关门（`pending::close`），关了以后谁都不再起写盘的线程，再等过了门的写完、`join` 完。起来时另起线程读的模型目录还没读完的不等：写盘的线程是它读完才起的，关了门它读完也不起，不会在放锁以后把删掉的数据根建回来（施工 TMP-3 的那一处，main b47ecacb3 的门禁，`miyu` 的 `heads` 测试：拉起核心、问完装了哪些界面就叫停）。施工 TMP-3 到 CS-8 之间是连还在读的也等：压力下读一份目录要一两分钟，收到一次 SIGTERM 的核心一直等它，只有 30 秒的总时限停得下（2026-11-27 基准测试 B1 第 5 次，测具 5 秒就 SIGKILL）。还在读的、过了门的、登记的都在一把锁里，等的一方照它判，叫醒不会丢；两处同时等的各自都等到全部写完（施工 TMP-3 补）。测试的夹具删临时目录之前另照 `models::settled` 等：不关门，连还在读的、读完起的写盘一起等完。这些都碰数据根，只许拿着锁的核心做：原来锁放了才等，下一份核心起来、测试删数据根时撞上，删掉的目录又被一层层建回来（2026-10-29 主会话定：这是核心的毛病，不在测试夹具上补）。在这之前先等记忆在后台的读写做完（施工 TMP-1 补3，`Core::finish_writes`）：回合开始问记忆、限时过了会话不等的读（头一回读一间要读记忆日志、开库，`memory.md` 第三条第 4 款），回合索引的后台，补齐旧会话的线程。空闲退出的没停会话，在这里等；收到信号的停会话时（`stop_sessions`）就等过了。原来交回时那次读还在读，CI 上测试删数据根时撞上，剩下记忆日志、回合库。
7. 停之前记一条 `INFO stopped reason=idle`（空闲）、`reason=signal`（信号）、`reason=request`（头叫停，第 10 条）或 `reason=gone`（数据根没了，第 9 条）：那时锁还在手里，下一个核心起来之前，这份日志还是它一个人写。
8. 然后断开连着的头、关上数据根里开着的库（`Core::close_databases`，施工 CS-9，`store.md`「怎么走」第 14 条第 5 款：关的时候不合 `-wal`，放了锁以后不再碰库文件；原来放下运行时才关，在放锁以后，还要把三个库的 `-wal` 各合一遍、同步两次，盘慢时一直跑到总时限）、放开锁，退出码 0。放下运行时不等阻塞线程池里还在跑的（`shutdown_background`，施工 CS-8）：要等的上面都等完了，剩下的是读目录这类只读的。原来放下运行时要等它们跑完，记了「停了」进程还在，收尾的总时限也管不到这里（计时的任务跟着运行时一起停了）。
9. **数据根没了就走**（施工 TMP-2）：没有连接、每次看空不空闲的时候（第 1 条），先看数据根里的锁文件还是不是拿着的这一个（`ipc.md` 的 `Held`）。不是了（测试删了临时目录、人手删了数据根，删了又建了一个新的也算），记一条 `WARN data root gone, stopping`，照第 4 条有计划地停下会话，再照第 6 到 8 条走，`reason=gone`。没有空闲时限的（默认）核心原来一直开着：测试漏掉的、数据根被删了的核心永远不退，2026-10-29 主会话的机器上攒了十个、跑了二十小时。没有空闲时限的每 30 秒看一次，所以最多三十秒走；连着头的不看，头走了以后照常看。
10. **头叫停**（施工 DT-4 下）：出示本机令牌连上的头调 `core.stop`（`protocol.md` 方法表），端点把回应 `{}` 写出去以后叫一声（`Activity::stop_requested`，`activity.rs`）；核心照第 4 条有计划地停下会话，再照第 6 到 8 条走，`reason=request`。桌面端托盘里的「退出」连核心一起关用它：头不知道核心的进程号，Windows 上拉起的核心脱了控制台，信号发不过去。和信号、空闲同时到的照先到的那一个停，只停一次。回应以前在 `run/` 里留下人停的记号 `stopped`（施工 CS-5，`ipc.md`「人停的不自己拉起」）：头断线以后自己重连不再拉起它。

### 出错

| 什么时候 | 那一行 | 退出码 |
|---|---|---|
| 已经有一个核心在跑 | `running` | 0 |
| 找不到数据根、建不了骨架、拿锁出了别的错、装不上运行日志、建不了默认工作区、找不到资源目录、起不了运行时、套接字放不下或者绑不上、令牌写不进、HTTP 客户端造不出来、工具的字读不出来或者登记时查不过 | `error <代码>: <原因>`（施工 EN-2） | 1 |
| 参数不对（`--idle-seconds` 不是非负整数、不认识的参数） | 不写那一行，clap 把错印在标准错误上 | 2 |

运行日志（目标 `miyu::core`）：

| 级别 | 这件事 |
|---|---|
| `INFO` | `starting version=… pid=… root=… tz=…` |
| `INFO` | `model profiles loaded profiles=…`（施工 8-6） |
| `INFO` | `catalog loaded source=… fetched=… providers=… models=… ms=…`（施工 8-7） |
| `WARN` | `catalog unreadable source=… error=…`、`catalog empty`（施工 8-7） |
| `DEBUG` | `catalog entry skipped entry=…`：目录里坏了跳过的一个模型、一家供应商（施工 8-7） |
| `INFO` | `catalog refreshed fetched=… providers=… models=…`、`catalog not modified`（施工 8-7） |
| `WARN` | `catalog refresh failed error=…`、`catalog cache unavailable reason=…`（施工 8-7） |
| `WARN` | `children not bound error=…`（Windows：进不了作业对象，施工 7-8） |
| `WARN` | `generate thread not started error=…`：生成参考文件的线程起不来，照旧在主线程上写（施工 LW-5） |
| `INFO` | `sandbox helper=… platform=… mechanisms=…`（施工 5-1） |
| `WARN` | `sandbox unavailable reason=…`（施工 5-1） |
| `WARN` | `not started stage=…` |
| `WARN` | `ready line not written error=…` |
| `INFO` | `trash purged removed=…`（施工 3-8 三补） |
| `WARN` | `trash entry kept session=… error=…`、`trash not read error=…` |
| `ERROR` | `trash purge panicked panic=…` |
| `ERROR` | `background write panicked`、`background writes not awaited panic=…`（施工 TMP-1 补2：另起线程写盘的、等它们的线程 panic 了） |
| `ERROR` | `memory writes wait panicked`（施工 TMP-1 补3：等记忆在后台的读写的那个线程 panic 了；目标 `miyu::endpoint`） |
| `WARN` | `database not closed db=… error=…`、`ERROR closing databases panicked`（施工 CS-9：放锁以前关库时出了错、关库的线程 panic 了；目标 `miyu::endpoint`） |
| `WARN` | `SIGTERM not watched error=…`、`Ctrl+C not watched error=…` |
| `WARN` | `second stop signal, exiting now`、`stopping took too long, exiting now`（施工 CS-6：第二次停的信号、收尾过了 30 秒，当场退出，退出码 1） |
| `WARN` | `data root gone, stopping`（施工 TMP-2：数据根里的锁文件不在了、换了，「停下」第 9 条） |
| `INFO` | `stopped reason=idle`、`stopped reason=signal`、`stopped reason=request`、`stopped reason=gone` |

配置那几行的目标是 `miyu::config`：`WARN config schema not written file=var/cache/config/<文件名> error=…`（第 6 步，施工 8-1）；`WARN config problems file=… errors=… warnings=…`、`WARN trust not read file=… error=…`、`WARN MIYU_LOG not understood, using config value=…`、`INFO log level level=… from=…`（第 6 步，施工 8-2）；`WARN config watch unavailable error=…`、`INFO config changed layer=… via=file keys=…`（第 6 步以后，施工 8-4）；`WARN secrets readable by others file=secrets.toml`（第 6 步、手改重读时）、`INFO secret changed name=… action=… via=…`（只有名字，施工 8-5）。

套接字的几行（`listening`、`stale socket removed`、`XDG_RUNTIME_DIR not usable`）见 `ipc.md`，连接、会话的见 `protocol.md`、`session/actor.md`。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu/tests/leftovers.rs`（施工 AE-7、AS-11、R3-1 补） | 真核心：起来时就建好 `trash/`、`blobs/`、`modules/`、`index/`、`kb/`；两次改名之间被杀的包起来时放回原来那一份，剩下的 `.old`、`.new` 清掉，认不出的名字不碰；崩了剩下、放了满 10 分钟的配置和密钥临时文件起来时删掉，`run/` 里的、数据根这一层的都删（施工 R3-1），刚写的、认不出的不碰 |
| `crates/miyu/tests/core.rs` | 头拉起真的 `miyu core`，等它说好了再连；握手不再说是谁；再连不再拉起；两个头同时只拉起一个；起不来的说原因（找不到资源目录），日志里只写 `stage=resources`；已经在跑的写 `running` 就走、不写日志；什么都没写就退了的；空闲了自己走，日志里一条 `starting`、一条 `stopped reason=idle`；`starting` 那一行的数据根在家目录下的写成 `~`、有和 UTC 差多少；工作目录是数据根；起来时清一次回收处：删了满 7 天的删、没满的留，记一条 `trash purged removed=1`（施工 3-8 三补）；起来时探一次沙盒的助手：旁边有助手的记 `sandbox` 那一行、平台是这台机器的、有手段那一格，没有的记找不到（施工 5-1）；握手报的沙盒和记下的对得上（施工 5-4 下）；头拉起的那种不空闲退出的核心没人停的，测试的夹具丢掉时收掉（施工 TMP-2）；`miyu core stop` 停掉真核心、回来时锁已放开、运行日志记 `stopped reason=request`，没在跑的不拉起（施工 CL-4）；`core_stopped.rs`（施工 CS-5）：`miyu core stop` 以后 `run/stopped` 在，自己重连交回 `StoppedByUser`、没拉起，人要用的拉起来、起来的核心删了记号，它空闲退出以后自己重连照旧拉起 |
| `crates/miyu-cli/src/core_status/tests.rs`、`crates/miyu/tests/core_start.rs`（施工 CL-8） | 假核心：没在跑的 `status` 印三行、退出码 3、不拉起，机读的是 `running: false`、`version: null`；在跑的版本照核心握手报的、退出码 0，文字照握手回的语言。真核心：`core start` 拉得起、什么都不印，再敲一次不多拉；`status` 的文字和 JSON；`stop` 以后 `status` 退 3；人停过的 `start` 照样拉得起、记号没了 |
| `crates/miyu-cli/src/core_stop/tests.rs`、`crates/miyu-ipc/src/lock/tests.rs`（施工 CL-4） | 假核心答完 `core.stop` 还拿着锁，`miyu core stop` 等锁放了才回、什么都不印；没在跑的退出码 0；`wait_released` 拿着时不醒、放了就醒、没有锁文件的当已放 |
| `crates/miyu-cli/src/core_stop/tests.rs`、`crates/miyu-ipc/src/unreachable/tests.rs`、`lock/tests.rs`、`crates/miyu/tests/core.rs`（施工 CS-3） | 假核心拿着锁、`run/` 里的令牌和位置删了，`miyu core stop` 说连不上、退 1；锁拿着的、锁文件删了但算出来的位置还在听的算在跑，从没起过的、走了只留下文件的不算；`held` 分得清拿着的和留下的锁文件；真核心跑着时删掉整个 `run/`（Linux 上套接字放在 `$XDG_RUNTIME_DIR`，Windows 上是管道），`miyu core stop` 退 1、核心还在 |
| `crates/miyu/tests/crash.rs`（施工 7-8） | 真的 `miyu core`，模型是本机回环上的假服务器：放一个心跳到后台（Unix 上是命令起的孙进程），硬杀核心（Unix `SIGKILL`，Windows 结束进程、不连子进程），心跳几秒内停下；三个平台都跑 |
| `crates/miyu-sandbox/tests/lifeline.rs`（施工 7-8，看门换成 `miyu-watchdog` 是施工 LW-28，Unix） | 看门的照 `miyu-watchdog` 起、退回 `/bin/sh`（没给、给的不在）都一样：生命线还在，组里的照常跑；写端一关，看门的把整个组杀掉，孙进程也在里面；经沙盒的助手起的（Landlock、Seatbelt）一样；`setsid`、换了进程组的收不掉；看门的被杀只有那一条没人看着，命令 `kill 0` 打到看门的；两根生命线各管各的；Linux 上看门的环境变量空、工作目录 `/`、只有 0、1、2；沙盒的助手不再认 `lifeline` |
| `crates/miyu-sandbox/tests/lifeline_kept.rs`（施工 LW-28，Unix） | 长住的子进程：一根生命线上 `keep` 几次都进同一个组、同一个看门的（组长是看门的），生命线断了全组连孙进程都没了；共用的看门的被杀了，下一次 `keep` 再起一个进同一个组，原来组里的照样被收掉；组里一个进程都没有了的另起一组；丢掉一个拿着的不收看门的，丢掉最后一个才收掉、不留僵尸；`miyu-watchdog`、`/bin/sh` 两种都测 |
| `crates/miyu-watchdog/tests/watchdog.rs`（施工 LW-28） | 不带参数的读到标准输入的结尾（空的、先读到字的）杀掉自己的组、什么都不印；带了参数的说用法、退出 125；Windows 上退出 125 |
| `crates/miyu/tests/crash.rs`（施工 7-8、V-48） | 真的 `miyu core` 被硬杀：后台命令跟着停（三个平台）；Unix 上前台、监视的（MN-1 上）、人用 `!` 跑的（SH-1 上）也跟着停，命令自己和孙进程都没了，组里看门的是 `miyu-watchdog`（主程序旁边有的话）；两个核心各管各的，有计划地停下不留看门的 |
| `crates/miyu/tests/crash_kept.rs`（施工 XH-1） | 真的 `miyu core` 拉起扩展（`miyu-test-extension` 一起来就 `hang`，再起一个睡着的子进程）、系统配置里的命令型 MCP 服务、`embed` 小程序包里的程序（这两种是一段 `sh` 先在后台起 `sleep` 再 `exec` 成 `miyu-test-mcp`、`miyu-test-embed`），硬杀核心以前两个进程在同一组、组长不是它们也不是核心的组（施工 LW-28 起这一组是长住的子进程共用的），硬杀以后两个都在 10 秒内没了；扩展那一条三个平台跑（Windows 上是核心的作业对象），另两条 Unix |
| `crates/miyu-core/tests/serve.rs` | 空闲退出、放开锁和套接字；收尾时锁还在、交回以后才放（施工 TMP-1 补2）；回合开始问记忆的读慢过限时、会话不等的，收到停的信号、空闲退出都等它读完才交回（施工 TMP-1 补3）；有头连着不退；空闲的钟从最后一个头走时算起；在跑的回合不退；有在跑的后台命令不退、结束了记下再退（施工 7-3）；收到停的信号先停下会话、跑到一半的记成重启了，没有空闲时限的删了数据根自己走、`reason=gone`（只在 Unix 上，施工 TMP-2），后台命令先记 `restarted`、落了盘再杀（施工 7-3）；没配模型的每次请求都说没有模型、分类 `no_model`、没发出去（施工 8-6）；头调 `core.stop` 先收到 `{}`，再照信号那样停下会话、原因是 `request`（施工 DT-4 下），回应时 `run/stopped` 已经在了（施工 CS-5） |
| `crates/miyu/tests/stopping.rs`（施工 CS-6，Unix） | 真的核心带一个停的时候要等 5 秒的扩展：收尾时第二次信号 2 秒内退、退出码 1、记一行、没记 `stopped`；总时限（测试口子 1 秒）到了退、退出码 1、记一行；收尾时连是 `NotRunning`；收尾时敲 `miyu core start` 说「核心还在启动」、等它走了拉起新的，回来时新的在跑 |
| `crates/miyu/tests/stop_kept.rs`（施工 CS-7，Unix） | 真的核心拉起扩展（`hang`，再起一个睡着的子进程）、命令型 MCP 服务（一段 `sh` 先在后台起 `sleep`），`SIGTERM` 让它照「停下」走完（退出码 0、`stopped reason=signal`），两个进程号都在 10 秒内没了；`miyu-sandbox` 的 `lifeline_kept.rs` 的 `the_last_holder_takes_the_leftovers_with_it`：只杀长住的子进程本身，还有人拿着时孙进程留着，最后一个丢了一个不剩。修之前两处都红 |
| `crates/miyu-core/src/stops/tests.rs`（施工 CS-6） | 装不上的、装上了又断了的信号当它不会来，来了的照常回；总时限是 30 秒 |
| `crates/miyu-core/src/serve/tests.rs` | 多久看一次：四分之一，最多 30 秒，最少 100 毫秒。最后一个头走了才开始算空闲、断开时叫一声（`tests/serve.rs`，施工 V-6 下拿掉断开时那一声就红） |
| `crates/miyu-core/src/pending/tests.rs`（施工 TMP-1 补2） | 读目录时另起线程写的整理好的一份，等完以后已经在缓存目录里了（写得慢的，测试里让它先睡 300 毫秒，也等）；登记了的都等；起来时读的目录还没读完就等的（读的也先睡 300 毫秒），等完删掉数据根，过 1.5 秒里面没有又出现整理好的一份（施工 TMP-3）；另一处先取走了登记、正在等，这一处也等到写完才交回（施工 TMP-3 补）；关门不等还在读的、关了以后不再起写盘的线程，过了门的照旧等写完（施工 CS-8，用自己的一份） |
| `crates/miyu/tests/stopping_early.rs`（施工 CS-8，Unix） | 真的核心，一次 `SIGTERM` 就退：卡在起来那一段（系统配置换成命名管道，卡在装好信号以后的那一次读）的 10 秒内退、退出码 0、记 `stopped reason=signal ready=false`、没记 `listening`；说了 `ready`、目录还在读（缓存目录里的目录换成命名管道）的 10 秒内退、退出码 0、记 `stopped reason=signal`、没记 `catalog loaded`。修之前两条都红：前一条一直不走，后一条收尾等读目录、去掉放下运行时的那一处也红（记了「停了」不走） |
| `crates/miyu/tests/stopping_create.rs`（施工 CS-9，Unix） | 真的核心，造会话卡在找人格上（家目录那一层的人格的 `prompts/reminders.md` 换成命名管道，那时拿着会话表的锁）时一次 `SIGTERM`：10 秒内退、退出码 0、记 `stopped reason=signal`、没有 `stopping took too long`，造会话的头收到 `session_stopped`，没留下会话（改前等满测试口子的总时限、退出码 1） |
| `crates/miyu-core/tests/closing.rs` 的 `the_databases_are_closed_before_the_lock_goes`（施工 CS-9） | 「停下」第 8 条：另一个线程一拿到锁就数，这个数据根里一个开着的库都没有；交回以后核心还在，库照样关着（改前放锁时会话列表的索引、用量汇总都开着） |
| `crates/miyu-core/tests/tools.rs` | 工具目录里是基础系统的七件；资源目录坏了，说是哪一份 |
| `crates/miyu-core/src/models/tests.rs` | 出厂的档案读得进来，DeepSeek 那一套开关和请求形状探针用的 `Compat::deepseek()` 一样、收图不收 PDF、图照官方的算法（施工 8-6，原来写在代码里，施工 4-13）；TOML 变成 JSON，写错的说是档案 |
| `crates/miyu-core/src/sandbox/tests.rs` | 探沙盒的助手：旁边没有的、不知道主程序在哪的记找不到，跑不了的记原因（施工 5-1）；探到了手段的交回能用和助手，手段是空的、找不到、探不成的交回用不了和原因（施工 5-4 上、下） |

### 出处

- `12-进程形态与分发.md` 第二节：按需运行、空闲多久、停的信号；「拉起时的握手」（那一行、先拿锁再装日志、清旧套接字的是核心）。第三节：一个主程序，`miyu core` 是它的子命令。
- `07-存储.md` 第二节（数据根、骨架、`run/`）、第十节（单实例与锁）。
- `15-模型与供应商.md` 第二节、`models.md`：模型从配置里来（施工 8-6 取代了配置做出来之前只认的 `DEEPSEEK_API_KEY`）。
- `28-运行日志.md`：写到 `var/log`，`MIYU_LOG`。
- `04-核心协议.md` 第九节 `session.delete`：删了的进回收处、留 7 天（2026-09-30 项目主人定，施工 3-8 三补）。

### 还没有的

设计里有、还没做的：

- 常驻：开了通讯平台桥、定时任务、远程访问、桌面语音时不退出，登记成登录时启动的服务（`12-进程形态与分发.md` 第二节，`miyu service install`）。
- 空闲时限放进配置（第二节）。
- 恢复会话、回收 blob 这类重活放到说好了之后（第二节「拉起时的握手」）。清回收处（施工 3-8 三补）已经照这样放在 `ready` 之后。
- 「核心先 bind 好套接字、能接受连接了就往管道里写」（第二节「拉起时的握手」）：现在绑好以后还要读供应商的档案、登记工具（读资源目录的字）才写 `ready`，比设计说的晚。
- 头发现核心比自己旧，请求它空闲时重启（`04-核心协议.md` 第八节）。
- 模型从配置、供应商、池来（`15-模型与供应商.md`）；首次运行时的引导（`12-进程形态与分发.md` 第七节）。
- `miyu status`、`miyu doctor`（`12-进程形态与分发.md` 第三节、R13）。
